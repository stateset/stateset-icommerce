//! Dispute resolution for A2A commerce.
//!
//! Provides a state machine for dispute lifecycle management, evidence
//! submission with SHA-256 content hashing, and deadline enforcement.
//!
//! ## Dispute Flow
//!
//! ```text
//! filed → evidence_period → under_review → resolved
//!                                        → escalated
//! ```

pub mod evidence;
pub mod state_machine;
pub mod types;

pub use evidence::{Evidence, hash_evidence};
pub use state_machine::{DisputeStatus, DisputeTransition};
pub use types::{
    DisputeCategory, DisputeRecord, EscrowAction, EvidenceType, ResolutionOutcome, ResolutionType,
    resolution_to_escrow_action,
};
