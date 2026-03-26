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
    /// p50 latency in milliseconds (0.0 if no histogram data).
    pub p50_ms: f64,
    /// p95 latency in milliseconds (0.0 if no histogram data).
    pub p95_ms: f64,
    /// p99 latency in milliseconds (0.0 if no histogram data).
    pub p99_ms: f64,
}

impl RedSnapshot {
    fn from_counts(requests: u64, errors: u64, duration_micros_total: u64) -> Self {
        let duration_total_ms = duration_micros_total as f64 / 1_000.0;
        let error_rate = if requests == 0 { 0.0 } else { errors as f64 / requests as f64 };
        let avg_duration_ms = if requests == 0 { 0.0 } else { duration_total_ms / requests as f64 };

        Self {
            requests,
            errors,
            duration_total_ms,
            error_rate,
            avg_duration_ms,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
        }
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
    /// Number of A2A quotes created.
    pub a2a_quotes_created: u64,
    /// Number of A2A purchases created.
    pub a2a_purchases_created: u64,
    /// Number of x402 payment intents created.
    pub x402_intents_created: u64,
    /// Number of x402 payment intents settled.
    pub x402_intents_settled: u64,
    /// Number of policy evaluations performed.
    pub policy_evaluations: u64,
    /// Number of policy denials.
    pub policy_denials: u64,
    /// Number of agent registrations.
    pub agent_registrations: u64,
    /// Number of webhook deliveries attempted.
    pub webhook_deliveries: u64,
    /// Number of webhook delivery failures.
    pub webhook_failures: u64,
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

/// Simple sorted-insert histogram for latency percentiles.
///
/// Provides p50/p95/p99 latency tracking for individual operations.
const MAX_HISTOGRAM_SAMPLES: usize = 4_096;

/// Simple sorted-insert histogram for latency percentiles.
///
/// Samples are bounded to keep memory usage stable under sustained traffic.
///
/// # Example
///
/// ```rust
/// use stateset_observability::LatencyHistogram;
///
/// let mut h = LatencyHistogram::new();
/// for i in 1..=100 {
///     h.record(i * 1000); // 1ms to 100ms in micros
/// }
/// assert!(h.percentile(0.50) > 0.0);
/// assert!(h.percentile(0.95) > h.percentile(0.50));
/// ```
#[derive(Debug, Clone)]
pub struct LatencyHistogram {
    values: Vec<u64>,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyHistogram {
    /// Create a new empty histogram.
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Record a latency value in microseconds.
    pub fn record(&mut self, duration_micros: u64) {
        if self.values.len() >= MAX_HISTOGRAM_SAMPLES {
            self.compact();
        }
        let pos = self.values.binary_search(&duration_micros).unwrap_or_else(|e| e);
        self.values.insert(pos, duration_micros);
    }

    /// Get a percentile (0.0 to 1.0) in milliseconds. Returns 0.0 if empty.
    #[must_use]
    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let p = p.clamp(0.0, 1.0);
        let idx = ((self.values.len() as f64 * p) - 1.0).max(0.0) as usize;
        let idx = idx.min(self.values.len() - 1);
        self.values[idx] as f64 / 1_000.0
    }

    /// Number of recorded values.
    #[must_use]
    pub fn count(&self) -> usize {
        self.values.len()
    }

    fn compact(&mut self) {
        self.values = self.values.iter().step_by(2).copied().collect();
    }
}

#[derive(Debug, Clone, Default)]
struct RedAccumulator {
    requests: u64,
    errors: u64,
    duration_micros_total: u64,
    histogram: LatencyHistogram,
}

impl RedAccumulator {
    fn record(&mut self, duration_micros: u64, is_error: bool) {
        self.requests = self.requests.saturating_add(1);
        if is_error {
            self.errors = self.errors.saturating_add(1);
        }
        self.duration_micros_total = self.duration_micros_total.saturating_add(duration_micros);
        self.histogram.record(duration_micros);
    }

    fn snapshot(&self) -> RedSnapshot {
        let mut snap =
            RedSnapshot::from_counts(self.requests, self.errors, self.duration_micros_total);
        snap.p50_ms = self.histogram.percentile(0.50);
        snap.p95_ms = self.histogram.percentile(0.95);
        snap.p99_ms = self.histogram.percentile(0.99);
        snap
    }
}

#[derive(Debug, Default)]
struct MetricTotals {
    // f64 totals (order_amount, payment_amount, inventory_delta) moved to
    // lock-free AtomicU64 fields on MetricsInner for contention-free hot paths.
    red_global: RedAccumulator,
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
    a2a_quotes_created: AtomicU64,
    a2a_purchases_created: AtomicU64,
    x402_intents_created: AtomicU64,
    x402_intents_settled: AtomicU64,
    policy_evaluations: AtomicU64,
    policy_denials: AtomicU64,
    agent_registrations: AtomicU64,
    webhook_deliveries: AtomicU64,
    webhook_failures: AtomicU64,
    requests_total: AtomicU64,
    request_errors_total: AtomicU64,
    request_duration_micros_total: AtomicU64,
    // Lock-free f64 accumulators (stored as u64 bits via to_bits/from_bits)
    order_amount_total_bits: AtomicU64,
    payment_amount_total_bits: AtomicU64,
    inventory_delta_total_bits: AtomicU64,
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
    const fn is_finite_metric_value(value: f64) -> bool {
        value.is_finite()
    }

    /// Lock-free f64 addition using CAS loop on `AtomicU64` bit representation.
    fn atomic_f64_add(target: &AtomicU64, value: f64) {
        let mut current = target.load(Ordering::Relaxed);
        loop {
            let current_f64 = f64::from_bits(current);
            let new_f64 = current_f64 + value;
            match target.compare_exchange_weak(
                current,
                new_f64.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Whether this metrics instance currently records values.
    #[must_use] 
    pub fn is_enabled(&self) -> bool {
        self.inner.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable metrics collection at runtime.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Return a point-in-time snapshot of current metrics values.
    #[must_use] 
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

        let red_global = totals.red_global.snapshot();

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
            a2a_quotes_created: self.inner.a2a_quotes_created.load(Ordering::Relaxed),
            a2a_purchases_created: self.inner.a2a_purchases_created.load(Ordering::Relaxed),
            x402_intents_created: self.inner.x402_intents_created.load(Ordering::Relaxed),
            x402_intents_settled: self.inner.x402_intents_settled.load(Ordering::Relaxed),
            policy_evaluations: self.inner.policy_evaluations.load(Ordering::Relaxed),
            policy_denials: self.inner.policy_denials.load(Ordering::Relaxed),
            agent_registrations: self.inner.agent_registrations.load(Ordering::Relaxed),
            webhook_deliveries: self.inner.webhook_deliveries.load(Ordering::Relaxed),
            webhook_failures: self.inner.webhook_failures.load(Ordering::Relaxed),
            order_amount_total: f64::from_bits(self.inner.order_amount_total_bits.load(Ordering::Relaxed)),
            payment_amount_total: f64::from_bits(self.inner.payment_amount_total_bits.load(Ordering::Relaxed)),
            inventory_delta_total: f64::from_bits(self.inner.inventory_delta_total_bits.load(Ordering::Relaxed)),
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
        if Self::is_finite_metric_value(amount) {
            Self::atomic_f64_add(&self.inner.order_amount_total_bits, amount);
        }
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
        if Self::is_finite_metric_value(amount) {
            Self::atomic_f64_add(&self.inner.payment_amount_total_bits, amount);
        }
    }

    /// Record an inventory adjustment.
    pub fn record_inventory_adjusted(&self, _sku: &str, delta: f64) {
        if !self.is_enabled() {
            return;
        }
        self.inner.inventory_adjustments.fetch_add(1, Ordering::Relaxed);
        if Self::is_finite_metric_value(delta) {
            Self::atomic_f64_add(&self.inner.inventory_delta_total_bits, delta);
        }
    }

    /// Record an A2A quote creation.
    pub fn record_a2a_quote_created(&self) {
        if !self.is_enabled() {
            return;
        }
        self.inner.a2a_quotes_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an A2A purchase creation.
    pub fn record_a2a_purchase_created(&self) {
        if !self.is_enabled() {
            return;
        }
        self.inner.a2a_purchases_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an x402 payment intent creation.
    pub fn record_x402_intent_created(&self) {
        if !self.is_enabled() {
            return;
        }
        self.inner.x402_intents_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an x402 payment intent settlement.
    pub fn record_x402_intent_settled(&self) {
        if !self.is_enabled() {
            return;
        }
        self.inner.x402_intents_settled.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a policy evaluation.
    pub fn record_policy_evaluation(&self, denied: bool) {
        if !self.is_enabled() {
            return;
        }
        self.inner.policy_evaluations.fetch_add(1, Ordering::Relaxed);
        if denied {
            self.inner.policy_denials.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record an agent registration.
    pub fn record_agent_registration(&self) {
        if !self.is_enabled() {
            return;
        }
        self.inner.agent_registrations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a webhook delivery attempt.
    pub fn record_webhook_delivery(&self, failed: bool) {
        if !self.is_enabled() {
            return;
        }
        self.inner.webhook_deliveries.fetch_add(1, Ordering::Relaxed);
        if failed {
            self.inner.webhook_failures.fetch_add(1, Ordering::Relaxed);
        }
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
        totals.red_global.record(duration_micros, is_error);
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
#[must_use] 
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
            a2a_quotes_created: AtomicU64::new(0),
            a2a_purchases_created: AtomicU64::new(0),
            x402_intents_created: AtomicU64::new(0),
            x402_intents_settled: AtomicU64::new(0),
            policy_evaluations: AtomicU64::new(0),
            policy_denials: AtomicU64::new(0),
            agent_registrations: AtomicU64::new(0),
            webhook_deliveries: AtomicU64::new(0),
            webhook_failures: AtomicU64::new(0),
            requests_total: AtomicU64::new(0),
            request_errors_total: AtomicU64::new(0),
            request_duration_micros_total: AtomicU64::new(0),
            order_amount_total_bits: AtomicU64::new(0u64),
            payment_amount_total_bits: AtomicU64::new(0u64),
            inventory_delta_total_bits: AtomicU64::new(0u64),
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
        assert!(snapshot.red_global.p50_ms > 0.0);
        assert!(snapshot.red_global.p95_ms >= snapshot.red_global.p50_ms);

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

    #[test]
    fn agentic_counters_record() {
        let metrics = init_metrics(MetricsConfig::default());
        metrics.record_a2a_quote_created();
        metrics.record_a2a_quote_created();
        metrics.record_a2a_purchase_created();
        metrics.record_x402_intent_created();
        metrics.record_x402_intent_settled();
        metrics.record_policy_evaluation(false);
        metrics.record_policy_evaluation(true);
        metrics.record_agent_registration();
        metrics.record_webhook_delivery(false);
        metrics.record_webhook_delivery(true);

        let snap = metrics.snapshot();
        assert_eq!(snap.a2a_quotes_created, 2);
        assert_eq!(snap.a2a_purchases_created, 1);
        assert_eq!(snap.x402_intents_created, 1);
        assert_eq!(snap.x402_intents_settled, 1);
        assert_eq!(snap.policy_evaluations, 2);
        assert_eq!(snap.policy_denials, 1);
        assert_eq!(snap.agent_registrations, 1);
        assert_eq!(snap.webhook_deliveries, 2);
        assert_eq!(snap.webhook_failures, 1);
    }

    #[test]
    fn histogram_empty() {
        let h = LatencyHistogram::new();
        assert_eq!(h.count(), 0);
        assert_eq!(h.percentile(0.5), 0.0);
        assert_eq!(h.percentile(0.99), 0.0);
    }

    #[test]
    fn histogram_single_value() {
        let mut h = LatencyHistogram::new();
        h.record(5000); // 5ms
        assert_eq!(h.count(), 1);
        assert_eq!(h.percentile(0.5), 5.0);
        assert_eq!(h.percentile(0.99), 5.0);
    }

    #[test]
    fn histogram_percentiles() {
        let mut h = LatencyHistogram::new();
        for i in 1..=100 {
            h.record(i * 1000);
        }
        assert_eq!(h.count(), 100);
        let p50 = h.percentile(0.50);
        let p95 = h.percentile(0.95);
        let p99 = h.percentile(0.99);
        assert!((49.0..=51.0).contains(&p50), "p50 was {p50}");
        assert!((94.0..=96.0).contains(&p95), "p95 was {p95}");
        assert!((98.0..=100.0).contains(&p99), "p99 was {p99}");
    }

    #[test]
    fn histogram_clamps_percentile() {
        let mut h = LatencyHistogram::new();
        h.record(1000);
        assert_eq!(h.percentile(-1.0), 1.0);
        assert_eq!(h.percentile(2.0), 1.0);
    }

    #[test]
    fn histogram_caps_retained_samples() {
        let mut h = LatencyHistogram::new();
        for i in 0..10_000 {
            h.record(i);
        }
        assert!(h.count() <= MAX_HISTOGRAM_SAMPLES);
    }

    #[test]
    fn red_snapshot_includes_percentiles() {
        let metrics = init_metrics(MetricsConfig::default());
        metrics.record_request_success("test.op", Duration::from_millis(10));
        metrics.record_request_success("test.op", Duration::from_millis(50));
        metrics.record_request_success("test.op", Duration::from_millis(100));

        let snap = metrics.snapshot();
        if let Some(red) = snap.red_by_operation.get("test_op") {
            assert_eq!(red.requests, 3);
            assert!(red.p50_ms > 0.0, "p50 should be > 0");
        } else {
            panic!("Expected test_op in red_by_operation");
        }
    }

    #[test]
    fn ignores_non_finite_metric_totals() {
        let metrics = init_metrics(MetricsConfig::default());
        metrics.record_order_created("cust-1", f64::NAN);
        metrics.record_payment_completed("pay-1", f64::INFINITY);
        metrics.record_inventory_adjusted("sku-1", f64::NEG_INFINITY);

        let snap = metrics.snapshot();
        assert_eq!(snap.orders_created, 1);
        assert_eq!(snap.payments_completed, 1);
        assert_eq!(snap.inventory_adjustments, 1);
        assert_close(snap.order_amount_total, 0.0);
        assert_close(snap.payment_amount_total, 0.0);
        assert_close(snap.inventory_delta_total, 0.0);
    }

    // ── MetricsConfig ──────────────────────────────────────────────────

    #[test]
    fn metrics_config_default_is_enabled() {
        let cfg = MetricsConfig::default();
        assert!(cfg.enabled);
    }

    #[test]
    fn metrics_config_disabled() {
        let cfg = MetricsConfig { enabled: false };
        assert!(!cfg.enabled);
    }

    #[test]
    fn metrics_config_debug() {
        let cfg = MetricsConfig::default();
        let debug = format!("{cfg:?}");
        assert!(debug.contains("MetricsConfig"));
        assert!(debug.contains("true"));
    }

    #[test]
    fn metrics_config_clone() {
        let cfg = MetricsConfig { enabled: false };
        #[allow(clippy::redundant_clone)]
        let cloned = cfg.clone();
        assert!(!cloned.enabled);
    }

    // ── Metrics default / debug / clone ────────────────────────────────

    #[test]
    fn metrics_default_is_enabled() {
        let m = Metrics::default();
        assert!(m.is_enabled());
    }

    #[test]
    fn metrics_debug_format() {
        let m = init_metrics(MetricsConfig::default());
        let debug = format!("{m:?}");
        assert!(debug.contains("Metrics"));
    }

    #[test]
    fn metrics_clone_shares_state() {
        let m = init_metrics(MetricsConfig::default());
        let m2 = m.clone();
        m.record_order_created("c", 10.0);
        let snap = m2.snapshot();
        assert_eq!(snap.orders_created, 1);
    }

    // ── set_enabled toggle ─────────────────────────────────────────────

    #[test]
    fn set_enabled_true_to_false() {
        let m = init_metrics(MetricsConfig { enabled: true });
        assert!(m.is_enabled());
        m.set_enabled(false);
        assert!(!m.is_enabled());
    }

    #[test]
    fn set_enabled_false_to_true() {
        let m = init_metrics(MetricsConfig { enabled: false });
        assert!(!m.is_enabled());
        m.set_enabled(true);
        assert!(m.is_enabled());
    }

    #[test]
    fn set_enabled_idempotent() {
        let m = init_metrics(MetricsConfig { enabled: true });
        m.set_enabled(true);
        m.set_enabled(true);
        assert!(m.is_enabled());
    }

    // ── Disabled mode: all record_* are no-ops ─────────────────────────

    #[test]
    fn disabled_does_not_record_a2a_counters() {
        let m = init_metrics(MetricsConfig { enabled: false });
        m.record_a2a_quote_created();
        m.record_a2a_purchase_created();
        m.record_x402_intent_created();
        m.record_x402_intent_settled();
        let snap = m.snapshot();
        assert_eq!(snap.a2a_quotes_created, 0);
        assert_eq!(snap.a2a_purchases_created, 0);
        assert_eq!(snap.x402_intents_created, 0);
        assert_eq!(snap.x402_intents_settled, 0);
    }

    #[test]
    fn disabled_does_not_record_policy_counters() {
        let m = init_metrics(MetricsConfig { enabled: false });
        m.record_policy_evaluation(false);
        m.record_policy_evaluation(true);
        let snap = m.snapshot();
        assert_eq!(snap.policy_evaluations, 0);
        assert_eq!(snap.policy_denials, 0);
    }

    #[test]
    fn disabled_does_not_record_agent_registration() {
        let m = init_metrics(MetricsConfig { enabled: false });
        m.record_agent_registration();
        assert_eq!(m.snapshot().agent_registrations, 0);
    }

    #[test]
    fn disabled_does_not_record_webhook_delivery() {
        let m = init_metrics(MetricsConfig { enabled: false });
        m.record_webhook_delivery(false);
        m.record_webhook_delivery(true);
        let snap = m.snapshot();
        assert_eq!(snap.webhook_deliveries, 0);
        assert_eq!(snap.webhook_failures, 0);
    }

    #[test]
    fn disabled_does_not_record_request_error() {
        let m = init_metrics(MetricsConfig { enabled: false });
        m.record_request_error("op", Duration::from_millis(100));
        let snap = m.snapshot();
        assert_eq!(snap.red_global.requests, 0);
        assert_eq!(snap.red_global.errors, 0);
    }

    // ── RedSnapshot calculations ───────────────────────────────────────

    #[test]
    fn red_snapshot_zero_requests() {
        let snap = RedSnapshot::from_counts(0, 0, 0);
        assert_eq!(snap.requests, 0);
        assert_eq!(snap.errors, 0);
        assert_close(snap.error_rate, 0.0);
        assert_close(snap.avg_duration_ms, 0.0);
        assert_close(snap.duration_total_ms, 0.0);
    }

    #[test]
    fn red_snapshot_all_errors() {
        let snap = RedSnapshot::from_counts(5, 5, 500_000);
        assert_eq!(snap.requests, 5);
        assert_eq!(snap.errors, 5);
        assert_close(snap.error_rate, 1.0);
        assert_close(snap.avg_duration_ms, 100.0);
    }

    #[test]
    fn red_snapshot_no_errors() {
        let snap = RedSnapshot::from_counts(10, 0, 1_000_000);
        assert_close(snap.error_rate, 0.0);
        assert_close(snap.avg_duration_ms, 100.0);
    }

    #[test]
    fn red_snapshot_partial_errors() {
        let snap = RedSnapshot::from_counts(4, 1, 400_000);
        assert_close(snap.error_rate, 0.25);
        assert_close(snap.avg_duration_ms, 100.0);
    }

    #[test]
    fn red_snapshot_duration_total_ms_conversion() {
        // 1_500_000 micros = 1500.0 ms
        let snap = RedSnapshot::from_counts(3, 0, 1_500_000);
        assert_close(snap.duration_total_ms, 1500.0);
        assert_close(snap.avg_duration_ms, 500.0);
    }

    #[test]
    fn red_snapshot_single_request() {
        let snap = RedSnapshot::from_counts(1, 0, 42_000);
        assert_close(snap.error_rate, 0.0);
        assert_close(snap.avg_duration_ms, 42.0);
    }

    #[test]
    fn red_snapshot_clone_eq() {
        let snap = RedSnapshot::from_counts(10, 2, 500_000);
        let snap2 = snap.clone();
        assert_eq!(snap, snap2);
    }

    #[test]
    fn red_snapshot_debug() {
        let snap = RedSnapshot::from_counts(1, 0, 1000);
        let debug = format!("{snap:?}");
        assert!(debug.contains("RedSnapshot"));
    }

    #[test]
    fn red_snapshot_default_percentiles_zero() {
        let snap = RedSnapshot::from_counts(5, 1, 100_000);
        assert_close(snap.p50_ms, 0.0);
        assert_close(snap.p95_ms, 0.0);
        assert_close(snap.p99_ms, 0.0);
    }

    // ── SLO evaluation edge cases ──────────────────────────────────────

    #[test]
    fn slo_evaluation_zero_requests() {
        let snap = RedSnapshot::from_counts(0, 0, 0);
        let target =
            SloTarget { min_success_rate: 0.99, max_avg_latency_ms: 100.0, min_requests: 1 };
        let eval = snap.evaluate_slo(target);
        assert!(!eval.passed);
        assert!(eval.reason.unwrap().contains("insufficient requests"));
    }

    #[test]
    fn slo_evaluation_min_requests_zero_threshold() {
        let snap = RedSnapshot::from_counts(0, 0, 0);
        let target =
            SloTarget { min_success_rate: 0.0, max_avg_latency_ms: f64::MAX, min_requests: 0 };
        let eval = snap.evaluate_slo(target);
        // 0 >= 0 min_requests, 0.0 error_rate -> success 1.0 >= 0.0, avg 0.0 <= MAX
        assert!(eval.passed);
    }

    #[test]
    fn slo_evaluation_exactly_at_threshold() {
        // 10 requests, 1 error -> success_rate = 0.9 exactly
        let snap = RedSnapshot::from_counts(10, 1, 1_000_000);
        let target =
            SloTarget { min_success_rate: 0.9, max_avg_latency_ms: 100.0, min_requests: 10 };
        let eval = snap.evaluate_slo(target);
        assert!(eval.passed, "Should pass when success_rate == min_success_rate");
        assert!(eval.reason.is_none());
    }

    #[test]
    fn slo_evaluation_just_below_success_threshold() {
        // 100 requests, 11 errors -> success_rate = 0.89
        let snap = RedSnapshot::from_counts(100, 11, 10_000_000);
        let target =
            SloTarget { min_success_rate: 0.9, max_avg_latency_ms: 200.0, min_requests: 10 };
        let eval = snap.evaluate_slo(target);
        assert!(!eval.passed);
        assert!(eval.reason.unwrap().contains("success rate"));
    }

    #[test]
    fn slo_evaluation_latency_just_above_threshold() {
        // 10 requests, 0 errors, total 2_000_000 micros -> avg = 200ms
        let snap = RedSnapshot::from_counts(10, 0, 2_000_000);
        let target =
            SloTarget { min_success_rate: 0.9, max_avg_latency_ms: 199.0, min_requests: 5 };
        let eval = snap.evaluate_slo(target);
        assert!(!eval.passed);
        assert!(eval.reason.unwrap().contains("avg latency"));
    }

    #[test]
    fn slo_evaluation_latency_exactly_at_threshold() {
        let snap = RedSnapshot::from_counts(10, 0, 1_000_000);
        let target =
            SloTarget { min_success_rate: 0.9, max_avg_latency_ms: 100.0, min_requests: 5 };
        let eval = snap.evaluate_slo(target);
        assert!(eval.passed);
    }

    #[test]
    fn slo_evaluation_all_errors() {
        let snap = RedSnapshot::from_counts(10, 10, 500_000);
        let target =
            SloTarget { min_success_rate: 0.01, max_avg_latency_ms: 1000.0, min_requests: 1 };
        let eval = snap.evaluate_slo(target);
        assert!(!eval.passed);
        assert!(eval.reason.unwrap().contains("success rate"));
    }

    #[test]
    fn slo_evaluation_report_fields() {
        let snap = RedSnapshot::from_counts(20, 4, 2_000_000);
        let target =
            SloTarget { min_success_rate: 0.5, max_avg_latency_ms: 200.0, min_requests: 10 };
        let eval = snap.evaluate_slo(target);
        assert!(eval.passed);
        assert_eq!(eval.requests, 20);
        assert_close(eval.error_rate, 0.2);
        assert_close(eval.success_rate, 0.8);
        assert_close(eval.avg_duration_ms, 100.0);
        assert!(eval.reason.is_none());
    }

    #[test]
    fn slo_default_target_values() {
        let target = SloTarget::default();
        assert_close(target.min_success_rate, 0.99);
        assert_close(target.max_avg_latency_ms, 250.0);
        assert_eq!(target.min_requests, 100);
    }

    #[test]
    fn slo_target_debug() {
        let target = SloTarget::default();
        let debug = format!("{target:?}");
        assert!(debug.contains("SloTarget"));
    }

    #[test]
    fn slo_target_clone() {
        let target =
            SloTarget { min_success_rate: 0.95, max_avg_latency_ms: 100.0, min_requests: 50 };
        let cloned = target;
        assert_eq!(target, cloned);
    }

    #[test]
    fn slo_evaluation_debug() {
        let snap = RedSnapshot::from_counts(5, 0, 50_000);
        let eval = snap.evaluate_slo(SloTarget::default());
        let debug = format!("{eval:?}");
        assert!(debug.contains("SloEvaluation"));
    }

    #[test]
    fn slo_evaluation_clone_eq() {
        let snap = RedSnapshot::from_counts(100, 1, 1_000_000);
        let eval1 = snap.evaluate_slo(SloTarget::default());
        let eval2 = eval1.clone();
        assert_eq!(eval1, eval2);
    }

    // ── Global SLO evaluation ──────────────────────────────────────────

    #[test]
    fn evaluate_global_slo_passes() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..100 {
            m.record_request_success("op", Duration::from_millis(10));
        }
        let snap = m.snapshot();
        let target =
            SloTarget { min_success_rate: 0.99, max_avg_latency_ms: 50.0, min_requests: 100 };
        let eval = snap.evaluate_global_slo(target);
        assert!(eval.passed);
    }

    #[test]
    fn evaluate_global_slo_fails_on_errors() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..90 {
            m.record_request_success("op", Duration::from_millis(10));
        }
        for _ in 0..10 {
            m.record_request_error("op", Duration::from_millis(10));
        }
        let snap = m.snapshot();
        let target =
            SloTarget { min_success_rate: 0.95, max_avg_latency_ms: 50.0, min_requests: 50 };
        let eval = snap.evaluate_global_slo(target);
        assert!(!eval.passed);
    }

    // ── evaluate_operation_slo ─────────────────────────────────────────

    #[test]
    fn evaluate_operation_slo_returns_none_for_unknown() {
        let m = init_metrics(MetricsConfig::default());
        let snap = m.snapshot();
        let result = snap.evaluate_operation_slo("nonexistent", SloTarget::default());
        assert!(result.is_none());
    }

    #[test]
    fn evaluate_operation_slo_normalizes_key() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("Order Created", Duration::from_millis(10));
        let snap = m.snapshot();
        // Query with different casing/separator — should still resolve via normalization
        let result = snap.evaluate_operation_slo(
            "order-created",
            SloTarget { min_success_rate: 0.5, max_avg_latency_ms: 100.0, min_requests: 1 },
        );
        assert!(result.is_some());
        assert!(result.unwrap().passed);
    }

    // ── Multiple operations tracked independently ──────────────────────

    #[test]
    fn multiple_operations_independent() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("order.create", Duration::from_millis(50));
        m.record_request_error("payment.charge", Duration::from_millis(200));
        m.record_request_success("inventory.check", Duration::from_millis(5));

        let snap = m.snapshot();
        assert_eq!(snap.red_by_operation.len(), 3);

        let order = snap.red_by_operation.get("order_create").unwrap();
        assert_eq!(order.requests, 1);
        assert_eq!(order.errors, 0);

        let payment = snap.red_by_operation.get("payment_charge").unwrap();
        assert_eq!(payment.requests, 1);
        assert_eq!(payment.errors, 1);

        let inv = snap.red_by_operation.get("inventory_check").unwrap();
        assert_eq!(inv.requests, 1);
        assert_eq!(inv.errors, 0);
    }

    #[test]
    fn same_operation_accumulates() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..50 {
            m.record_request_success("checkout", Duration::from_millis(20));
        }
        for _ in 0..5 {
            m.record_request_error("checkout", Duration::from_millis(100));
        }
        let snap = m.snapshot();
        let checkout = snap.red_by_operation.get("checkout").unwrap();
        assert_eq!(checkout.requests, 55);
        assert_eq!(checkout.errors, 5);
        assert_close(checkout.error_rate, 5.0 / 55.0);
    }

    // ── Snapshot isolation ─────────────────────────────────────────────

    #[test]
    fn snapshot_is_point_in_time() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c", 10.0);
        let snap1 = m.snapshot();

        // Record more after snapshot
        m.record_order_created("c", 20.0);
        m.record_order_created("c", 30.0);

        // snap1 should still reflect original state
        assert_eq!(snap1.orders_created, 1);
        assert_close(snap1.order_amount_total, 10.0);

        // New snapshot should reflect new state
        let snap2 = m.snapshot();
        assert_eq!(snap2.orders_created, 3);
        assert_close(snap2.order_amount_total, 60.0);
    }

    #[test]
    fn snapshot_red_isolation() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::from_millis(10));
        let snap1 = m.snapshot();

        m.record_request_error("op", Duration::from_millis(200));
        let snap2 = m.snapshot();

        assert_eq!(snap1.red_global.requests, 1);
        assert_eq!(snap1.red_global.errors, 0);
        assert_eq!(snap2.red_global.requests, 2);
        assert_eq!(snap2.red_global.errors, 1);
    }

    // ── record_* with various values ───────────────────────────────────

    #[test]
    fn record_order_zero_amount() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c", 0.0);
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 1);
        assert_close(snap.order_amount_total, 0.0);
    }

    #[test]
    fn record_order_large_amount() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c", 1_000_000.99);
        let snap = m.snapshot();
        assert_close(snap.order_amount_total, 1_000_000.99);
    }

    #[test]
    fn record_order_negative_amount() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c", -50.0);
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 1);
        assert_close(snap.order_amount_total, -50.0);
    }

    #[test]
    fn record_payment_zero_amount() {
        let m = init_metrics(MetricsConfig::default());
        m.record_payment_completed("p", 0.0);
        let snap = m.snapshot();
        assert_eq!(snap.payments_completed, 1);
        assert_close(snap.payment_amount_total, 0.0);
    }

    #[test]
    fn record_payment_large_amount() {
        let m = init_metrics(MetricsConfig::default());
        m.record_payment_completed("p", 999_999.99);
        let snap = m.snapshot();
        assert_close(snap.payment_amount_total, 999_999.99);
    }

    #[test]
    fn record_inventory_zero_delta() {
        let m = init_metrics(MetricsConfig::default());
        m.record_inventory_adjusted("sku", 0.0);
        let snap = m.snapshot();
        assert_eq!(snap.inventory_adjustments, 1);
        assert_close(snap.inventory_delta_total, 0.0);
    }

    #[test]
    fn record_inventory_positive_delta() {
        let m = init_metrics(MetricsConfig::default());
        m.record_inventory_adjusted("sku", 100.0);
        let snap = m.snapshot();
        assert_close(snap.inventory_delta_total, 100.0);
    }

    #[test]
    fn record_inventory_negative_delta() {
        let m = init_metrics(MetricsConfig::default());
        m.record_inventory_adjusted("sku", -5.5);
        let snap = m.snapshot();
        assert_close(snap.inventory_delta_total, -5.5);
    }

    #[test]
    fn record_multiple_inventory_adjustments_accumulate() {
        let m = init_metrics(MetricsConfig::default());
        m.record_inventory_adjusted("a", 10.0);
        m.record_inventory_adjusted("b", -3.0);
        m.record_inventory_adjusted("c", 7.5);
        let snap = m.snapshot();
        assert_eq!(snap.inventory_adjustments, 3);
        assert_close(snap.inventory_delta_total, 14.5);
    }

    #[test]
    fn record_multiple_orders_accumulate_amounts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c1", 10.0);
        m.record_order_created("c2", 20.0);
        m.record_order_created("c3", 30.0);
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 3);
        assert_close(snap.order_amount_total, 60.0);
    }

    #[test]
    fn record_multiple_payments_accumulate_amounts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_payment_completed("p1", 100.0);
        m.record_payment_completed("p2", 200.0);
        let snap = m.snapshot();
        assert_eq!(snap.payments_completed, 2);
        assert_close(snap.payment_amount_total, 300.0);
    }

    // ── Individual counter tests ───────────────────────────────────────

    #[test]
    fn record_customer_created_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_customer_created("c1");
        m.record_customer_created("c2");
        m.record_customer_created("c3");
        assert_eq!(m.snapshot().customers_created, 3);
    }

    #[test]
    fn record_product_created_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_product_created("p1");
        m.record_product_created("p2");
        assert_eq!(m.snapshot().products_created, 2);
    }

    #[test]
    fn record_return_requested_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_return_requested("r1");
        m.record_return_requested("r2");
        m.record_return_requested("r3");
        m.record_return_requested("r4");
        assert_eq!(m.snapshot().returns_requested, 4);
    }

    #[test]
    fn record_cart_created_counts() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..10 {
            m.record_cart_created("cart");
        }
        assert_eq!(m.snapshot().carts_created, 10);
    }

    #[test]
    fn record_cart_checkout_completed_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_cart_checkout_completed("cart1", "ord1");
        m.record_cart_checkout_completed("cart2", "ord2");
        assert_eq!(m.snapshot().cart_checkouts_completed, 2);
    }

    #[test]
    fn record_shipment_created_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_shipment_created("s1");
        m.record_shipment_created("s2");
        m.record_shipment_created("s3");
        assert_eq!(m.snapshot().shipments_created, 3);
    }

    #[test]
    fn record_shipment_delivered_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_shipment_delivered("s1");
        assert_eq!(m.snapshot().shipments_delivered, 1);
    }

    #[test]
    fn record_subscription_created_counts() {
        let m = init_metrics(MetricsConfig::default());
        m.record_subscription_created("sub1");
        m.record_subscription_created("sub2");
        assert_eq!(m.snapshot().subscriptions_created, 2);
    }

    #[test]
    fn record_a2a_quotes_multiple() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..5 {
            m.record_a2a_quote_created();
        }
        assert_eq!(m.snapshot().a2a_quotes_created, 5);
    }

    #[test]
    fn record_a2a_purchases_multiple() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..3 {
            m.record_a2a_purchase_created();
        }
        assert_eq!(m.snapshot().a2a_purchases_created, 3);
    }

    #[test]
    fn record_x402_intents_created_multiple() {
        let m = init_metrics(MetricsConfig::default());
        m.record_x402_intent_created();
        m.record_x402_intent_created();
        assert_eq!(m.snapshot().x402_intents_created, 2);
    }

    #[test]
    fn record_x402_intents_settled_multiple() {
        let m = init_metrics(MetricsConfig::default());
        m.record_x402_intent_settled();
        m.record_x402_intent_settled();
        m.record_x402_intent_settled();
        assert_eq!(m.snapshot().x402_intents_settled, 3);
    }

    #[test]
    fn record_policy_evaluations_all_allowed() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..10 {
            m.record_policy_evaluation(false);
        }
        let snap = m.snapshot();
        assert_eq!(snap.policy_evaluations, 10);
        assert_eq!(snap.policy_denials, 0);
    }

    #[test]
    fn record_policy_evaluations_all_denied() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..5 {
            m.record_policy_evaluation(true);
        }
        let snap = m.snapshot();
        assert_eq!(snap.policy_evaluations, 5);
        assert_eq!(snap.policy_denials, 5);
    }

    #[test]
    fn record_policy_evaluations_mixed() {
        let m = init_metrics(MetricsConfig::default());
        m.record_policy_evaluation(false);
        m.record_policy_evaluation(true);
        m.record_policy_evaluation(false);
        m.record_policy_evaluation(true);
        m.record_policy_evaluation(false);
        let snap = m.snapshot();
        assert_eq!(snap.policy_evaluations, 5);
        assert_eq!(snap.policy_denials, 2);
    }

    #[test]
    fn record_agent_registrations_multiple() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..7 {
            m.record_agent_registration();
        }
        assert_eq!(m.snapshot().agent_registrations, 7);
    }

    #[test]
    fn record_webhook_deliveries_all_success() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..10 {
            m.record_webhook_delivery(false);
        }
        let snap = m.snapshot();
        assert_eq!(snap.webhook_deliveries, 10);
        assert_eq!(snap.webhook_failures, 0);
    }

    #[test]
    fn record_webhook_deliveries_all_failed() {
        let m = init_metrics(MetricsConfig::default());
        for _ in 0..3 {
            m.record_webhook_delivery(true);
        }
        let snap = m.snapshot();
        assert_eq!(snap.webhook_deliveries, 3);
        assert_eq!(snap.webhook_failures, 3);
    }

    #[test]
    fn record_webhook_deliveries_mixed() {
        let m = init_metrics(MetricsConfig::default());
        m.record_webhook_delivery(false);
        m.record_webhook_delivery(true);
        m.record_webhook_delivery(false);
        let snap = m.snapshot();
        assert_eq!(snap.webhook_deliveries, 3);
        assert_eq!(snap.webhook_failures, 1);
    }

    // ── RED request recording ──────────────────────────────────────────

    #[test]
    fn record_request_success_increments_global() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::from_millis(10));
        let snap = m.snapshot();
        assert_eq!(snap.red_global.requests, 1);
        assert_eq!(snap.red_global.errors, 0);
    }

    #[test]
    fn record_request_error_increments_global_errors() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_error("op", Duration::from_millis(10));
        let snap = m.snapshot();
        assert_eq!(snap.red_global.requests, 1);
        assert_eq!(snap.red_global.errors, 1);
    }

    #[test]
    fn record_request_zero_duration() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::ZERO);
        let snap = m.snapshot();
        assert_eq!(snap.red_global.requests, 1);
        assert_close(snap.red_global.avg_duration_ms, 0.0);
    }

    #[test]
    fn record_request_large_duration() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::from_secs(60));
        let snap = m.snapshot();
        assert_close(snap.red_global.avg_duration_ms, 60_000.0);
    }

    #[test]
    fn record_request_creates_operation_entry() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("my.custom.op", Duration::from_millis(5));
        let snap = m.snapshot();
        assert!(snap.red_by_operation.contains_key("my_custom_op"));
    }

    #[test]
    fn record_request_mixed_success_and_error() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::from_millis(10));
        m.record_request_success("op", Duration::from_millis(20));
        m.record_request_error("op", Duration::from_millis(30));
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("op").unwrap();
        assert_eq!(op.requests, 3);
        assert_eq!(op.errors, 1);
        assert_close(op.error_rate, 1.0 / 3.0);
        assert_close(op.avg_duration_ms, 20.0);
    }

    // ── Snapshot enabled field ─────────────────────────────────────────

    #[test]
    fn snapshot_reflects_enabled_true() {
        let m = init_metrics(MetricsConfig { enabled: true });
        assert!(m.snapshot().enabled);
    }

    #[test]
    fn snapshot_reflects_enabled_false() {
        let m = init_metrics(MetricsConfig { enabled: false });
        assert!(!m.snapshot().enabled);
    }

    #[test]
    fn snapshot_reflects_runtime_toggle() {
        let m = init_metrics(MetricsConfig { enabled: true });
        assert!(m.snapshot().enabled);
        m.set_enabled(false);
        assert!(!m.snapshot().enabled);
        m.set_enabled(true);
        assert!(m.snapshot().enabled);
    }

    // ── MetricsSnapshot debug/clone ────────────────────────────────────

    #[test]
    fn metrics_snapshot_debug() {
        let m = init_metrics(MetricsConfig::default());
        let snap = m.snapshot();
        let debug = format!("{snap:?}");
        assert!(debug.contains("MetricsSnapshot"));
    }

    #[test]
    fn metrics_snapshot_clone() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c", 42.0);
        let snap1 = m.snapshot();
        let snap2 = snap1.clone();
        assert_eq!(snap1.orders_created, snap2.orders_created);
        assert_close(snap1.order_amount_total, snap2.order_amount_total);
    }

    // ── is_finite_metric_value ─────────────────────────────────────────

    #[test]
    fn finite_metric_value_normal() {
        assert!(Metrics::is_finite_metric_value(0.0));
        assert!(Metrics::is_finite_metric_value(1.0));
        assert!(Metrics::is_finite_metric_value(-1.0));
        assert!(Metrics::is_finite_metric_value(f64::MAX));
        assert!(Metrics::is_finite_metric_value(f64::MIN));
        assert!(Metrics::is_finite_metric_value(f64::MIN_POSITIVE));
    }

    #[test]
    fn finite_metric_value_non_finite() {
        assert!(!Metrics::is_finite_metric_value(f64::NAN));
        assert!(!Metrics::is_finite_metric_value(f64::INFINITY));
        assert!(!Metrics::is_finite_metric_value(f64::NEG_INFINITY));
    }

    // ── Histogram additional tests ─────────────────────────────────────

    #[test]
    fn histogram_default_is_empty() {
        let h = LatencyHistogram::default();
        assert_eq!(h.count(), 0);
    }

    #[test]
    fn histogram_two_values() {
        let mut h = LatencyHistogram::new();
        h.record(10_000); // 10ms
        h.record(20_000); // 20ms
        assert_eq!(h.count(), 2);
        let p50 = h.percentile(0.5);
        assert!((10.0..=20.0).contains(&p50), "p50={p50}");
    }

    #[test]
    fn histogram_identical_values() {
        let mut h = LatencyHistogram::new();
        for _ in 0..100 {
            h.record(5_000);
        }
        assert_eq!(h.count(), 100);
        assert_close(h.percentile(0.5), 5.0);
        assert_close(h.percentile(0.99), 5.0);
    }

    #[test]
    fn histogram_zero_values() {
        let mut h = LatencyHistogram::new();
        for _ in 0..10 {
            h.record(0);
        }
        assert_eq!(h.count(), 10);
        assert_close(h.percentile(0.5), 0.0);
    }

    #[test]
    fn histogram_large_value() {
        let mut h = LatencyHistogram::new();
        h.record(u64::MAX);
        assert_eq!(h.count(), 1);
        let p99 = h.percentile(0.99);
        assert!(p99 > 0.0);
    }

    #[test]
    fn histogram_percentile_zero() {
        let mut h = LatencyHistogram::new();
        h.record(10_000);
        h.record(20_000);
        let p0 = h.percentile(0.0);
        assert_close(p0, 10.0);
    }

    #[test]
    fn histogram_percentile_one() {
        let mut h = LatencyHistogram::new();
        h.record(10_000);
        h.record(20_000);
        let p100 = h.percentile(1.0);
        assert_close(p100, 20.0);
    }

    #[test]
    fn histogram_sorted_order() {
        let mut h = LatencyHistogram::new();
        h.record(30_000);
        h.record(10_000);
        h.record(20_000);
        assert_eq!(h.count(), 3);
        // p50 should be the middle value
        let p50 = h.percentile(0.5);
        assert_close(p50, 10.0); // index (3*0.5 - 1).max(0) = 0.5 -> 0 -> 10ms
    }

    #[test]
    fn histogram_debug() {
        let h = LatencyHistogram::new();
        let debug = format!("{h:?}");
        assert!(debug.contains("LatencyHistogram"));
    }

    #[test]
    fn histogram_clone() {
        let mut h = LatencyHistogram::new();
        h.record(5_000);
        let h2 = h.clone();
        assert_eq!(h.count(), h2.count());
        assert_close(h.percentile(0.5), h2.percentile(0.5));
    }

    #[test]
    fn histogram_compaction_preserves_order() {
        let mut h = LatencyHistogram::new();
        // Fill with ascending values beyond MAX_HISTOGRAM_SAMPLES
        for i in 0..=(MAX_HISTOGRAM_SAMPLES as u64 + 100) {
            h.record(i * 1000);
        }
        // After compaction, count should be <= MAX_HISTOGRAM_SAMPLES
        assert!(h.count() <= MAX_HISTOGRAM_SAMPLES + 1);
        // Percentiles should still be monotonically non-decreasing
        assert!(h.percentile(0.95) >= h.percentile(0.50));
        assert!(h.percentile(0.99) >= h.percentile(0.95));
    }

    // ── RedAccumulator (tested through Metrics) ────────────────────────

    #[test]
    fn red_accumulator_single_success() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("test", Duration::from_millis(42));
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("test").unwrap();
        assert_eq!(op.requests, 1);
        assert_eq!(op.errors, 0);
        assert_close(op.error_rate, 0.0);
        assert_close(op.avg_duration_ms, 42.0);
    }

    #[test]
    fn red_accumulator_single_error() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_error("test", Duration::from_millis(99));
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("test").unwrap();
        assert_eq!(op.requests, 1);
        assert_eq!(op.errors, 1);
        assert_close(op.error_rate, 1.0);
        assert_close(op.avg_duration_ms, 99.0);
    }

    #[test]
    fn red_accumulator_percentiles_populated() {
        let m = init_metrics(MetricsConfig::default());
        for i in 1..=100 {
            m.record_request_success("perc_test", Duration::from_millis(i));
        }
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("perc_test").unwrap();
        assert_eq!(op.requests, 100);
        assert!(op.p50_ms > 0.0);
        assert!(op.p95_ms > op.p50_ms);
        assert!(op.p99_ms >= op.p95_ms);
    }

    // ── Thread safety (basic concurrent access) ────────────────────────

    #[test]
    fn concurrent_record_from_multiple_threads() {
        use std::thread;

        let m = init_metrics(MetricsConfig::default());
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let m = m.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        m.record_order_created(&format!("c{i}"), 1.0);
                        m.record_request_success("concurrent_op", Duration::from_millis(1));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 800);
        assert_close(snap.order_amount_total, 800.0);
        let op = snap.red_by_operation.get("concurrent_op").unwrap();
        assert_eq!(op.requests, 800);
    }

    #[test]
    fn concurrent_toggle_and_record() {
        use std::thread;

        let m = init_metrics(MetricsConfig::default());

        // One thread toggles, others record
        let m_toggle = m.clone();
        let toggle_handle = thread::spawn(move || {
            for _ in 0..100 {
                m_toggle.set_enabled(false);
                m_toggle.set_enabled(true);
            }
        });

        let mut record_handles = Vec::new();
        for _ in 0..4 {
            let m = m.clone();
            record_handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    m.record_order_created("c", 1.0);
                }
            }));
        }

        toggle_handle.join().unwrap();
        for h in record_handles {
            h.join().unwrap();
        }

        // Just verify no panics occurred and we can take a snapshot
        let _snap = m.snapshot();
    }

    #[test]
    fn concurrent_snapshot_while_recording() {
        use std::thread;

        let m = init_metrics(MetricsConfig::default());

        let m_record = m.clone();
        let record_handle = thread::spawn(move || {
            for _ in 0..500 {
                m_record.record_order_created("c", 1.0);
                m_record.record_request_success("op", Duration::from_millis(1));
            }
        });

        let m_snap = m;
        let snap_handle = thread::spawn(move || {
            let mut snapshots = Vec::new();
            for _ in 0..50 {
                snapshots.push(m_snap.snapshot());
            }
            snapshots
        });

        record_handle.join().unwrap();
        let snapshots = snap_handle.join().unwrap();

        // Each snapshot should have monotonically non-decreasing order counts
        for i in 1..snapshots.len() {
            assert!(
                snapshots[i].orders_created >= snapshots[i - 1].orders_created,
                "Snapshot order counts should be monotonically non-decreasing"
            );
        }
    }

    // ── Fresh metrics have all-zero snapshot ───────────────────────────

    #[test]
    fn fresh_metrics_all_zero() {
        let m = init_metrics(MetricsConfig::default());
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 0);
        assert_eq!(snap.customers_created, 0);
        assert_eq!(snap.products_created, 0);
        assert_eq!(snap.returns_requested, 0);
        assert_eq!(snap.carts_created, 0);
        assert_eq!(snap.cart_checkouts_completed, 0);
        assert_eq!(snap.shipments_created, 0);
        assert_eq!(snap.shipments_delivered, 0);
        assert_eq!(snap.subscriptions_created, 0);
        assert_eq!(snap.payments_completed, 0);
        assert_eq!(snap.inventory_adjustments, 0);
        assert_eq!(snap.a2a_quotes_created, 0);
        assert_eq!(snap.a2a_purchases_created, 0);
        assert_eq!(snap.x402_intents_created, 0);
        assert_eq!(snap.x402_intents_settled, 0);
        assert_eq!(snap.policy_evaluations, 0);
        assert_eq!(snap.policy_denials, 0);
        assert_eq!(snap.agent_registrations, 0);
        assert_eq!(snap.webhook_deliveries, 0);
        assert_eq!(snap.webhook_failures, 0);
        assert_close(snap.order_amount_total, 0.0);
        assert_close(snap.payment_amount_total, 0.0);
        assert_close(snap.inventory_delta_total, 0.0);
        assert_eq!(snap.red_global.requests, 0);
        assert!(snap.red_by_operation.is_empty());
    }

    // ── Operation label normalization in RED ────────────────────────────

    #[test]
    fn red_operation_labels_normalized_spaces() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("  Order  Created  ", Duration::from_millis(10));
        let snap = m.snapshot();
        assert!(snap.red_by_operation.contains_key("order_created"));
    }

    #[test]
    fn red_operation_labels_normalized_dots() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("order.create.v2", Duration::from_millis(10));
        let snap = m.snapshot();
        assert!(snap.red_by_operation.contains_key("order_create_v2"));
    }

    #[test]
    fn red_operation_labels_normalized_slashes() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("api/v1/orders", Duration::from_millis(10));
        let snap = m.snapshot();
        assert!(snap.red_by_operation.contains_key("api_v1_orders"));
    }

    #[test]
    fn red_operation_labels_normalized_mixed() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("Order---Created", Duration::from_millis(10));
        let snap = m.snapshot();
        assert!(snap.red_by_operation.contains_key("order_created"));
    }

    #[test]
    fn red_same_op_different_casing_merges() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("Order", Duration::from_millis(10));
        m.record_request_success("ORDER", Duration::from_millis(20));
        m.record_request_success("order", Duration::from_millis(30));
        let snap = m.snapshot();
        assert_eq!(snap.red_by_operation.len(), 1);
        let order = snap.red_by_operation.get("order").unwrap();
        assert_eq!(order.requests, 3);
    }

    // ── red_by_operation is BTreeMap (sorted) ──────────────────────────

    #[test]
    fn red_by_operation_sorted() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("zzz_op", Duration::from_millis(1));
        m.record_request_success("aaa_op", Duration::from_millis(1));
        m.record_request_success("mmm_op", Duration::from_millis(1));
        let snap = m.snapshot();
        let keys: Vec<&String> = snap.red_by_operation.keys().collect();
        assert_eq!(keys, vec!["aaa_op", "mmm_op", "zzz_op"]);
    }

    // ── Comprehensive non-finite amount tests ──────────────────────────

    #[test]
    fn order_nan_amount_does_not_affect_total() {
        let m = init_metrics(MetricsConfig::default());
        m.record_order_created("c1", 100.0);
        m.record_order_created("c2", f64::NAN);
        m.record_order_created("c3", 200.0);
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 3);
        assert_close(snap.order_amount_total, 300.0);
    }

    #[test]
    fn payment_infinity_amount_does_not_affect_total() {
        let m = init_metrics(MetricsConfig::default());
        m.record_payment_completed("p1", 50.0);
        m.record_payment_completed("p2", f64::INFINITY);
        let snap = m.snapshot();
        assert_eq!(snap.payments_completed, 2);
        assert_close(snap.payment_amount_total, 50.0);
    }

    #[test]
    fn inventory_neg_infinity_does_not_affect_total() {
        let m = init_metrics(MetricsConfig::default());
        m.record_inventory_adjusted("s1", 10.0);
        m.record_inventory_adjusted("s2", f64::NEG_INFINITY);
        let snap = m.snapshot();
        assert_eq!(snap.inventory_adjustments, 2);
        assert_close(snap.inventory_delta_total, 10.0);
    }

    // ── Enable/disable during RED recording ────────────────────────────

    #[test]
    fn toggle_during_red_recording() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op", Duration::from_millis(10));

        m.set_enabled(false);
        m.record_request_error("op", Duration::from_millis(200));

        m.set_enabled(true);
        m.record_request_success("op", Duration::from_millis(30));

        let snap = m.snapshot();
        let op = snap.red_by_operation.get("op").unwrap();
        assert_eq!(op.requests, 2); // Only 2 recorded (skipped the disabled one)
        assert_eq!(op.errors, 0);
    }

    // ── Large-scale recording ──────────────────────────────────────────

    #[test]
    fn large_scale_order_recording() {
        let m = init_metrics(MetricsConfig::default());
        for i in 0..1000 {
            m.record_order_created(&format!("c{i}"), 1.0);
        }
        let snap = m.snapshot();
        assert_eq!(snap.orders_created, 1000);
        assert_close(snap.order_amount_total, 1000.0);
    }

    #[test]
    fn large_scale_red_recording() {
        let m = init_metrics(MetricsConfig::default());
        for i in 0..500 {
            let is_error = i % 10 == 0;
            m.record_request("bulk_op", Duration::from_millis(10 + (i % 50)), is_error);
        }
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("bulk_op").unwrap();
        assert_eq!(op.requests, 500);
        assert_eq!(op.errors, 50); // every 10th
        assert!(op.avg_duration_ms > 0.0);
        assert!(op.p50_ms > 0.0);
        assert!(op.p95_ms > 0.0);
        assert!(op.p99_ms > 0.0);
    }

    // ── record_request directly (not via success/error helpers) ────────

    #[test]
    fn record_request_direct_success() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request("direct", Duration::from_millis(5), false);
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("direct").unwrap();
        assert_eq!(op.requests, 1);
        assert_eq!(op.errors, 0);
    }

    #[test]
    fn record_request_direct_error() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request("direct_err", Duration::from_millis(5), true);
        let snap = m.snapshot();
        let op = snap.red_by_operation.get("direct_err").unwrap();
        assert_eq!(op.requests, 1);
        assert_eq!(op.errors, 1);
    }

    // ── Global RED aggregates all operations ───────────────────────────

    #[test]
    fn global_red_aggregates_all_operations() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("op_a", Duration::from_millis(10));
        m.record_request_error("op_b", Duration::from_millis(20));
        m.record_request_success("op_c", Duration::from_millis(30));
        let snap = m.snapshot();
        assert_eq!(snap.red_global.requests, 3);
        assert_eq!(snap.red_global.errors, 1);
        assert_close(snap.red_global.avg_duration_ms, 20.0);
    }

    // ── MetricsSnapshot red_by_operation BTreeMap ordering ─────────────

    #[test]
    fn red_by_operation_btreemap_iteration_order() {
        let m = init_metrics(MetricsConfig::default());
        m.record_request_success("zebra", Duration::from_millis(1));
        m.record_request_success("alpha", Duration::from_millis(1));
        m.record_request_success("middle", Duration::from_millis(1));

        let snap = m.snapshot();
        let keys: Vec<String> = snap.red_by_operation.keys().cloned().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    // ── Multiple metric types in one session ───────────────────────────

    #[test]
    fn comprehensive_mixed_recording_session() {
        let m = init_metrics(MetricsConfig::default());

        // Commerce events
        m.record_order_created("c1", 100.0);
        m.record_order_created("c2", 200.0);
        m.record_customer_created("c1");
        m.record_product_created("p1");
        m.record_return_requested("r1");
        m.record_cart_created("cart1");
        m.record_cart_checkout_completed("cart1", "ord1");
        m.record_shipment_created("s1");
        m.record_shipment_delivered("s1");
        m.record_subscription_created("sub1");
        m.record_payment_completed("pay1", 100.0);
        m.record_payment_completed("pay2", 200.0);
        m.record_inventory_adjusted("sku1", 50.0);
        m.record_inventory_adjusted("sku1", -10.0);

        // A2A events
        m.record_a2a_quote_created();
        m.record_a2a_purchase_created();
        m.record_x402_intent_created();
        m.record_x402_intent_settled();

        // Policy events
        m.record_policy_evaluation(false);
        m.record_policy_evaluation(true);

        // Agent events
        m.record_agent_registration();

        // Webhook events
        m.record_webhook_delivery(false);
        m.record_webhook_delivery(true);

        // RED events
        m.record_request_success("order.create", Duration::from_millis(50));
        m.record_request_error("order.create", Duration::from_millis(200));
        m.record_request_success("payment.charge", Duration::from_millis(80));

        let snap = m.snapshot();

        // Verify all counters
        assert_eq!(snap.orders_created, 2);
        assert_eq!(snap.customers_created, 1);
        assert_eq!(snap.products_created, 1);
        assert_eq!(snap.returns_requested, 1);
        assert_eq!(snap.carts_created, 1);
        assert_eq!(snap.cart_checkouts_completed, 1);
        assert_eq!(snap.shipments_created, 1);
        assert_eq!(snap.shipments_delivered, 1);
        assert_eq!(snap.subscriptions_created, 1);
        assert_eq!(snap.payments_completed, 2);
        assert_eq!(snap.inventory_adjustments, 2);
        assert_eq!(snap.a2a_quotes_created, 1);
        assert_eq!(snap.a2a_purchases_created, 1);
        assert_eq!(snap.x402_intents_created, 1);
        assert_eq!(snap.x402_intents_settled, 1);
        assert_eq!(snap.policy_evaluations, 2);
        assert_eq!(snap.policy_denials, 1);
        assert_eq!(snap.agent_registrations, 1);
        assert_eq!(snap.webhook_deliveries, 2);
        assert_eq!(snap.webhook_failures, 1);

        // Verify amounts
        assert_close(snap.order_amount_total, 300.0);
        assert_close(snap.payment_amount_total, 300.0);
        assert_close(snap.inventory_delta_total, 40.0);

        // Verify RED
        assert_eq!(snap.red_global.requests, 3);
        assert_eq!(snap.red_global.errors, 1);
        assert_eq!(snap.red_by_operation.len(), 2);
    }
}
