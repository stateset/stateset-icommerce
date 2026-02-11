#![deny(unsafe_code)]

//! Metrics and observability layer for StateSet iCommerce
//!
//! Provides structured metrics, tracing, and telemetry for production monitoring.
//!
//! # Features
//!
//! - **Structured Tracing Bootstrap**: `tracing_subscriber` initialization with `RUST_LOG`
//! - **Business Metric Counters**: Order/payment/inventory event totals
//! - **Runtime Toggle**: Enable/disable in-process metrics collection
//! - **Snapshot API**: Read consistent metric values for export or diagnostics
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
