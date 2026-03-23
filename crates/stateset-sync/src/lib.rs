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
//! - [`transport`] -- Async [`Transport`] trait for push/pull
//! - [`http_transport`] -- Concrete HTTP transport for the StateSet sequencer
//! - [`engine`] -- The main [`SyncEngine`] orchestrator
//! - [`config`] -- [`SyncConfig`] for engine configuration
//! - [`state`] -- [`SyncState`] and [`SyncStatus`] types
//! - [`error`] -- [`SyncError`] error type

pub mod buffer;
pub mod config;
pub mod conflict;
pub mod engine;
pub mod error;
pub mod event;
pub mod http_transport;
pub mod outbox;
pub mod state;
pub mod transport;

// Re-exports for convenience
pub use buffer::EventBuffer;
pub use config::SyncConfig;
pub use conflict::{ConflictResolver, ConflictStrategy, Resolution};
pub use engine::{DeadLetter, PushConfirmation, SyncEngine};
pub use error::SyncError;
pub use event::{SequenceAuthority, SyncEvent};
pub use http_transport::SequencerHttpTransport;
pub use outbox::Outbox;
pub use state::{SyncState, SyncStatus};
pub use transport::{
    NullTransport, PullPage, PullResult, PushAcknowledgement, PushRejection, PushResult,
    RemoteHead, Transport,
};
