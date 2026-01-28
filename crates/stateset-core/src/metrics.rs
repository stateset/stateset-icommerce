//! Metrics and observability for StateSet iCommerce
//!
//! Provides comprehensive metrics collection using OpenTelemetry
//! with histograms for request latency and counters for operation tracking.

use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge, HistogramVec,
    IntCounterVec, IntGauge, TextEncoder,
};
use std::time::Instant;

// ============================================================================
// Metrics Registry
// ============================================================================

/// Histogram for measuring operation latency in milliseconds
pub static OPERATION_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "stateset_operation_duration_milliseconds",
        "Duration of commerce operations in milliseconds",
        &["operation", "domain"],
        vec![0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0],
    )
    .expect("Failed to register OPERATION_LATENCY histogram")
});

/// Counter for successful operations
pub static OPERATIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "stateset_operations_total",
        "Total number of commerce operations",
        &["operation", "domain"],
    )
    .expect("Failed to register OPERATIONS_TOTAL counter")
});

/// Counter for failed operations
pub static OPERATIONS_FAILED: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "stateset_operations_failed_total",
        "Total number of failed commerce operations",
        &["operation", "domain"],
    )
    .expect("Failed to register OPERATIONS_FAILED counter")
});

/// Gauge for active reservations
pub static ACTIVE_RESERVATIONS: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "stateset_inventory_active_reservations",
        "Total number of active inventory reservations",
    )
    .expect("Failed to register ACTIVE_RESERVATIONS gauge")
});

// ============================================================================
// Timer for Latency Measurement
// ============================================================================

/// Timer that records operation duration when dropped
#[must_use]
pub struct OperationTimer {
    operation: &'static str,
    labels: Vec<String>,
    start: Instant,
}

impl OperationTimer {
    /// Start a new operation timer
    pub fn start(operation: &'static str, labels: Vec<String>) -> Self {
        Self {
            operation,
            labels,
            start: Instant::now(),
        }
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_millis() as f64;
        let domain = domain_from_labels(&self.labels);
        OPERATION_LATENCY
            .with_label_values(&[self.operation, domain])
            .observe(duration);
    }
}

// ============================================================================
// Metrics Macros
// ============================================================================

/// Extract the domain label if present (format: "domain:<value>")
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
                $crate::metrics::OPERATIONS_TOTAL
                    .with_label_values(&[$op, domain])
                    .inc();
                Ok(value)
            }
            Err(error) => {
                let domain = $crate::metrics::domain_from_labels(&$labels);
                $crate::metrics::OPERATIONS_FAILED
                    .with_label_values(&[$op, domain])
                    .inc();
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
        track_operation!(
            operation = "orders.create",
            labels = vec!["domain:orders".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }

    /// Track an order status transition
    pub fn track_order_status_transition(_order_id: &str, _from: &str, _to: &str) {
        track_operation!(
            operation = "orders.status_transition",
            labels = vec!["domain:orders".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }
}

/// Metrics for inventory operations
pub mod inventory {
    use super::*;

    /// Track an inventory reservation
    pub fn track_reservation(_sku: &str, _quantity: f64) {
        ACTIVE_RESERVATIONS.inc();
        track_operation!(
            operation = "inventory.reserve",
            labels = vec!["domain:inventory".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }

    /// Track an inventory stock adjustment
    pub fn track_stock_adjustment(_sku: &str, _delta: f64) {
        track_operation!(
            operation = "inventory.adjust",
            labels = vec!["domain:inventory".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }
}

/// Metrics for payment operations
pub mod payments {
    use super::*;

    /// Track a payment processing operation
    pub fn track_payment_processing(_order_id: &str, _amount: f64) {
        track_operation!(
            operation = "payments.process",
            labels = vec!["domain:payments".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }

    /// Track a payment refund operation
    pub fn track_refund(_payment_id: &str, _amount: f64) {
        track_operation!(
            operation = "payments.refund",
            labels = vec!["domain:payments".to_string()],
            OPERATIONS_TOTAL.inc()
        );
    }
}

// ============================================================================
// Labels Helper
// ============================================================================

/// Helper to build consistent metric labels
pub struct LabelsBuilder {
    labels: Vec<String>,
}

impl LabelsBuilder {
    /// Create a new labels builder
    pub fn new() -> Self {
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
    Ok(String::from_utf8(buffer).unwrap())
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
        let labels = LabelsBuilder::new()
            .add("domain", "orders")
            .add("operation", "create")
            .build();
        assert_eq!(labels.len(), 2);
    }
}
