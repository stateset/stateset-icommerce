//! Metrics helpers for StateSet iCommerce.
//!
//! This module provides a minimal, dependency-light metrics surface.
//! Downstream applications can wrap these hooks with their preferred
//! metrics exporter.

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

/// Lightweight metrics handle (no-op by default).
#[derive(Debug, Clone, Default)]
pub struct Metrics;

impl Metrics {
    /// Record a new order creation event.
    pub fn record_order_created(&self, _customer_id: &str, _amount: f64) {}

    /// Record a completed payment.
    pub fn record_payment_completed(&self, _payment_id: &str, _amount: f64) {}

    /// Record an inventory adjustment.
    pub fn record_inventory_adjusted(&self, _sku: &str, _delta: f64) {}
}

/// Initialize metrics and return a handle.
pub fn init_metrics(_config: MetricsConfig) -> Metrics {
    Metrics
}
