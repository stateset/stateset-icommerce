//! Public C API surface for the sync runtime wrapper.
//!
//! This module exposes a minimal, C-ABI-safe handle over [`stateset_sdk::SyncRuntime`]
//! for local recording and JSON snapshot inspection.

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

/// Opaque handle to a sync runtime instance.
///
/// Created by [`stateset_sync_runtime_init_from_json`] or
/// [`stateset_sync_runtime_init_from_file`] and destroyed by
/// [`stateset_sync_runtime_destroy`].
pub type SyncRuntimeHandle = *mut Mutex<SyncRuntime>;

#[derive(Debug, Clone, Copy)]
struct SyncHandleState {
    runtime_ptr: usize,
    in_flight: usize,
    destroying: bool,
}

#[derive(Debug)]
struct SyncHandleRegistry {
    active: HashMap<usize, SyncHandleState>,
    next_handle_id: usize,
}

impl Default for SyncHandleRegistry {
    fn default() -> Self {
        Self { active: HashMap::new(), next_handle_id: 1 }
    }
}

static SYNC_HANDLE_REGISTRY: OnceLock<(Mutex<SyncHandleRegistry>, Condvar)> = OnceLock::new();

fn sync_handle_registry() -> &'static (Mutex<SyncHandleRegistry>, Condvar) {
    SYNC_HANDLE_REGISTRY.get_or_init(|| (Mutex::new(SyncHandleRegistry::default()), Condvar::new()))
}

fn with_sync_handle_registry<T>(f: impl FnOnce(&mut SyncHandleRegistry) -> T) -> T {
    let (mutex, _) = sync_handle_registry();
    let mut handles = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut handles)
}

const fn sync_handle_id_to_token(id: usize) -> SyncRuntimeHandle {
    id as SyncRuntimeHandle
}

struct SyncRuntimeLease {
    handle_id: usize,
    runtime_ptr: usize,
}

impl SyncRuntimeLease {
    #[allow(clippy::missing_const_for_fn)]
    #[allow(unsafe_code)]
    fn runtime(&self) -> &Mutex<SyncRuntime> {
        // SAFETY: The handle is registered and held by this lease until drop.
        unsafe { &*(self.runtime_ptr as *const Mutex<SyncRuntime>) }
    }
}

impl Drop for SyncRuntimeLease {
    fn drop(&mut self) {
        let key = self.handle_id;
        let (mutex, cvar) = sync_handle_registry();
        let mut handles = match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(state) = handles.active.get_mut(&key) {
            if state.in_flight > 0 {
                state.in_flight -= 1;
            }
            if state.destroying && state.in_flight == 0 {
                cvar.notify_all();
            }
        }
    }
}

fn begin_sync_runtime_use(runtime: SyncRuntimeHandle) -> Result<SyncRuntimeLease, FfiErrorCode> {
    if runtime.is_null() {
        set_last_error("null sync runtime handle");
        return Err(FfiErrorCode::NullPointer);
    }

    let key = runtime as usize;
    let lease_ptr = with_sync_handle_registry(|handles| match handles.active.get_mut(&key) {
        Some(state) if !state.destroying => {
            state.in_flight += 1;
            Some(state.runtime_ptr)
        }
        _ => None,
    });

    let Some(runtime_ptr) = lease_ptr else {
        set_last_error("invalid or stale sync runtime handle");
        return Err(FfiErrorCode::InvalidArgument);
    };

    Ok(SyncRuntimeLease { handle_id: key, runtime_ptr })
}

fn next_available_handle_id(handles: &mut SyncHandleRegistry) -> Result<usize, FfiErrorCode> {
    let start = handles.next_handle_id;
    loop {
        let candidate = handles.next_handle_id;
        handles.next_handle_id = handles.next_handle_id.wrapping_add(1);
        if handles.next_handle_id == 0 {
            handles.next_handle_id = 1;
        }

        if candidate != 0 && !handles.active.contains_key(&candidate) {
            return Ok(candidate);
        }
        if handles.next_handle_id == start {
            set_last_error("failed to allocate sync runtime handle id");
            return Err(FfiErrorCode::InternalError);
        }
    }
}

#[allow(unsafe_code)]
fn drop_sync_runtime_ptr(runtime_ptr: usize) {
    // SAFETY: `runtime_ptr` must have been allocated with `Box::into_raw`.
    unsafe { drop(Box::from_raw(runtime_ptr as *mut Mutex<SyncRuntime>)) };
}

fn register_new_sync_runtime_handle(
    boxed: Box<Mutex<SyncRuntime>>,
) -> Result<SyncRuntimeHandle, FfiErrorCode> {
    let runtime_ptr = Box::into_raw(boxed) as usize;
    let (mutex, _) = sync_handle_registry();
    let mut handles = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let id = match next_available_handle_id(&mut handles) {
        Ok(id) => id,
        Err(code) => {
            drop_sync_runtime_ptr(runtime_ptr);
            return Err(code);
        }
    };

    handles.active.insert(id, SyncHandleState { runtime_ptr, in_flight: 0, destroying: false });
    Ok(sync_handle_id_to_token(id))
}

fn lock_sync_runtime(runtime: &Mutex<SyncRuntime>) -> std::sync::MutexGuard<'_, SyncRuntime> {
    match runtime.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Debug)]
enum AsyncRuntimeError {
    Runtime(String),
    Sync(SyncError),
}

fn set_async_runtime_error(error: AsyncRuntimeError) -> FfiErrorCode {
    match error {
        AsyncRuntimeError::Runtime(message) => {
            set_last_error(&message);
            FfiErrorCode::InternalError
        }
        AsyncRuntimeError::Sync(error) => set_sync_error(&error),
    }
}

fn sync_runtime_thread_panic(payload: Box<dyn Any + Send>) -> FfiErrorCode {
    let message = if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "sync runtime worker thread panicked".to_string()
    };
    set_last_error(&message);
    FfiErrorCode::InternalError
}

fn run_sync_runtime_async<T, F>(lease: SyncRuntimeLease, operation: F) -> Result<T, FfiErrorCode>
where
    T: Send + 'static,
    F: FnOnce(&mut SyncRuntime, &tokio::runtime::Runtime) -> Result<T, SyncError> + Send + 'static,
{
    let join = std::thread::spawn(move || {
        let executor = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(
            |error| {
                AsyncRuntimeError::Runtime(format!(
                    "failed to build sync runtime executor: {error}"
                ))
            },
        )?;
        let runtime = lease.runtime();
        let mut runtime = lock_sync_runtime(runtime);
        operation(&mut runtime, &executor).map_err(AsyncRuntimeError::Sync)
    });

    match join.join() {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(set_async_runtime_error(error)),
        Err(payload) => Err(sync_runtime_thread_panic(payload)),
    }
}

pub(crate) fn init_sync_runtime_from_json_safe(
    config_json: &str,
) -> Result<Box<Mutex<SyncRuntime>>, FfiErrorCode> {
    let config =
        SyncRuntimeConfig::from_json_str(config_json).map_err(|err| set_sync_error(&err))?;
    let runtime = config.build().map_err(|err| set_sync_error(&err))?;
    Ok(Box::new(Mutex::new(runtime)))
}

pub(crate) fn init_sync_runtime_from_file_safe(
    config_path: &str,
) -> Result<Box<Mutex<SyncRuntime>>, FfiErrorCode> {
    let config = SyncRuntimeConfig::from_file(config_path).map_err(|err| set_sync_error(&err))?;
    let runtime = config.build().map_err(|err| set_sync_error(&err))?;
    Ok(Box::new(Mutex::new(runtime)))
}

pub(crate) fn record_sync_runtime_event_json_safe(
    runtime: &Mutex<SyncRuntime>,
    event_type: &str,
    entity_type: &str,
    entity_id: &str,
    payload_json: &str,
) -> Result<u64, FfiErrorCode> {
    let payload = serde_json::from_str(payload_json).map_err(|error| {
        set_last_error(&format!("invalid event payload json: {error}"));
        FfiErrorCode::SerializationError
    })?;
    let mut runtime = lock_sync_runtime(runtime);
    runtime
        .record(SyncEvent::new(event_type, entity_type, entity_id, payload))
        .map_err(|err| set_sync_error(&err))
}

pub(crate) fn record_sync_runtime_full_event_json_safe(
    runtime: &Mutex<SyncRuntime>,
    event_json: &str,
) -> Result<u64, FfiErrorCode> {
    let event: SyncEvent = serde_json::from_str(event_json).map_err(|error| {
        set_last_error(&format!("invalid sync event json: {error}"));
        FfiErrorCode::SerializationError
    })?;
    let mut runtime = lock_sync_runtime(runtime);
    runtime.record(event).map_err(|err| set_sync_error(&err))
}

pub(crate) fn snapshot_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    pretty: bool,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    if pretty {
        runtime.snapshot_json_pretty().map_err(|err| set_sync_error(&err))
    } else {
        runtime.snapshot_json().map_err(|err| set_sync_error(&err))
    }
}

pub(crate) fn requeue_sync_runtime_dead_letter_safe(
    runtime: &Mutex<SyncRuntime>,
    event_id: FfiUuid,
) -> Result<u64, FfiErrorCode> {
    let event_id: Uuid = event_id.into();
    let mut runtime = lock_sync_runtime(runtime);
    runtime.requeue_dead_letter(event_id).map_err(|err| set_sync_error(&err))
}

pub(crate) fn discard_sync_runtime_dead_letter_json_safe(
    runtime: &Mutex<SyncRuntime>,
    event_id: FfiUuid,
) -> Result<String, FfiErrorCode> {
    let event_id: Uuid = event_id.into();
    let mut runtime = lock_sync_runtime(runtime);
    let dead_letter = runtime.discard_dead_letter(event_id).map_err(|err| set_sync_error(&err))?;
    serde_json::to_string(&dead_letter).map_err(|error| {
        set_last_error(&format!("failed to serialize discarded dead letter: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn healthcheck_sync_runtime_safe(lease: SyncRuntimeLease) -> Result<u8, FfiErrorCode> {
    run_sync_runtime_async(lease, |runtime, executor| {
        executor.block_on(runtime.healthcheck()).map(|()| 1)
    })
}

fn refresh_sync_runtime_remote_head_json_safe(
    lease: SyncRuntimeLease,
) -> Result<String, FfiErrorCode> {
    let head = run_sync_runtime_async(lease, |runtime, executor| {
        executor.block_on(runtime.refresh_remote_head())
    })?;
    serde_json::to_string(&head).map_err(|error| {
        set_last_error(&format!("failed to serialize remote head: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn push_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let result =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.push()))?;
    serde_json::to_string(&result).map_err(|error| {
        set_last_error(&format!("failed to serialize push result: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn pull_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let result =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.pull()))?;
    serde_json::to_string(&result).map_err(|error| {
        set_last_error(&format!("failed to serialize pull result: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn full_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let (push, pull) =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.full_sync()))?;
    serde_json::to_string(&serde_json::json!({ "push": push, "pull": pull })).map_err(|error| {
        set_last_error(&format!("failed to serialize full sync result: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn confirmations_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(runtime.confirmations()).map_err(|error| {
        set_last_error(&format!("failed to serialize confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn confirmation_for_event_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    event_id: FfiUuid,
) -> Result<String, FfiErrorCode> {
    let event_id: Uuid = event_id.into();
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmation_for_event(event_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize confirmation lookup: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn drain_confirmations_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let confirmations = runtime.drain_confirmations().map_err(|err| set_sync_error(&err))?;
    serde_json::to_string(&confirmations).map_err(|error| {
        set_last_error(&format!("failed to serialize drained confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn dead_letters_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(runtime.dead_letters()).map_err(|error| {
        set_last_error(&format!("failed to serialize dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn dead_letter_for_event_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    event_id: FfiUuid,
) -> Result<String, FfiErrorCode> {
    let event_id: Uuid = event_id.into();
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.dead_letter_for_event(event_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize dead-letter lookup: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn drain_dead_letters_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let dead_letters = runtime.drain_dead_letters().map_err(|err| set_sync_error(&err))?;
    serde_json::to_string(&dead_letters).map_err(|error| {
        set_last_error(&format!("failed to serialize drained dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn buffered_events_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.snapshot().buffered_events).map_err(|error| {
        set_last_error(&format!("failed to serialize buffered events: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn drain_buffer_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let events = runtime.drain_buffer();
    serde_json::to_string(&events).map_err(|error| {
        set_last_error(&format!("failed to serialize drained buffered events: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn confirmation_for_remote_sequence_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    remote_sequence: u64,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmation_for_remote_sequence(remote_sequence)).map_err(
        |error| {
            set_last_error(&format!(
                "failed to serialize remote-sequence confirmation lookup: {error}"
            ));
            FfiErrorCode::SerializationError
        },
    )
}

fn confirmations_for_receipt_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    receipt: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmations_for_receipt(receipt)).map_err(|error| {
        set_last_error(&format!("failed to serialize receipt confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn confirmations_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmations_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize command confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn confirmations_for_entity_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    entity_type: &str,
    entity_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmations_for_entity(entity_type, entity_id)).map_err(
        |error| {
            set_last_error(&format!("failed to serialize entity confirmations: {error}"));
            FfiErrorCode::SerializationError
        },
    )
}

fn latest_confirmation_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_confirmation_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize latest command confirmation: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn latest_confirmation_for_entity_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    entity_type: &str,
    entity_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_confirmation_for_entity(entity_type, entity_id)).map_err(
        |error| {
            set_last_error(&format!("failed to serialize latest entity confirmation: {error}"));
            FfiErrorCode::SerializationError
        },
    )
}

fn dead_letters_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.dead_letters_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize command dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn dead_letters_for_entity_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    entity_type: &str,
    entity_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.dead_letters_for_entity(entity_type, entity_id)).map_err(
        |error| {
            set_last_error(&format!("failed to serialize entity dead letters: {error}"));
            FfiErrorCode::SerializationError
        },
    )
}

fn latest_dead_letter_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_dead_letter_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize latest command dead letter: {error}"));
        FfiErrorCode::SerializationError
    })
}

fn latest_dead_letter_for_entity_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    entity_type: &str,
    entity_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_dead_letter_for_entity(entity_type, entity_id)).map_err(
        |error| {
            set_last_error(&format!("failed to serialize latest entity dead letter: {error}"));
            FfiErrorCode::SerializationError
        },
    )
}

fn sync_runtime_status_safe(runtime: &Mutex<SyncRuntime>) -> stateset_sdk::sync::SyncStatus {
    let runtime = lock_sync_runtime(runtime);
    runtime.status()
}

const fn bool_to_ffi(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

/// Initialize a sync runtime from a JSON config document.
///
/// # Safety
///
/// `config_json` must be a valid, null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_init_from_json(
    config_json: *const c_char,
) -> FfiResult<SyncRuntimeHandle> {
    catch_ffi_result(|| {
        clear_last_error();

        let config_json = match unsafe { c_string_to_rust(config_json) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };

        match init_sync_runtime_from_json_safe(config_json) {
            Ok(boxed) => match register_new_sync_runtime_handle(boxed) {
                Ok(handle) => FfiResult::ok(handle),
                Err(code) => FfiResult::err(code),
            },
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Initialize a sync runtime from a JSON config file.
///
/// # Safety
///
/// `config_path` must be a valid, null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_init_from_file(
    config_path: *const c_char,
) -> FfiResult<SyncRuntimeHandle> {
    catch_ffi_result(|| {
        clear_last_error();

        let config_path = match unsafe { c_string_to_rust(config_path) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };

        match init_sync_runtime_from_file_safe(config_path) {
            Ok(boxed) => match register_new_sync_runtime_handle(boxed) {
                Ok(handle) => FfiResult::ok(handle),
                Err(code) => FfiResult::err(code),
            },
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Destroy a sync runtime, releasing all resources.
///
/// Passing `NULL` is a safe no-op.
///
/// # Safety
///
/// `runtime` must be either null or a pointer returned by one of the
/// `stateset_sync_runtime_init_*` functions.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_destroy(runtime: SyncRuntimeHandle) {
    catch_ffi_void(|| {
        clear_last_error();
        if runtime.is_null() {
            return;
        }

        let key = runtime as usize;
        let (mutex, cvar) = sync_handle_registry();
        let mut handles = match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let Some(state) = handles.active.get_mut(&key) else {
            set_last_error("invalid or stale sync runtime handle");
            return;
        };
        state.destroying = true;

        loop {
            let Some(state) = handles.active.get(&key) else {
                return;
            };
            if state.in_flight == 0 {
                break;
            }
            handles = match cvar.wait(handles) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }

        let Some(state) = handles.active.remove(&key) else {
            set_last_error("invalid or stale sync runtime handle");
            return;
        };
        drop(handles);
        drop_sync_runtime_ptr(state.runtime_ptr);
    });
}

/// Record a local event into the sync runtime outbox using a JSON payload.
///
/// # Safety
///
/// `runtime` must be a valid handle. All string arguments must be valid,
/// null-terminated UTF-8 C strings.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_record_json(
    runtime: SyncRuntimeHandle,
    event_type: *const c_char,
    entity_type: *const c_char,
    entity_id: *const c_char,
    payload_json: *const c_char,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let runtime = lease.runtime();

        let event_type = match unsafe { c_string_to_rust(event_type) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        let payload_json = match unsafe { c_string_to_rust(payload_json) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };

        match record_sync_runtime_event_json_safe(
            runtime,
            event_type,
            entity_type,
            entity_id,
            payload_json,
        ) {
            Ok(sequence) => FfiResult::ok(sequence),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Record a fully-specified sync event document into the local outbox.
///
/// Unlike [`stateset_sync_runtime_record_json`], this accepts a serialized
/// [`SyncEvent`] and preserves optional signature and VES metadata for later
/// push operations.
///
/// # Safety
///
/// `runtime` must be a valid handle and `event_json` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_record_event_json(
    runtime: SyncRuntimeHandle,
    event_json: *const c_char,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let runtime = lease.runtime();

        let event_json = match unsafe { c_string_to_rust(event_json) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };

        match record_sync_runtime_full_event_json_safe(runtime, event_json) {
            Ok(sequence) => FfiResult::ok(sequence),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Requeue a dead-lettered event back into the local outbox.
///
/// Returns the newly assigned local outbox sequence number.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_requeue_dead_letter(
    runtime: SyncRuntimeHandle,
    event_id: FfiUuid,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        match requeue_sync_runtime_dead_letter_safe(lease.runtime(), event_id) {
            Ok(sequence) => FfiResult::ok(sequence),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Discard a dead-letter entry and return the discarded entry as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_discard_dead_letter_json(
    runtime: SyncRuntimeHandle,
    event_id: FfiUuid,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match discard_sync_runtime_dead_letter_json_safe(lease.runtime(), event_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Probe the remote sequencer health endpoint.
///
/// Returns `1` on success.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_healthcheck(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u8> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        match healthcheck_sync_runtime_safe(lease) {
            Ok(ok) => FfiResult::ok(ok),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Refresh the known remote head and return it as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_refresh_remote_head_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match refresh_sync_runtime_remote_head_json_safe(lease) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Push pending local events through the configured transport and return the
/// push result as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_push_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match push_sync_runtime_json_safe(lease) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Pull canonical remote events through the configured transport and return the
/// pull result as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_pull_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match pull_sync_runtime_json_safe(lease) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Perform a full push-then-pull sync and return both results as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_full_sync_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match full_sync_runtime_json_safe(lease) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return whether the sync runtime has been initialized.
///
/// Returns `1` for true and `0` for false.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_initialized(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u8> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(bool_to_ffi(sync_runtime_status_safe(lease.runtime()).initialized))
    })
}

/// Return whether the sync runtime is currently caught up.
///
/// Returns `1` for true and `0` for false.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_caught_up(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u8> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(bool_to_ffi(sync_runtime_status_safe(lease.runtime()).caught_up))
    })
}

/// Return the current local outbox head sequence.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_local_head(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).local_head)
    })
}

/// Return the current remote head sequence.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_remote_head(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).remote_head)
    })
}

/// Return the current canonical remote cursor.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_remote_cursor(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).remote_cursor)
    })
}

/// Return the current canonical lag.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_lag(runtime: SyncRuntimeHandle) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).lag)
    })
}

/// Return the number of pending local outbox events.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_pending_count(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).pending as u64)
    })
}

/// Return the number of retained push confirmations.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmation_count(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).retained_confirmations as u64)
    })
}

/// Return the number of retained dead letters.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_dead_letter_count(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).dead_letters as u64)
    })
}

/// Return the number of buffered pulled events.
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_buffered_count(
    runtime: SyncRuntimeHandle,
) -> FfiResult<u64> {
    catch_ffi_result(|| {
        clear_last_error();
        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        FfiResult::ok(sync_runtime_status_safe(lease.runtime()).buffered_events as u64)
    })
}

/// Return the retained push confirmations as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmations_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmations_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the retained confirmation for a local event id as JSON, or `null`
/// when no confirmation has been retained for that event.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmation_for_event_json(
    runtime: SyncRuntimeHandle,
    event_id: FfiUuid,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmation_for_event_sync_runtime_json_safe(lease.runtime(), event_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Drain and return retained push confirmations as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_drain_confirmations_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match drain_confirmations_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained dead letters as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_dead_letters_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match dead_letters_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the retained dead letter for a local event id as JSON, or `null`
/// when that event is not currently dead-lettered.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_dead_letter_for_event_json(
    runtime: SyncRuntimeHandle,
    event_id: FfiUuid,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match dead_letter_for_event_sync_runtime_json_safe(lease.runtime(), event_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Drain and return retained dead letters as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_drain_dead_letters_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match drain_dead_letters_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return buffered pulled events as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_buffered_events_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match buffered_events_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Drain and return buffered pulled events as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_drain_buffer_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match drain_buffer_sync_runtime_json_safe(lease.runtime()) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the retained confirmation for a canonical remote sequence as JSON,
/// or `null` when no retained confirmation matches that sequence.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmation_for_remote_sequence_json(
    runtime: SyncRuntimeHandle,
    remote_sequence: u64,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmation_for_remote_sequence_sync_runtime_json_safe(
            lease.runtime(),
            remote_sequence,
        ) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained confirmations that share a receipt handle as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and `receipt` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmations_for_receipt_json(
    runtime: SyncRuntimeHandle,
    receipt: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let receipt = match unsafe { c_string_to_rust(receipt) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmations_for_receipt_sync_runtime_json_safe(lease.runtime(), receipt) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained confirmations associated with a command id as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and `command_id` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmations_for_command_json(
    runtime: SyncRuntimeHandle,
    command_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let command_id = match unsafe { c_string_to_rust(command_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmations_for_command_sync_runtime_json_safe(lease.runtime(), command_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained confirmations for an entity identity as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and string arguments must be valid,
/// null-terminated UTF-8 C strings.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_confirmations_for_entity_json(
    runtime: SyncRuntimeHandle,
    entity_type: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match confirmations_for_entity_sync_runtime_json_safe(
            lease.runtime(),
            entity_type,
            entity_id,
        ) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the latest retained confirmation for a command id as JSON, or `null`
/// when no retained confirmation matches that command.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and `command_id` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_latest_confirmation_for_command_json(
    runtime: SyncRuntimeHandle,
    command_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let command_id = match unsafe { c_string_to_rust(command_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match latest_confirmation_for_command_sync_runtime_json_safe(lease.runtime(), command_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the latest retained confirmation for an entity identity as JSON, or
/// `null` when no retained confirmation matches that entity.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and string arguments must be valid,
/// null-terminated UTF-8 C strings.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_latest_confirmation_for_entity_json(
    runtime: SyncRuntimeHandle,
    entity_type: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match latest_confirmation_for_entity_sync_runtime_json_safe(
            lease.runtime(),
            entity_type,
            entity_id,
        ) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained dead letters associated with a command id as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and `command_id` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_dead_letters_for_command_json(
    runtime: SyncRuntimeHandle,
    command_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let command_id = match unsafe { c_string_to_rust(command_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match dead_letters_for_command_sync_runtime_json_safe(lease.runtime(), command_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return retained dead letters for an entity identity as JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and string arguments must be valid,
/// null-terminated UTF-8 C strings.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_dead_letters_for_entity_json(
    runtime: SyncRuntimeHandle,
    entity_type: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match dead_letters_for_entity_sync_runtime_json_safe(
            lease.runtime(),
            entity_type,
            entity_id,
        ) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the latest retained dead letter for a command id as JSON, or `null`
/// when no retained dead letter matches that command.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and `command_id` must be a valid,
/// null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_latest_dead_letter_for_command_json(
    runtime: SyncRuntimeHandle,
    command_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let command_id = match unsafe { c_string_to_rust(command_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match latest_dead_letter_for_command_sync_runtime_json_safe(lease.runtime(), command_id) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Return the latest retained dead letter for an entity identity as JSON, or
/// `null` when no retained dead letter matches that entity.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle and string arguments must be valid,
/// null-terminated UTF-8 C strings.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_latest_dead_letter_for_entity_json(
    runtime: SyncRuntimeHandle,
    entity_type: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match latest_dead_letter_for_entity_sync_runtime_json_safe(
            lease.runtime(),
            entity_type,
            entity_id,
        ) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Snapshot the sync runtime as compact JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_snapshot_json(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match snapshot_sync_runtime_json_safe(lease.runtime(), false) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Snapshot the sync runtime as pretty-printed JSON.
///
/// The caller owns the returned string and must free it with
/// [`crate::strings::stateset_string_free`].
///
/// # Safety
///
/// `runtime` must be a valid handle.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_sync_runtime_snapshot_json_pretty(
    runtime: SyncRuntimeHandle,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let lease = match begin_sync_runtime_use(runtime) {
            Ok(lease) => lease,
            Err(_) => return std::ptr::null_mut(),
        };
        match snapshot_sync_runtime_json_safe(lease.runtime(), true) {
            Ok(json) => rust_to_c_string(&json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use async_trait::async_trait;
    use serde_json::{Value, json};
    use stateset_sdk::sync::{PullResult, PushRejection, PushResult, RemoteHead, Transport};
    use tempfile::NamedTempFile;

    use super::*;

    #[derive(Debug)]
    struct CapturedRequest {
        request_line: String,
        body: String,
    }

    #[derive(Debug)]
    struct StubResponse {
        status: String,
        body: Value,
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    fn content_length(headers: &str) -> usize {
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    return value.trim().parse::<usize>().ok();
                }
                None
            })
            .unwrap_or(0)
    }

    fn spawn_response_server(
        responses: Vec<StubResponse>,
    ) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = Vec::new();
                let header_end = loop {
                    let mut chunk = [0_u8; 1024];
                    let bytes_read = stream.read(&mut chunk).unwrap();
                    if bytes_read == 0 {
                        break buffer.len();
                    }
                    buffer.extend_from_slice(&chunk[..bytes_read]);
                    if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
                        break position + 4;
                    }
                };

                let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
                let expected_body_bytes = content_length(&header_text);
                while buffer.len() < header_end + expected_body_bytes {
                    let mut chunk = [0_u8; 1024];
                    let bytes_read = stream.read(&mut chunk).unwrap();
                    if bytes_read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..bytes_read]);
                }

                let body = String::from_utf8_lossy(
                    &buffer[header_end..buffer.len().min(header_end + expected_body_bytes)],
                )
                .to_string();
                let request_line = header_text.lines().next().unwrap_or_default().to_string();
                tx.send(CapturedRequest { request_line, body }).unwrap();

                let response_body = serde_json::to_string(&response.body).unwrap();
                let payload = format!(
                    "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    response.status,
                    response_body.len(),
                    response_body
                );
                stream.write_all(payload.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        });

        (format!("http://{address}"), rx)
    }

    fn runtime_config_json() -> String {
        runtime_config_json_for("https://sequencer.stateset.com")
    }

    fn runtime_config_json_for(base_url: &str) -> String {
        serde_json::to_string(&SyncRuntimeConfig::new(
            base_url,
            stateset_sdk::sync::SyncConfig::new("agent-ffi", "tenant-ffi", "store-ffi"),
        ))
        .unwrap()
    }

    fn signed_event_json(label: &str) -> String {
        serde_json::to_string(
            &SyncEvent::new(
                format!("order.{label}"),
                "order",
                format!("ORD-FFI-{label}"),
                json!({ "label": label }),
            )
            .with_signature(format!("sig-{label}"))
            .with_command_id(format!("cmd-{label}"))
            .with_source_agent_id("agent-ffi")
            .with_agent_key_id(7),
        )
        .unwrap()
    }

    #[derive(Debug, Clone, Default)]
    struct RejectingTransport;

    #[async_trait]
    impl Transport for RejectingTransport {
        async fn push_events(
            &self,
            events: &[SyncEvent],
        ) -> Result<PushResult, stateset_sdk::sync::SyncError> {
            let rejections = events
                .iter()
                .map(|event| {
                    PushRejection::new(event.id)
                        .with_code("invalid_event")
                        .with_reason("event rejected")
                        .with_retryable(false)
                })
                .collect();
            Ok(PushResult::accepted_only(0, 0).with_rejections(rejections))
        }

        async fn pull_events(
            &self,
            _since: u64,
            _limit: usize,
        ) -> Result<PullResult, stateset_sdk::sync::SyncError> {
            Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, stateset_sdk::sync::SyncError> {
            Ok(RemoteHead::new(0))
        }
    }

    #[allow(clippy::await_holding_lock)]
    async fn seed_dead_letter(handle: SyncRuntimeHandle) -> Uuid {
        let lease = begin_sync_runtime_use(handle).unwrap();
        let runtime = lease.runtime();
        let mut runtime = lock_sync_runtime(runtime);
        let event =
            SyncEvent::new("payment.failed", "payment", "PAY-FFI-1", json!({"status": "failed"}));
        let event_id = event.id;
        runtime.record(event).unwrap();
        runtime.engine_mut().push(&RejectingTransport).await.unwrap();
        event_id
    }

    #[test]
    fn sync_runtime_init_from_json_and_destroy() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let result = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert!(!result.value.is_null());

        unsafe { stateset_sync_runtime_destroy(result.value) };
    }

    #[test]
    fn sync_runtime_init_from_file_via_c_api() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), runtime_config_json()).unwrap();
        let path = CString::new(file.path().display().to_string()).unwrap();

        let result = unsafe { stateset_sync_runtime_init_from_file(path.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert!(!result.value.is_null());

        unsafe { stateset_sync_runtime_destroy(result.value) };
    }

    #[test]
    fn sync_runtime_record_and_snapshot_json_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_type = CString::new("order.created").unwrap();
        let entity_type = CString::new("order").unwrap();
        let entity_id = CString::new("ORD-FFI-1").unwrap();
        let payload = CString::new(json!({"total": 99, "currency": "USD"}).to_string()).unwrap();

        let record = unsafe {
            stateset_sync_runtime_record_json(
                init.value,
                event_type.as_ptr(),
                entity_type.as_ptr(),
                entity_id.as_ptr(),
                payload.as_ptr(),
            )
        };
        assert_eq!(record.code, FfiErrorCode::Ok);
        assert_eq!(record.value, 1);

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };

        let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
        assert_eq!(snapshot["status"]["pending"], 1);
        assert_eq!(snapshot["status"]["local_head"], 1);
        assert_eq!(snapshot["status"]["dead_letters"], 0);
        assert_eq!(snapshot["confirmations"].as_array().unwrap().len(), 0);
        assert_eq!(snapshot["dead_letters"].as_array().unwrap().len(), 0);
        assert_eq!(snapshot["buffered_events"].as_array().unwrap().len(), 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_snapshot_pretty_json_includes_newlines() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json_pretty(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };

        assert!(snapshot_text.contains('\n'));
        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_healthcheck_via_c_api() {
        let (base_url, requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({ "ok": true }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let health = unsafe { stateset_sync_runtime_healthcheck(init.value) };
        assert_eq!(health.code, FfiErrorCode::Ok);
        assert_eq!(health.value, 1);

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.starts_with("GET /health "));

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_refresh_remote_head_json_via_c_api() {
        let (base_url, requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "head_sequence": 42,
                "state_root": "root-42",
                "latest_commitment": {
                    "batch_id": "BATCH-42"
                }
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let remote_head_ptr = unsafe { stateset_sync_runtime_refresh_remote_head_json(init.value) };
        assert!(!remote_head_ptr.is_null());
        let remote_head_text =
            unsafe { CStr::from_ptr(remote_head_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(remote_head_ptr) };

        let remote_head: Value = serde_json::from_str(&remote_head_text).unwrap();
        assert_eq!(remote_head["remote_head"], 42);
        assert_eq!(remote_head["state_root"], "root-42");
        assert_eq!(remote_head["last_commitment_id"], "BATCH-42");

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.contains("GET /api/v1/head?"));

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_record_event_and_push_json_via_c_api() {
        let (base_url, requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "batchId": "B-FFI-1",
                "eventsAccepted": 1,
                "eventsRejected": 0,
                "sequenceStart": 11,
                "sequenceEnd": 11,
                "headSequence": 11
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_json = signed_event_json("created");
        let event: SyncEvent = serde_json::from_str(&event_json).unwrap();
        let event_json = CString::new(event_json).unwrap();
        let record =
            unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
        assert_eq!(record.code, FfiErrorCode::Ok);
        assert_eq!(record.value, 1);

        let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
        assert!(!push_ptr.is_null());
        let push_text = unsafe { CStr::from_ptr(push_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(push_ptr) };

        let push: Value = serde_json::from_str(&push_text).unwrap();
        assert_eq!(push["accepted"], 1);
        assert_eq!(push["remote_head"], 11);
        assert_eq!(push["acknowledgements"][0]["event_id"], json!(event.id));
        assert_eq!(push["acknowledgements"][0]["remote_sequence"], 11);

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.starts_with("POST /api/v1/ves/events/ingest "));
        let body: Value = serde_json::from_str(&captured.body).unwrap();
        assert_eq!(body["events"][0]["event_id"], json!(event.id));
        assert_eq!(body["events"][0]["agent_signature"], json!("sig-created"));
        assert_eq!(body["events"][0]["command_id"], json!("cmd-created"));

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
        let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
        assert_eq!(snapshot["status"]["pending"], 0);
        assert_eq!(snapshot["confirmations"].as_array().unwrap().len(), 1);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_pull_json_via_c_api() {
        let event_id = Uuid::new_v4();
        let (base_url, requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-PULL-1",
                            "event_type": "order.shipped",
                            "payload": { "status": "shipped" },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 7
                        },
                        "sequenced_at": "2024-03-01T00:00:01Z"
                    }
                ],
                "head_sequence": 7,
                "has_more": false
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
        assert!(!pull_ptr.is_null());
        let pull_text = unsafe { CStr::from_ptr(pull_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(pull_ptr) };

        let pull: Value = serde_json::from_str(&pull_text).unwrap();
        assert_eq!(pull["remote_head"], 7);
        assert_eq!(pull["events"].as_array().unwrap().len(), 1);
        assert_eq!(pull["events"][0]["id"], json!(event_id));

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.contains("GET /api/v1/events?"));
        assert!(captured.request_line.contains("tenant_id=tenant-ffi"));
        assert!(captured.request_line.contains("store_id=store-ffi"));

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
        let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
        assert_eq!(snapshot["buffered_events"].as_array().unwrap().len(), 1);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_full_sync_json_via_c_api() {
        let event_id = Uuid::new_v4();
        let (base_url, requests) = spawn_response_server(vec![
            StubResponse {
                status: "200 OK".to_string(),
                body: json!({
                    "batchId": "B-FFI-2",
                    "eventsAccepted": 1,
                    "eventsRejected": 0,
                    "sequenceStart": 12,
                    "sequenceEnd": 12,
                    "headSequence": 12
                }),
            },
            StubResponse {
                status: "200 OK".to_string(),
                body: json!({
                    "events": [
                        {
                            "envelope": {
                                "event_id": event_id,
                                "entity_type": "order",
                                "entity_id": "ORD-FULL-1",
                                "event_type": "order.confirmed",
                                "payload": { "status": "confirmed" },
                                "created_at": "2024-03-01T00:00:00Z",
                                "sequence_number": 13
                            },
                            "sequenced_at": "2024-03-01T00:00:02Z"
                        }
                    ],
                    "head_sequence": 13,
                    "has_more": false
                }),
            },
        ]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_json = CString::new(signed_event_json("confirmed")).unwrap();
        let record =
            unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
        assert_eq!(record.code, FfiErrorCode::Ok);

        let full_sync_ptr = unsafe { stateset_sync_runtime_full_sync_json(init.value) };
        assert!(!full_sync_ptr.is_null());
        let full_sync_text = unsafe { CStr::from_ptr(full_sync_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(full_sync_ptr) };

        let full_sync: Value = serde_json::from_str(&full_sync_text).unwrap();
        assert_eq!(full_sync["push"]["accepted"], 1);
        assert_eq!(full_sync["pull"]["events"].as_array().unwrap().len(), 1);
        assert_eq!(full_sync["pull"]["events"][0]["id"], json!(event_id));

        let first = requests.recv().unwrap();
        assert!(first.request_line.starts_with("POST /api/v1/ves/events/ingest "));
        let second = requests.recv().unwrap();
        assert!(second.request_line.contains("GET /api/v1/events?"));

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_confirmations_json_and_drain_via_c_api() {
        let (base_url, _requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "batchId": "B-FFI-3",
                "eventsAccepted": 1,
                "eventsRejected": 0,
                "sequenceStart": 21,
                "sequenceEnd": 21,
                "headSequence": 21
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_json = signed_event_json("confirmations");
        let event: SyncEvent = serde_json::from_str(&event_json).unwrap();
        let event_json = CString::new(event_json).unwrap();
        let record =
            unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
        assert_eq!(record.code, FfiErrorCode::Ok);
        let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
        assert!(!push_ptr.is_null());
        unsafe { crate::strings::stateset_string_free(push_ptr) };

        let confirmations_ptr = unsafe { stateset_sync_runtime_confirmations_json(init.value) };
        assert!(!confirmations_ptr.is_null());
        let confirmations_text =
            unsafe { CStr::from_ptr(confirmations_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(confirmations_ptr) };
        let confirmations: Value = serde_json::from_str(&confirmations_text).unwrap();
        assert_eq!(confirmations.as_array().unwrap().len(), 1);
        assert_eq!(confirmations[0]["event_id"], json!(event.id));

        let confirmation_ptr = unsafe {
            stateset_sync_runtime_confirmation_for_event_json(init.value, FfiUuid::from(event.id))
        };
        assert!(!confirmation_ptr.is_null());
        let confirmation_text =
            unsafe { CStr::from_ptr(confirmation_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(confirmation_ptr) };
        let confirmation: Value = serde_json::from_str(&confirmation_text).unwrap();
        assert_eq!(confirmation["remote_sequence"], 21);

        let drained_ptr = unsafe { stateset_sync_runtime_drain_confirmations_json(init.value) };
        assert!(!drained_ptr.is_null());
        let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(drained_ptr) };
        let drained: Value = serde_json::from_str(&drained_text).unwrap();
        assert_eq!(drained.as_array().unwrap().len(), 1);

        let empty_ptr = unsafe { stateset_sync_runtime_confirmations_json(init.value) };
        assert!(!empty_ptr.is_null());
        let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(empty_ptr) };
        let empty: Value = serde_json::from_str(&empty_text).unwrap();
        assert_eq!(empty.as_array().unwrap().len(), 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[tokio::test]
    async fn sync_runtime_dead_letters_json_and_drain_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_id = seed_dead_letter(init.value).await;

        let dead_letters_ptr = unsafe { stateset_sync_runtime_dead_letters_json(init.value) };
        assert!(!dead_letters_ptr.is_null());
        let dead_letters_text =
            unsafe { CStr::from_ptr(dead_letters_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(dead_letters_ptr) };
        let dead_letters: Value = serde_json::from_str(&dead_letters_text).unwrap();
        assert_eq!(dead_letters.as_array().unwrap().len(), 1);
        assert_eq!(dead_letters[0]["event"]["id"], json!(event_id));

        let dead_letter_ptr = unsafe {
            stateset_sync_runtime_dead_letter_for_event_json(init.value, FfiUuid::from(event_id))
        };
        assert!(!dead_letter_ptr.is_null());
        let dead_letter_text =
            unsafe { CStr::from_ptr(dead_letter_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(dead_letter_ptr) };
        let dead_letter: Value = serde_json::from_str(&dead_letter_text).unwrap();
        assert_eq!(dead_letter["event"]["id"], json!(event_id));

        let drained_ptr = unsafe { stateset_sync_runtime_drain_dead_letters_json(init.value) };
        assert!(!drained_ptr.is_null());
        let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(drained_ptr) };
        let drained: Value = serde_json::from_str(&drained_text).unwrap();
        assert_eq!(drained.as_array().unwrap().len(), 1);

        let empty_ptr = unsafe { stateset_sync_runtime_dead_letters_json(init.value) };
        assert!(!empty_ptr.is_null());
        let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(empty_ptr) };
        let empty: Value = serde_json::from_str(&empty_text).unwrap();
        assert_eq!(empty.as_array().unwrap().len(), 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_buffered_events_json_and_drain_via_c_api() {
        let event_id = Uuid::new_v4();
        let (base_url, _requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-BUFFER-1",
                            "event_type": "order.buffered",
                            "payload": { "status": "buffered" },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 31
                        },
                        "sequenced_at": "2024-03-01T00:00:01Z"
                    }
                ],
                "head_sequence": 31,
                "has_more": false
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
        assert!(!pull_ptr.is_null());
        unsafe { crate::strings::stateset_string_free(pull_ptr) };

        let buffered_ptr = unsafe { stateset_sync_runtime_buffered_events_json(init.value) };
        assert!(!buffered_ptr.is_null());
        let buffered_text = unsafe { CStr::from_ptr(buffered_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(buffered_ptr) };
        let buffered: Value = serde_json::from_str(&buffered_text).unwrap();
        assert_eq!(buffered.as_array().unwrap().len(), 1);
        assert_eq!(buffered[0]["id"], json!(event_id));

        let drained_ptr = unsafe { stateset_sync_runtime_drain_buffer_json(init.value) };
        assert!(!drained_ptr.is_null());
        let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(drained_ptr) };
        let drained: Value = serde_json::from_str(&drained_text).unwrap();
        assert_eq!(drained.as_array().unwrap().len(), 1);
        assert_eq!(drained[0]["id"], json!(event_id));

        let empty_ptr = unsafe { stateset_sync_runtime_buffered_events_json(init.value) };
        assert!(!empty_ptr.is_null());
        let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(empty_ptr) };
        let empty: Value = serde_json::from_str(&empty_text).unwrap();
        assert_eq!(empty.as_array().unwrap().len(), 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_scalar_status_defaults_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        assert_eq!(unsafe { stateset_sync_runtime_initialized(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_local_head(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_remote_cursor(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_confirmation_count(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_dead_letter_count(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_buffered_count(init.value) }.value, 0);

        let event_type = CString::new("order.created").unwrap();
        let entity_type = CString::new("order").unwrap();
        let entity_id = CString::new("ORD-SCALAR-1").unwrap();
        let payload = CString::new(json!({"total": 1}).to_string()).unwrap();
        let record = unsafe {
            stateset_sync_runtime_record_json(
                init.value,
                event_type.as_ptr(),
                entity_type.as_ptr(),
                entity_id.as_ptr(),
                payload.as_ptr(),
            )
        };
        assert_eq!(record.code, FfiErrorCode::Ok);

        assert_eq!(unsafe { stateset_sync_runtime_local_head(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_scalar_status_after_push_via_c_api() {
        let (base_url, _requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "batchId": "B-FFI-5",
                "eventsAccepted": 1,
                "eventsRejected": 0,
                "sequenceStart": 51,
                "sequenceEnd": 51,
                "headSequence": 51
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_json = CString::new(signed_event_json("scalar-push")).unwrap();
        let record =
            unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
        assert_eq!(record.code, FfiErrorCode::Ok);
        let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
        assert!(!push_ptr.is_null());
        unsafe { crate::strings::stateset_string_free(push_ptr) };

        assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 51);
        assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 51);
        assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_confirmation_count(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_scalar_status_after_pull_via_c_api() {
        let event_id = Uuid::new_v4();
        let (base_url, _requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-SCALAR-PULL-1",
                            "event_type": "order.pulled",
                            "payload": { "status": "pulled" },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 31
                        },
                        "sequenced_at": "2024-03-01T00:00:01Z"
                    }
                ],
                "head_sequence": 31,
                "has_more": false
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
        assert!(!pull_ptr.is_null());
        unsafe { crate::strings::stateset_string_free(pull_ptr) };

        assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 31);
        assert_eq!(unsafe { stateset_sync_runtime_remote_cursor(init.value) }.value, 31);
        assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 0);
        assert_eq!(unsafe { stateset_sync_runtime_buffered_count(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 1);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[tokio::test]
    async fn sync_runtime_scalar_dead_letter_count_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let _event_id = seed_dead_letter(init.value).await;
        assert_eq!(unsafe { stateset_sync_runtime_dead_letter_count(init.value) }.value, 1);
        assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_confirmation_scoped_queries_via_c_api() {
        let (base_url, _requests) = spawn_response_server(vec![StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "batchId": "B-FFI-4",
                "eventsAccepted": 2,
                "eventsRejected": 0,
                "sequenceStart": 41,
                "sequenceEnd": 42,
                "headSequence": 42,
                "receipt": {
                    "batchId": "B-FFI-4",
                    "receiptHash": "receipt-scope"
                }
            }),
        }]);
        let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let first = SyncEvent::new("order.created", "order", "ORD-SCOPE-1", json!({ "step": 1 }))
            .with_signature("sig-scope-1")
            .with_command_id("cmd-scope")
            .with_source_agent_id("agent-ffi")
            .with_agent_key_id(7);
        let second =
            SyncEvent::new("order.confirmed", "order", "ORD-SCOPE-1", json!({ "step": 2 }))
                .with_signature("sig-scope-2")
                .with_command_id("cmd-scope")
                .with_source_agent_id("agent-ffi")
                .with_agent_key_id(7);

        for event in [&first, &second] {
            let event_json = CString::new(serde_json::to_string(event).unwrap()).unwrap();
            let record =
                unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
            assert_eq!(record.code, FfiErrorCode::Ok);
        }

        let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
        assert!(!push_ptr.is_null());
        unsafe { crate::strings::stateset_string_free(push_ptr) };

        let by_remote_ptr =
            unsafe { stateset_sync_runtime_confirmation_for_remote_sequence_json(init.value, 42) };
        assert!(!by_remote_ptr.is_null());
        let by_remote_text = unsafe { CStr::from_ptr(by_remote_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_remote_ptr) };
        let by_remote: Value = serde_json::from_str(&by_remote_text).unwrap();
        assert_eq!(by_remote["event_id"], json!(second.id));

        let receipt = CString::new("receipt-scope").unwrap();
        let by_receipt_ptr = unsafe {
            stateset_sync_runtime_confirmations_for_receipt_json(init.value, receipt.as_ptr())
        };
        assert!(!by_receipt_ptr.is_null());
        let by_receipt_text =
            unsafe { CStr::from_ptr(by_receipt_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_receipt_ptr) };
        let by_receipt: Value = serde_json::from_str(&by_receipt_text).unwrap();
        assert_eq!(by_receipt.as_array().unwrap().len(), 2);

        let command_id = CString::new("cmd-scope").unwrap();
        let by_command_ptr = unsafe {
            stateset_sync_runtime_confirmations_for_command_json(init.value, command_id.as_ptr())
        };
        assert!(!by_command_ptr.is_null());
        let by_command_text =
            unsafe { CStr::from_ptr(by_command_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_command_ptr) };
        let by_command: Value = serde_json::from_str(&by_command_text).unwrap();
        assert_eq!(by_command.as_array().unwrap().len(), 2);

        let entity_type = CString::new("order").unwrap();
        let entity_id = CString::new("ORD-SCOPE-1").unwrap();
        let by_entity_ptr = unsafe {
            stateset_sync_runtime_confirmations_for_entity_json(
                init.value,
                entity_type.as_ptr(),
                entity_id.as_ptr(),
            )
        };
        assert!(!by_entity_ptr.is_null());
        let by_entity_text = unsafe { CStr::from_ptr(by_entity_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_entity_ptr) };
        let by_entity: Value = serde_json::from_str(&by_entity_text).unwrap();
        assert_eq!(by_entity.as_array().unwrap().len(), 2);

        let latest_command_ptr = unsafe {
            stateset_sync_runtime_latest_confirmation_for_command_json(
                init.value,
                command_id.as_ptr(),
            )
        };
        assert!(!latest_command_ptr.is_null());
        let latest_command_text =
            unsafe { CStr::from_ptr(latest_command_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(latest_command_ptr) };
        let latest_command: Value = serde_json::from_str(&latest_command_text).unwrap();
        assert_eq!(latest_command["event_id"], json!(second.id));
        assert_eq!(latest_command["remote_sequence"], 42);

        let latest_entity_ptr = unsafe {
            stateset_sync_runtime_latest_confirmation_for_entity_json(
                init.value,
                entity_type.as_ptr(),
                entity_id.as_ptr(),
            )
        };
        assert!(!latest_entity_ptr.is_null());
        let latest_entity_text =
            unsafe { CStr::from_ptr(latest_entity_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(latest_entity_ptr) };
        let latest_entity: Value = serde_json::from_str(&latest_entity_text).unwrap();
        assert_eq!(latest_entity["event_id"], json!(second.id));
        assert_eq!(latest_entity["remote_sequence"], 42);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn sync_runtime_dead_letter_scoped_queries_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let lease = begin_sync_runtime_use(init.value).unwrap();
        let runtime = lease.runtime();
        let mut runtime = lock_sync_runtime(runtime);
        let first =
            SyncEvent::new("payment.failed", "payment", "PAY-SCOPE-1", json!({ "step": 1 }))
                .with_command_id("cmd-dead-scope");
        let first_id = first.id;
        let second =
            SyncEvent::new("payment.retry_failed", "payment", "PAY-SCOPE-1", json!({ "step": 2 }))
                .with_command_id("cmd-dead-scope");
        let second_id = second.id;
        runtime.record(first).unwrap();
        runtime.record(second).unwrap();
        runtime.engine_mut().push(&RejectingTransport).await.unwrap();
        drop(runtime);
        drop(lease);

        let command_id = CString::new("cmd-dead-scope").unwrap();
        let by_command_ptr = unsafe {
            stateset_sync_runtime_dead_letters_for_command_json(init.value, command_id.as_ptr())
        };
        assert!(!by_command_ptr.is_null());
        let by_command_text =
            unsafe { CStr::from_ptr(by_command_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_command_ptr) };
        let by_command: Value = serde_json::from_str(&by_command_text).unwrap();
        assert_eq!(by_command.as_array().unwrap().len(), 2);

        let entity_type = CString::new("payment").unwrap();
        let entity_id = CString::new("PAY-SCOPE-1").unwrap();
        let by_entity_ptr = unsafe {
            stateset_sync_runtime_dead_letters_for_entity_json(
                init.value,
                entity_type.as_ptr(),
                entity_id.as_ptr(),
            )
        };
        assert!(!by_entity_ptr.is_null());
        let by_entity_text = unsafe { CStr::from_ptr(by_entity_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_entity_ptr) };
        let by_entity: Value = serde_json::from_str(&by_entity_text).unwrap();
        assert_eq!(by_entity.as_array().unwrap().len(), 2);

        let latest_command_ptr = unsafe {
            stateset_sync_runtime_latest_dead_letter_for_command_json(
                init.value,
                command_id.as_ptr(),
            )
        };
        assert!(!latest_command_ptr.is_null());
        let latest_command_text =
            unsafe { CStr::from_ptr(latest_command_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(latest_command_ptr) };
        let latest_command: Value = serde_json::from_str(&latest_command_text).unwrap();
        assert_eq!(latest_command["event"]["id"], json!(second_id));

        let latest_entity_ptr = unsafe {
            stateset_sync_runtime_latest_dead_letter_for_entity_json(
                init.value,
                entity_type.as_ptr(),
                entity_id.as_ptr(),
            )
        };
        assert!(!latest_entity_ptr.is_null());
        let latest_entity_text =
            unsafe { CStr::from_ptr(latest_entity_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(latest_entity_ptr) };
        let latest_entity: Value = serde_json::from_str(&latest_entity_text).unwrap();
        assert_eq!(latest_entity["event"]["id"], json!(second_id));

        let by_event_ptr = unsafe {
            stateset_sync_runtime_dead_letter_for_event_json(init.value, FfiUuid::from(first_id))
        };
        assert!(!by_event_ptr.is_null());
        let by_event_text = unsafe { CStr::from_ptr(by_event_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(by_event_ptr) };
        let by_event: Value = serde_json::from_str(&by_event_text).unwrap();
        assert_eq!(by_event["event"]["id"], json!(first_id));

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[tokio::test]
    async fn sync_runtime_requeue_dead_letter_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_id = seed_dead_letter(init.value).await;

        let requeue = unsafe {
            stateset_sync_runtime_requeue_dead_letter(init.value, FfiUuid::from(event_id))
        };
        assert_eq!(requeue.code, FfiErrorCode::Ok);
        assert_eq!(requeue.value, 2);

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
        let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
        assert_eq!(snapshot["status"]["pending"], 1);
        assert_eq!(snapshot["status"]["dead_letters"], 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[tokio::test]
    async fn sync_runtime_discard_dead_letter_json_via_c_api() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let event_id = seed_dead_letter(init.value).await;

        let discarded_ptr = unsafe {
            stateset_sync_runtime_discard_dead_letter_json(init.value, FfiUuid::from(event_id))
        };
        assert!(!discarded_ptr.is_null());
        let discarded_text = unsafe { CStr::from_ptr(discarded_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(discarded_ptr) };

        let discarded: Value = serde_json::from_str(&discarded_text).unwrap();
        assert_eq!(discarded["event"]["entity_id"], "PAY-FFI-1");
        assert_eq!(discarded["rejection"]["code"], "invalid_event");

        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
        assert!(!snapshot_ptr.is_null());
        let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
        unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
        let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
        assert_eq!(snapshot["status"]["pending"], 0);
        assert_eq!(snapshot["status"]["dead_letters"], 0);

        unsafe { stateset_sync_runtime_destroy(init.value) };
    }

    #[test]
    fn sync_runtime_record_null_handle_is_rejected() {
        let event_type = CString::new("order.created").unwrap();
        let entity_type = CString::new("order").unwrap();
        let entity_id = CString::new("ORD-NULL").unwrap();
        let payload = CString::new("{}").unwrap();

        let result = unsafe {
            stateset_sync_runtime_record_json(
                std::ptr::null_mut(),
                event_type.as_ptr(),
                entity_type.as_ptr(),
                entity_id.as_ptr(),
                payload.as_ptr(),
            )
        };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn sync_runtime_snapshot_after_destroy_is_rejected() {
        let config_json = CString::new(runtime_config_json()).unwrap();
        let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);
        let handle = init.value;

        unsafe { stateset_sync_runtime_destroy(handle) };
        let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(handle) };
        assert!(snapshot_ptr.is_null());
        let err = crate::error::last_error_as_str();
        assert!(
            err.as_deref().is_some_and(|msg| msg.contains("invalid or stale sync runtime handle"))
        );
    }
}
