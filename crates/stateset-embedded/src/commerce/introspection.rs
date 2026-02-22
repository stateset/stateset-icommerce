use super::{Commerce, CommerceBackend};

use chrono::{DateTime, Utc};
use stateset_core::OrderFilter;
use stateset_db::Database;
use stateset_observability::{Metrics, MetricsSnapshot};

#[cfg(all(feature = "sqlite", feature = "vector"))]
use stateset_core::CommerceError;

#[cfg(all(feature = "sqlite", feature = "vector"))]
use crate::Vector;

/// Runtime health status for a [`Commerce`] engine instance.
#[derive(Debug, Clone)]
pub struct CommerceHealth {
    /// Whether the engine is healthy at the time of the check.
    pub healthy: bool,
    /// Active database backend.
    pub backend: CommerceBackend,
    /// Whether a basic database probe succeeded.
    pub database_reachable: bool,
    /// Error details from the last database probe, if any.
    pub database_error: Option<String>,
    /// Point-in-time metrics snapshot.
    pub metrics: MetricsSnapshot,
    /// Number of active event subscribers (when `events` is enabled).
    #[cfg(feature = "events")]
    pub event_subscribers: usize,
    /// Check timestamp in UTC.
    pub checked_at: DateTime<Utc>,
}

impl Commerce {
    /// Get the underlying database (for advanced use cases).
    pub fn database(&self) -> &dyn Database {
        &*self.db
    }

    /// Get the active database backend kind.
    pub const fn backend(&self) -> CommerceBackend {
        self.backend
    }

    /// Access engine metrics handle.
    pub const fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Capture a point-in-time metrics snapshot.
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Run a lightweight engine health check.
    ///
    /// The database probe uses `orders().count(Default::default())` and does not
    /// mutate state.
    pub fn health_check(&self) -> CommerceHealth {
        let probe = self.db.orders().count(OrderFilter::default());
        let metrics = self.metrics_snapshot();
        CommerceHealth {
            healthy: probe.is_ok(),
            backend: self.backend,
            database_reachable: probe.is_ok(),
            database_error: probe.err().map(|e| e.to_string()),
            metrics,
            #[cfg(feature = "events")]
            event_subscribers: self.event_system.subscriber_count(),
            checked_at: Utc::now(),
        }
    }

    /// Access vector search operations.
    ///
    /// Requires the `vector` feature and an OpenAI API key for embedding generation.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key for generating embeddings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    /// let api_key = std::env::var("OPENAI_API_KEY")?;
    ///
    /// let vector = commerce.vector(api_key)?;
    ///
    /// // Index products for search
    /// for product in commerce.products().list(Default::default())? {
    ///     vector.index_product(&product)?;
    /// }
    ///
    /// // Semantic search
    /// let results = vector.search_products("wireless bluetooth headphones", 10)?;
    /// for result in results {
    ///     println!("{}: {} (score: {:.2})", result.entity.name, result.entity.id, result.score);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(all(feature = "sqlite", feature = "vector"))]
    pub fn vector(&self, api_key: String) -> Result<Vector, CommerceError> {
        match &self.sqlite_db {
            Some(db) => Ok(Vector::new(db.vector(), api_key)),
            None => Err(CommerceError::NotPermitted(
                "Vector search requires SQLite database. Use Commerce::new() instead of with_database() or with_postgres().".to_string()
            )),
        }
    }
}
