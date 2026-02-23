//! Split payment state machine.
//!
//! ```text
//! pending -> processing -> completed
//!                       -> partial
//!                       -> failed
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Status of a split payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SplitPaymentStatus {
    /// Created but not yet executed.
    Pending,
    /// Payments are being sent to recipients.
    Processing,
    /// All recipient payments succeeded.
    Completed,
    /// Some recipient payments succeeded, some failed.
    Partial,
    /// All recipient payments failed.
    Failed,
}

impl std::fmt::Display for SplitPaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Partial => write!(f, "partial"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl SplitPaymentStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Processing],
            Self::Processing => &[Self::Completed, Self::Partial, Self::Failed],
            Self::Completed | Self::Partial | Self::Failed => &[],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }
}

/// A validated split payment state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitPaymentTransition {
    /// State before the transition.
    pub from: SplitPaymentStatus,
    /// State after the transition.
    pub to: SplitPaymentStatus,
}

impl SplitPaymentTransition {
    /// Validate and create a transition.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the transition is not allowed.
    pub fn new(from: SplitPaymentStatus, to: SplitPaymentStatus) -> A2AResult<Self> {
        if from.can_transition_to(to) {
            Ok(Self { from, to })
        } else {
            let allowed: Vec<&str> = from
                .allowed_transitions()
                .iter()
                .map(|s| match s {
                    SplitPaymentStatus::Pending => "pending",
                    SplitPaymentStatus::Processing => "processing",
                    SplitPaymentStatus::Completed => "completed",
                    SplitPaymentStatus::Partial => "partial",
                    SplitPaymentStatus::Failed => "failed",
                })
                .collect();
            Err(A2AError::invalid_transition(from, to, &allowed))
        }
    }
}

/// Determine the final split payment status based on execution results.
///
/// - If no recipients failed, the status is [`SplitPaymentStatus::Completed`].
/// - If all recipients failed, the status is [`SplitPaymentStatus::Failed`].
/// - Otherwise, the status is [`SplitPaymentStatus::Partial`].
#[must_use]
pub const fn determine_final_status(completed_count: usize, failed_count: usize) -> SplitPaymentStatus {
    if failed_count == 0 {
        SplitPaymentStatus::Completed
    } else if completed_count == 0 {
        SplitPaymentStatus::Failed
    } else {
        SplitPaymentStatus::Partial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_can_go_to_processing() {
        assert!(SplitPaymentStatus::Pending.can_transition_to(SplitPaymentStatus::Processing));
    }

    #[test]
    fn pending_cannot_go_to_completed() {
        assert!(!SplitPaymentStatus::Pending.can_transition_to(SplitPaymentStatus::Completed));
    }

    #[test]
    fn processing_can_go_to_completed() {
        assert!(SplitPaymentStatus::Processing.can_transition_to(SplitPaymentStatus::Completed));
    }

    #[test]
    fn processing_can_go_to_partial() {
        assert!(SplitPaymentStatus::Processing.can_transition_to(SplitPaymentStatus::Partial));
    }

    #[test]
    fn processing_can_go_to_failed() {
        assert!(SplitPaymentStatus::Processing.can_transition_to(SplitPaymentStatus::Failed));
    }

    #[test]
    fn completed_is_terminal() {
        assert!(!SplitPaymentStatus::Completed.can_transition_to(SplitPaymentStatus::Pending));
        assert!(!SplitPaymentStatus::Completed.can_transition_to(SplitPaymentStatus::Processing));
        assert!(SplitPaymentStatus::Completed.allowed_transitions().is_empty());
    }

    #[test]
    fn failed_is_terminal() {
        assert!(SplitPaymentStatus::Failed.allowed_transitions().is_empty());
    }

    #[test]
    fn partial_is_terminal() {
        assert!(SplitPaymentStatus::Partial.allowed_transitions().is_empty());
    }

    #[test]
    fn transition_new_valid() {
        let t = SplitPaymentTransition::new(
            SplitPaymentStatus::Pending,
            SplitPaymentStatus::Processing,
        )
        .unwrap();
        assert_eq!(t.from, SplitPaymentStatus::Pending);
        assert_eq!(t.to, SplitPaymentStatus::Processing);
    }

    #[test]
    fn transition_new_invalid() {
        let err = SplitPaymentTransition::new(
            SplitPaymentStatus::Pending,
            SplitPaymentStatus::Completed,
        )
        .unwrap_err();
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn determine_final_all_completed() {
        assert_eq!(
            determine_final_status(5, 0),
            SplitPaymentStatus::Completed
        );
    }

    #[test]
    fn determine_final_all_failed() {
        assert_eq!(
            determine_final_status(0, 5),
            SplitPaymentStatus::Failed
        );
    }

    #[test]
    fn determine_final_partial() {
        assert_eq!(
            determine_final_status(3, 2),
            SplitPaymentStatus::Partial
        );
    }

    #[test]
    fn status_display() {
        assert_eq!(SplitPaymentStatus::Pending.to_string(), "pending");
        assert_eq!(SplitPaymentStatus::Processing.to_string(), "processing");
        assert_eq!(SplitPaymentStatus::Completed.to_string(), "completed");
        assert_eq!(SplitPaymentStatus::Partial.to_string(), "partial");
        assert_eq!(SplitPaymentStatus::Failed.to_string(), "failed");
    }
}
