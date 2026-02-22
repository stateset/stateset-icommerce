//! Metrics helpers for StateSet iCommerce.
//!
//! This module provides a minimal, dependency-light metrics surface using
//! lock-free atomic counters. Downstream applications can wrap these hooks
//! with their preferred metrics exporter (Prometheus, `StatsD`, etc.).
//!
//! # Example
//!
//! ```rust
//! use stateset_observability::{init_metrics, MetricsConfig};
//!
//! let metrics = init_metrics(MetricsConfig::default());
//! assert!(metrics.is_enabled());
//!
//! // Record events
//! metrics.record_order_created("cust-1", 49.99);
//! metrics.record_order_created("cust-2", 150.00);
//!
//! // Snapshot for export
//! let snap = metrics.snapshot();
//! assert_eq!(snap.orders_created, 2);
//!
//! // Runtime toggle
//! metrics.set_enabled(false);
//! metrics.record_order_created("cust-3", 25.00);
//! assert_eq!(metrics.snapshot().orders_created, 2); // unchanged
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
}

#[derive(Debug, Default)]
struct MetricTotals {
    order_amount_total: f64,
    payment_amount_total: f64,
    inventory_delta_total: f64,
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
    }
}
