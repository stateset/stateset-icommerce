//! Metrics helpers for StateSet iCommerce.
//!
//! This module provides a minimal, dependency-light metrics surface using
//! lock-free atomic counters. Downstream applications can wrap these hooks
//! with their preferred metrics exporter (Prometheus, `StatsD`, etc.).
//!
//! It includes RED primitives (rate/errors/duration) and SLO evaluation
//! helpers for operation-level telemetry.
//!
//! # Example
//!
//! ```rust
//! use std::time::Duration;
//! use stateset_observability::{init_metrics, MetricsConfig, SloTarget};
//!
//! let metrics = init_metrics(MetricsConfig::default());
//! assert!(metrics.is_enabled());
//!
//! metrics.record_order_created("cust-1", 49.99);
//! metrics.record_request_success("order.create", Duration::from_millis(42));
//! metrics.record_request_error("order.create", Duration::from_millis(120));
//!
//! let snap = metrics.snapshot();
//! let report = snap.evaluate_operation_slo("order.create", SloTarget {
//!     min_success_rate: 0.95,
//!     max_avg_latency_ms: 150.0,
//!     min_requests: 2,
//! });
//! assert!(report.is_some());
//! ```

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::conventions;

/// Configuration for metrics initialization.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether metrics are enabled.
    pub enabled: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// RED snapshot for a single operation or global aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct RedSnapshot {
    /// Request count (rate denominator).
    pub requests: u64,
    /// Error count.
    pub errors: u64,
    /// Total request duration in milliseconds.
    pub duration_total_ms: f64,
    /// Error rate (`errors / requests`) in [0, 1].
    pub error_rate: f64,
    /// Average request duration in milliseconds.
    pub avg_duration_ms: f64,
}

impl RedSnapshot {
    fn from_counts(requests: u64, errors: u64, duration_micros_total: u64) -> Self {
        let duration_total_ms = duration_micros_total as f64 / 1_000.0;
        let error_rate = if requests == 0 { 0.0 } else { errors as f64 / requests as f64 };
        let avg_duration_ms = if requests == 0 { 0.0 } else { duration_total_ms / requests as f64 };

        Self { requests, errors, duration_total_ms, error_rate, avg_duration_ms }
    }

    /// Evaluate this RED snapshot against an SLO target.
    #[must_use]
    pub fn evaluate_slo(&self, target: SloTarget) -> SloEvaluation {
        let success_rate = 1.0 - self.error_rate;

        let enough_requests = self.requests >= target.min_requests;
        let meets_success = success_rate >= target.min_success_rate;
        let meets_latency = self.avg_duration_ms <= target.max_avg_latency_ms;

        let passed = enough_requests && meets_success && meets_latency;
        let reason = if passed {
            None
        } else if !enough_requests {
            Some(format!("insufficient requests: {} < {}", self.requests, target.min_requests))
        } else if !meets_success {
            Some(format!(
                "success rate {:.4} below target {:.4}",
                success_rate, target.min_success_rate
            ))
        } else {
            Some(format!(
                "avg latency {:.3}ms above target {:.3}ms",
                self.avg_duration_ms, target.max_avg_latency_ms
            ))
        };

        SloEvaluation {
            passed,
            requests: self.requests,
            success_rate,
            error_rate: self.error_rate,
            avg_duration_ms: self.avg_duration_ms,
            reason,
        }
    }
}

/// SLO target for RED metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SloTarget {
    /// Minimum required success rate, in [0, 1].
    pub min_success_rate: f64,
    /// Maximum allowed average latency in milliseconds.
    pub max_avg_latency_ms: f64,
    /// Minimum number of requests required before evaluating pass/fail.
    pub min_requests: u64,
}

impl Default for SloTarget {
    fn default() -> Self {
        Self { min_success_rate: 0.99, max_avg_latency_ms: 250.0, min_requests: 100 }
    }
}

/// Result of evaluating RED metrics against an SLO target.
#[derive(Debug, Clone, PartialEq)]
pub struct SloEvaluation {
    /// Whether the SLO target passed.
    pub passed: bool,
    /// Number of requests used for evaluation.
    pub requests: u64,
    /// Success rate (`1 - error_rate`).
    pub success_rate: f64,
    /// Error rate (`errors / requests`).
    pub error_rate: f64,
    /// Average request latency in milliseconds.
    pub avg_duration_ms: f64,
    /// Explanation for failure.
    pub reason: Option<String>,
}

/// Snapshot of recorded metrics values.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Whether metrics collection is currently enabled.
    pub enabled: bool,
    /// Number of order creation events.
    pub orders_created: u64,
    /// Number of customer creation events.
    pub customers_created: u64,
    /// Number of product creation events.
    pub products_created: u64,
    /// Number of return request events.
    pub returns_requested: u64,
    /// Number of cart creation events.
    pub carts_created: u64,
    /// Number of completed cart checkouts.
    pub cart_checkouts_completed: u64,
    /// Number of shipment creation events.
    pub shipments_created: u64,
    /// Number of delivered shipments.
    pub shipments_delivered: u64,
    /// Number of created subscriptions.
    pub subscriptions_created: u64,
    /// Number of completed payments.
    pub payments_completed: u64,
    /// Number of inventory adjustment events.
    pub inventory_adjustments: u64,
    /// Sum of recorded order amounts.
    pub order_amount_total: f64,
    /// Sum of recorded payment amounts.
    pub payment_amount_total: f64,
    /// Sum of recorded inventory deltas.
    pub inventory_delta_total: f64,
    /// Global RED aggregate across all operations.
    pub red_global: RedSnapshot,
    /// Operation-level RED aggregates keyed by normalized operation label.
    pub red_by_operation: BTreeMap<String, RedSnapshot>,
}

impl MetricsSnapshot {
    /// Evaluate global RED metrics against an SLO target.
    #[must_use]
    pub fn evaluate_global_slo(&self, target: SloTarget) -> SloEvaluation {
        self.red_global.evaluate_slo(target)
    }

    /// Evaluate an operation's RED metrics against an SLO target.
    ///
    /// Returns `None` when the operation has no recorded requests.
    #[must_use]
    pub fn evaluate_operation_slo(
        &self,
        operation: &str,
        target: SloTarget,
    ) -> Option<SloEvaluation> {
        let key = conventions::operation_metric_label(operation);
        self.red_by_operation.get(&key).map(|snapshot| snapshot.evaluate_slo(target))
    }
}

#[derive(Debug, Clone, Default)]
struct RedAccumulator {
    requests: u64,
    errors: u64,
    duration_micros_total: u64,
}

impl RedAccumulator {
    fn record(&mut self, duration_micros: u64, is_error: bool) {
        self.requests = self.requests.saturating_add(1);
        if is_error {
            self.errors = self.errors.saturating_add(1);
        }
        self.duration_micros_total = self.duration_micros_total.saturating_add(duration_micros);
    }

    fn snapshot(&self) -> RedSnapshot {
        RedSnapshot::from_counts(self.requests, self.errors, self.duration_micros_total)
    }
}

#[derive(Debug, Default)]
struct MetricTotals {
    order_amount_total: f64,
    payment_amount_total: f64,
    inventory_delta_total: f64,
    red_by_operation: HashMap<String, RedAccumulator>,
}

#[derive(Debug)]
struct MetricsInner {
    enabled: AtomicBool,
    orders_created: AtomicU64,
    customers_created: AtomicU64,
    products_created: AtomicU64,
    returns_requested: AtomicU64,
    carts_created: AtomicU64,
    cart_checkouts_completed: AtomicU64,
    shipments_created: AtomicU64,
    shipments_delivered: AtomicU64,
    subscriptions_created: AtomicU64,
    payments_completed: AtomicU64,
    inventory_adjustments: AtomicU64,
    requests_total: AtomicU64,
    request_errors_total: AtomicU64,
    request_duration_micros_total: AtomicU64,
    totals: Mutex<MetricTotals>,
}

/// Thread-safe metrics handle for in-process telemetry.
#[derive(Debug, Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

impl Default for Metrics {
    fn default() -> Self {
        init_metrics(MetricsConfig::default())
    }
}

impl Metrics {
    /// Whether this metrics instance currently records values.
    pub fn is_enabled(&self) -> bool {
        self.inner.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable metrics collection at runtime.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Return a point-in-time snapshot of current metrics values.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let totals = match self.inner.totals.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let red_by_operation: BTreeMap<String, RedSnapshot> = totals
            .red_by_operation
            .iter()
            .map(|(operation, acc)| (operation.clone(), acc.snapshot()))
            .collect();

        let red_global = RedSnapshot::from_counts(
            self.inner.requests_total.load(Ordering::Relaxed),
            self.inner.request_errors_total.load(Ordering::Relaxed),
            self.inner.request_duration_micros_total.load(Ordering::Relaxed),
        );

        MetricsSnapshot {
            enabled: self.is_enabled(),
            orders_created: self.inner.orders_created.load(Ordering::Relaxed),
            customers_created: self.inner.customers_created.load(Ordering::Relaxed),
            products_created: self.inner.products_created.load(Ordering::Relaxed),
            returns_requested: self.inner.returns_requested.load(Ordering::Relaxed),
            carts_created: self.inner.carts_created.load(Ordering::Relaxed),
            cart_checkouts_completed: self.inner.cart_checkouts_completed.load(Ordering::Relaxed),
            shipments_created: self.inner.shipments_created.load(Ordering::Relaxed),
            shipments_delivered: self.inner.shipments_delivered.load(Ordering::Relaxed),
            subscriptions_created: self.inner.subscriptions_created.load(Ordering::Relaxed),
            payments_completed: self.inner.payments_completed.load(Ordering::Relaxed),
            inventory_adjustments: self.inner.inventory_adjustments.load(Ordering::Relaxed),
            order_amount_total: totals.order_amount_total,
            payment_amount_total: totals.payment_amount_total,
            inventory_delta_total: totals.inventory_delta_total,
            red_global,
            red_by_operation,
        }
    }

    /// Record a new order creation event.
    pub fn record_order_created(&self, _customer_id: &str, amount: f64) {
        if !self.is_enabled() {
            return;
        }
        self.inner.orders_created.fetch_add(1, Ordering::Relaxed);
        let mut totals = match self.inner.totals.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        totals.order_amount_total += amount;
    }

    /// Record a new customer creation event.
    pub fn record_customer_created(&self, _customer_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.customers_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a new product creation event.
    pub fn record_product_created(&self, _product_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.products_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a new return request event.
    pub fn record_return_requested(&self, _return_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.returns_requested.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a new cart creation event.
    pub fn record_cart_created(&self, _cart_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.carts_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a completed cart checkout event.
    pub fn record_cart_checkout_completed(&self, _cart_id: &str, _order_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.cart_checkouts_completed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a new shipment creation event.
    pub fn record_shipment_created(&self, _shipment_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.shipments_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a shipment delivery event.
    pub fn record_shipment_delivered(&self, _shipment_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.shipments_delivered.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a new subscription creation event.
    pub fn record_subscription_created(&self, _subscription_id: &str) {
        if !self.is_enabled() {
            return;
        }
        self.inner.subscriptions_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a completed payment.
    pub fn record_payment_completed(&self, _payment_id: &str, amount: f64) {
        if !self.is_enabled() {
            return;
        }
        self.inner.payments_completed.fetch_add(1, Ordering::Relaxed);
        let mut totals = match self.inner.totals.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        totals.payment_amount_total += amount;
    }

    /// Record an inventory adjustment.
    pub fn record_inventory_adjusted(&self, _sku: &str, delta: f64) {
        if !self.is_enabled() {
            return;
        }
        self.inner.inventory_adjustments.fetch_add(1, Ordering::Relaxed);
        let mut totals = match self.inner.totals.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        totals.inventory_delta_total += delta;
    }

    /// Record a request in RED metrics.
    ///
    /// `operation` is normalized to a low-cardinality label.
    pub fn record_request(&self, operation: &str, duration: Duration, is_error: bool) {
        if !self.is_enabled() {
            return;
        }

        let op = conventions::operation_metric_label(operation);
        let duration_micros = duration.as_micros().min(u128::from(u64::MAX)) as u64;

        self.inner.requests_total.fetch_add(1, Ordering::Relaxed);
        if is_error {
            self.inner.request_errors_total.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.request_duration_micros_total.fetch_add(duration_micros, Ordering::Relaxed);

        let mut totals = match self.inner.totals.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        totals.red_by_operation.entry(op).or_default().record(duration_micros, is_error);
    }

    /// Record a successful request in RED metrics.
    pub fn record_request_success(&self, operation: &str, duration: Duration) {
        self.record_request(operation, duration, false);
    }

    /// Record a failed request in RED metrics.
    pub fn record_request_error(&self, operation: &str, duration: Duration) {
        self.record_request(operation, duration, true);
    }
}

/// Initialize metrics and return a handle.
pub fn init_metrics(config: MetricsConfig) -> Metrics {
    Metrics {
        inner: Arc::new(MetricsInner {
            enabled: AtomicBool::new(config.enabled),
            orders_created: AtomicU64::new(0),
            customers_created: AtomicU64::new(0),
            products_created: AtomicU64::new(0),
            returns_requested: AtomicU64::new(0),
            carts_created: AtomicU64::new(0),
            cart_checkouts_completed: AtomicU64::new(0),
            shipments_created: AtomicU64::new(0),
            shipments_delivered: AtomicU64::new(0),
            subscriptions_created: AtomicU64::new(0),
            payments_completed: AtomicU64::new(0),
            inventory_adjustments: AtomicU64::new(0),
            requests_total: AtomicU64::new(0),
            request_errors_total: AtomicU64::new(0),
            request_duration_micros_total: AtomicU64::new(0),
            totals: Mutex::new(MetricTotals::default()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {a} to be close to {b}");
    }

    #[test]
    fn records_values_when_enabled() {
        let metrics = init_metrics(MetricsConfig { enabled: true });
        metrics.record_order_created("cust-1", 99.99);
        metrics.record_customer_created("cust-1");
        metrics.record_product_created("prod-1");
        metrics.record_return_requested("ret-1");
        metrics.record_cart_created("cart-1");
        metrics.record_cart_checkout_completed("cart-1", "ord-1");
        metrics.record_shipment_created("shp-1");
        metrics.record_shipment_delivered("shp-1");
        metrics.record_subscription_created("sub-1");
        metrics.record_payment_completed("pay-1", 99.99);
        metrics.record_inventory_adjusted("sku-1", -2.0);
        metrics.record_inventory_adjusted("sku-1", 1.0);

        let snapshot = metrics.snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.orders_created, 1);
        assert_eq!(snapshot.customers_created, 1);
        assert_eq!(snapshot.products_created, 1);
        assert_eq!(snapshot.returns_requested, 1);
        assert_eq!(snapshot.carts_created, 1);
        assert_eq!(snapshot.cart_checkouts_completed, 1);
        assert_eq!(snapshot.shipments_created, 1);
        assert_eq!(snapshot.shipments_delivered, 1);
        assert_eq!(snapshot.subscriptions_created, 1);
        assert_eq!(snapshot.payments_completed, 1);
        assert_eq!(snapshot.inventory_adjustments, 2);
        assert_close(snapshot.order_amount_total, 99.99);
        assert_close(snapshot.payment_amount_total, 99.99);
        assert_close(snapshot.inventory_delta_total, -1.0);
        assert_eq!(snapshot.red_global.requests, 0);
    }

    #[test]
    fn does_not_record_when_disabled() {
        let metrics = init_metrics(MetricsConfig { enabled: false });
        metrics.record_order_created("cust-1", 50.0);
        metrics.record_customer_created("cust-1");
        metrics.record_product_created("prod-1");
        metrics.record_return_requested("ret-1");
        metrics.record_cart_created("cart-1");
        metrics.record_cart_checkout_completed("cart-1", "ord-1");
        metrics.record_shipment_created("shp-1");
        metrics.record_shipment_delivered("shp-1");
        metrics.record_subscription_created("sub-1");
        metrics.record_payment_completed("pay-1", 50.0);
        metrics.record_inventory_adjusted("sku-1", 3.0);
        metrics.record_request_success("order.create", Duration::from_millis(25));

        let snapshot = metrics.snapshot();
        assert!(!snapshot.enabled);
        assert_eq!(snapshot.orders_created, 0);
        assert_eq!(snapshot.customers_created, 0);
        assert_eq!(snapshot.products_created, 0);
        assert_eq!(snapshot.returns_requested, 0);
        assert_eq!(snapshot.carts_created, 0);
        assert_eq!(snapshot.cart_checkouts_completed, 0);
        assert_eq!(snapshot.shipments_created, 0);
        assert_eq!(snapshot.shipments_delivered, 0);
        assert_eq!(snapshot.subscriptions_created, 0);
        assert_eq!(snapshot.payments_completed, 0);
        assert_eq!(snapshot.inventory_adjustments, 0);
        assert_close(snapshot.order_amount_total, 0.0);
        assert_close(snapshot.payment_amount_total, 0.0);
        assert_close(snapshot.inventory_delta_total, 0.0);
        assert_eq!(snapshot.red_global.requests, 0);
        assert!(snapshot.red_by_operation.is_empty());
    }

    #[test]
    fn can_toggle_collection_runtime() {
        let metrics = init_metrics(MetricsConfig { enabled: true });
        metrics.record_order_created("cust-1", 10.0);
        metrics.record_customer_created("cust-1");
        metrics.record_product_created("prod-1");
        metrics.record_cart_created("cart-1");
        metrics.record_shipment_created("shp-1");
        metrics.record_subscription_created("sub-1");

        metrics.set_enabled(false);
        metrics.record_order_created("cust-2", 20.0);
        metrics.record_customer_created("cust-2");
        metrics.record_product_created("prod-2");
        metrics.record_return_requested("ret-1");
        metrics.record_cart_created("cart-2");
        metrics.record_cart_checkout_completed("cart-1", "ord-1");
        metrics.record_shipment_created("shp-2");
        metrics.record_shipment_delivered("shp-1");
        metrics.record_subscription_created("sub-2");
        metrics.record_payment_completed("pay-2", 20.0);
        metrics.record_request_error("order.create", Duration::from_millis(100));

        let snapshot = metrics.snapshot();
        assert!(!snapshot.enabled);
        assert_eq!(snapshot.orders_created, 1);
        assert_eq!(snapshot.customers_created, 1);
        assert_eq!(snapshot.products_created, 1);
        assert_eq!(snapshot.returns_requested, 0);
        assert_eq!(snapshot.carts_created, 1);
        assert_eq!(snapshot.cart_checkouts_completed, 0);
        assert_eq!(snapshot.shipments_created, 1);
        assert_eq!(snapshot.shipments_delivered, 0);
        assert_eq!(snapshot.subscriptions_created, 1);
        assert_eq!(snapshot.payments_completed, 0);
        assert_close(snapshot.order_amount_total, 10.0);
        assert_eq!(snapshot.red_global.requests, 0);
    }

    #[test]
    fn records_red_metrics_with_normalized_operation_labels() {
        let metrics = init_metrics(MetricsConfig { enabled: true });
        metrics.record_request_success("Order Created", Duration::from_millis(120));
        metrics.record_request_error("Order Created", Duration::from_millis(220));
        metrics.record_request_success("payment/authorize", Duration::from_millis(80));

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.red_global.requests, 3);
        assert_eq!(snapshot.red_global.errors, 1);
        assert_close(snapshot.red_global.error_rate, 1.0 / 3.0);
        assert_close(snapshot.red_global.avg_duration_ms, 140.0);

        let order = snapshot.red_by_operation.get("order_created").unwrap();
        assert_eq!(order.requests, 2);
        assert_eq!(order.errors, 1);
        assert_close(order.avg_duration_ms, 170.0);

        let payment = snapshot.red_by_operation.get("payment_authorize").unwrap();
        assert_eq!(payment.requests, 1);
        assert_eq!(payment.errors, 0);
        assert_close(payment.avg_duration_ms, 80.0);
    }

    #[test]
    fn slo_evaluation_passes_and_fails() {
        let metrics = init_metrics(MetricsConfig { enabled: true });
        metrics.record_request_success("checkout", Duration::from_millis(100));
        metrics.record_request_success("checkout", Duration::from_millis(120));
        metrics.record_request_error("checkout", Duration::from_millis(180));

        let snapshot = metrics.snapshot();
        let pass_target =
            SloTarget { min_success_rate: 0.60, max_avg_latency_ms: 200.0, min_requests: 3 };
        let pass = snapshot.evaluate_operation_slo("checkout", pass_target).unwrap();
        assert!(pass.passed);

        let fail_target =
            SloTarget { min_success_rate: 0.90, max_avg_latency_ms: 120.0, min_requests: 3 };
        let fail = snapshot.evaluate_operation_slo("checkout", fail_target).unwrap();
        assert!(!fail.passed);
        assert!(fail.reason.is_some());
    }

    #[test]
    fn slo_evaluation_requires_minimum_samples() {
        let metrics = init_metrics(MetricsConfig { enabled: true });
        metrics.record_request_success("inventory.sync", Duration::from_millis(20));

        let snapshot = metrics.snapshot();
        let report = snapshot
            .evaluate_operation_slo(
                "inventory.sync",
                SloTarget { min_success_rate: 0.99, max_avg_latency_ms: 50.0, min_requests: 10 },
            )
            .unwrap();
        assert!(!report.passed);
        assert!(report.reason.unwrap().contains("insufficient requests"));
    }
}
