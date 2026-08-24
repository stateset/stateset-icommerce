//! Public C API surface for the sync runtime wrapper.
//!
//! This module exposes a minimal, C-ABI-safe handle over [`stateset_sdk::SyncRuntime`]
//! for local recording and JSON snapshot inspection.
//!
//! # Layout
//!
//! The `extern "C"` entry points live in private thematic submodules and are
//! re-exported here, so `stateset_ffi::sync_api::stateset_sync_runtime_*` remain
//! the canonical paths. Exported symbol names are unaffected by module layout.

use std::any::Any;
use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::{Condvar, Mutex, OnceLock};

use stateset_sdk::sync::{SyncError, SyncEvent};
use stateset_sdk::{SyncRuntime, SyncRuntimeConfig};
use uuid::Uuid;

use crate::types::FfiUuid;

use crate::error::{
    FfiErrorCode, FfiResult, catch_ffi_mut_ptr, catch_ffi_result, catch_ffi_void, clear_last_error,
    set_last_error, set_sync_error,
};
use crate::strings::{c_string_to_rust, rust_to_c_string};

mod buffer;
mod confirmations;
mod dead_letters;
mod executor;
mod handles;
mod lifecycle;
mod record;
mod status;
mod transport;

#[cfg(test)]
mod tests;

/// Opaque handle to a sync runtime instance.
///
/// Created by [`stateset_sync_runtime_init_from_json`] or
/// [`stateset_sync_runtime_init_from_file`] and destroyed by
/// [`stateset_sync_runtime_destroy`].
pub type SyncRuntimeHandle = *mut Mutex<SyncRuntime>;

pub use buffer::{
    stateset_sync_runtime_buffered_events_json, stateset_sync_runtime_drain_buffer_json,
};
pub use confirmations::{
    stateset_sync_runtime_confirmation_for_event_json,
    stateset_sync_runtime_confirmation_for_remote_sequence_json,
    stateset_sync_runtime_confirmations_for_command_json,
    stateset_sync_runtime_confirmations_for_entity_json,
    stateset_sync_runtime_confirmations_for_receipt_json, stateset_sync_runtime_confirmations_json,
    stateset_sync_runtime_drain_confirmations_json,
    stateset_sync_runtime_latest_confirmation_for_command_json,
    stateset_sync_runtime_latest_confirmation_for_entity_json,
};
pub use dead_letters::{
    stateset_sync_runtime_dead_letter_for_event_json,
    stateset_sync_runtime_dead_letters_for_command_json,
    stateset_sync_runtime_dead_letters_for_entity_json, stateset_sync_runtime_dead_letters_json,
    stateset_sync_runtime_discard_dead_letter_json, stateset_sync_runtime_drain_dead_letters_json,
    stateset_sync_runtime_latest_dead_letter_for_command_json,
    stateset_sync_runtime_latest_dead_letter_for_entity_json,
    stateset_sync_runtime_requeue_dead_letter,
};
pub use lifecycle::{
    stateset_sync_runtime_destroy, stateset_sync_runtime_init_from_file,
    stateset_sync_runtime_init_from_json,
};
pub use record::{
    stateset_sync_runtime_record_event_json, stateset_sync_runtime_record_json,
    stateset_sync_runtime_snapshot_json, stateset_sync_runtime_snapshot_json_pretty,
};
pub use status::{
    stateset_sync_runtime_buffered_count, stateset_sync_runtime_caught_up,
    stateset_sync_runtime_confirmation_count, stateset_sync_runtime_dead_letter_count,
    stateset_sync_runtime_initialized, stateset_sync_runtime_lag, stateset_sync_runtime_local_head,
    stateset_sync_runtime_pending_count, stateset_sync_runtime_remote_cursor,
    stateset_sync_runtime_remote_head,
};
pub use transport::{
    stateset_sync_runtime_full_sync_json, stateset_sync_runtime_healthcheck,
    stateset_sync_runtime_pull_json, stateset_sync_runtime_push_json,
    stateset_sync_runtime_refresh_remote_head_json,
};

/// Crate-internal helpers shared by the submodules (and by unit tests).
pub(crate) mod prelude {
    pub(crate) use super::executor::run_sync_runtime_async;
    pub(crate) use super::handles::{
        SyncRuntimeLease, begin_sync_runtime_use, drop_sync_runtime_ptr, lock_sync_runtime,
        register_new_sync_runtime_handle, sync_handle_registry,
    };
}

use prelude::*;
