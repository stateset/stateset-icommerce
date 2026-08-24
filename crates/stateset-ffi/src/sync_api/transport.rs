//! Transport-backed operations: healthcheck, remote head refresh, push, pull, and full sync.

use super::*;

pub(super) fn healthcheck_sync_runtime_safe(lease: SyncRuntimeLease) -> Result<u8, FfiErrorCode> {
    run_sync_runtime_async(lease, |runtime, executor| {
        executor.block_on(runtime.healthcheck()).map(|()| 1)
    })
}

pub(super) fn refresh_sync_runtime_remote_head_json_safe(
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

pub(super) fn push_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let result =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.push()))?;
    serde_json::to_string(&result).map_err(|error| {
        set_last_error(&format!("failed to serialize push result: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn pull_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let result =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.pull()))?;
    serde_json::to_string(&result).map_err(|error| {
        set_last_error(&format!("failed to serialize pull result: {error}"));
        FfiErrorCode::SerializationError
    })
}

pub(super) fn full_sync_runtime_json_safe(lease: SyncRuntimeLease) -> Result<String, FfiErrorCode> {
    let (push, pull) =
        run_sync_runtime_async(lease, |runtime, executor| executor.block_on(runtime.full_sync()))?;
    serde_json::to_string(&serde_json::json!({ "push": push, "pull": pull })).map_err(|error| {
        set_last_error(&format!("failed to serialize full sync result: {error}"));
        FfiErrorCode::SerializationError
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
