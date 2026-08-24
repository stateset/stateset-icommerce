//! Retained push-confirmation queries and drains.

use super::*;

pub(super) fn confirmations_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(runtime.confirmations()).map_err(|error| {
        set_last_error(&format!("failed to serialize confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn confirmation_for_event_sync_runtime_json_safe(
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

pub(super) fn drain_confirmations_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let confirmations = runtime.drain_confirmations().map_err(|err| set_sync_error(&err))?;
    serde_json::to_string(&confirmations).map_err(|error| {
        set_last_error(&format!("failed to serialize drained confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn confirmation_for_remote_sequence_sync_runtime_json_safe(
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

pub(super) fn confirmations_for_receipt_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    receipt: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmations_for_receipt(receipt)).map_err(|error| {
        set_last_error(&format!("failed to serialize receipt confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn confirmations_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.confirmations_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize command confirmations: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn confirmations_for_entity_sync_runtime_json_safe(
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

pub(super) fn latest_confirmation_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_confirmation_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize latest command confirmation: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn latest_confirmation_for_entity_sync_runtime_json_safe(
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
        // SAFETY: the caller guarantees `receipt` is a valid, null-terminated C string
        // (see `# Safety`).
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
        // SAFETY: the caller guarantees `command_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
        // SAFETY: the caller guarantees `entity_type` is a valid, null-terminated C string
        // (see `# Safety`).
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        // SAFETY: the caller guarantees `entity_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
        // SAFETY: the caller guarantees `command_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
        // SAFETY: the caller guarantees `entity_type` is a valid, null-terminated C string
        // (see `# Safety`).
        let entity_type = match unsafe { c_string_to_rust(entity_type) } {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        // SAFETY: the caller guarantees `entity_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
