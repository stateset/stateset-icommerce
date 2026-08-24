//! Local outbox recording and JSON snapshot inspection.

use super::*;

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

        // SAFETY: the caller guarantees `event_type` is a valid, null-terminated C string
        // (see `# Safety`).
        let event_type = match unsafe { c_string_to_rust(event_type) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        // SAFETY: the caller guarantees `entity_type` is a valid, null-terminated C string
        // (see `# Safety`).
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        // SAFETY: the caller guarantees `entity_id` is a valid, null-terminated C string
        // (see `# Safety`).
        let entity_id = match unsafe { c_string_to_rust(entity_id) } {
            Ok(value) => value,
            Err(code) => return FfiResult::err(code),
        };
        // SAFETY: the caller guarantees `payload_json` is a valid, null-terminated C string
        // (see `# Safety`).
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

        // SAFETY: the caller guarantees `event_json` is a valid, null-terminated C string
        // (see `# Safety`).
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
