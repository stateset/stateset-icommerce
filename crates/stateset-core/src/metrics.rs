//! Metrics and observability for StateSet iCommerce
//!
//! Provides comprehensive metrics collection using OpenTelemetry
//! with histograms for request latency and counters for operation tracking.

use once_cell::sync::Lazy;
use prometheus::{
    core::AtomicU64, register_histogram, register_int_counter, Histogram, IntCounter,
};
use std::time::Instant;

// ============================================================================
// Metrics Registry
// ============================================================================

/// Histogram for measuring operation latency in milliseconds
pub static OPERATION_LATENCY: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "stateset_operation_duration_milliseconds",
        "Duration of commerce operations in milliseconds",
        vec![0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0],
    )
    .expect("Failed to register OPERATION_LATENCY histogram")
});

/// Counter for successful operations
pub static OPERATIONS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "stateset_operations_total",
        "Total number of commerce operations",
    )
    .expect("Failed to register OPERATIONS_TOTAL counter")
});

/// Counter for failed operations
pub static OPERATIONS_FAILED: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "stateset_operations_failed_total",
        "Total number of failed commerce operations",
    )
    .expect("Failed to register OPERATIONS_FAILED counter")
});

/// Counter for active reservations
pub static ACTIVE_RESERVATIONS: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "stateset_inventory_active_reservations",
        "Total number of active inventory reservations",
    )
    .expect("Failed to register ACTIVE_RESERVATIONS counter")
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
        let mut labels = vec![self.operation.to_string()];
        labels.extend(self.labels.clone());
        OPERATION_LATENCY.observe(&labels, duration);
    }
}

// ============================================================================
// Metrics Macros
// ============================================================================

/// Macro to track operation success/failure
#[macro_export]
macro_rules! track_operation {
    (operation = $op:expr, labels = $labels:expr, $body:expr) => {{
        let _timer = OperationTimer::start($op, $labels.clone());
        let result = $body;
        match result {
            Ok(value) => {
                OPERATIONS_TOTAL.inc();
                Ok(value)
            }
            Err(error) => {
                let mut labels = vec![$op.to_string()];
                labels.extend($labels);
                labels.push(error.to_string());
                OPERATIONS_FAILED.inc();
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
                track_operation!(
                    operation = operation_name,
                    labels = vec![$repository_type.to_string()],
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

    pub fn track_order_creation(customer_id: &str) {
        track_operation!(
            operation = "orders.create",
            labels = vec![
                "domain:orders".to_string(),
                format!("customer:{}", customer_id)
            ],
            OPERATIONS_TOTAL.inc()
        );
    }

    pub fn track_order_status_transition(order_id: &str, from: &str, to: &str) {
        track_operation!(
            operation = "orders.status_transition",
            labels = vec![
                "domain:orders".to_string(),
                format!("order:{}", order_id),
                format!("transition:{}->{}", from, to),
            ],
            OPERATIONS_TOTAL.inc()
        );
    }
}

/// Metrics for inventory operations
pub mod inventory {
    use super::*;

    pub fn track_reservation(sku: &str, quantity: f64) {
        ACTIVE_RESERVATIONS.inc();
        track_operation!(
            operation = "inventory.reserve",
            labels = vec![
                "domain:inventory".to_string(),
                format!("sku:{}", sku),
                format!("quantity:{}", quantity),
            ],
            OPERATIONS_TOTAL.inc()
        );
    }

    pub fn track_stock_adjustment(sku: &str, delta: f64) {
        track_operation!(
            operation = "inventory.adjust",
            labels = vec![
                "domain:inventory".to_string(),
                format!("sku:{}", sku),
                format!("delta:{}", delta),
            ],
            OPERATIONS_TOTAL.inc()
        );
    }
}

/// Metrics for payment operations
pub mod payments {
    use super::*;

    pub fn track_payment_processing(order_id: &str, amount: f64) {
        track_operation!(
            operation = "payments.process",
            labels = vec![
                "domain:payments".to_string(),
                format!("order:{}", order_id),
                format!("amount:{}", amount),
            ],
            OPERATIONS_TOTAL.inc()
        );
    }

    pub fn track_refund(payment_id: &str, amount: f64) {
        track_operation!(
            operation = "payments.refund",
            labels = vec![
                "domain:payments".to_string(),
                format!("payment:{}", payment_id),
                format!("amount:{}", amount),
            ],
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
    pub fn new() -> Self {
        Self { labels: Vec::new() }
    }

    pub fn add(mut self, key: &str, value: &str) -> Self {
        self.labels.push(format!("{}:{}", key, value));
        self
    }

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
    let encoder = prometheus::TextEncoder::new();
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
