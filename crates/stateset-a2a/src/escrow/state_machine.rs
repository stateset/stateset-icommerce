//! Escrow state machine.
//!
//! ```text
//! created -> funded -> active -> released
//!                             -> refunded
//!                             -> disputed
//!                             -> expired
//!            funded -> released
//!            funded -> refunded
//!            funded -> disputed
//!            funded -> expired
//! created -> refunded
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Status of an escrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EscrowStatus {
    /// Escrow created, awaiting funding.
    Created,
    /// Funds deposited.
    Funded,
    /// Actively held, conditions being evaluated.
    Active,
    /// Funds released to the seller.
    Released,
    /// Funds returned to the buyer.
    Refunded,
    /// Dispute raised, pending resolution.
    Disputed,
    /// Escrow expired (auto-refund).
    Expired,
}

impl std::fmt::Display for EscrowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Funded => write!(f, "funded"),
            Self::Active => write!(f, "active"),
            Self::Released => write!(f, "released"),
            Self::Refunded => write!(f, "refunded"),
            Self::Disputed => write!(f, "disputed"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl EscrowStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Created => &[Self::Funded, Self::Refunded],
            Self::Funded => {
                &[Self::Active, Self::Released, Self::Refunded, Self::Disputed, Self::Expired]
            }
            Self::Active => &[Self::Released, Self::Refunded, Self::Disputed, Self::Expired],
            Self::Released | Self::Refunded | Self::Disputed | Self::Expired => &[],
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
        matches!(self, Self::Released | Self::Refunded | Self::Disputed | Self::Expired)
    }
}

/// A validated escrow state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EscrowTransition {
    /// State before the transition.
    pub from: EscrowStatus,
    /// State after the transition.
    pub to: EscrowStatus,
}

impl EscrowTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: EscrowStatus, to: EscrowStatus) -> A2AResult<Self> {
        if from.can_transition_to(to) {
            Ok(Self { from, to })
        } else {
            let allowed: Vec<&str> = from
                .allowed_transitions()
                .iter()
                .map(|s| match s {
                    EscrowStatus::Created => "created",
                    EscrowStatus::Funded => "funded",
                    EscrowStatus::Active => "active",
                    EscrowStatus::Released => "released",
                    EscrowStatus::Refunded => "refunded",
                    EscrowStatus::Disputed => "disputed",
                    EscrowStatus::Expired => "expired",
                })
                .collect();
            Err(A2AError::invalid_transition(from, to, &allowed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Created transitions =====

    #[test]
    fn created_can_go_to_funded() {
        assert!(EscrowStatus::Created.can_transition_to(EscrowStatus::Funded));
    }

    #[test]
    fn created_can_go_to_refunded() {
        assert!(EscrowStatus::Created.can_transition_to(EscrowStatus::Refunded));
    }

    #[test]
    fn created_cannot_go_to_released() {
        assert!(!EscrowStatus::Created.can_transition_to(EscrowStatus::Released));
    }

    #[test]
    fn created_cannot_go_to_active() {
        assert!(!EscrowStatus::Created.can_transition_to(EscrowStatus::Active));
    }

    // ===== Funded transitions =====

    #[test]
    fn funded_can_go_to_active() {
        assert!(EscrowStatus::Funded.can_transition_to(EscrowStatus::Active));
    }

    #[test]
    fn funded_can_go_to_released() {
        assert!(EscrowStatus::Funded.can_transition_to(EscrowStatus::Released));
    }

    #[test]
    fn funded_can_go_to_refunded() {
        assert!(EscrowStatus::Funded.can_transition_to(EscrowStatus::Refunded));
    }

    #[test]
    fn funded_can_go_to_disputed() {
        assert!(EscrowStatus::Funded.can_transition_to(EscrowStatus::Disputed));
    }

    #[test]
    fn funded_can_go_to_expired() {
        assert!(EscrowStatus::Funded.can_transition_to(EscrowStatus::Expired));
    }

    // ===== Active transitions =====

    #[test]
    fn active_can_go_to_released() {
        assert!(EscrowStatus::Active.can_transition_to(EscrowStatus::Released));
    }

    #[test]
    fn active_can_go_to_refunded() {
        assert!(EscrowStatus::Active.can_transition_to(EscrowStatus::Refunded));
    }

    #[test]
    fn active_can_go_to_disputed() {
        assert!(EscrowStatus::Active.can_transition_to(EscrowStatus::Disputed));
    }

    #[test]
    fn active_can_go_to_expired() {
        assert!(EscrowStatus::Active.can_transition_to(EscrowStatus::Expired));
    }

    #[test]
    fn active_cannot_go_back_to_created() {
        assert!(!EscrowStatus::Active.can_transition_to(EscrowStatus::Created));
    }

    // ===== Terminal states =====

    #[test]
    fn released_is_terminal() {
        assert!(EscrowStatus::Released.is_terminal());
        assert!(EscrowStatus::Released.allowed_transitions().is_empty());
    }

    #[test]
    fn refunded_is_terminal() {
        assert!(EscrowStatus::Refunded.is_terminal());
        assert!(EscrowStatus::Refunded.allowed_transitions().is_empty());
    }

    #[test]
    fn disputed_is_terminal() {
        assert!(EscrowStatus::Disputed.is_terminal());
        assert!(EscrowStatus::Disputed.allowed_transitions().is_empty());
    }

    #[test]
    fn expired_is_terminal() {
        assert!(EscrowStatus::Expired.is_terminal());
        assert!(EscrowStatus::Expired.allowed_transitions().is_empty());
    }

    // ===== Non-terminal checks =====

    #[test]
    fn created_is_not_terminal() {
        assert!(!EscrowStatus::Created.is_terminal());
    }

    #[test]
    fn funded_is_not_terminal() {
        assert!(!EscrowStatus::Funded.is_terminal());
    }

    #[test]
    fn active_is_not_terminal() {
        assert!(!EscrowStatus::Active.is_terminal());
    }

    // ===== Transition struct =====

    #[test]
    fn transition_new_valid() {
        let t = EscrowTransition::new(EscrowStatus::Created, EscrowStatus::Funded).unwrap();
        assert_eq!(t.from, EscrowStatus::Created);
        assert_eq!(t.to, EscrowStatus::Funded);
    }

    #[test]
    fn transition_new_invalid() {
        let err = EscrowTransition::new(EscrowStatus::Created, EscrowStatus::Released).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn transition_funded_to_active() {
        let t = EscrowTransition::new(EscrowStatus::Funded, EscrowStatus::Active).unwrap();
        assert_eq!(t.to, EscrowStatus::Active);
    }

    #[test]
    fn transition_active_to_released() {
        let t = EscrowTransition::new(EscrowStatus::Active, EscrowStatus::Released).unwrap();
        assert_eq!(t.to, EscrowStatus::Released);
    }

    #[test]
    fn transition_from_terminal_fails() {
        let err = EscrowTransition::new(EscrowStatus::Released, EscrowStatus::Active).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    // ===== Display =====

    #[test]
    fn status_display() {
        assert_eq!(EscrowStatus::Created.to_string(), "created");
        assert_eq!(EscrowStatus::Funded.to_string(), "funded");
        assert_eq!(EscrowStatus::Active.to_string(), "active");
        assert_eq!(EscrowStatus::Released.to_string(), "released");
        assert_eq!(EscrowStatus::Refunded.to_string(), "refunded");
        assert_eq!(EscrowStatus::Disputed.to_string(), "disputed");
        assert_eq!(EscrowStatus::Expired.to_string(), "expired");
    }
}
