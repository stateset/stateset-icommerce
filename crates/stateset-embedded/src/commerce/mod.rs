//! Main Commerce struct - the entry point to the library

mod accessors;
mod builder;
mod constructors;
mod events;
mod introspection;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use stateset_db::Database;
use stateset_observability::Metrics;

#[cfg(feature = "events")]
use crate::events::EventSystem;

#[cfg(all(feature = "sqlite", feature = "vector"))]
use stateset_db::SqliteDatabase;

pub use builder::CommerceBuilder;
pub use introspection::CommerceHealth;

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
