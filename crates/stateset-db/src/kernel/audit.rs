//! Backend-neutral access to the sealed kernel receipt audit chain.
//!
//! The chain is verified identically on both backends; only the SQL differs.
//! [`KernelAuditChain`] is the object-safe seam that lets a caller holding an
//! erased [`crate::Database`] verify and checkpoint the chain without knowing
//! which backend seals it.

use crate::kernel_outbox::{KernelAuditCheckpoint, KernelAuditVerification};
use stateset_core::Result;

/// Read-only verification of the append-only receipt hash chain.
pub trait KernelAuditChain: Send + Sync + std::fmt::Debug {
    /// Recompute every link and report the first broken chain position.
    fn verify_chain(&self) -> Result<KernelAuditVerification>;

    /// Mint a portable checkpoint of the current chain head.
    fn checkpoint(&self) -> Result<KernelAuditCheckpoint>;

    /// Verify an externally retained checkpoint against the local chain.
    fn verify_checkpoint(&self, checkpoint: &KernelAuditCheckpoint) -> Result<bool>;
}
