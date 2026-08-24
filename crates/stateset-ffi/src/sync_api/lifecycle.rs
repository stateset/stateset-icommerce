//! Runtime construction (`init_from_json` / `init_from_file`) and destruction.

use super::*;

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

        // SAFETY: the caller guarantees `config_json` is a valid, null-terminated C string
        // (see `# Safety`).
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

        // SAFETY: the caller guarantees `config_path` is a valid, null-terminated C string
        // (see `# Safety`).
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
