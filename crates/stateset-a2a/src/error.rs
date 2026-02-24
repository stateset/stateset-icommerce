//! Error types for the A2A service layer.

use std::fmt;

/// Top-level error type for A2A operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum A2AError {
    /// A required field was missing or empty.
    #[error("validation error: {0}")]
    Validation(String),

    /// An invalid state transition was attempted.
    #[error(
        "invalid state transition from `{from}` to `{to}`: allowed transitions are [{allowed}]"
    )]
    InvalidTransition {
        /// Current state.
        from: String,
        /// Requested state.
        to: String,
        /// Comma-separated list of allowed target states.
        allowed: String,
    },

    /// The requested entity was not found.
    #[error("{entity} not found")]
    NotFound {
        /// Entity kind (e.g. "split payment", "subscription", "escrow").
        entity: &'static str,
    },

    /// A split payment rounding error occurred.
    #[error(
        "rounding drift: total distributed {distributed} does not equal input total {expected}"
    )]
    RoundingDrift {
        /// What was actually distributed.
        distributed: rust_decimal::Decimal,
        /// What was expected.
        expected: rust_decimal::Decimal,
    },

    /// SSRF validation blocked a URL.
    #[error("SSRF blocked: {0}")]
    SsrfBlocked(String),

    /// HMAC signature verification failed.
    #[error("HMAC verification failed")]
    HmacVerificationFailed,

    /// A billing interval string was invalid.
    #[error(
        "invalid billing interval: `{0}`. Must be one of: weekly, biweekly, monthly, quarterly, annual"
    )]
    InvalidBillingInterval(String),

    /// The escrow release conditions are not all met.
    #[error("release conditions not met: {unmet_descriptions:?}")]
    ConditionsNotMet {
        /// Human-readable descriptions of unmet conditions.
        unmet_descriptions: Vec<String>,
    },

    /// Percentage shares do not sum to 100.
    #[error("recipient percentages must sum to 100, got {actual}")]
    PercentageSumMismatch {
        /// Actual sum of percentages.
        actual: rust_decimal::Decimal,
    },

    /// Fixed share amounts do not sum to the expected remaining amount.
    #[error("fixed recipient amounts must sum to {expected}, got {actual}")]
    FixedSumMismatch {
        /// Expected sum.
        expected: rust_decimal::Decimal,
        /// Actual sum.
        actual: rust_decimal::Decimal,
    },

    /// Generic internal error (wraps arbitrary messages).
    #[error("{0}")]
    Internal(String),
}

/// Convenience alias for `Result<T, A2AError>`.
pub type A2AResult<T> = Result<T, A2AError>;

impl A2AError {
    /// Create a validation error with a formatted message.
    pub fn validation(msg: impl fmt::Display) -> Self {
        Self::Validation(msg.to_string())
    }

    /// Create an invalid-transition error.
    pub fn invalid_transition(
        from: impl fmt::Display,
        to: impl fmt::Display,
        allowed: &[&str],
    ) -> Self {
        Self::InvalidTransition {
            from: from.to_string(),
            to: to.to_string(),
            allowed: allowed.join(", "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_display() {
        let err = A2AError::validation("senderAddress is required");
        assert_eq!(err.to_string(), "validation error: senderAddress is required");
    }

    #[test]
    fn invalid_transition_display() {
        let err = A2AError::invalid_transition("pending", "released", &["processing"]);
        assert!(err.to_string().contains("pending"));
        assert!(err.to_string().contains("released"));
        assert!(err.to_string().contains("processing"));
    }

    #[test]
    fn not_found_display() {
        let err = A2AError::NotFound { entity: "subscription" };
        assert_eq!(err.to_string(), "subscription not found");
    }

    #[test]
    fn ssrf_blocked_display() {
        let err = A2AError::SsrfBlocked("localhost".into());
        assert!(err.to_string().contains("localhost"));
    }

    #[test]
    fn percentage_sum_mismatch_display() {
        use rust_decimal_macros::dec;
        let err = A2AError::PercentageSumMismatch { actual: dec!(95) };
        assert!(err.to_string().contains("95"));
    }
}
