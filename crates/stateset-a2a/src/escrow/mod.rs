//! Escrow service types and logic.
//!
//! Manages conditional fund holding with multi-condition release,
//! time-based expiry, and dispute escalation.

pub mod conditions;
pub mod state_machine;

pub use conditions::{Condition, ConditionEvaluation, ConditionType};
pub use state_machine::{EscrowStatus, EscrowTransition};
