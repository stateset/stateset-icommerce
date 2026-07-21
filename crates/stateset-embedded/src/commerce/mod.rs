//! Main Commerce struct - the entry point to the library

mod accessors;
mod builder;
mod constructors;
mod events;
mod introspection;

#[cfg(all(test, feature = "sqlite"))]
mod tests;

use std::sync::Arc;

#[cfg(feature = "postgres")]
use stateset_core::CommerceError;
use stateset_db::Database;
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
use stateset_observability::Metrics;

#[cfg(feature = "events")]
use crate::events::EventSystem;

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

pub use builder::CommerceBuilder;
pub use introspection::CommerceHealth;

#[cfg(feature = "postgres")]
fn new_postgres_runtime() -> Result<tokio::runtime::Runtime, CommerceError> {
    tokio::runtime::Runtime::new()
        .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {e}")))
}

#[cfg(feature = "postgres")]
fn block_on_postgres_connect(url: String) -> Result<PostgresDatabase, CommerceError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) =>
        {
            tokio::task::block_in_place(|| handle.block_on(PostgresDatabase::connect(url)))
        }
        Ok(_) => std::thread::spawn(move || {
            let rt = new_postgres_runtime()?;
            rt.block_on(PostgresDatabase::connect(url))
        })
        .join()
        .map_err(|_| {
            CommerceError::Internal("PostgreSQL initialization thread panicked".to_string())
        })?,
        Err(_) => {
            let rt = new_postgres_runtime()?;
            rt.block_on(PostgresDatabase::connect(url))
        }
    }
}

#[cfg(feature = "postgres")]
fn block_on_postgres_connect_with_options(
    url: String,
    max_connections: u32,
    acquire_timeout_secs: u64,
) -> Result<PostgresDatabase, CommerceError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) =>
        {
            tokio::task::block_in_place(|| {
                handle.block_on(PostgresDatabase::connect_with_options(
                    url,
                    max_connections,
                    acquire_timeout_secs,
                ))
            })
        }
        Ok(_) => std::thread::spawn(move || {
            let rt = new_postgres_runtime()?;
            rt.block_on(PostgresDatabase::connect_with_options(
                url,
                max_connections,
                acquire_timeout_secs,
            ))
        })
        .join()
        .map_err(|_| {
            CommerceError::Internal("PostgreSQL initialization thread panicked".to_string())
        })?,
        Err(_) => {
            let rt = new_postgres_runtime()?;
            rt.block_on(PostgresDatabase::connect_with_options(
                url,
                max_connections,
                acquire_timeout_secs,
            ))
        }
    }
}

/// Active database backend used by a [`Commerce`] instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    #[cfg(feature = "sqlite")]
    sqlite_db: Option<Arc<SqliteDatabase>>,
    /// Filesystem path of the SQLite database, when one is backing this
    /// instance. Needed by backup/restore, which operate on files rather than
    /// on the repository abstraction. `None` for `:memory:` and non-SQLite.
    #[cfg(feature = "sqlite")]
    sqlite_path: Option<String>,
}

impl std::fmt::Debug for Commerce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commerce").field("backend", &self.backend).finish_non_exhaustive()
    }
}
