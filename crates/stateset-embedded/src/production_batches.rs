//! Production batch operations (grouping manufacturing work orders).

use stateset_core::{
    CreateProductionBatch, ProductionBatch, ProductionBatchFilter, ProductionBatchId, Result,
    UpdateProductionBatch,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

/// Production batch operations.
pub struct ProductionBatches {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for ProductionBatches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionBatches").finish_non_exhaustive()
    }
}

impl ProductionBatches {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether production batches are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::ProductionBatches)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::ProductionBatches)
    }

    /// Create a new production batch.
    pub fn create(&self, input: CreateProductionBatch) -> Result<ProductionBatch> {
        self.ensure()?;
        self.db.production_batches().create(input)
    }

    /// Get a production batch by ID.
    pub fn get(&self, id: ProductionBatchId) -> Result<Option<ProductionBatch>> {
        self.ensure()?;
        self.db.production_batches().get(id)
    }

    /// Update a production batch.
    pub fn update(
        &self,
        id: ProductionBatchId,
        input: UpdateProductionBatch,
    ) -> Result<ProductionBatch> {
        self.ensure()?;
        self.db.production_batches().update(id, input)
    }

    /// List production batches with optional filtering.
    pub fn list(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        self.ensure()?;
        self.db.production_batches().list(filter)
    }

    /// Delete a production batch.
    pub fn delete(&self, id: ProductionBatchId) -> Result<()> {
        self.ensure()?;
        self.db.production_batches().delete(id)
    }

    /// Link work orders to a batch.
    pub fn add_work_orders(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        self.ensure()?;
        self.db.production_batches().add_work_orders(id, work_order_ids)
    }

    /// Remove a work order from a batch.
    pub fn remove_work_order(
        &self,
        id: ProductionBatchId,
        work_order_id: Uuid,
    ) -> Result<ProductionBatch> {
        self.ensure()?;
        self.db.production_batches().remove_work_order(id, work_order_id)
    }
}
