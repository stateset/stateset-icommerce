//! Subscription state machine.
//!
//! ```text
//! trial   -> active    (trial expires or manually activated)
//! active  -> paused    (subscriber request)
//! active  -> past_due  (billing failure)
//! active  -> cancelled (immediate or at period end)
//! paused  -> active    (resume)
//! paused  -> cancelled
//! past_due -> active   (payment recovered)
//! past_due -> cancelled (max retries exceeded)
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Status of a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SubscriptionStatus {
    /// In trial period, not yet billing.
    Trial,
    /// Active and billing normally.
    Active,
    /// Billing suspended at subscriber request.
    Paused,
    /// Billing attempted but payment failed.
    PastDue,
    /// Terminated (no further billing).
    Cancelled,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trial => write!(f, "trial"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::PastDue => write!(f, "past_due"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl SubscriptionStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Trial => &[Self::Active, Self::Cancelled],
            Self::Active => &[Self::Paused, Self::PastDue, Self::Cancelled],
            Self::Paused => &[Self::Active, Self::Cancelled],
            Self::PastDue => &[Self::Active, Self::Cancelled],
            Self::Cancelled => &[],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Whether this status is terminal (no further transitions possible).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// A validated subscription state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionTransition {
    /// State before the transition.
    pub from: SubscriptionStatus,
    /// State after the transition.
    pub to: SubscriptionStatus,
}

impl SubscriptionTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: SubscriptionStatus, to: SubscriptionStatus) -> A2AResult<Self> {
        if from.can_transition_to(to) {
            Ok(Self { from, to })
        } else {
            let allowed: Vec<&str> = from
                .allowed_transitions()
                .iter()
                .map(|s| match s {
                    SubscriptionStatus::Trial => "trial",
                    SubscriptionStatus::Active => "active",
                    SubscriptionStatus::Paused => "paused",
                    SubscriptionStatus::PastDue => "past_due",
                    SubscriptionStatus::Cancelled => "cancelled",
                })
                .collect();
            Err(A2AError::invalid_transition(from, to, &allowed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Trial transitions =====

    #[test]
    fn trial_to_active() {
        assert!(SubscriptionStatus::Trial.can_transition_to(SubscriptionStatus::Active));
    }

    #[test]
    fn trial_to_cancelled() {
        assert!(SubscriptionStatus::Trial.can_transition_to(SubscriptionStatus::Cancelled));
    }

    #[test]
    fn trial_cannot_go_to_paused() {
        assert!(!SubscriptionStatus::Trial.can_transition_to(SubscriptionStatus::Paused));
    }

    #[test]
    fn trial_cannot_go_to_past_due() {
        assert!(!SubscriptionStatus::Trial.can_transition_to(SubscriptionStatus::PastDue));
    }

    // ===== Active transitions =====

    #[test]
    fn active_to_paused() {
        assert!(SubscriptionStatus::Active.can_transition_to(SubscriptionStatus::Paused));
    }

    #[test]
    fn active_to_cancelled() {
        assert!(SubscriptionStatus::Active.can_transition_to(SubscriptionStatus::Cancelled));
    }

    #[test]
    fn active_to_past_due() {
        assert!(SubscriptionStatus::Active.can_transition_to(SubscriptionStatus::PastDue));
    }

    #[test]
    fn active_cannot_go_to_trial() {
        assert!(!SubscriptionStatus::Active.can_transition_to(SubscriptionStatus::Trial));
    }

    // ===== Paused transitions =====

    #[test]
    fn paused_to_active() {
        assert!(SubscriptionStatus::Paused.can_transition_to(SubscriptionStatus::Active));
    }

    #[test]
    fn paused_to_cancelled() {
        assert!(SubscriptionStatus::Paused.can_transition_to(SubscriptionStatus::Cancelled));
    }

    #[test]
    fn paused_cannot_go_to_trial() {
        assert!(!SubscriptionStatus::Paused.can_transition_to(SubscriptionStatus::Trial));
    }

    // ===== PastDue transitions =====

    #[test]
    fn past_due_to_active() {
        assert!(SubscriptionStatus::PastDue.can_transition_to(SubscriptionStatus::Active));
    }

    #[test]
    fn past_due_to_cancelled() {
        assert!(SubscriptionStatus::PastDue.can_transition_to(SubscriptionStatus::Cancelled));
    }

    #[test]
    fn past_due_cannot_go_to_paused() {
        assert!(!SubscriptionStatus::PastDue.can_transition_to(SubscriptionStatus::Paused));
    }

    // ===== Cancelled is terminal =====

    #[test]
    fn cancelled_is_terminal() {
        assert!(SubscriptionStatus::Cancelled.is_terminal());
        assert!(SubscriptionStatus::Cancelled.allowed_transitions().is_empty());
    }

    #[test]
    fn cancelled_cannot_go_anywhere() {
        assert!(!SubscriptionStatus::Cancelled.can_transition_to(SubscriptionStatus::Active));
        assert!(!SubscriptionStatus::Cancelled.can_transition_to(SubscriptionStatus::Paused));
        assert!(!SubscriptionStatus::Cancelled.can_transition_to(SubscriptionStatus::Trial));
    }

    // ===== Non-terminal checks =====

    #[test]
    fn trial_is_not_terminal() {
        assert!(!SubscriptionStatus::Trial.is_terminal());
    }

    #[test]
    fn active_is_not_terminal() {
        assert!(!SubscriptionStatus::Active.is_terminal());
    }

    // ===== Transition struct =====

    #[test]
    fn transition_new_valid() {
        let t = SubscriptionTransition::new(
            SubscriptionStatus::Active,
            SubscriptionStatus::Paused,
        )
        .unwrap();
        assert_eq!(t.from, SubscriptionStatus::Active);
        assert_eq!(t.to, SubscriptionStatus::Paused);
    }

    #[test]
    fn transition_new_invalid() {
        let err = SubscriptionTransition::new(
            SubscriptionStatus::Trial,
            SubscriptionStatus::Paused,
        )
        .unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn transition_trial_to_active() {
        let t = SubscriptionTransition::new(
            SubscriptionStatus::Trial,
            SubscriptionStatus::Active,
        )
        .unwrap();
        assert_eq!(t.to, SubscriptionStatus::Active);
    }

    // ===== Display =====

    #[test]
    fn status_display() {
        assert_eq!(SubscriptionStatus::Trial.to_string(), "trial");
        assert_eq!(SubscriptionStatus::Active.to_string(), "active");
        assert_eq!(SubscriptionStatus::Paused.to_string(), "paused");
        assert_eq!(SubscriptionStatus::PastDue.to_string(), "past_due");
        assert_eq!(SubscriptionStatus::Cancelled.to_string(), "cancelled");
    }
}
