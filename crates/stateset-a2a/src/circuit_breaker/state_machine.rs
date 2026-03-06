//! Circuit breaker state machine.
//!
//! ```text
//! closed ──(failure rate exceeded)──→ open
//!   ↑                                    ↓
//!   └──(success threshold met)── half_open ←──(cooldown elapsed)
//!                                    ↓
//!                               open (any failure)
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// State of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CircuitState {
    /// Normal operation — all transactions allowed.
    Closed,
    /// Tripped — all transactions blocked until cooldown.
    Open,
    /// Recovery testing — limited transactions allowed.
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::Closed
    }
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "closed"),
            Self::Open => write!(f, "open"),
            Self::HalfOpen => write!(f, "half_open"),
        }
    }
}

impl CircuitState {
    /// Return the set of states this state can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Closed => &[Self::Open],
            Self::Open => &[Self::HalfOpen],
            Self::HalfOpen => &[Self::Closed, Self::Open],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Whether this state blocks all transactions.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(self, Self::Open)
    }

    /// Whether this state allows limited transactions (`half_open` testing).
    #[must_use]
    pub const fn is_testing(self) -> bool {
        matches!(self, Self::HalfOpen)
    }

    /// Whether this state allows normal transactions.
    #[must_use]
    pub const fn is_normal(self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Get the `snake_case` string for this state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }
}

/// A validated circuit state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitTransition {
    /// State before the transition.
    pub from: CircuitState,
    /// State after the transition.
    pub to: CircuitState,
}

impl CircuitTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: CircuitState, to: CircuitState) -> A2AResult<Self> {
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

    // ===== Closed transitions =====

    #[test]
    fn closed_can_go_to_open() {
        assert!(CircuitState::Closed.can_transition_to(CircuitState::Open));
    }

    #[test]
    fn closed_cannot_go_to_half_open() {
        assert!(!CircuitState::Closed.can_transition_to(CircuitState::HalfOpen));
    }

    // ===== Open transitions =====

    #[test]
    fn open_can_go_to_half_open() {
        assert!(CircuitState::Open.can_transition_to(CircuitState::HalfOpen));
    }

    #[test]
    fn open_cannot_go_to_closed() {
        assert!(!CircuitState::Open.can_transition_to(CircuitState::Closed));
    }

    // ===== HalfOpen transitions =====

    #[test]
    fn half_open_can_go_to_closed() {
        assert!(CircuitState::HalfOpen.can_transition_to(CircuitState::Closed));
    }

    #[test]
    fn half_open_can_go_to_open() {
        assert!(CircuitState::HalfOpen.can_transition_to(CircuitState::Open));
    }

    // ===== State properties =====

    #[test]
    fn closed_is_normal() {
        assert!(CircuitState::Closed.is_normal());
        assert!(!CircuitState::Closed.is_blocking());
        assert!(!CircuitState::Closed.is_testing());
    }

    #[test]
    fn open_is_blocking() {
        assert!(CircuitState::Open.is_blocking());
        assert!(!CircuitState::Open.is_normal());
        assert!(!CircuitState::Open.is_testing());
    }

    #[test]
    fn half_open_is_testing() {
        assert!(CircuitState::HalfOpen.is_testing());
        assert!(!CircuitState::HalfOpen.is_blocking());
        assert!(!CircuitState::HalfOpen.is_normal());
    }

    // ===== Default =====

    #[test]
    fn default_is_closed() {
        assert_eq!(CircuitState::default(), CircuitState::Closed);
    }

    // ===== Display =====

    #[test]
    fn state_display() {
        assert_eq!(CircuitState::Closed.to_string(), "closed");
        assert_eq!(CircuitState::Open.to_string(), "open");
        assert_eq!(CircuitState::HalfOpen.to_string(), "half_open");
    }

    // ===== Transition struct =====

    #[test]
    fn transition_valid() {
        let t = CircuitTransition::new(CircuitState::Closed, CircuitState::Open).unwrap();
        assert_eq!(t.from, CircuitState::Closed);
        assert_eq!(t.to, CircuitState::Open);
    }

    #[test]
    fn transition_invalid() {
        let err = CircuitTransition::new(CircuitState::Closed, CircuitState::HalfOpen).unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn full_recovery_cycle() {
        let t1 = CircuitTransition::new(CircuitState::Closed, CircuitState::Open).unwrap();
        let t2 = CircuitTransition::new(t1.to, CircuitState::HalfOpen).unwrap();
        let t3 = CircuitTransition::new(t2.to, CircuitState::Closed).unwrap();
        assert_eq!(t3.to, CircuitState::Closed);
    }

    #[test]
    fn half_open_failure_reopens() {
        let t1 = CircuitTransition::new(CircuitState::HalfOpen, CircuitState::Open).unwrap();
        assert_eq!(t1.to, CircuitState::Open);
    }
}
