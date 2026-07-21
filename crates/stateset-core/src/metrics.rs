//! Prometheus metrics helpers for StateSet iCommerce.
//!
//! This module provides a small set of Prometheus metrics primitives:
//! - Histograms for operation latency
//! - Counters for success/failure totals
//! - Gauges for active counts (e.g., reservations)

use once_cell::sync::Lazy;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, TextEncoder,
    register_histogram_vec, register_int_counter_vec, register_int_gauge,
};
use std::time::Instant;

// ============================================================================
// Metrics Registry
// ============================================================================

/// Histogram for measuring operation latency in milliseconds.
///
/// Metrics are best-effort: if registration/creation fails, this is `None`
/// and callers should skip recording rather than panic.
pub static OPERATION_LATENCY: Lazy<Option<HistogramVec>> = Lazy::new(|| {
    register_histogram_vec_resilient(
        "stateset_operation_duration_milliseconds",
        "Duration of commerce operations in milliseconds",
        &["operation", "domain"],
        vec![0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0],
    )
});

/// Counter for successful operations
pub static OPERATIONS_TOTAL: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    register_int_counter_vec_resilient(
        "stateset_operations_total",
        "Total number of commerce operations",
        &["operation", "domain"],
    )
});

/// Counter for failed operations
pub static OPERATIONS_FAILED: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    register_int_counter_vec_resilient(
        "stateset_operations_failed_total",
        "Total number of failed commerce operations",
        &["operation", "domain"],
    )
});

/// Gauge for active reservations
pub static ACTIVE_RESERVATIONS: Lazy<Option<IntGauge>> = Lazy::new(|| {
    register_int_gauge_resilient(
        "stateset_inventory_active_reservations",
        "Total number of active inventory reservations",
    )
});

fn register_histogram_vec_resilient(
    name: &str,
    help: &str,
    labels: &[&str],
    buckets: Vec<f64>,
) -> Option<HistogramVec> {
    match register_histogram_vec!(name, help, labels, buckets.clone()) {
        Ok(metric) => Some(metric),
        Err(err) => {
            eprintln!(
                "stateset-core metrics: failed to register histogram '{}' ({}); using local fallback collector",
                name, err
            );
            let opts = HistogramOpts::new(name, help).buckets(buckets);
            match HistogramVec::new(opts, labels) {
                Ok(metric) => Some(metric),
                Err(create_err) => {
                    eprintln!(
                        "stateset-core metrics: failed to create fallback histogram '{}' ({}); disabling metric",
                        name, create_err
                    );
                    None
                }
            }
        }
    }
}

fn register_int_counter_vec_resilient(
    name: &str,
    help: &str,
    labels: &[&str],
) -> Option<IntCounterVec> {
    match register_int_counter_vec!(name, help, labels) {
        Ok(metric) => Some(metric),
        Err(err) => {
            eprintln!(
                "stateset-core metrics: failed to register counter '{}' ({}); using local fallback collector",
                name, err
            );
            match IntCounterVec::new(Opts::new(name, help), labels) {
                Ok(metric) => Some(metric),
                Err(create_err) => {
                    eprintln!(
                        "stateset-core metrics: failed to create fallback counter '{}' ({}); disabling metric",
                        name, create_err
                    );
                    None
                }
            }
        }
    }
}

fn register_int_gauge_resilient(name: &str, help: &str) -> Option<IntGauge> {
    match register_int_gauge!(name, help) {
        Ok(metric) => Some(metric),
        Err(err) => {
            eprintln!(
                "stateset-core metrics: failed to register gauge '{}' ({}); using local fallback collector",
                name, err
            );
            match IntGauge::new(name, help) {
                Ok(metric) => Some(metric),
                Err(create_err) => {
                    eprintln!(
                        "stateset-core metrics: failed to create fallback gauge '{}' ({}); disabling metric",
                        name, create_err
                    );
                    None
                }
            }
        }
    }
}

// ============================================================================
// Timer for Latency Measurement
// ============================================================================

/// Timer that records operation duration when dropped
#[must_use]
#[derive(Debug)]
pub struct OperationTimer {
    operation: &'static str,
    labels: Vec<String>,
    start: Instant,
}

impl OperationTimer {
    /// Start a new operation timer
    pub fn start(operation: &'static str, labels: Vec<String>) -> Self {
        Self { operation, labels, start: Instant::now() }
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        // Millisecond precision as a float (supports sub-ms durations).
        let duration = self.start.elapsed().as_secs_f64() * 1000.0;
        let domain = domain_from_labels(&self.labels);
        if let Some(metric) = OPERATION_LATENCY.as_ref() {
            metric.with_label_values(&[self.operation, domain]).observe(duration);
        }
    }
}

// ============================================================================
// Metrics Macros
// ============================================================================

/// Extract the domain label if present (format: `domain:<value>`)
pub fn domain_from_labels(labels: &[String]) -> &str {
    labels
        .iter()
        .find_map(|label| label.strip_prefix("domain:"))
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
}

/// Macro to track operation success/failure
#[macro_export]
macro_rules! track_operation {
    (operation = $op:expr, labels = $labels:expr, $body:expr) => {{
        let _timer = $crate::metrics::OperationTimer::start($op, $labels.clone());
        let result = $body;
        match result {
            Ok(value) => {
                let domain = $crate::metrics::domain_from_labels(&$labels);
                if let Some(metric) = $crate::metrics::OPERATIONS_TOTAL.as_ref() {
                    metric.with_label_values(&[$op, domain]).inc();
                }
                Ok(value)
            }
            Err(error) => {
                let domain = $crate::metrics::domain_from_labels(&$labels);
                if let Some(metric) = $crate::metrics::OPERATIONS_FAILED.as_ref() {
                    metric.with_label_values(&[$op, domain]).inc();
                }
                Err(error)
            }
        }
    }};
}

/// Macro to wrap repository methods with metrics
#[macro_export]
macro_rules! instrument_repository {
    ($repository_type:ident, $method:ident, $self:expr) => {
        |operation_name| {
            move || {
                $crate::track_operation!(
                    operation = operation_name,
                    labels = vec![format!("domain:{}", stringify!($repository_type))],
                    $self.$method(operation_name)
                )
            }
        }
    };
}

// ============================================================================
// Domain-Specific Metrics
// ============================================================================

/// Metrics for order operations
pub mod orders {
    use super::*;

    /// Track an order creation event
    pub fn track_order_creation(_customer_id: &str) {
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["orders.create", "orders"]).inc();
        }
    }

    /// Track an order status transition
    pub fn track_order_status_transition(_order_id: &str, _from: &str, _to: &str) {
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["orders.status_transition", "orders"]).inc();
        }
    }
}

/// Metrics for inventory operations
pub mod inventory {
    use super::*;

    /// Track an inventory reservation
    pub fn track_reservation(_sku: &str, _quantity: f64) {
        if let Some(metric) = ACTIVE_RESERVATIONS.as_ref() {
            metric.inc();
        }
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["inventory.reserve", "inventory"]).inc();
        }
    }

    /// Track an inventory stock adjustment
    pub fn track_stock_adjustment(_sku: &str, _delta: f64) {
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["inventory.adjust", "inventory"]).inc();
        }
    }
}

/// Metrics for payment operations
pub mod payments {
    use super::*;

    /// Track a payment processing operation
    pub fn track_payment_processing(_order_id: &str, _amount: rust_decimal::Decimal) {
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["payments.process", "payments"]).inc();
        }
    }

    /// Track a payment refund operation
    pub fn track_refund(_payment_id: &str, _amount: rust_decimal::Decimal) {
        if let Some(metric) = OPERATIONS_TOTAL.as_ref() {
            metric.with_label_values(&["payments.refund", "payments"]).inc();
        }
    }
}

// ============================================================================
// Labels Helper
// ============================================================================

/// Helper to build consistent metric labels
#[derive(Debug)]
#[must_use]
pub struct LabelsBuilder {
    labels: Vec<String>,
}

impl LabelsBuilder {
    /// Create a new labels builder
    pub const fn new() -> Self {
        Self { labels: Vec::new() }
    }

    /// Add a key/value label pair
    pub fn add(mut self, key: &str, value: &str) -> Self {
        self.labels.push(format!("{}:{}", key, value));
        self
    }

    /// Build the final label list
    pub fn build(self) -> Vec<String> {
        self.labels
    }
}

impl Default for LabelsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Metrics Export
// ============================================================================

/// Export all metrics in Prometheus format
pub fn export_metrics() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    String::from_utf8(buffer).map_err(|e| prometheus::Error::Msg(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        // Ensure metrics are registered
        let _ = &OPERATION_LATENCY;
        let _ = &OPERATIONS_TOTAL;
        let _ = &OPERATIONS_FAILED;
        let _ = &ACTIVE_RESERVATIONS;
    }

    #[test]
    fn test_operation_timer() {
        let _timer = OperationTimer::start(
            "test_operation",
            vec!["label1:value1".to_string(), "label2:value2".to_string()],
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        // Timer records on drop
    }

    #[test]
    fn test_labels_builder() {
        let labels =
            LabelsBuilder::new().add("domain", "orders").add("operation", "create").build();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn test_domain_from_labels() {
        let labels = vec!["foo:bar".to_string(), "domain:orders".to_string()];
        assert_eq!(domain_from_labels(&labels), "orders");

        let labels = vec!["domain:".to_string()];
        assert_eq!(domain_from_labels(&labels), "unknown");
    }
}
