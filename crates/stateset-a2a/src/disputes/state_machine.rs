//! Dispute status state machine.
//!
//! ```text
//! filed → evidence_period → under_review → resolved
//!                                        → escalated
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Status of a dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DisputeStatus {
    /// Dispute has been filed.
    Filed,
    /// Evidence collection period (72 hours).
    EvidencePeriod,
    /// Under review by mediator/arbitrator.
    UnderReview,
    /// Dispute resolved with an outcome.
    Resolved,
    /// Dispute escalated to human review.
    Escalated,
}

impl std::fmt::Display for DisputeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Filed => write!(f, "filed"),
            Self::EvidencePeriod => write!(f, "evidence_period"),
            Self::UnderReview => write!(f, "under_review"),
            Self::Resolved => write!(f, "resolved"),
            Self::Escalated => write!(f, "escalated"),
        }
    }
}

impl DisputeStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Filed => &[Self::EvidencePeriod],
            Self::EvidencePeriod => &[Self::UnderReview],
            Self::UnderReview => &[Self::Resolved, Self::Escalated],
            Self::Resolved | Self::Escalated => &[],
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
        matches!(self, Self::Resolved | Self::Escalated)
    }

    /// Whether evidence submission is allowed in this status.
    #[must_use]
    pub const fn allows_evidence(self) -> bool {
        matches!(self, Self::Filed | Self::EvidencePeriod)
    }

    /// Whether resolution is allowed in this status.
    #[must_use]
    pub const fn allows_resolution(self) -> bool {
        matches!(self, Self::Filed | Self::EvidencePeriod | Self::UnderReview)
    }
}

/// A validated dispute state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisputeTransition {
    /// State before the transition.
    pub from: DisputeStatus,
    /// State after the transition.
    pub to: DisputeStatus,
}

impl DisputeTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: DisputeStatus, to: DisputeStatus) -> A2AResult<Self> {
        if from.can_transition_to(to) {
            Ok(Self { from, to })
        } else {
            let allowed: Vec<&str> =
                from.allowed_transitions().iter().map(|s| s.as_str()).collect();
            Err(A2AError::invalid_transition(from, to, &allowed))
        }
    }
}

impl DisputeStatus {
    /// Get the `snake_case` string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filed => "filed",
            Self::EvidencePeriod => "evidence_period",
            Self::UnderReview => "under_review",
            Self::Resolved => "resolved",
            Self::Escalated => "escalated",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Filed transitions =====

    #[test]
    fn filed_can_go_to_evidence_period() {
        assert!(DisputeStatus::Filed.can_transition_to(DisputeStatus::EvidencePeriod));
    }

    #[test]
    fn filed_cannot_go_to_under_review() {
        assert!(!DisputeStatus::Filed.can_transition_to(DisputeStatus::UnderReview));
    }

    #[test]
    fn filed_cannot_go_to_resolved() {
        assert!(!DisputeStatus::Filed.can_transition_to(DisputeStatus::Resolved));
    }

    // ===== EvidencePeriod transitions =====

    #[test]
    fn evidence_period_can_go_to_under_review() {
        assert!(DisputeStatus::EvidencePeriod.can_transition_to(DisputeStatus::UnderReview));
    }

    #[test]
    fn evidence_period_cannot_go_to_resolved() {
        assert!(!DisputeStatus::EvidencePeriod.can_transition_to(DisputeStatus::Resolved));
    }

    // ===== UnderReview transitions =====

    #[test]
    fn under_review_can_go_to_resolved() {
        assert!(DisputeStatus::UnderReview.can_transition_to(DisputeStatus::Resolved));
    }

    #[test]
    fn under_review_can_go_to_escalated() {
        assert!(DisputeStatus::UnderReview.can_transition_to(DisputeStatus::Escalated));
    }

    #[test]
    fn under_review_cannot_go_back_to_filed() {
        assert!(!DisputeStatus::UnderReview.can_transition_to(DisputeStatus::Filed));
    }

    // ===== Terminal states =====

    #[test]
    fn resolved_is_terminal() {
        assert!(DisputeStatus::Resolved.is_terminal());
        assert!(DisputeStatus::Resolved.allowed_transitions().is_empty());
    }

    #[test]
    fn escalated_is_terminal() {
        assert!(DisputeStatus::Escalated.is_terminal());
        assert!(DisputeStatus::Escalated.allowed_transitions().is_empty());
    }

    #[test]
    fn filed_is_not_terminal() {
        assert!(!DisputeStatus::Filed.is_terminal());
    }

    #[test]
    fn evidence_period_is_not_terminal() {
        assert!(!DisputeStatus::EvidencePeriod.is_terminal());
    }

    // ===== Evidence submission =====

    #[test]
    fn filed_allows_evidence() {
        assert!(DisputeStatus::Filed.allows_evidence());
    }

    #[test]
    fn evidence_period_allows_evidence() {
        assert!(DisputeStatus::EvidencePeriod.allows_evidence());
    }

    #[test]
    fn under_review_does_not_allow_evidence() {
        assert!(!DisputeStatus::UnderReview.allows_evidence());
    }

    #[test]
    fn resolved_does_not_allow_evidence() {
        assert!(!DisputeStatus::Resolved.allows_evidence());
    }

    // ===== Resolution allowed =====

    #[test]
    fn filed_allows_resolution() {
        assert!(DisputeStatus::Filed.allows_resolution());
    }

    #[test]
    fn under_review_allows_resolution() {
        assert!(DisputeStatus::UnderReview.allows_resolution());
    }

    #[test]
    fn resolved_does_not_allow_resolution() {
        assert!(!DisputeStatus::Resolved.allows_resolution());
    }

    #[test]
    fn escalated_does_not_allow_resolution() {
        assert!(!DisputeStatus::Escalated.allows_resolution());
    }

    // ===== Display =====

    #[test]
    fn status_display() {
        assert_eq!(DisputeStatus::Filed.to_string(), "filed");
        assert_eq!(DisputeStatus::EvidencePeriod.to_string(), "evidence_period");
        assert_eq!(DisputeStatus::UnderReview.to_string(), "under_review");
        assert_eq!(DisputeStatus::Resolved.to_string(), "resolved");
        assert_eq!(DisputeStatus::Escalated.to_string(), "escalated");
    }

    // ===== Transition struct =====

    #[test]
    fn transition_valid() {
        let t =
            DisputeTransition::new(DisputeStatus::Filed, DisputeStatus::EvidencePeriod).unwrap();
        assert_eq!(t.from, DisputeStatus::Filed);
        assert_eq!(t.to, DisputeStatus::EvidencePeriod);
    }

    #[test]
    fn transition_invalid() {
        let err =
            DisputeTransition::new(DisputeStatus::Filed, DisputeStatus::Resolved).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn transition_from_terminal_fails() {
        let err =
            DisputeTransition::new(DisputeStatus::Resolved, DisputeStatus::Filed).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn full_lifecycle_transition() {
        let t1 =
            DisputeTransition::new(DisputeStatus::Filed, DisputeStatus::EvidencePeriod).unwrap();
        let t2 = DisputeTransition::new(t1.to, DisputeStatus::UnderReview).unwrap();
        let t3 = DisputeTransition::new(t2.to, DisputeStatus::Resolved).unwrap();
        assert_eq!(t3.to, DisputeStatus::Resolved);
    }
}
