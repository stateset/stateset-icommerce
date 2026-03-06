//! Tracing helpers for StateSet iCommerce.
//!
//! This module intentionally keeps initialization lightweight to avoid
//! imposing a tracing backend on downstream applications. It configures a
//! global `tracing_subscriber` formatter that respects the `RUST_LOG`
//! environment variable (defaulting to `info`).
//!
//! If the host application has already set a global subscriber, initialization
//! is a no-op — this avoids double-init panics in test and library contexts.

use crate::conventions;
use crate::{ObservabilityError, Result};
use tracing_subscriber::EnvFilter;

/// Tracing configuration parameters.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for trace attribution.
    pub service_name: String,
    /// Deployment environment (e.g. production, staging).
    pub environment: String,
    /// Region or cluster identifier.
    pub region: String,
}

impl TracingConfig {
    /// Create a new tracing configuration.
    pub fn new(
        service_name: impl Into<String>,
        environment: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            environment: environment.into(),
            region: region.into(),
        }
    }
}

/// Initialize tracing with the provided identifiers.
///
/// This function configures a global `tracing_subscriber` formatter using
/// `RUST_LOG` if present, or `info` as a default level.
///
/// If a global subscriber is already configured by the host application,
/// this function is a no-op and returns `Ok(())`.
pub fn init_tracing(service_name: &str, environment: &str, region: &str) -> Result<()> {
    if service_name.is_empty() {
        return Err(ObservabilityError::InvalidConfig(
            "service_name must be non-empty".to_string(),
        ));
    }
    if environment.is_empty() {
        return Err(ObservabilityError::InvalidConfig("environment must be non-empty".to_string()));
    }
    if region.is_empty() {
        return Err(ObservabilityError::InvalidConfig("region must be non-empty".to_string()));
    }

    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter).with_target(true).finish();

    match tracing::subscriber::set_global_default(subscriber) {
        Ok(()) => {
            tracing::info!(
                service_name = service_name,
                environment = environment,
                region = region,
                "initialized tracing subscriber"
            );
            Ok(())
        }
        Err(_err) if tracing::dispatcher::has_been_set() => Ok(()),
        Err(err) => Err(ObservabilityError::TracingInitError(err.to_string())),
    }
}

/// Initialize tracing from a configuration struct.
pub fn init_tracing_with(config: TracingConfig) -> Result<()> {
    init_tracing(&config.service_name, &config.environment, &config.region)
}

/// Build a canonical StateSet span name for an operation.
#[must_use]
pub fn canonical_span_name(operation: &str) -> String {
    conventions::operation_span_name(operation)
}

/// Initialize tracing with OpenTelemetry OTLP export.
///
/// Sends traces to an OTLP-compatible collector (e.g. Jaeger, Tempo, Datadog).
/// The endpoint defaults to `http://localhost:4317` and can be overridden via
/// the `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable.
///
/// Requires the `otel` feature flag.
///
/// # Errors
///
/// Returns an error if the OTLP exporter or tracer pipeline cannot be initialized.
#[cfg(feature = "otel")]
pub fn init_tracing_otel(config: &TracingConfig) -> Result<()> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::Resource;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint(&endpoint))
        .with_trace_config(opentelemetry_sdk::trace::Config::default().with_resource(
            Resource::new(vec![
                KeyValue::new("service.name", config.service_name.clone()),
                KeyValue::new("deployment.environment", config.environment.clone()),
                KeyValue::new("cloud.region", config.region.clone()),
            ]),
        ))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .map_err(|e| ObservabilityError::ExporterError(e.to_string()))?;

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| ObservabilityError::TracingInitError(e.to_string()))?;

    tracing::info!(
        service_name = config.service_name.as_str(),
        environment = config.environment.as_str(),
        endpoint = endpoint.as_str(),
        "initialized OpenTelemetry OTLP tracing"
    );

    Ok(())
}

/// Flush and shut down the OpenTelemetry tracer provider.
///
/// Call this during graceful shutdown to ensure all pending spans are exported.
#[cfg(feature = "otel")]
pub fn shutdown_otel() {
    opentelemetry::global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_service_name() {
        let result = init_tracing("", "test", "local");
        assert!(matches!(result, Err(ObservabilityError::InvalidConfig(_))));
    }

    #[test]
    fn rejects_empty_environment() {
        let result = init_tracing("stateset", "", "local");
        assert!(matches!(result, Err(ObservabilityError::InvalidConfig(_))));
    }

    #[test]
    fn rejects_empty_region() {
        let result = init_tracing("stateset", "test", "");
        assert!(matches!(result, Err(ObservabilityError::InvalidConfig(_))));
    }

    #[test]
    fn canonical_span_name_is_normalized() {
        assert_eq!(canonical_span_name("Order Created"), "stateset.order_created");
    }
}
