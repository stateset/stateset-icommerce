//! Payment-domain errors.

use uuid::Uuid;

/// Errors specific to payment operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::PaymentError;
///
/// let err = PaymentError::not_found(uuid::Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PaymentError {
    /// Payment with the given ID was not found.
    #[error("payment not found: {0}")]
    NotFound(Uuid),

    /// Payment was declined by the processor.
    #[error("payment declined: {reason}")]
    Declined {
        /// The decline reason from the processor.
        reason: String,
    },

    /// Refund cannot be processed.
    #[error("refund failed: {reason}")]
    RefundFailed {
        /// The reason the refund failed.
        reason: String,
    },

    /// Currency mismatch between payment and order.
    #[error("currency mismatch: expected {expected}, got {got}")]
    CurrencyMismatch {
        /// The expected currency code.
        expected: String,
        /// The actual currency code provided.
        got: String,
    },

    /// Invalid status transition for a payment.
    #[error("{0}")]
    InvalidTransition(super::StateTransitionError<String>),

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl PaymentError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `Declined`.
    #[track_caller]
    pub fn declined(reason: impl Into<String>) -> Self {
        Self::Declined { reason: reason.into() }
    }

    /// Convenience constructor for `RefundFailed`.
    #[track_caller]
    pub fn refund_failed(reason: impl Into<String>) -> Self {
        Self::RefundFailed { reason: reason.into() }
    }

    /// Convenience constructor for `CurrencyMismatch`.
    #[track_caller]
    pub fn currency_mismatch(expected: impl Into<String>, got: impl Into<String>) -> Self {
        Self::CurrencyMismatch { expected: expected.into(), got: got.into() }
    }

    /// Convenience constructor for `InvalidTransition`.
    #[track_caller]
    pub fn invalid_transition(from: impl std::fmt::Display, to: impl std::fmt::Display) -> Self {
        Self::InvalidTransition(super::StateTransitionError::new(from.to_string(), to.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = PaymentError::not_found(Uuid::nil());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn declined_display() {
        let err = PaymentError::declined("insufficient funds");
        assert!(err.to_string().contains("insufficient funds"));
    }

    #[test]
    fn refund_failed_display() {
        let err = PaymentError::refund_failed("already refunded");
        assert!(err.to_string().contains("already refunded"));
    }

    #[test]
    fn currency_mismatch_display() {
        let err = PaymentError::currency_mismatch("USD", "EUR");
        let msg = err.to_string();
        assert!(msg.contains("USD"));
        assert!(msg.contains("EUR"));
    }

    #[test]
    fn invalid_transition_display() {
        let err = PaymentError::invalid_transition("pending", "refunded");
        assert!(err.to_string().contains("pending"));
        assert!(err.to_string().contains("refunded"));
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let pay_err = PaymentError::not_found(Uuid::nil());
        let commerce_err: CommerceError = pay_err.into();
        assert!(commerce_err.is_not_found());
    }
}
