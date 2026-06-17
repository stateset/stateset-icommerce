//! Integration field-mapping operations (field-path mappings).

use stateset_core::{
    CreateIntegrationFieldMapping, IntegrationFieldMapping, IntegrationFieldMappingFilter,
    IntegrationFieldMappingId, Result, UpdateIntegrationFieldMapping,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Integration field-mapping operations.
pub struct IntegrationFieldMappings {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for IntegrationFieldMappings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationFieldMappings").finish_non_exhaustive()
    }
}

impl IntegrationFieldMappings {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether integration field mappings are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::IntegrationFieldMappings)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::IntegrationFieldMappings)
    }

    /// Create a field mapping.
    pub fn create(&self, input: CreateIntegrationFieldMapping) -> Result<IntegrationFieldMapping> {
        self.ensure()?;
        self.db.integration_field_mappings().create(input)
    }

    /// Get a field mapping by ID.
    pub fn get(&self, id: IntegrationFieldMappingId) -> Result<Option<IntegrationFieldMapping>> {
        self.ensure()?;
        self.db.integration_field_mappings().get(id)
    }

    /// Update a field mapping.
    pub fn update(
        &self,
        id: IntegrationFieldMappingId,
        input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        self.ensure()?;
        self.db.integration_field_mappings().update(id, input)
    }

    /// List field mappings with optional filtering.
    pub fn list(
        &self,
        filter: IntegrationFieldMappingFilter,
    ) -> Result<Vec<IntegrationFieldMapping>> {
        self.ensure()?;
        self.db.integration_field_mappings().list(filter)
    }

    /// Delete a field mapping.
    pub fn delete(&self, id: IntegrationFieldMappingId) -> Result<()> {
        self.ensure()?;
        self.db.integration_field_mappings().delete(id)
    }

    /// Bulk create field mappings.
    pub fn bulk_create(&self, items: Vec<CreateIntegrationFieldMapping>) -> Result<u64> {
        self.ensure()?;
        self.db.integration_field_mappings().bulk_create(items)
    }

    /// Bulk delete field mappings by ID.
    pub fn bulk_delete(&self, ids: Vec<IntegrationFieldMappingId>) -> Result<u64> {
        self.ensure()?;
        self.db.integration_field_mappings().bulk_delete(ids)
    }

    /// List the distinct mapping groups for an integration account.
    pub fn distinct_groups(&self, integration_account: &str) -> Result<Vec<String>> {
        self.ensure()?;
        self.db.integration_field_mappings().distinct_groups(integration_account)
    }
}
