//! Error types for the authorization system.


use serde::{Deserialize, Serialize};

/// Errors that can occur during authorization operations.
///
/// ```rust
/// use stateset_authz::AuthzError;
///
/// let err = AuthzError::unauthorized("missing API key");
/// assert!(err.to_string().contains("missing API key"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[non_exhaustive]
pub enum AuthzError {
    /// The actor is not authenticated (401).
    #[error("unauthorized: {reason}")]
    Unauthorized {
        /// Why authentication failed.
        reason: String,
    },

    /// The actor is authenticated but lacks permission (403).
    #[error("forbidden: {reason}")]
    Forbidden {
        /// Why the operation was denied.
        reason: String,
    },

    /// The actor has exceeded their rate limit (429).
    #[error("rate limited: {reason}")]
    RateLimited {
        /// Details about the rate limit violation.
        reason: String,
    },

    /// An invalid role was referenced.
    #[error("invalid role: {name}")]
    InvalidRole {
        /// The role name that was not found.
        name: String,
    },

    /// An invalid resource was referenced.
    #[error("invalid resource: {name}")]
    InvalidResource {
        /// The resource name that was not found.
        name: String,
    },
}

impl AuthzError {
    /// Creates an [`Unauthorized`](Self::Unauthorized) error.
    #[must_use]
    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::Unauthorized {
            reason: reason.into(),
        }
    }

    /// Creates a [`Forbidden`](Self::Forbidden) error.
    #[must_use]
    pub fn forbidden(reason: impl Into<String>) -> Self {
        Self::Forbidden {
            reason: reason.into(),
        }
    }

    /// Creates a [`RateLimited`](Self::RateLimited) error.
    #[must_use]
    pub fn rate_limited(reason: impl Into<String>) -> Self {
        Self::RateLimited {
            reason: reason.into(),
        }
    }

    /// Creates an [`InvalidRole`](Self::InvalidRole) error.
    #[must_use]
    pub fn invalid_role(name: impl Into<String>) -> Self {
        Self::InvalidRole {
            name: name.into(),
        }
    }

    /// Creates an [`InvalidResource`](Self::InvalidResource) error.
    #[must_use]
    pub fn invalid_resource(name: impl Into<String>) -> Self {
        Self::InvalidResource {
            name: name.into(),
        }
    }

    /// Returns `true` if this is an unauthorized error.
    #[must_use]
    pub const fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized { .. })
    }

    /// Returns `true` if this is a forbidden error.
    #[must_use]
    pub const fn is_forbidden(&self) -> bool {
        matches!(self, Self::Forbidden { .. })
    }

    /// Returns `true` if this is a rate limited error.
    #[must_use]
    pub const fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimited { .. })
    }
}

/// Type alias for results using [`AuthzError`].
pub type AuthzResult<T> = Result<T, AuthzError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_display() {
        let err = AuthzError::unauthorized("missing token");
        assert_eq!(err.to_string(), "unauthorized: missing token");
    }

    #[test]
    fn forbidden_display() {
        let err = AuthzError::forbidden("insufficient permission");
        assert_eq!(err.to_string(), "forbidden: insufficient permission");
    }

    #[test]
    fn rate_limited_display() {
        let err = AuthzError::rate_limited("too many requests");
        assert_eq!(err.to_string(), "rate limited: too many requests");
    }

    #[test]
    fn invalid_role_display() {
        let err = AuthzError::invalid_role("superuser");
        assert_eq!(err.to_string(), "invalid role: superuser");
    }

    #[test]
    fn invalid_resource_display() {
        let err = AuthzError::invalid_resource("widgets");
        assert_eq!(err.to_string(), "invalid resource: widgets");
    }

    #[test]
    fn is_helpers() {
        assert!(AuthzError::unauthorized("x").is_unauthorized());
        assert!(!AuthzError::unauthorized("x").is_forbidden());
        assert!(!AuthzError::unauthorized("x").is_rate_limited());

        assert!(AuthzError::forbidden("x").is_forbidden());
        assert!(AuthzError::rate_limited("x").is_rate_limited());
    }

    #[test]
    fn error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(AuthzError::unauthorized("test"));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn serde_roundtrip() {
        let errors = vec![
            AuthzError::unauthorized("a"),
            AuthzError::forbidden("b"),
            AuthzError::rate_limited("c"),
            AuthzError::invalid_role("d"),
            AuthzError::invalid_resource("e"),
        ];

        for err in errors {
            let json = serde_json::to_string(&err).unwrap();
            let parsed: AuthzError = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, err);
        }
    }

    #[test]
    fn equality() {
        assert_eq!(
            AuthzError::unauthorized("x"),
            AuthzError::unauthorized("x")
        );
        assert_ne!(
            AuthzError::unauthorized("x"),
            AuthzError::unauthorized("y")
        );
        assert_ne!(
            AuthzError::unauthorized("x"),
            AuthzError::forbidden("x")
        );
    }
}
