#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
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
//! - **[`conventions`]** — Canonical span/metric names and low-cardinality
//!   normalization helpers for operation labels.
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

pub mod conventions;
pub mod metrics;
pub mod tracing_ext;

pub use conventions::*;
pub use metrics::*;
pub use tracing_ext::*;

use thiserror::Error;

/// Result type for observability operations
pub type Result<T> = std::result::Result<T, ObservabilityError>;

/// Errors that can occur in observability operations
#[derive(Debug, Error)]
#[non_exhaustive]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── ObservabilityError Display formatting ──────────────────────────

    #[test]
    fn display_tracing_init_error() {
        let err = ObservabilityError::TracingInitError("subscriber failed".to_string());
        assert_eq!(err.to_string(), "Tracing initialization error: subscriber failed");
    }

    #[test]
    fn display_metrics_init_error() {
        let err = ObservabilityError::MetricsInitError("counter overflow".to_string());
        assert_eq!(err.to_string(), "Metrics initialization error: counter overflow");
    }

    #[test]
    fn display_exporter_error() {
        let err = ObservabilityError::ExporterError("connection refused".to_string());
        assert_eq!(err.to_string(), "Exporter error: connection refused");
    }

    #[test]
    fn display_invalid_config() {
        let err = ObservabilityError::InvalidConfig("missing field".to_string());
        assert_eq!(err.to_string(), "Invalid configuration: missing field");
    }

    #[test]
    fn display_tracing_init_error_empty_message() {
        let err = ObservabilityError::TracingInitError(String::new());
        assert_eq!(err.to_string(), "Tracing initialization error: ");
    }

    #[test]
    fn display_metrics_init_error_empty_message() {
        let err = ObservabilityError::MetricsInitError(String::new());
        assert_eq!(err.to_string(), "Metrics initialization error: ");
    }

    #[test]
    fn display_exporter_error_empty_message() {
        let err = ObservabilityError::ExporterError(String::new());
        assert_eq!(err.to_string(), "Exporter error: ");
    }

    #[test]
    fn display_invalid_config_empty_message() {
        let err = ObservabilityError::InvalidConfig(String::new());
        assert_eq!(err.to_string(), "Invalid configuration: ");
    }

    // ── ObservabilityError Debug formatting ────────────────────────────

    #[test]
    fn debug_tracing_init_error() {
        let err = ObservabilityError::TracingInitError("boom".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("TracingInitError"));
        assert!(debug.contains("boom"));
    }

    #[test]
    fn debug_metrics_init_error() {
        let err = ObservabilityError::MetricsInitError("metrics boom".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("MetricsInitError"));
        assert!(debug.contains("metrics boom"));
    }

    #[test]
    fn debug_exporter_error() {
        let err = ObservabilityError::ExporterError("export failure".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("ExporterError"));
        assert!(debug.contains("export failure"));
    }

    #[test]
    fn debug_invalid_config() {
        let err = ObservabilityError::InvalidConfig("bad config".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("InvalidConfig"));
        assert!(debug.contains("bad config"));
    }

    // ── Error implements std::error::Error ─────────────────────────────

    #[test]
    fn error_trait_source_is_none() {
        use std::error::Error;
        let err = ObservabilityError::TracingInitError("x".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn error_trait_source_is_none_for_all_variants() {
        use std::error::Error;
        let variants: Vec<ObservabilityError> = vec![
            ObservabilityError::TracingInitError("a".into()),
            ObservabilityError::MetricsInitError("b".into()),
            ObservabilityError::ExporterError("c".into()),
            ObservabilityError::InvalidConfig("d".into()),
        ];
        for v in &variants {
            assert!(v.source().is_none(), "Expected None source for {v:?}");
        }
    }

    // ── Result type alias ──────────────────────────────────────────────

    #[test]
    fn result_type_alias_ok() {
        let r: Result<u32> = Ok(42);
        assert!(r.is_ok());
        match r {
            Ok(v) => assert_eq!(v, 42),
            Err(_) => panic!("expected Ok"),
        }
    }

    #[test]
    fn result_type_alias_err() {
        let r: Result<u32> = Err(ObservabilityError::InvalidConfig("test".to_string()));
        assert!(r.is_err());
    }

    #[test]
    fn result_type_alias_map() {
        let r: Result<u32> = Ok(10);
        let mapped = r.map(|v| v * 2);
        assert_eq!(mapped.unwrap(), 20);
    }

    // ── Non-exhaustive attribute present ─────────────────────────────────

    #[test]
    fn error_variants_are_all_constructable() {
        // Confirm each variant can be constructed and displayed
        let variants: Vec<ObservabilityError> = vec![
            ObservabilityError::TracingInitError("a".into()),
            ObservabilityError::MetricsInitError("b".into()),
            ObservabilityError::ExporterError("c".into()),
            ObservabilityError::InvalidConfig("d".into()),
        ];
        assert_eq!(variants.len(), 4);
        for v in &variants {
            assert!(!v.to_string().is_empty());
        }
    }

    // ── Display with special characters ────────────────────────────────

    #[test]
    fn display_with_unicode_message() {
        let err = ObservabilityError::TracingInitError("failed: \u{26a0} warning".to_string());
        let s = err.to_string();
        assert!(s.contains("\u{26a0}"));
    }

    #[test]
    fn display_with_newlines() {
        let err = ObservabilityError::ExporterError("line1\nline2".to_string());
        assert!(err.to_string().contains('\n'));
    }

    #[test]
    fn display_with_long_message() {
        let long = "x".repeat(10_000);
        let err = ObservabilityError::InvalidConfig(long.clone());
        assert!(err.to_string().contains(&long));
    }

    // ── init_metrics from lib.rs re-export ─────────────────────────────

    #[test]
    fn init_metrics_via_reexport() {
        let m = init_metrics(MetricsConfig::default());
        assert!(m.is_enabled());
    }

    #[test]
    fn init_metrics_disabled_via_reexport() {
        let m = init_metrics(MetricsConfig { enabled: false });
        assert!(!m.is_enabled());
    }

    // ── TracingConfig ──────────────────────────────────────────────────

    #[test]
    fn tracing_config_new() {
        let cfg = TracingConfig::new("svc", "prod", "us-east-1");
        assert_eq!(cfg.service_name, "svc");
        assert_eq!(cfg.environment, "prod");
        assert_eq!(cfg.region, "us-east-1");
    }

    #[test]
    fn tracing_config_new_from_string() {
        let cfg = TracingConfig::new(
            String::from("my-svc"),
            String::from("staging"),
            String::from("eu-west-1"),
        );
        assert_eq!(cfg.service_name, "my-svc");
        assert_eq!(cfg.environment, "staging");
        assert_eq!(cfg.region, "eu-west-1");
    }

    #[test]
    fn tracing_config_debug() {
        let cfg = TracingConfig::new("svc", "prod", "us-east-1");
        let debug = format!("{cfg:?}");
        assert!(debug.contains("svc"));
        assert!(debug.contains("prod"));
        assert!(debug.contains("us-east-1"));
    }

    #[test]
    fn tracing_config_clone() {
        let cfg = TracingConfig::new("svc", "prod", "us-east-1");
        let cloned = cfg.clone();
        assert_eq!(cloned.service_name, cfg.service_name);
        assert_eq!(cloned.environment, cfg.environment);
        assert_eq!(cloned.region, cfg.region);
    }

    // ── init_tracing validation ────────────────────────────────────────

    #[test]
    fn init_tracing_rejects_empty_service_name() {
        let r = init_tracing("", "test", "local");
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("service_name"));
    }

    #[test]
    fn init_tracing_rejects_empty_environment() {
        let r = init_tracing("svc", "", "local");
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("environment"));
    }

    #[test]
    fn init_tracing_rejects_empty_region() {
        let r = init_tracing("svc", "test", "");
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("region"));
    }

    #[test]
    fn init_tracing_with_rejects_empty_service() {
        let cfg = TracingConfig::new("", "test", "local");
        let r = init_tracing_with(cfg);
        assert!(r.is_err());
    }

    // ── Convention re-exports ──────────────────────────────────────────

    #[test]
    fn metric_names_constants_accessible() {
        assert_eq!(metric_names::REQUESTS_TOTAL, "stateset_requests_total");
        assert_eq!(metric_names::REQUEST_ERRORS_TOTAL, "stateset_request_errors_total");
        assert_eq!(metric_names::REQUEST_DURATION_MS_TOTAL, "stateset_request_duration_ms_total");
    }

    #[test]
    fn metric_labels_constants_accessible() {
        assert_eq!(metric_labels::SERVICE, "service");
        assert_eq!(metric_labels::OPERATION, "operation");
        assert_eq!(metric_labels::ENVIRONMENT, "environment");
        assert_eq!(metric_labels::REGION, "region");
        assert_eq!(metric_labels::OUTCOME, "outcome");
    }

    #[test]
    fn span_fields_constants_accessible() {
        assert_eq!(span_fields::SERVICE, "stateset.service");
        assert_eq!(span_fields::ENVIRONMENT, "stateset.environment");
        assert_eq!(span_fields::REGION, "stateset.region");
        assert_eq!(span_fields::OPERATION, "stateset.operation");
        assert_eq!(span_fields::OUTCOME, "stateset.outcome");
        assert_eq!(span_fields::ERROR, "stateset.error");
    }

    #[test]
    fn normalize_name_re_export() {
        assert_eq!(normalize_name("Foo/Bar"), "foo_bar");
    }

    #[test]
    fn operation_span_name_re_export() {
        assert_eq!(operation_span_name("test op"), "stateset.test_op");
    }

    #[test]
    fn operation_metric_label_re_export() {
        assert_eq!(operation_metric_label("test op"), "test_op");
    }
}
