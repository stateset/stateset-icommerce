//! Dead-letter queries, drains, requeue, and discard.

use super::*;

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

pub(super) fn dead_letters_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(runtime.dead_letters()).map_err(|error| {
        set_last_error(&format!("failed to serialize dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn dead_letter_for_event_sync_runtime_json_safe(
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

pub(super) fn drain_dead_letters_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let dead_letters = runtime.drain_dead_letters().map_err(|err| set_sync_error(&err))?;
    serde_json::to_string(&dead_letters).map_err(|error| {
        set_last_error(&format!("failed to serialize drained dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn dead_letters_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.dead_letters_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize command dead letters: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn dead_letters_for_entity_sync_runtime_json_safe(
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

pub(super) fn latest_dead_letter_for_command_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
    command_id: &str,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.latest_dead_letter_for_command(command_id)).map_err(|error| {
        set_last_error(&format!("failed to serialize latest command dead letter: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn latest_dead_letter_for_entity_sync_runtime_json_safe(
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
        // SAFETY: the caller guarantees `command_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
        // SAFETY: the caller guarantees `command_id` is a valid, null-terminated C string
        // (see `# Safety`).
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
