//! RFQ and response status state machines.

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Status of an RFQ (Request for Quote).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RfqStatus {
    /// Open and accepting responses.
    Open,
    /// Winner selected.
    Awarded,
    /// Past deadline, no award.
    Expired,
    /// Manually closed without award.
    Closed,
}

impl std::fmt::Display for RfqStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Awarded => write!(f, "awarded"),
            Self::Expired => write!(f, "expired"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

impl RfqStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Open => &[Self::Awarded, Self::Expired, Self::Closed],
            Self::Awarded | Self::Expired | Self::Closed => &[],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Whether this status is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Awarded | Self::Expired | Self::Closed)
    }

    /// Get the `snake_case` string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Awarded => "awarded",
            Self::Expired => "expired",
            Self::Closed => "closed",
        }
    }
}

/// Status of an individual RFQ response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RfqResponseStatus {
    /// Awaiting quote from seller.
    Pending,
    /// Quote received and scored.
    Scored,
    /// Lost to a competitor.
    Declined,
    /// Selected as the winner.
    Awarded,
}

impl std::fmt::Display for RfqResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Scored => write!(f, "scored"),
            Self::Declined => write!(f, "declined"),
            Self::Awarded => write!(f, "awarded"),
        }
    }
}

impl RfqResponseStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Scored, Self::Declined],
            Self::Scored => &[Self::Awarded, Self::Declined],
            Self::Declined | Self::Awarded => &[],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Whether this status is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Declined | Self::Awarded)
    }
}

/// A validated RFQ state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RfqTransition {
    /// State before the transition.
    pub from: RfqStatus,
    /// State after the transition.
    pub to: RfqStatus,
}

impl RfqTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: RfqStatus, to: RfqStatus) -> A2AResult<Self> {
        if from.can_transition_to(to) {
            Ok(Self { from, to })
        } else {
            let allowed: Vec<&str> =
                from.allowed_transitions().iter().map(|s| s.as_str()).collect();
            Err(A2AError::invalid_transition(from, to, &allowed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== RfqStatus transitions =====

    #[test]
    fn open_can_go_to_awarded() {
        assert!(RfqStatus::Open.can_transition_to(RfqStatus::Awarded));
    }

    #[test]
    fn open_can_go_to_expired() {
        assert!(RfqStatus::Open.can_transition_to(RfqStatus::Expired));
    }

    #[test]
    fn open_can_go_to_closed() {
        assert!(RfqStatus::Open.can_transition_to(RfqStatus::Closed));
    }

    #[test]
    fn awarded_is_terminal() {
        assert!(RfqStatus::Awarded.is_terminal());
        assert!(RfqStatus::Awarded.allowed_transitions().is_empty());
    }

    #[test]
    fn expired_is_terminal() {
        assert!(RfqStatus::Expired.is_terminal());
    }

    #[test]
    fn closed_is_terminal() {
        assert!(RfqStatus::Closed.is_terminal());
    }

    #[test]
    fn open_is_not_terminal() {
        assert!(!RfqStatus::Open.is_terminal());
    }

    #[test]
    fn rfq_display() {
        assert_eq!(RfqStatus::Open.to_string(), "open");
        assert_eq!(RfqStatus::Awarded.to_string(), "awarded");
        assert_eq!(RfqStatus::Expired.to_string(), "expired");
        assert_eq!(RfqStatus::Closed.to_string(), "closed");
    }

    // ===== RfqResponseStatus transitions =====

    #[test]
    fn pending_can_go_to_scored() {
        assert!(RfqResponseStatus::Pending.can_transition_to(RfqResponseStatus::Scored));
    }

    #[test]
    fn pending_can_go_to_declined() {
        assert!(RfqResponseStatus::Pending.can_transition_to(RfqResponseStatus::Declined));
    }

    #[test]
    fn scored_can_go_to_awarded() {
        assert!(RfqResponseStatus::Scored.can_transition_to(RfqResponseStatus::Awarded));
    }

    #[test]
    fn scored_can_go_to_declined() {
        assert!(RfqResponseStatus::Scored.can_transition_to(RfqResponseStatus::Declined));
    }

    #[test]
    fn awarded_response_is_terminal() {
        assert!(RfqResponseStatus::Awarded.is_terminal());
    }

    #[test]
    fn declined_response_is_terminal() {
        assert!(RfqResponseStatus::Declined.is_terminal());
    }

    #[test]
    fn response_display() {
        assert_eq!(RfqResponseStatus::Pending.to_string(), "pending");
        assert_eq!(RfqResponseStatus::Scored.to_string(), "scored");
        assert_eq!(RfqResponseStatus::Declined.to_string(), "declined");
        assert_eq!(RfqResponseStatus::Awarded.to_string(), "awarded");
    }

    // ===== RfqTransition =====

    #[test]
    fn transition_open_to_awarded() {
        let t = RfqTransition::new(RfqStatus::Open, RfqStatus::Awarded).unwrap();
        assert_eq!(t.to, RfqStatus::Awarded);
    }

    #[test]
    fn transition_from_terminal_fails() {
        let err = RfqTransition::new(RfqStatus::Awarded, RfqStatus::Open).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }
}
