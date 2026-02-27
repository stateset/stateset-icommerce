//! Span and metric naming conventions for StateSet telemetry.

/// Canonical metric names used for RED/SLO instrumentation.
pub mod metric_names {
    /// Total number of requests observed.
    pub const REQUESTS_TOTAL: &str = "stateset_requests_total";
    /// Total number of failed requests.
    pub const REQUEST_ERRORS_TOTAL: &str = "stateset_request_errors_total";
    /// Total request latency in milliseconds.
    pub const REQUEST_DURATION_MS_TOTAL: &str = "stateset_request_duration_ms_total";
}

/// Canonical metric labels used with RED counters.
pub mod metric_labels {
    /// Logical service name.
    pub const SERVICE: &str = "service";
    /// Operation identifier, normalized for cardinality safety.
    pub const OPERATION: &str = "operation";
    /// Deployment environment (production/staging/dev).
    pub const ENVIRONMENT: &str = "environment";
    /// Region or cluster.
    pub const REGION: &str = "region";
    /// Outcome class (`ok`/`error`).
    pub const OUTCOME: &str = "outcome";
}

/// Canonical tracing span fields.
pub mod span_fields {
    /// Service name bound to each span root.
    pub const SERVICE: &str = "stateset.service";
    /// Deployment environment.
    pub const ENVIRONMENT: &str = "stateset.environment";
    /// Region/cluster metadata.
    pub const REGION: &str = "stateset.region";
    /// Logical operation name.
    pub const OPERATION: &str = "stateset.operation";
    /// Outcome status (`ok`/`error`).
    pub const OUTCOME: &str = "stateset.outcome";
    /// Error details, if any.
    pub const ERROR: &str = "stateset.error";
}

/// Normalize a raw name into a low-cardinality telemetry-safe identifier.
///
/// - Lowercases ASCII letters.
/// - Replaces non-alphanumeric runs with a single underscore.
/// - Trims leading/trailing underscores.
/// - Returns `"unknown"` when the result is empty.
#[must_use]
pub fn normalize_name(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut last_was_sep = false;

    for ch in raw.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            normalized.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            normalized.push('_');
            last_was_sep = true;
        }
    }

    let trimmed = normalized.trim_matches('_');
    if trimmed.is_empty() { "unknown".to_string() } else { trimmed.to_string() }
}

/// Build a canonical span name for an operation.
#[must_use]
pub fn operation_span_name(operation: &str) -> String {
    format!("stateset.{}", normalize_name(operation))
}

/// Build a canonical metric label value for an operation.
#[must_use]
pub fn operation_metric_label(operation: &str) -> String {
    normalize_name(operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_lowercases_and_sanitizes() {
        assert_eq!(normalize_name("Order.Created"), "order_created");
        assert_eq!(normalize_name("  checkout / start "), "checkout_start");
        assert_eq!(normalize_name("A---B___C"), "a_b_c");
    }

    #[test]
    fn normalize_name_falls_back_to_unknown() {
        assert_eq!(normalize_name(""), "unknown");
        assert_eq!(normalize_name("___"), "unknown");
        assert_eq!(normalize_name("   "), "unknown");
    }

    #[test]
    fn operation_names_use_normalized_suffix() {
        assert_eq!(operation_span_name("Order Paid"), "stateset.order_paid");
        assert_eq!(operation_metric_label("Order Paid"), "order_paid");
    }
}
