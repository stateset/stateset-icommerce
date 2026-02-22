#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! Metrics and observability layer for StateSet iCommerce.
//!
//! Provides structured metrics, tracing, and telemetry for production monitoring
//! of commerce operations.
//!
//! # Architecture
//!
//! The observability layer is split into two independent subsystems:
//!
//! - **[`tracing_ext`]** — Bootstrap for [`tracing_subscriber`] with `RUST_LOG` env
//!   filter support. Deliberately lightweight to avoid imposing a backend on
//!   downstream applications.
//! - **[`metrics`]** — Lock-free atomic counters for business events (orders,
//!   payments, inventory adjustments, etc.) with a snapshot API for periodic
//!   export to Prometheus, `StatsD`, or any other metrics backend.
//!
//! # Features
//!
//! - **Structured Tracing Bootstrap**: `tracing_subscriber` initialization with `RUST_LOG`
//! - **Business Metric Counters**: Order/payment/inventory event totals
//! - **Runtime Toggle**: Enable/disable in-process metrics collection
//! - **Snapshot API**: Read consistent metric values for export or diagnostics
//! - **Thread-Safe**: All counters use `AtomicU64`; safe to clone and share across threads
//!
//! # Quick Start
//!
//! ```rust
//! use stateset_observability::{init_metrics, MetricsConfig};
//!
//! // Initialize metrics with default configuration
//! let metrics = init_metrics(MetricsConfig::default());
//!
//! // Record commerce events
//! metrics.record_order_created("customer-123", 99.99);
//! metrics.record_payment_completed("pay-456", 99.99);
//! metrics.record_inventory_adjusted("SKU-001", -1.0);
//!
//! // Read a point-in-time snapshot for export
//! let snap = metrics.snapshot();
//! assert_eq!(snap.orders_created, 1);
//! assert_eq!(snap.payments_completed, 1);
//! assert_eq!(snap.inventory_adjustments, 1);
//! ```
//!
//! # Tracing Initialization
//!
//! ```rust,no_run
//! use stateset_observability::init_tracing;
//!
//! // Initialize tracing — reads RUST_LOG or defaults to `info`
//! init_tracing("stateset-marketplace", "production", "us-east-1")
//!     .expect("tracing init");
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
