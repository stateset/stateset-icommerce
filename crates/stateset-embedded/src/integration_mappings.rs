//! Integration mapping operations (external↔internal value translation).

use stateset_core::{
    CreateIntegrationMapping, IntegrationMapping, IntegrationMappingFilter, IntegrationMappingId,
    MappingLookup, Result, UpdateIntegrationMapping,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Integration mapping operations.
pub struct IntegrationMappings {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for IntegrationMappings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationMappings").finish_non_exhaustive()
    }
}

impl IntegrationMappings {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether integration mappings are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::IntegrationMappings)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::IntegrationMappings)
    }

    /// Create a new integration mapping.
    pub fn create(&self, input: CreateIntegrationMapping) -> Result<IntegrationMapping> {
        self.ensure()?;
        self.db.integration_mappings().create(input)
    }

    /// Get a mapping by ID.
    pub fn get(&self, id: IntegrationMappingId) -> Result<Option<IntegrationMapping>> {
        self.ensure()?;
        self.db.integration_mappings().get(id)
    }

    /// Update a mapping.
    pub fn update(
        &self,
        id: IntegrationMappingId,
        input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        self.ensure()?;
        self.db.integration_mappings().update(id, input)
    }

    /// List mappings with optional filtering.
    pub fn list(&self, filter: IntegrationMappingFilter) -> Result<Vec<IntegrationMapping>> {
        self.ensure()?;
        self.db.integration_mappings().list(filter)
    }

    /// Delete a mapping.
    pub fn delete(&self, id: IntegrationMappingId) -> Result<()> {
        self.ensure()?;
        self.db.integration_mappings().delete(id)
    }

    /// Bulk upsert mappings. Returns the number of rows affected.
    pub fn bulk_upsert(&self, items: Vec<CreateIntegrationMapping>) -> Result<u64> {
        self.ensure()?;
        self.db.integration_mappings().bulk_upsert(items)
    }

    /// Resolve the internal value for an external value.
    pub fn resolve(&self, lookup: &MappingLookup) -> Result<Option<String>> {
        self.ensure()?;
        self.db.integration_mappings().resolve(lookup)
    }
}
