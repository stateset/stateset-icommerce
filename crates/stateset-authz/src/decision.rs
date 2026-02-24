//! Access decisions returned by authorization checks.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The outcome of an authorization check.
///
/// ```rust
/// use stateset_authz::AccessDecision;
///
/// let allowed = AccessDecision::Allowed;
/// assert!(allowed.is_allowed());
/// assert!(!allowed.is_denied());
///
/// let denied = AccessDecision::denied("insufficient permission");
/// assert!(denied.is_denied());
/// assert!(!denied.is_allowed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AccessDecision {
    /// The operation is allowed.
    Allowed,
    /// The operation is denied.
    Denied {
        /// Human-readable explanation for the denial.
        reason: String,
    },
    /// The operation requires explicit approval before proceeding.
    RequiresApproval {
        /// Human-readable explanation for why approval is needed.
        reason: String,
    },
}

impl AccessDecision {
    /// Creates a [`Denied`](Self::Denied) decision with the given reason.
    #[must_use]
    pub fn denied(reason: impl Into<String>) -> Self {
        Self::Denied { reason: reason.into() }
    }

    /// Creates a [`RequiresApproval`](Self::RequiresApproval) decision with the given reason.
    #[must_use]
    pub fn requires_approval(reason: impl Into<String>) -> Self {
        Self::RequiresApproval { reason: reason.into() }
    }

    /// Returns `true` if the decision is [`Allowed`](Self::Allowed).
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Returns `true` if the decision is [`Denied`](Self::Denied).
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }

    /// Returns `true` if the decision is [`RequiresApproval`](Self::RequiresApproval).
    #[must_use]
    pub const fn requires_approval_check(&self) -> bool {
        matches!(self, Self::RequiresApproval { .. })
    }

    /// Returns the reason string, if any.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allowed => None,
            Self::Denied { reason } | Self::RequiresApproval { reason } => Some(reason),
        }
    }
}

impl fmt::Display for AccessDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allowed => f.write_str("allowed"),
            Self::Denied { reason } => write!(f, "denied: {reason}"),
            Self::RequiresApproval { reason } => write!(f, "requires approval: {reason}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_helpers() {
        let d = AccessDecision::Allowed;
        assert!(d.is_allowed());
        assert!(!d.is_denied());
        assert!(!d.requires_approval_check());
        assert!(d.reason().is_none());
    }

    #[test]
    fn denied_helpers() {
        let d = AccessDecision::denied("no access");
        assert!(!d.is_allowed());
        assert!(d.is_denied());
        assert!(!d.requires_approval_check());
        assert_eq!(d.reason(), Some("no access"));
    }

    #[test]
    fn requires_approval_helpers() {
        let d = AccessDecision::requires_approval("large order");
        assert!(!d.is_allowed());
        assert!(!d.is_denied());
        assert!(d.requires_approval_check());
        assert_eq!(d.reason(), Some("large order"));
    }

    #[test]
    fn display_allowed() {
        assert_eq!(AccessDecision::Allowed.to_string(), "allowed");
    }

    #[test]
    fn display_denied() {
        let d = AccessDecision::denied("rate limited");
        assert_eq!(d.to_string(), "denied: rate limited");
    }

    #[test]
    fn display_requires_approval() {
        let d = AccessDecision::requires_approval("high value");
        assert_eq!(d.to_string(), "requires approval: high value");
    }

    #[test]
    fn serde_roundtrip_allowed() {
        let d = AccessDecision::Allowed;
        let json = serde_json::to_string(&d).unwrap();
        let parsed: AccessDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, d);
    }

    #[test]
    fn serde_roundtrip_denied() {
        let d = AccessDecision::denied("forbidden");
        let json = serde_json::to_string(&d).unwrap();
        let parsed: AccessDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, d);
    }

    #[test]
    fn serde_roundtrip_requires_approval() {
        let d = AccessDecision::requires_approval("needs review");
        let json = serde_json::to_string(&d).unwrap();
        let parsed: AccessDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, d);
    }

    #[test]
    fn equality() {
        assert_eq!(AccessDecision::Allowed, AccessDecision::Allowed);
        assert_ne!(AccessDecision::Allowed, AccessDecision::denied("x"));
        assert_ne!(AccessDecision::denied("a"), AccessDecision::denied("b"));
    }
}
