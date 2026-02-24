//! Event type filtering with wildcard and prefix matching.
//!
//! Matching rules:
//!
//! - `"*"` matches everything (wildcard).
//! - Exact string equality.
//! - Prefix wildcard: `"a2a_payment.*"` matches any event type starting
//!   with `"a2a_payment."`.

/// Check whether an event type matches any of the given filter patterns.
///
/// # Matching Rules
///
/// 1. `"*"` matches all event types.
/// 2. Exact match: `"payment.completed"` matches only `"payment.completed"`.
/// 3. Prefix wildcard: `"a2a_payment.*"` matches `"a2a_payment.created"`,
///    `"a2a_payment.completed"`, etc.
///
/// Returns `false` if `filters` is empty.
///
/// # Example
///
/// ```
/// use stateset_a2a::events::matches_event_filter;
///
/// let filters = &["a2a_payment.*".to_string(), "escrow.released".to_string()];
///
/// assert!(matches_event_filter("a2a_payment.created", filters));
/// assert!(matches_event_filter("a2a_payment.completed", filters));
/// assert!(matches_event_filter("escrow.released", filters));
/// assert!(!matches_event_filter("escrow.funded", filters));
/// ```
#[must_use]
pub fn matches_event_filter(event_type: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return false;
    }

    for filter in filters {
        // Wildcard matches everything
        if filter == "*" {
            return true;
        }

        // Exact match
        if filter == event_type {
            return true;
        }

        // Prefix wildcard: "a2a_payment.*" -> prefix = "a2a_payment."
        if let Some(prefix) = filter.strip_suffix('*') {
            if event_type.starts_with(prefix) {
                return true;
            }
        }
    }

    false
}

/// Same as [`matches_event_filter`] but takes `&str` slices instead of `String` references.
///
/// Convenience function for testing and contexts where filter patterns are string literals.
#[must_use]
pub fn matches_event_filter_str(event_type: &str, filters: &[&str]) -> bool {
    if filters.is_empty() {
        return false;
    }

    for filter in filters {
        if *filter == "*" {
            return true;
        }

        if *filter == event_type {
            return true;
        }

        if let Some(prefix) = filter.strip_suffix('*') {
            if event_type.starts_with(prefix) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Wildcard =====

    #[test]
    fn wildcard_matches_everything() {
        assert!(matches_event_filter_str("payment.created", &["*"]));
        assert!(matches_event_filter_str("anything.at.all", &["*"]));
        assert!(matches_event_filter_str("", &["*"]));
    }

    #[test]
    fn wildcard_in_list_matches_all() {
        assert!(matches_event_filter_str("foo", &["specific.event", "*", "other.event"]));
    }

    // ===== Exact match =====

    #[test]
    fn exact_match() {
        assert!(matches_event_filter_str("payment.completed", &["payment.completed"]));
    }

    #[test]
    fn exact_match_case_sensitive() {
        assert!(!matches_event_filter_str("payment.Completed", &["payment.completed"]));
    }

    #[test]
    fn exact_no_match() {
        assert!(!matches_event_filter_str("payment.created", &["payment.completed"]));
    }

    // ===== Prefix wildcard =====

    #[test]
    fn prefix_wildcard_matches() {
        assert!(matches_event_filter_str("a2a_payment.created", &["a2a_payment.*"]));
        assert!(matches_event_filter_str("a2a_payment.completed", &["a2a_payment.*"]));
        assert!(matches_event_filter_str("a2a_payment.failed", &["a2a_payment.*"]));
    }

    #[test]
    fn prefix_wildcard_does_not_match_different_prefix() {
        assert!(!matches_event_filter_str("escrow.released", &["a2a_payment.*"]));
    }

    #[test]
    fn prefix_wildcard_requires_dot_separator() {
        // "a2a_payment." prefix means "a2a_paymentX" should NOT match
        assert!(!matches_event_filter_str("a2a_paymentExtra", &["a2a_payment.*"]));
    }

    #[test]
    fn multiple_prefix_wildcards() {
        let filters = &["a2a_payment.*", "escrow.*"];
        assert!(matches_event_filter_str("a2a_payment.created", filters));
        assert!(matches_event_filter_str("escrow.released", filters));
        assert!(!matches_event_filter_str("subscription.cancelled", filters));
    }

    // ===== Empty filters =====

    #[test]
    fn empty_filters_never_match() {
        assert!(!matches_event_filter_str("anything", &[]));
    }

    // ===== Mixed filters =====

    #[test]
    fn mixed_exact_and_prefix() {
        let filters = &["a2a_payment.*", "escrow.released"];
        assert!(matches_event_filter_str("a2a_payment.created", filters));
        assert!(matches_event_filter_str("escrow.released", filters));
        assert!(!matches_event_filter_str("escrow.funded", filters));
    }

    // ===== String ownership variant =====

    #[test]
    fn matches_event_filter_owned_strings() {
        let filters = vec!["a2a_payment.*".to_string(), "escrow.released".to_string()];
        assert!(matches_event_filter("a2a_payment.created", &filters));
        assert!(matches_event_filter("escrow.released", &filters));
        assert!(!matches_event_filter("escrow.funded", &filters));
    }

    #[test]
    fn owned_wildcard() {
        let filters = vec!["*".to_string()];
        assert!(matches_event_filter("anything", &filters));
    }

    #[test]
    fn owned_empty() {
        let filters: Vec<String> = vec![];
        assert!(!matches_event_filter("anything", &filters));
    }

    // ===== Edge cases =====

    #[test]
    fn filter_is_just_star_dot() {
        // ".*" should match any event that starts with "."
        assert!(matches_event_filter_str(".hidden", &[".*"]));
        assert!(!matches_event_filter_str("visible", &[".*"]));
    }

    #[test]
    fn event_type_with_many_dots() {
        assert!(matches_event_filter_str("a2a.payment.v2.created", &["a2a.payment.*"]));
    }

    #[test]
    fn empty_event_type_exact() {
        assert!(matches_event_filter_str("", &[""]));
    }

    #[test]
    fn empty_event_type_no_match() {
        assert!(!matches_event_filter_str("", &["something"]));
    }
}
