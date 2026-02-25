//! Main Commerce struct - the entry point to the library

mod accessors;
mod builder;
mod constructors;
mod events;
mod introspection;

#[cfg(test)]
mod tests;

use std::sync::Arc;

#[cfg(feature = "postgres")]
use stateset_core::CommerceError;
use stateset_db::Database;
use stateset_observability::Metrics;

#[cfg(feature = "events")]
use crate::events::EventSystem;

#[cfg(all(feature = "sqlite", feature = "vector"))]
use stateset_db::SqliteDatabase;

pub use builder::CommerceBuilder;
pub use introspection::CommerceHealth;

#[cfg(feature = "postgres")]
fn block_on_postgres<T, Fut, MakeFut>(make_fut: MakeFut) -> Result<T, CommerceError>
where
    T: Send + 'static,
    Fut: std::future::Future<Output = Result<T, CommerceError>> + Send + 'static,
    MakeFut: FnOnce() -> Fut + Send + 'static,
{
    let new_runtime = || {
        tokio::runtime::Runtime::new()
            .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {e}")))
    };

    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) =>
        {
            tokio::task::block_in_place(|| handle.block_on(make_fut()))
        }
        Ok(_) => std::thread::spawn(move || {
            let rt = new_runtime()?;
            rt.block_on(make_fut())
        })
        .join()
        .map_err(|_| {
            CommerceError::Internal("PostgreSQL initialization thread panicked".to_string())
        })?,
        Err(_) => {
            let rt = new_runtime()?;
            rt.block_on(make_fut())
        }
    }
}

/// Active database backend used by a [`Commerce`] instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommerceBackend {
    /// Embedded SQLite backend.
    Sqlite,
    /// PostgreSQL backend.
    Postgres,
    /// Caller-provided database implementation.
    External,
}

/// The main commerce interface.
///
/// This is the entry point to all commerce operations. Initialize it once
/// and use the accessor methods to perform operations.
///
/// # Example
///
/// ```rust,ignore
/// use stateset_embedded::Commerce;
///
/// // SQLite (default)
/// let commerce = Commerce::new("./store.db")?;
///
/// // Access different domains
/// let orders = commerce.orders();
/// let inventory = commerce.inventory();
/// let customers = commerce.customers();
/// let products = commerce.products();
/// let returns = commerce.returns();
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct Commerce {
    db: Arc<dyn Database>,
    backend: CommerceBackend,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
    #[cfg(all(feature = "sqlite", feature = "vector"))]
    sqlite_db: Option<Arc<SqliteDatabase>>,
}

impl std::fmt::Debug for Commerce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commerce").field("backend", &self.backend).finish_non_exhaustive()
    }
}
