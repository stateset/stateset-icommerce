//! Tracing helpers for StateSet iCommerce.
//!
//! This module intentionally keeps initialization lightweight to avoid
//! imposing a tracing backend on downstream applications. It configures a
//! global `tracing_subscriber` formatter that respects the `RUST_LOG`
//! environment variable (defaulting to `info`).
//!
//! If the host application has already set a global subscriber, initialization
//! is a no-op — this avoids double-init panics in test and library contexts.

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
}
