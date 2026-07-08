#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![doc = include_str!("../README.md")]
//!
//! ## Modules
//!
//! - [`event`] -- The [`SyncEvent`] type representing state changes
//! - [`outbox`] -- Append-only event [`Outbox`] for recording local events
//! - [`buffer`] -- Bounded FIFO [`EventBuffer`] for pulled events
//! - [`conflict`] -- [`ConflictResolver`] with pluggable strategies
//! - [`attestation`] -- Verifiable command settlement proofs anchored in remote commitments
//! - [`commitment`] -- Signed commitment manifests for counterparty/sequencer trust
//! - [`convergence`] -- Command-level counterparty convergence derived from kernel receipts
//! - [`kernel`] -- Local transaction-kernel types for policy/budget enforcement
//! - [`transport`] -- Async [`Transport`] trait for push/pull
//! - [`http_transport`] -- Concrete HTTP transport for the StateSet sequencer
//! - [`engine`] -- The main [`SyncEngine`] orchestrator
//! - [`config`] -- [`SyncConfig`] for engine configuration
//! - [`state`] -- [`SyncState`] and [`SyncStatus`] types
//! - [`error`] -- [`SyncError`] error type

pub mod attestation;
pub mod buffer;
pub mod commitment;
pub mod config;
pub mod conflict;
pub mod convergence;
pub mod engine;
pub mod error;
pub mod event;
pub mod http_transport;
pub mod kernel;
pub mod outbox;
pub mod state;
pub mod transport;

// Re-exports for convenience
pub use attestation::{
    AttestationError, CommandAttestation, CommandInclusionProof, compute_command_settlement_leaf,
    verify_command_inclusion_proof,
};
pub use buffer::EventBuffer;
pub use commitment::{
    CommitmentManifest, ManifestVerificationError, VerifiedCommitmentManifest,
    compute_commitment_manifest_hash, sign_commitment_manifest, verify_commitment_manifest,
    verify_commitment_manifest_against_state,
};
pub use config::{CommitmentTrustPolicy, SignerTrustMode, SyncConfig};
pub use conflict::{ConflictResolver, ConflictStrategy, Resolution};
pub use convergence::{CommandConvergence, CounterpartyCommitment, CounterpartyConvergenceStatus};
pub use engine::{DeadLetter, KernelReceipt, KernelReceiptStatus, PushConfirmation, SyncEngine};
pub use error::SyncError;
pub use event::{
    BudgetCheckpoint, KernelMetadata, PolicyCheckpoint, PolicyDecision, SequenceAuthority,
    SyncEvent,
};
pub use http_transport::SequencerHttpTransport;
pub use kernel::{BudgetAuthorization, KernelExecutionError, KernelTransaction};
pub use outbox::Outbox;
pub use state::{SyncState, SyncStatus};
pub use transport::{
    NullTransport, PullPage, PullResult, PushAcknowledgement, PushRejection, PushResult,
    RemoteHead, Transport,
};
