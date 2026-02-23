use super::{Commerce, CommerceBackend, CommerceBuilder};

use std::sync::Arc;

use stateset_core::CommerceError;
use stateset_db::{Database, DatabaseConfig};
use stateset_observability::{MetricsConfig, init_metrics};

#[cfg(feature = "events")]
use crate::events::EventSystem;

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;

impl Commerce {
    /// Create a new Commerce instance with a SQLite database.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file. Creates if not exists.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// // File-based database
    /// let commerce = Commerce::new("./my-store.db")?;
    ///
    /// // In-memory database (useful for testing)
    /// let commerce = Commerce::new(":memory:")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn new(path: &str) -> Result<Self, CommerceError> {
        let config = if path == ":memory:" {
            DatabaseConfig::in_memory()
        } else {
            DatabaseConfig::sqlite(path)
        };

        let sqlite_db = Arc::new(SqliteDatabase::new(&config)?);
        let db: Arc<dyn Database> = sqlite_db;
        let metrics = init_metrics(MetricsConfig::default());

        Ok(Self {
            db,
            backend: CommerceBackend::Sqlite,
            metrics,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
            #[cfg(all(feature = "sqlite", feature = "vector"))]
            sqlite_db: Some(sqlite_db),
        })
    }

    // ========================================================================
    // Convenience Constructors
    // ========================================================================

    /// Create a Commerce instance with SQLite (convenience method).
    ///
    /// This is an alias for `Commerce::new()` with a clearer name.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::sqlite("./store.db")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn sqlite(path: &str) -> Result<Self, CommerceError> {
        Self::new(path)
    }

    /// Create a Commerce instance with an in-memory SQLite database.
    ///
    /// This is useful for testing or ephemeral data that doesn't need persistence.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::in_memory()?;
    /// // Data will be lost when commerce is dropped
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn in_memory() -> Result<Self, CommerceError> {
        Self::new(":memory:")
    }

    /// Create a Commerce instance with SQLite and custom pool size.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file
    /// * `max_connections` - Maximum number of connections in the pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// // Create with larger connection pool for high concurrency
    /// let commerce = Commerce::sqlite_pool("./store.db", 10)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn sqlite_pool(path: &str, max_connections: u32) -> Result<Self, CommerceError> {
        Self::builder().sqlite(path).max_connections(max_connections).build()
    }

    /// Create a Commerce instance with PostgreSQL and custom pool size.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string
    /// * `max_connections` - Maximum number of connections in the pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::postgres_pool(
    ///     "postgres://user:pass@localhost/db",
    ///     20,
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "postgres")]
    pub fn postgres_pool(url: &str, max_connections: u32) -> Result<Self, CommerceError> {
        Self::with_postgres_options(url, max_connections, 30)
    }

    /// Create a Commerce instance connected to PostgreSQL.
    ///
    /// This requires the `postgres` feature to be enabled and creates
    /// a new Tokio runtime for async operations.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string (e.g., "postgres://user:pass@localhost/db")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::with_postgres("postgres://localhost/stateset")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "postgres")]
    pub fn with_postgres(url: &str) -> Result<Self, CommerceError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;

        let db = rt.block_on(PostgresDatabase::connect(url))?;
        let db: Arc<dyn Database> = Arc::new(db);
        let metrics = init_metrics(MetricsConfig::default());

        Ok(Self {
            db,
            backend: CommerceBackend::Postgres,
            metrics,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
            #[cfg(all(feature = "sqlite", feature = "vector"))]
            sqlite_db: None,
        })
    }

    /// Create a Commerce instance connected to PostgreSQL with custom options.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string
    /// * `max_connections` - Maximum number of connections in the pool
    /// * `acquire_timeout_secs` - Timeout in seconds for acquiring a connection
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::with_postgres_options(
    ///     "postgres://localhost/stateset",
    ///     20,  // max connections
    ///     60,  // timeout in seconds
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "postgres")]
    pub fn with_postgres_options(
        url: &str,
        max_connections: u32,
        acquire_timeout_secs: u64,
    ) -> Result<Self, CommerceError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;

        let db = rt.block_on(PostgresDatabase::connect_with_options(
            url,
            max_connections,
            acquire_timeout_secs,
        ))?;
        let db: Arc<dyn Database> = Arc::new(db);
        let metrics = init_metrics(MetricsConfig::default());

        Ok(Self {
            db,
            backend: CommerceBackend::Postgres,
            metrics,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
            #[cfg(all(feature = "sqlite", feature = "vector"))]
            sqlite_db: None,
        })
    }

    /// Create a Commerce instance with a pre-connected database.
    ///
    /// This is useful when you want to manage the database connection yourself.
    /// Note: Tax operations and vector search will not be available when using this method.
    pub fn with_database(db: Arc<dyn Database>) -> Self {
        let metrics = init_metrics(MetricsConfig::default());
        Self {
            db,
            backend: CommerceBackend::External,
            metrics,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
            #[cfg(all(feature = "sqlite", feature = "vector"))]
            sqlite_db: None,
        }
    }

    /// Create a Commerce instance with custom configuration.
    ///
    /// Use `CommerceBuilder` for more control over initialization.
    pub fn builder() -> CommerceBuilder {
        CommerceBuilder::default()
    }
}
