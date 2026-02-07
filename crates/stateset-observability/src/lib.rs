//! Metrics and observability layer for StateSet iCommerce
//!
//! Provides structured metrics, tracing, and telemetry for production monitoring.
//!
//! # Features
//!
//! - **OpenTelemetry Integration**: Distributed tracing with OTLP exporters
//! - **Key Performance Metrics**: Query latency, pool utilization, error rates
//! - **Contextual Tracing**: Instrumented operations with span propagation
//! - **Custom Metrics Framework**: Business metrics (orders, revenue, inventory)
//! - **Performance Monitoring**: Connection pool health, query execution time
//!
//! # Usage
//!
//! ```ignore
//! use stateset_observability::{init_tracing, init_metrics, MetricsConfig};
//!
//! // Initialize tracing
//! init_tracing("stateset-marketplace", "production", "us-east-1")?;
//!
//! // Initialize metrics
//! let metrics = init_metrics(MetricsConfig::default());
//! metrics.record_order_created("customer-123", 99.99);
//! ```

pub mod metrics;
pub mod tracing_ext;

pub use metrics::*;
pub use tracing_ext::*;

use thiserror::Error;

/// Result type for observability operations
pub type Result<T> = std::result::Result<T, ObservabilityError>;

/// Errors that can occur in observability operations
#[derive(Debug, Error)]
pub enum ObservabilityError {
    #[error("Tracing initialization error: {0}")]
    TracingInitError(String),

    #[error("Metrics initialization error: {0}")]
    MetricsInitError(String),

    #[error("Exporter error: {0}")]
    ExporterError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
