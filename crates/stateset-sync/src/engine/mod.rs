//! The [`SyncEngine`] orchestrator.
//!
//! # Layout
//!
//! The engine is one `struct` with its inherent methods spread across private
//! thematic submodules (construction, persistence, push, pull, trust, ...),
//! all re-exported here so `stateset_sync::engine::SyncEngine` and the public
//! value types keep their canonical paths.

mod commitment;
mod confirmations;
mod dead_letters;
mod lifecycle;
mod persistence;
mod pull;
mod push;
mod receipts;
mod record;
mod remote_head;
mod types;

#[cfg(test)]
mod tests;

pub use types::{DeadLetter, KernelReceipt, KernelReceiptStatus, PushConfirmation};

// Shared imports for every submodule in this directory —
// `pub(crate)` so submodules can pull the whole prelude via `use super::*`.
pub(crate) use std::collections::{BTreeMap, HashMap, HashSet};
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};

pub(crate) use chrono::{DateTime, Utc};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use uuid::Uuid;

pub(crate) use crate::attestation::{
    AttestationError, CommandAttestation, CommandInclusionProof, verify_command_inclusion_proof,
};
pub(crate) use crate::buffer::EventBuffer;
pub(crate) use crate::commitment::{
    CommitmentManifest, ManifestVerificationError, VerifiedCommitmentManifest,
    verify_commitment_manifest_against_state,
};
pub(crate) use crate::config::{SignerTrustMode, SyncConfig};
pub(crate) use crate::conflict::{ConflictResolver, ConflictStrategy, Resolution};
pub(crate) use crate::convergence::CommandConvergence;
pub(crate) use crate::error::SyncError;
pub(crate) use crate::event::{BudgetCheckpoint, KernelMetadata, PolicyDecision, SyncEvent};
pub(crate) use crate::kernel::{KernelExecutionError, KernelTransaction};
pub(crate) use crate::outbox::Outbox;
pub(crate) use crate::state::{SyncState, SyncStatus};
pub(crate) use crate::transport::{
    PullPage, PullResult, PushAcknowledgement, PushRejection, PushResult, RemoteHead, Transport,
    derive_next_cursor,
};

/// The sync engine orchestrates synchronization between local state and
/// a remote sequencer.
///
/// This is the Rust equivalent of the JS `SyncEngine` class, providing:
/// - Event recording to the outbox
/// - Push (outbox -> remote) via a [`Transport`]
/// - Pull (remote -> buffer) via a [`Transport`]
/// - Conflict resolution during pull
/// - Status reporting
///
/// # Examples
///
/// ```
/// use stateset_sync::{SyncEngine, SyncConfig, SyncEvent};
/// use serde_json::json;
///
/// let config = SyncConfig::new("agent-1", "tenant-1", "store-1");
/// let mut engine = SyncEngine::new(config).expect("valid sync config");
///
/// let seq = engine.record(SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 99})));
/// assert!(seq.is_ok());
/// assert_eq!(engine.pending_count(), 1);
/// ```
#[derive(Debug)]
pub struct SyncEngine {
    config: SyncConfig,
    state: SyncState,
    outbox: Outbox,
    buffer: EventBuffer,
    resolver: ConflictResolver,
    state_path: Option<PathBuf>,
    next_pull_cursor: Option<u64>,
    dead_letters: Vec<DeadLetter>,
    confirmations: Vec<PushConfirmation>,
    attestations: Vec<CommandAttestation>,
    manifests: Vec<VerifiedCommitmentManifest>,
    tofu_signer_pins: BTreeMap<String, String>,
    initialized: bool,
}
