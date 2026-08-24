//! Off-thread Tokio executor used to drive async sync-runtime operations from the C ABI.

use super::*;

#[derive(Debug)]
pub(super) enum AsyncRuntimeError {
    Runtime(String),
    Sync(SyncError),
}

pub(super) fn set_async_runtime_error(error: AsyncRuntimeError) -> FfiErrorCode {
    match error {
        AsyncRuntimeError::Runtime(message) => {
            set_last_error(&message);
            FfiErrorCode::InternalError
        }
        AsyncRuntimeError::Sync(error) => set_sync_error(&error),
    }
}

pub(super) fn sync_runtime_thread_panic(payload: Box<dyn Any + Send>) -> FfiErrorCode {
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

pub(crate) fn run_sync_runtime_async<T, F>(
    lease: SyncRuntimeLease,
    operation: F,
) -> Result<T, FfiErrorCode>
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
