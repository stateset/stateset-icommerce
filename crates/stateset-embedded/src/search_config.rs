//! Search configuration operations for managing search settings
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateSearchConfig};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let config = commerce.search_config().create(CreateSearchConfig {
//!     name: "Default Search".into(),
//!     ..Default::default()
//! })?;
//!
//! // Set it as the active configuration
//! let config = commerce.search_config().set_active(config.id)?;
//! println!("Active config: {}", config.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateSearchConfig, Result, SearchConfig, SearchConfigFilter, SearchConfigId,
    UpdateSearchConfig,
};
use stateset_db::Database;
use std::sync::Arc;

/// Search configuration operations.
pub struct SearchConfigs {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for SearchConfigs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchConfigs").finish_non_exhaustive()
    }
}

impl SearchConfigs {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new search configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateSearchConfig};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let config = commerce.search_config().create(CreateSearchConfig {
    ///     name: "Product Search v2".into(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateSearchConfig) -> Result<SearchConfig> {
        self.db.search_configs().create(input)
    }

    /// Get a search configuration by ID.
    pub fn get(&self, id: SearchConfigId) -> Result<Option<SearchConfig>> {
        self.db.search_configs().get(id)
    }

    /// Update a search configuration.
    pub fn update(&self, id: SearchConfigId, input: UpdateSearchConfig) -> Result<SearchConfig> {
        self.db.search_configs().update(id, input)
    }

    /// List search configurations with optional filtering.
    pub fn list(&self, filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        self.db.search_configs().list(filter)
    }

    /// Delete a search configuration.
    pub fn delete(&self, id: SearchConfigId) -> Result<()> {
        self.db.search_configs().delete(id)
    }

    /// Get the currently active search configuration.
    pub fn get_active(&self) -> Result<Option<SearchConfig>> {
        self.db.search_configs().get_active()
    }

    /// Set a configuration as active (deactivating any current one).
    pub fn set_active(&self, id: SearchConfigId) -> Result<SearchConfig> {
        self.db.search_configs().set_active(id)
    }
}
