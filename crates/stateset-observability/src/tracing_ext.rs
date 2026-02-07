//! Tracing helpers for StateSet iCommerce.
//!
//! This module intentionally keeps initialization lightweight to avoid
//! imposing a tracing backend on downstream applications.

use crate::{ObservabilityError, Result};

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
/// This function is a no-op placeholder that validates inputs and returns `Ok(())`.
/// Integrations should wrap or replace it with their preferred tracing pipeline.
pub fn init_tracing(service_name: &str, environment: &str, region: &str) -> Result<()> {
    if service_name.is_empty() {
        return Err(ObservabilityError::InvalidConfig(
            "service_name must be non-empty".to_string(),
        ));
    }
    if environment.is_empty() {
        return Err(ObservabilityError::InvalidConfig(
            "environment must be non-empty".to_string(),
        ));
    }
    if region.is_empty() {
        return Err(ObservabilityError::InvalidConfig(
            "region must be non-empty".to_string(),
        ));
    }
    Ok(())
}

/// Initialize tracing from a configuration struct.
pub fn init_tracing_with(config: TracingConfig) -> Result<()> {
    init_tracing(&config.service_name, &config.environment, &config.region)
}
