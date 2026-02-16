//! Return/RMA-domain errors.

use uuid::Uuid;

/// Errors specific to return/RMA operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::ReturnError;
///
/// let err = ReturnError::not_found(uuid::Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReturnError {
    /// Return with the given ID was not found.
    #[error("return not found: {0}")]
    NotFound(Uuid),

    /// Return cannot be approved in its current status.
    #[error("return cannot be approved in status: {status}")]
    CannotApprove {
        /// The current return status.
        status: String,
    },

    /// Return period has expired.
    #[error("return period expired")]
    PeriodExpired,

    /// Item is not eligible for return.
    #[error("item not eligible for return")]
    ItemNotEligible,

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ReturnError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    pub fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `CannotApprove`.
    #[track_caller]
    pub fn cannot_approve(status: impl Into<String>) -> Self {
        Self::CannotApprove { status: status.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = ReturnError::not_found(Uuid::nil());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn cannot_approve_display() {
        let err = ReturnError::cannot_approve("completed");
        assert!(err.to_string().contains("completed"));
    }

    #[test]
    fn period_expired_display() {
        assert_eq!(ReturnError::PeriodExpired.to_string(), "return period expired");
    }

    #[test]
    fn item_not_eligible_display() {
        assert_eq!(ReturnError::ItemNotEligible.to_string(), "item not eligible for return");
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let ret_err = ReturnError::not_found(Uuid::nil());
        let commerce_err: CommerceError = ret_err.into();
        assert!(commerce_err.is_not_found());
    }
}
