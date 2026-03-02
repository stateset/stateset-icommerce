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

    /// A dispute operation failed (invalid evidence, missing escrow, etc.).
    #[error("dispute error: {0}")]
    DisputeError(String),

    /// A reputation score was outside the valid 1–5 range.
    #[error("score out of range: {value} (must be 1–5)")]
    ScoreOutOfRange {
        /// The invalid score value.
        value: rust_decimal::Decimal,
    },

    /// A circuit breaker blocked the transaction.
    #[error("circuit breaker blocked: {reason}")]
    CircuitBreakerBlocked {
        /// Human-readable reason for the block.
        reason: String,
    },

    /// A spending limit was exceeded.
    #[error("spending limit exceeded: {limit_type} limit is {limit}, attempted {attempted}")]
    SpendingLimitExceeded {
        /// Which limit was hit (e.g. "`per_transaction`", "daily", "monthly").
        limit_type: String,
        /// The configured limit.
        limit: rust_decimal::Decimal,
        /// The attempted amount.
        attempted: rust_decimal::Decimal,
    },

    /// A marketplace/RFQ operation failed.
    #[error("marketplace error: {0}")]
    MarketplaceError(String),

    /// An SLA violation was detected.
    #[error("SLA violation: {metric} — actual {actual}, required {required}")]
    SlaViolation {
        /// Which metric was violated.
        metric: String,
        /// The actual measured value.
        actual: rust_decimal::Decimal,
        /// The required threshold.
        required: rust_decimal::Decimal,
    },

    /// An agent card validation or discovery error.
    #[error("agent card error: {0}")]
    AgentCardError(String),

    /// The failure rate threshold was exceeded.
    #[error("failure rate exceeded: {rate}% (threshold: {threshold}%)")]
    FailureRateExceeded {
        /// Current failure rate as a percentage.
        rate: rust_decimal::Decimal,
        /// Configured threshold as a percentage.
        threshold: rust_decimal::Decimal,
    },

    /// An RFQ has expired.
    #[error("RFQ expired: {rfq_id}")]
    RfqExpired {
        /// The expired RFQ identifier.
        rfq_id: String,
    },

    /// A dispute deadline was missed.
    #[error("dispute deadline missed: {deadline_type}")]
    DeadlineMissed {
        /// Which deadline was missed (e.g. "`evidence_period`", "review").
        deadline_type: String,
    },

    /// A trust tier requirement was not met.
    #[error("trust tier requirement not met: requires {required}, agent has {actual}")]
    TrustTierInsufficient {
        /// The required tier.
        required: String,
        /// The agent's current tier.
        actual: String,
    },

    /// The negotiation round limit was exceeded.
    #[error("negotiation round limit exceeded: {max_rounds} rounds maximum")]
    NegotiationLimitExceeded {
        /// Maximum allowed rounds.
        max_rounds: u32,
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

    /// Create a dispute error with a formatted message.
    pub fn dispute(msg: impl fmt::Display) -> Self {
        Self::DisputeError(msg.to_string())
    }

    /// Create a marketplace error with a formatted message.
    pub fn marketplace(msg: impl fmt::Display) -> Self {
        Self::MarketplaceError(msg.to_string())
    }

    /// Create an agent card error with a formatted message.
    pub fn agent_card(msg: impl fmt::Display) -> Self {
        Self::AgentCardError(msg.to_string())
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

    #[test]
    fn dispute_error_display() {
        let err = A2AError::dispute("evidence period expired");
        assert!(err.to_string().contains("evidence period expired"));
    }

    #[test]
    fn score_out_of_range_display() {
        use rust_decimal_macros::dec;
        let err = A2AError::ScoreOutOfRange { value: dec!(6) };
        assert!(err.to_string().contains("6"));
    }

    #[test]
    fn circuit_breaker_blocked_display() {
        let err = A2AError::CircuitBreakerBlocked { reason: "open state".into() };
        assert!(err.to_string().contains("open state"));
    }

    #[test]
    fn spending_limit_exceeded_display() {
        use rust_decimal_macros::dec;
        let err = A2AError::SpendingLimitExceeded {
            limit_type: "daily".into(),
            limit: dec!(10000),
            attempted: dec!(15000),
        };
        let msg = err.to_string();
        assert!(msg.contains("daily"));
        assert!(msg.contains("10000"));
        assert!(msg.contains("15000"));
    }

    #[test]
    fn marketplace_error_display() {
        let err = A2AError::marketplace("RFQ already awarded");
        assert!(err.to_string().contains("RFQ already awarded"));
    }

    #[test]
    fn sla_violation_display() {
        use rust_decimal_macros::dec;
        let err = A2AError::SlaViolation {
            metric: "uptime_percent".into(),
            actual: dec!(95.5),
            required: dec!(99.9),
        };
        let msg = err.to_string();
        assert!(msg.contains("uptime_percent"));
        assert!(msg.contains("95.5"));
        assert!(msg.contains("99.9"));
    }

    #[test]
    fn agent_card_error_display() {
        let err = A2AError::agent_card("wallet address required");
        assert!(err.to_string().contains("wallet address required"));
    }

    #[test]
    fn failure_rate_exceeded_display() {
        use rust_decimal_macros::dec;
        let err = A2AError::FailureRateExceeded { rate: dec!(45), threshold: dec!(30) };
        let msg = err.to_string();
        assert!(msg.contains("45"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn rfq_expired_display() {
        let err = A2AError::RfqExpired { rfq_id: "rfq_123".into() };
        assert!(err.to_string().contains("rfq_123"));
    }

    #[test]
    fn deadline_missed_display() {
        let err = A2AError::DeadlineMissed { deadline_type: "evidence_period".into() };
        assert!(err.to_string().contains("evidence_period"));
    }

    #[test]
    fn trust_tier_insufficient_display() {
        let err = A2AError::TrustTierInsufficient {
            required: "verified".into(),
            actual: "sandbox".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("verified"));
        assert!(msg.contains("sandbox"));
    }

    #[test]
    fn negotiation_limit_exceeded_display() {
        let err = A2AError::NegotiationLimitExceeded { max_rounds: 5 };
        assert!(err.to_string().contains("5"));
    }
}
