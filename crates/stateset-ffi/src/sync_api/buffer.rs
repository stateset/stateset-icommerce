//! Buffered pulled-event queries and drains.

use super::*;

pub(super) fn buffered_events_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let runtime = lock_sync_runtime(runtime);
    serde_json::to_string(&runtime.snapshot().buffered_events).map_err(|error| {
        set_last_error(&format!("failed to serialize buffered events: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn drain_buffer_sync_runtime_json_safe(
    runtime: &Mutex<SyncRuntime>,
) -> Result<String, FfiErrorCode> {
    let mut runtime = lock_sync_runtime(runtime);
    let events = runtime.drain_buffer();
    serde_json::to_string(&events).map_err(|error| {
        set_last_error(&format!("failed to serialize drained buffered events: {error}"));
        FfiErrorCode::SerializationError
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
