#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]
//!
//! ## Modules
//!
//! - [`event`] -- The [`SyncEvent`] type representing state changes
//! - [`outbox`] -- Append-only event [`Outbox`] for recording local events
//! - [`buffer`] -- Bounded FIFO [`EventBuffer`] for pulled events
//! - [`conflict`] -- [`ConflictResolver`] with pluggable strategies
//! - [`transport`] -- Async [`Transport`] trait for push/pull
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
pub mod outbox;
pub mod state;
pub mod transport;

// Re-exports for convenience
pub use buffer::EventBuffer;
pub use config::SyncConfig;
pub use conflict::{ConflictResolver, ConflictStrategy, Resolution};
pub use engine::SyncEngine;
pub use error::SyncError;
pub use event::SyncEvent;
pub use outbox::Outbox;
pub use state::{SyncState, SyncStatus};
pub use transport::{NullTransport, PullPage, PullResult, PushResult, Transport};
