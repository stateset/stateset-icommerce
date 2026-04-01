#[cfg(feature = "postgres")]
use super::block_on_postgres_connect_with_options;
use super::{Commerce, CommerceBackend};

use std::sync::Arc;

use stateset_core::CommerceError;
use stateset_db::{Database, DatabaseConfig};
use stateset_observability::{MetricsConfig, init_metrics};

#[cfg(feature = "events")]
use crate::events::{EventConfig, EventSystem};

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

/// Builder for creating a Commerce instance with custom configuration.
#[derive(Default)]
#[must_use]
pub struct CommerceBuilder {
    sqlite_path: Option<String>,
    #[cfg(feature = "postgres")]
    postgres_url: Option<String>,
    max_connections: Option<u32>,
    #[cfg(feature = "postgres")]
    acquire_timeout_secs: Option<u64>,
    #[cfg(feature = "events")]
    event_config: Option<EventConfig>,
    metrics_config: MetricsConfig,
}

impl std::fmt::Debug for CommerceBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommerceBuilder").finish_non_exhaustive()
    }
}

impl CommerceBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the SQLite database path.
    #[cfg(feature = "sqlite")]
    pub fn sqlite(mut self, path: &str) -> Self {
        self.sqlite_path = Some(path.to_string());
        self
    }

    /// Set the database path (alias for sqlite).
    #[cfg(feature = "sqlite")]
    pub fn database(self, path: &str) -> Self {
        self.sqlite(path)
    }

    /// Configure for in-memory SQLite database.
    #[cfg(feature = "sqlite")]
    pub fn in_memory(mut self) -> Self {
        self.sqlite_path = Some(":memory:".to_string());
        self
    }

    /// Set the PostgreSQL connection URL.
    ///
    /// When this is set, the builder will create a PostgreSQL connection
    /// instead of SQLite.
    #[cfg(feature = "postgres")]
    pub fn postgres(mut self, url: &str) -> Self {
        self.postgres_url = Some(url.to_string());
        self
    }

    /// Set the maximum number of database connections.
    pub const fn max_connections(mut self, count: u32) -> Self {
        self.max_connections = Some(count);
        self
    }

    /// Configure in-process engine metrics collection.
    pub const fn metrics_config(mut self, config: MetricsConfig) -> Self {
        self.metrics_config = config;
        self
    }

    /// Disable in-process engine metrics collection.
    pub const fn disable_metrics(mut self) -> Self {
        self.metrics_config.enabled = false;
        self
    }

    /// Set the acquire timeout for PostgreSQL connections.
    #[cfg(feature = "postgres")]
    pub const fn acquire_timeout_secs(mut self, secs: u64) -> Self {
        self.acquire_timeout_secs = Some(secs);
        self
    }

    /// Configure the event system.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, EventConfig};
    ///
    /// let commerce = Commerce::builder()
    ///     .database(":memory:")
    ///     .event_config(EventConfig {
    ///         channel_capacity: 2048,
    ///         enable_webhooks: true,
    ///         ..Default::default()
    ///     })
    ///     .build()?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "events")]
    pub fn event_config(mut self, config: EventConfig) -> Self {
        self.event_config = Some(config);
        self
    }

    /// Build with sensible defaults for development/testing.
    ///
    /// Creates an in-memory SQLite database with default settings.
    /// This is the quickest way to get started for testing.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::builder().build_with_defaults()?;
    /// // Equivalent to Commerce::in_memory()
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn build_with_defaults(self) -> Result<Commerce, CommerceError> {
        self.in_memory().build()
    }

    /// Build the Commerce instance.
    pub fn build(self) -> Result<Commerce, CommerceError> {
        let metrics = init_metrics(self.metrics_config.clone());

        // Create event system if events feature is enabled
        #[cfg(feature = "events")]
        let event_system =
            Arc::new(self.event_config.map(EventSystem::with_config).unwrap_or_default());

        // Check if PostgreSQL URL is set
        #[cfg(feature = "postgres")]
        if let Some(url) = self.postgres_url {
            let max_connections = self.max_connections.unwrap_or(10);
            let acquire_timeout_secs = self.acquire_timeout_secs.unwrap_or(30);
            let db =
                block_on_postgres_connect_with_options(url, max_connections, acquire_timeout_secs)?;
            let db: Arc<dyn Database> = Arc::new(db);

            return Ok(Commerce {
                db,
                backend: CommerceBackend::Postgres,
                metrics,
                #[cfg(feature = "events")]
                event_system,
                #[cfg(all(feature = "sqlite", feature = "vector"))]
                sqlite_db: None,
            });
        }

        // Default to SQLite
        #[cfg(feature = "sqlite")]
        {
            let path = self.sqlite_path.unwrap_or_else(|| "stateset.db".to_string());

            let mut config = if path == ":memory:" {
                DatabaseConfig::in_memory()
            } else {
                DatabaseConfig::sqlite(&path)
            };
            if let Some(max) = self.max_connections {
                config.max_connections = max;
            }

            let sqlite_db = Arc::new(SqliteDatabase::new(&config)?);
            #[cfg(all(feature = "sqlite", feature = "vector"))]
            let db: Arc<dyn Database> = sqlite_db.clone();
            #[cfg(not(all(feature = "sqlite", feature = "vector")))]
            let db: Arc<dyn Database> = sqlite_db;
            Ok(Commerce {
                db,
                backend: CommerceBackend::Sqlite,
                metrics,
                #[cfg(feature = "events")]
                event_system,
                #[cfg(all(feature = "sqlite", feature = "vector"))]
                sqlite_db: Some(sqlite_db),
            })
        }

        #[cfg(not(any(feature = "sqlite", feature = "postgres")))]
        Err(CommerceError::Internal(
            "No database backend enabled. Enable 'sqlite' or 'postgres' feature.".to_string(),
        ))
    }
}
