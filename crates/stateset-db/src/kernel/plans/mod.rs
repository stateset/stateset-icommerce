//! Pure per-operation plans: validation against a backend-provided snapshot
//! that yields either a typed rejection or the effects to apply.

pub mod catalog;
pub mod escrow;
pub mod finance;
pub mod inventory;
pub mod orders;
pub mod payments;
pub mod returns;

use crate::kernel::envelope::GuardRejection;

/// Verdict of evaluating a command against its aggregate snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOutcome<E> {
    /// Seal a rejection receipt and commit no aggregate change.
    Reject {
        /// Typed rejection.
        rejection: GuardRejection,
        /// Aggregate version observed, when one was loaded.
        version_before: Option<i32>,
        /// Aggregate id observed, when one was loaded.
        aggregate_id: Option<String>,
    },
    /// Guards passed; apply (or preview) these effects.
    Proceed(E),
}

impl<E> PlanOutcome<E> {
    pub(crate) const fn reject(rejection: GuardRejection) -> Self {
        Self::Reject { rejection, version_before: None, aggregate_id: None }
    }
}
