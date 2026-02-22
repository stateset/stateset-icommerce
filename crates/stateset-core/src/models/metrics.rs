//! Prometheus metrics and structured logging support
//!
//! Provides observability primitives for monitoring commerce operations.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Metric Types
// ============================================================================

/// Counter metric (monotonically increasing)
#[derive(Debug)]
pub struct Counter {
    name: String,
    help: String,
    labels: Vec<String>,
    values: RwLock<HashMap<Vec<String>, AtomicU64>>,
}

impl Counter {
    /// Create a new counter metric
    pub fn new(name: &str, help: &str, labels: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Increment the counter by 1 for the given label values
    pub fn inc(&self, label_values: &[&str]) {
        self.add(label_values, 1);
    }

    /// Add a value to the counter for the given label values
    pub fn add(&self, label_values: &[&str], value: u64) {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        values.entry(key).or_insert_with(|| AtomicU64::new(0)).fetch_add(value, Ordering::SeqCst);
    }

    /// Get the current counter value for the given label values
    pub fn get(&self, label_values: &[&str]) -> u64 {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        values.get(&key).map(|v| v.load(Ordering::SeqCst)).unwrap_or(0)
    }

    /// Render the counter in Prometheus exposition format
    pub fn render_prometheus(&self) -> String {
        let mut output = format!("# HELP {} {}\n", self.name, self.help);
        output.push_str(&format!("# TYPE {} counter\n", self.name));

        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        for (labels, value) in values.iter() {
            let label_str = if !labels.is_empty() && !self.labels.is_empty() {
                let pairs: Vec<String> = self
                    .labels
                    .iter()
                    .zip(labels.iter())
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            } else {
                String::new()
            };
            output.push_str(&format!(
                "{}{} {}\n",
                self.name,
                label_str,
                value.load(Ordering::SeqCst)
            ));
        }

        output
    }
}

/// Gauge metric (can go up or down)
#[derive(Debug)]
pub struct Gauge {
    name: String,
    help: String,
    labels: Vec<String>,
    values: RwLock<HashMap<Vec<String>, f64>>,
}

impl Gauge {
    /// Create a new gauge metric
    pub fn new(name: &str, help: &str, labels: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Set the gauge to an absolute value
    pub fn set(&self, label_values: &[&str], value: f64) {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        values.insert(key, value);
    }

    /// Increment the gauge by 1
    pub fn inc(&self, label_values: &[&str]) {
        self.add(label_values, 1.0);
    }

    /// Decrement the gauge by 1
    pub fn dec(&self, label_values: &[&str]) {
        self.add(label_values, -1.0);
    }

    /// Add a delta to the gauge
    pub fn add(&self, label_values: &[&str], delta: f64) {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        let current = values.get(&key).copied().unwrap_or(0.0);
        values.insert(key, current + delta);
    }

    /// Get the current gauge value for the given labels
    pub fn get(&self, label_values: &[&str]) -> f64 {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        values.get(&key).copied().unwrap_or(0.0)
    }

    /// Render the gauge in Prometheus exposition format
    pub fn render_prometheus(&self) -> String {
        let mut output = format!("# HELP {} {}\n", self.name, self.help);
        output.push_str(&format!("# TYPE {} gauge\n", self.name));

        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        for (labels, value) in values.iter() {
            let label_str = if !labels.is_empty() && !self.labels.is_empty() {
                let pairs: Vec<String> = self
                    .labels
                    .iter()
                    .zip(labels.iter())
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            } else {
                String::new()
            };
            output.push_str(&format!("{}{} {}\n", self.name, label_str, value));
        }

        output
    }
}

/// Histogram metric for measuring distributions
#[derive(Debug)]
pub struct Histogram {
    name: String,
    help: String,
    labels: Vec<String>,
    buckets: Vec<f64>,
    observations: RwLock<HashMap<Vec<String>, HistogramData>>,
}

#[derive(Debug, Default)]
struct HistogramData {
    bucket_counts: Vec<AtomicU64>,
    sum: std::sync::atomic::AtomicU64, // Store as bits
    count: AtomicU64,
}

impl Histogram {
    /// Create a new histogram metric
    pub fn new(name: &str, help: &str, labels: Vec<&str>, buckets: Vec<f64>) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            buckets,
            observations: RwLock::new(HashMap::new()),
        }
    }

    /// Default buckets for request durations (in seconds)
    pub fn default_buckets() -> Vec<f64> {
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    }

    /// Record an observation value for the histogram
    pub fn observe(&self, label_values: &[&str], value: f64) {
        let key: Vec<String> = label_values.iter().map(|s| s.to_string()).collect();
        let mut observations = self.observations.write().unwrap_or_else(|e| e.into_inner());

        let data = observations.entry(key).or_insert_with(|| {
            let mut bucket_counts = Vec::with_capacity(self.buckets.len());
            for _ in &self.buckets {
                bucket_counts.push(AtomicU64::new(0));
            }
            HistogramData {
                bucket_counts,
                sum: std::sync::atomic::AtomicU64::new(0),
                count: AtomicU64::new(0),
            }
        });

        // Update buckets
        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                data.bucket_counts[i].fetch_add(1, Ordering::SeqCst);
            }
        }

        // Update sum and count
        let current_bits = data.sum.load(Ordering::SeqCst);
        let current = f64::from_bits(current_bits);
        let new_bits = (current + value).to_bits();
        data.sum.store(new_bits, Ordering::SeqCst);
        data.count.fetch_add(1, Ordering::SeqCst);
    }

    /// Render the histogram in Prometheus exposition format
    pub fn render_prometheus(&self) -> String {
        let mut output = format!("# HELP {} {}\n", self.name, self.help);
        output.push_str(&format!("# TYPE {} histogram\n", self.name));

        let observations = self.observations.read().unwrap_or_else(|e| e.into_inner());
        for (labels, data) in observations.iter() {
            let base_label_str = if !labels.is_empty() && !self.labels.is_empty() {
                let pairs: Vec<String> = self
                    .labels
                    .iter()
                    .zip(labels.iter())
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                pairs.join(",")
            } else {
                String::new()
            };

            // Bucket lines
            let mut cumulative = 0u64;
            for (i, bucket) in self.buckets.iter().enumerate() {
                cumulative += data.bucket_counts[i].load(Ordering::SeqCst);
                let label_str = if base_label_str.is_empty() {
                    format!("{{le=\"{}\"}}", bucket)
                } else {
                    format!("{{{},le=\"{}\"}}", base_label_str, bucket)
                };
                output.push_str(&format!("{}_bucket{} {}\n", self.name, label_str, cumulative));
            }

            // +Inf bucket
            let total = data.count.load(Ordering::SeqCst);
            let inf_label = if base_label_str.is_empty() {
                "{le=\"+Inf\"}".to_string()
            } else {
                format!("{{{},le=\"+Inf\"}}", base_label_str)
            };
            output.push_str(&format!("{}_bucket{} {}\n", self.name, inf_label, total));

            // Sum and count
            let sum = f64::from_bits(data.sum.load(Ordering::SeqCst));
            let label_str = if base_label_str.is_empty() {
                String::new()
            } else {
                format!("{{{}}}", base_label_str)
            };
            output.push_str(&format!("{}_sum{} {}\n", self.name, label_str, sum));
            output.push_str(&format!("{}_count{} {}\n", self.name, label_str, total));
        }

        output
    }
}

// ============================================================================
// Commerce-Specific Metrics
// ============================================================================

/// Collection of commerce metrics
// Debug is implemented manually because prometheus types do not derive Debug.
pub struct CommerceMetrics {
    // Order metrics
    pub orders_created: Counter,
    pub orders_completed: Counter,
    pub orders_cancelled: Counter,
    pub order_total_amount: Counter,

    // Inventory metrics
    pub inventory_adjustments: Counter,
    pub inventory_reservations: Counter,
    pub inventory_available: Gauge,

    // Customer metrics
    pub customers_created: Counter,
    pub customers_active: Gauge,

    // Performance metrics
    pub request_duration: Histogram,
    pub database_query_duration: Histogram,

    // Error metrics
    pub errors: Counter,
}

impl std::fmt::Debug for CommerceMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommerceMetrics").finish_non_exhaustive()
    }
}

impl CommerceMetrics {
    /// Create a registry of commerce-specific metrics
    pub fn new() -> Self {
        Self {
            orders_created: Counter::new(
                "stateset_orders_created_total",
                "Total number of orders created",
                vec!["status"],
            ),
            orders_completed: Counter::new(
                "stateset_orders_completed_total",
                "Total number of orders completed",
                vec![],
            ),
            orders_cancelled: Counter::new(
                "stateset_orders_cancelled_total",
                "Total number of orders cancelled",
                vec!["reason"],
            ),
            order_total_amount: Counter::new(
                "stateset_order_total_amount",
                "Total amount of all orders (in cents)",
                vec!["currency"],
            ),
            inventory_adjustments: Counter::new(
                "stateset_inventory_adjustments_total",
                "Total number of inventory adjustments",
                vec!["type", "sku"],
            ),
            inventory_reservations: Counter::new(
                "stateset_inventory_reservations_total",
                "Total number of inventory reservations",
                vec!["status"],
            ),
            inventory_available: Gauge::new(
                "stateset_inventory_available",
                "Current available inventory by SKU",
                vec!["sku"],
            ),
            customers_created: Counter::new(
                "stateset_customers_created_total",
                "Total number of customers created",
                vec![],
            ),
            customers_active: Gauge::new(
                "stateset_customers_active",
                "Number of active customers",
                vec![],
            ),
            request_duration: Histogram::new(
                "stateset_request_duration_seconds",
                "Request duration in seconds",
                vec!["operation", "status"],
                Histogram::default_buckets(),
            ),
            database_query_duration: Histogram::new(
                "stateset_database_query_duration_seconds",
                "Database query duration in seconds",
                vec!["query_type", "table"],
                Histogram::default_buckets(),
            ),
            errors: Counter::new(
                "stateset_errors_total",
                "Total number of errors",
                vec!["type", "operation"],
            ),
        }
    }

    /// Render all metrics in Prometheus format
    pub fn render_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str(&self.orders_created.render_prometheus());
        output.push_str(&self.orders_completed.render_prometheus());
        output.push_str(&self.orders_cancelled.render_prometheus());
        output.push_str(&self.order_total_amount.render_prometheus());
        output.push_str(&self.inventory_adjustments.render_prometheus());
        output.push_str(&self.inventory_reservations.render_prometheus());
        output.push_str(&self.inventory_available.render_prometheus());
        output.push_str(&self.customers_created.render_prometheus());
        output.push_str(&self.customers_active.render_prometheus());
        output.push_str(&self.request_duration.render_prometheus());
        output.push_str(&self.database_query_duration.render_prometheus());
        output.push_str(&self.errors.render_prometheus());
        output
    }
}

impl Default for CommerceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Structured Logging
// ============================================================================

/// Log level for structured logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Target/module that generated the log
    pub target: Option<String>,
    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
    /// Span ID for distributed tracing
    pub span_id: Option<String>,
    /// Additional structured fields
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    /// Create a new log entry with level and message
    pub fn new(level: LogLevel, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            message: message.to_string(),
            target: None,
            trace_id: None,
            span_id: None,
            fields: HashMap::new(),
        }
    }

    /// Set the log target/module
    #[must_use]
    pub fn with_target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    /// Attach a trace ID for distributed tracing
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: &str) -> Self {
        self.trace_id = Some(trace_id.to_string());
        self
    }

    /// Attach a span ID for distributed tracing
    #[must_use]
    pub fn with_span_id(mut self, span_id: &str) -> Self {
        self.span_id = Some(span_id.to_string());
        self
    }

    /// Add an arbitrary structured field
    #[must_use]
    pub fn with_field<V: Serialize>(mut self, key: &str, value: V) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.fields.insert(key.to_string(), json_value);
        }
        self
    }

    /// Render as JSON string
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| format!("{{\"message\": \"{}\"}}", self.message))
    }

    /// Render as human-readable string
    pub fn to_human(&self) -> String {
        let mut parts = vec![
            format!("{}", self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f")),
            format!("[{}]", self.level),
        ];

        if let Some(ref target) = self.target {
            parts.push(format!("({})", target));
        }

        parts.push(self.message.clone());

        if !self.fields.is_empty() {
            let fields_str: Vec<String> =
                self.fields.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            parts.push(format!("{{{}}}", fields_str.join(", ")));
        }

        parts.join(" ")
    }
}

/// Commerce-specific log helpers
#[derive(Debug)]
pub struct CommerceLogger;

impl CommerceLogger {
    /// Build a log entry for order creation
    pub fn order_created(order_id: &str, customer_id: &str, total: Decimal) -> LogEntry {
        LogEntry::new(LogLevel::Info, "Order created")
            .with_target("stateset::orders")
            .with_field("order_id", order_id)
            .with_field("customer_id", customer_id)
            .with_field("total", total.to_string())
    }

    /// Build a log entry for inventory adjustments
    pub fn inventory_adjusted(sku: &str, quantity: Decimal, reason: &str) -> LogEntry {
        LogEntry::new(LogLevel::Info, "Inventory adjusted")
            .with_target("stateset::inventory")
            .with_field("sku", sku)
            .with_field("quantity", quantity.to_string())
            .with_field("reason", reason)
    }

    /// Build a log entry for payment processing
    pub fn payment_processed(payment_id: &str, amount: Decimal, status: &str) -> LogEntry {
        LogEntry::new(LogLevel::Info, "Payment processed")
            .with_target("stateset::payments")
            .with_field("payment_id", payment_id)
            .with_field("amount", amount.to_string())
            .with_field("status", status)
    }

    /// Build a log entry for an operation error
    pub fn error(operation: &str, error: &str) -> LogEntry {
        LogEntry::new(LogLevel::Error, &format!("Operation failed: {}", error))
            .with_target("stateset::error")
            .with_field("operation", operation)
            .with_field("error", error)
    }

    /// Build a log entry for database queries
    pub fn database_query(query_type: &str, table: &str, duration_ms: u64) -> LogEntry {
        LogEntry::new(LogLevel::Debug, "Database query executed")
            .with_target("stateset::database")
            .with_field("query_type", query_type)
            .with_field("table", table)
            .with_field("duration_ms", duration_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new("test_counter", "A test counter", vec!["method"]);
        counter.inc(&["GET"]);
        counter.inc(&["GET"]);
        counter.add(&["POST"], 5);

        assert_eq!(counter.get(&["GET"]), 2);
        assert_eq!(counter.get(&["POST"]), 5);
        assert_eq!(counter.get(&["PUT"]), 0);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new("test_gauge", "A test gauge", vec!["sku"]);
        gauge.set(&["SKU-001"], 100.0);
        gauge.add(&["SKU-001"], -10.0);
        gauge.inc(&["SKU-001"]);

        assert_eq!(gauge.get(&["SKU-001"]), 91.0);
    }

    #[test]
    fn test_histogram() {
        let histogram = Histogram::new(
            "test_histogram",
            "A test histogram",
            vec!["operation"],
            vec![0.1, 0.5, 1.0],
        );
        histogram.observe(&["read"], 0.05);
        histogram.observe(&["read"], 0.3);
        histogram.observe(&["read"], 0.8);

        let output = histogram.render_prometheus();
        assert!(output.contains("test_histogram_bucket"));
        assert!(output.contains("test_histogram_sum"));
        assert!(output.contains("test_histogram_count"));
    }

    #[test]
    fn test_log_entry_json() {
        let entry = LogEntry::new(LogLevel::Info, "Test message")
            .with_target("test")
            .with_field("key", "value");

        let json = entry.to_json();
        assert!(json.contains("\"message\":\"Test message\""));
        assert!(json.contains("\"level\":\"info\""));
    }

    #[test]
    fn test_commerce_logger() {
        let log = CommerceLogger::order_created("ORD-001", "CUST-001", Decimal::from(100));
        assert_eq!(log.level, LogLevel::Info);
        assert!(log.fields.contains_key("order_id"));
    }
}
