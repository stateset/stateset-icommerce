//! Customer-domain errors.

use uuid::Uuid;

/// Errors specific to customer operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::CustomerError;
///
/// let err = CustomerError::not_found(uuid::Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CustomerError {
    /// Customer with the given ID was not found.
    #[error("customer not found: {0}")]
    NotFound(Uuid),

    /// Email address is already registered.
    #[error("email already exists: {0}")]
    EmailAlreadyExists(String),

    /// Customer account is not active.
    #[error("customer is not active")]
    NotActive,

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl CustomerError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    pub fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `EmailAlreadyExists`.
    #[track_caller]
    pub fn email_already_exists(email: impl Into<String>) -> Self {
        Self::EmailAlreadyExists(email.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = CustomerError::not_found(Uuid::nil());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn email_exists_display() {
        let err = CustomerError::email_already_exists("alice@example.com");
        assert!(err.to_string().contains("alice@example.com"));
    }

    #[test]
    fn not_active_display() {
        let err = CustomerError::NotActive;
        assert_eq!(err.to_string(), "customer is not active");
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let cust_err = CustomerError::not_found(Uuid::nil());
        let commerce_err: CommerceError = cust_err.into();
        assert!(commerce_err.is_not_found());

        let dup_err = CustomerError::email_already_exists("x@y.com");
        let commerce_err: CommerceError = dup_err.into();
        assert!(commerce_err.is_conflict());
    }
}
