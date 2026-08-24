//! Handle registry, lease bookkeeping, and runtime locking for [`SyncRuntimeHandle`].

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncHandleState {
    pub(crate) runtime_ptr: usize,
    pub(crate) in_flight: usize,
    pub(crate) destroying: bool,
}

#[derive(Debug)]
pub(crate) struct SyncHandleRegistry {
    pub(crate) active: HashMap<usize, SyncHandleState>,
    pub(crate) next_handle_id: usize,
}

impl Default for SyncHandleRegistry {
    fn default() -> Self {
        Self { active: HashMap::new(), next_handle_id: 1 }
    }
}

pub(super) static SYNC_HANDLE_REGISTRY: OnceLock<(Mutex<SyncHandleRegistry>, Condvar)> =
    OnceLock::new();

pub(crate) fn sync_handle_registry() -> &'static (Mutex<SyncHandleRegistry>, Condvar) {
    SYNC_HANDLE_REGISTRY.get_or_init(|| (Mutex::new(SyncHandleRegistry::default()), Condvar::new()))
}

pub(super) fn with_sync_handle_registry<T>(f: impl FnOnce(&mut SyncHandleRegistry) -> T) -> T {
    let (mutex, _) = sync_handle_registry();
    let mut handles = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut handles)
}

pub(super) const fn sync_handle_id_to_token(id: usize) -> SyncRuntimeHandle {
    id as SyncRuntimeHandle
}

pub(crate) struct SyncRuntimeLease {
    handle_id: usize,
    pub(crate) runtime_ptr: usize,
}

impl SyncRuntimeLease {
    #[allow(clippy::missing_const_for_fn)]
    #[allow(unsafe_code)]
    pub(crate) fn runtime(&self) -> &Mutex<SyncRuntime> {
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

pub(crate) fn begin_sync_runtime_use(
    runtime: SyncRuntimeHandle,
) -> Result<SyncRuntimeLease, FfiErrorCode> {
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

pub(super) fn next_available_handle_id(
    handles: &mut SyncHandleRegistry,
) -> Result<usize, FfiErrorCode> {
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
pub(crate) fn drop_sync_runtime_ptr(runtime_ptr: usize) {
    // SAFETY: `runtime_ptr` must have been allocated with `Box::into_raw`.
    unsafe { drop(Box::from_raw(runtime_ptr as *mut Mutex<SyncRuntime>)) };
}

pub(crate) fn register_new_sync_runtime_handle(
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

pub(crate) fn lock_sync_runtime(
    runtime: &Mutex<SyncRuntime>,
) -> std::sync::MutexGuard<'_, SyncRuntime> {
    match runtime.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
