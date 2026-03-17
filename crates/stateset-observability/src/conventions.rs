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

    // ── normalize_name additional cases ────────────────────────────────

    #[test]
    fn normalize_name_single_word() {
        assert_eq!(normalize_name("hello"), "hello");
    }

    #[test]
    fn normalize_name_uppercase() {
        assert_eq!(normalize_name("HELLO"), "hello");
    }

    #[test]
    fn normalize_name_mixed_case() {
        assert_eq!(normalize_name("hElLo"), "hello");
    }

    #[test]
    fn normalize_name_alphanumeric() {
        assert_eq!(normalize_name("order123"), "order123");
    }

    #[test]
    fn normalize_name_numbers_only() {
        assert_eq!(normalize_name("12345"), "12345");
    }

    #[test]
    fn normalize_name_leading_trailing_separators() {
        assert_eq!(normalize_name("---hello---"), "hello");
    }

    #[test]
    fn normalize_name_dots() {
        assert_eq!(normalize_name("a.b.c"), "a_b_c");
    }

    #[test]
    fn normalize_name_slashes() {
        assert_eq!(normalize_name("a/b/c"), "a_b_c");
    }

    #[test]
    fn normalize_name_colons() {
        assert_eq!(normalize_name("a:b:c"), "a_b_c");
    }

    #[test]
    fn normalize_name_tabs_and_newlines() {
        assert_eq!(normalize_name("a\tb\nc"), "a_b_c");
    }

    #[test]
    fn normalize_name_consecutive_separators_collapse() {
        assert_eq!(normalize_name("a...b///c---d"), "a_b_c_d");
    }

    #[test]
    fn normalize_name_single_separator() {
        assert_eq!(normalize_name("."), "unknown");
        assert_eq!(normalize_name("-"), "unknown");
        assert_eq!(normalize_name("/"), "unknown");
    }

    #[test]
    fn normalize_name_unicode_non_ascii() {
        // Non-ASCII chars are not ascii_alphanumeric, so treated as separators
        assert_eq!(normalize_name("order\u{00e9}created"), "order_created");
    }

    #[test]
    fn normalize_name_single_char() {
        assert_eq!(normalize_name("a"), "a");
        assert_eq!(normalize_name("Z"), "z");
        assert_eq!(normalize_name("5"), "5");
    }

    #[test]
    fn normalize_name_very_long() {
        let long = "a".repeat(10_000);
        let result = normalize_name(&long);
        assert_eq!(result.len(), 10_000);
    }

    // ── operation_span_name ────────────────────────────────────────────

    #[test]
    fn operation_span_name_empty_input() {
        assert_eq!(operation_span_name(""), "stateset.unknown");
    }

    #[test]
    fn operation_span_name_with_dots() {
        assert_eq!(operation_span_name("order.create"), "stateset.order_create");
    }

    #[test]
    fn operation_span_name_preserves_numbers() {
        assert_eq!(operation_span_name("api/v2/orders"), "stateset.api_v2_orders");
    }

    // ── operation_metric_label ─────────────────────────────────────────

    #[test]
    fn operation_metric_label_empty_input() {
        assert_eq!(operation_metric_label(""), "unknown");
    }

    #[test]
    fn operation_metric_label_already_normalized() {
        assert_eq!(operation_metric_label("order_create"), "order_create");
    }

    #[test]
    fn operation_metric_label_complex() {
        assert_eq!(operation_metric_label("API/v1/Checkout---Start"), "api_v1_checkout_start");
    }

    // ── metric_names constants ─────────────────────────────────────────

    #[test]
    fn metric_names_have_stateset_prefix() {
        assert!(metric_names::REQUESTS_TOTAL.starts_with("stateset_"));
        assert!(metric_names::REQUEST_ERRORS_TOTAL.starts_with("stateset_"));
        assert!(metric_names::REQUEST_DURATION_MS_TOTAL.starts_with("stateset_"));
    }

    #[test]
    fn metric_names_are_distinct() {
        let names = [
            metric_names::REQUESTS_TOTAL,
            metric_names::REQUEST_ERRORS_TOTAL,
            metric_names::REQUEST_DURATION_MS_TOTAL,
        ];
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(names[i], names[j], "Metric names must be unique");
            }
        }
    }

    // ── metric_labels constants ────────────────────────────────────────

    #[test]
    fn metric_labels_are_distinct() {
        let labels = [
            metric_labels::SERVICE,
            metric_labels::OPERATION,
            metric_labels::ENVIRONMENT,
            metric_labels::REGION,
            metric_labels::OUTCOME,
        ];
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j], "Metric labels must be unique");
            }
        }
    }

    // ── span_fields constants ──────────────────────────────────────────

    #[test]
    fn span_fields_have_stateset_prefix() {
        assert!(span_fields::SERVICE.starts_with("stateset."));
        assert!(span_fields::ENVIRONMENT.starts_with("stateset."));
        assert!(span_fields::REGION.starts_with("stateset."));
        assert!(span_fields::OPERATION.starts_with("stateset."));
        assert!(span_fields::OUTCOME.starts_with("stateset."));
        assert!(span_fields::ERROR.starts_with("stateset."));
    }

    #[test]
    fn span_fields_are_distinct() {
        let fields = [
            span_fields::SERVICE,
            span_fields::ENVIRONMENT,
            span_fields::REGION,
            span_fields::OPERATION,
            span_fields::OUTCOME,
            span_fields::ERROR,
        ];
        for i in 0..fields.len() {
            for j in (i + 1)..fields.len() {
                assert_ne!(fields[i], fields[j], "Span fields must be unique");
            }
        }
    }
}
