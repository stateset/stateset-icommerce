//! Scalar status accessors (heads, cursors, lag, and retained-collection counts).

use super::*;

pub(super) fn sync_runtime_status_safe(
    runtime: &Mutex<SyncRuntime>,
) -> stateset_sdk::sync::SyncStatus {
    let runtime = lock_sync_runtime(runtime);
    runtime.status()
}

pub(super) const fn bool_to_ffi(value: bool) -> u8 {
    if value { 1 } else { 0 }
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
