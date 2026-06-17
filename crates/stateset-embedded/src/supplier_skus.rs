//! Supplier SKU operations (per-supplier SKU / unit-cost overrides).

use stateset_core::{
    BulkSupplierSkuItem, CreateSupplierSku, Result, SupplierSku, SupplierSkuFilter, SupplierSkuId,
    UpdateSupplierSku,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

/// Supplier SKU operations.
pub struct SupplierSkus {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for SupplierSkus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplierSkus").finish_non_exhaustive()
    }
}

impl SupplierSkus {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether supplier SKUs are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::SupplierSkus)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::SupplierSkus)
    }

    /// Create a new supplier SKU.
    pub fn create(&self, input: CreateSupplierSku) -> Result<SupplierSku> {
        self.ensure()?;
        self.db.supplier_skus().create(input)
    }

    /// Get a supplier SKU by ID.
    pub fn get(&self, id: SupplierSkuId) -> Result<Option<SupplierSku>> {
        self.ensure()?;
        self.db.supplier_skus().get(id)
    }

    /// Update a supplier SKU.
    pub fn update(&self, id: SupplierSkuId, input: UpdateSupplierSku) -> Result<SupplierSku> {
        self.ensure()?;
        self.db.supplier_skus().update(id, input)
    }

    /// List supplier SKUs with optional filtering.
    pub fn list(&self, filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>> {
        self.ensure()?;
        self.db.supplier_skus().list(filter)
    }

    /// Delete a supplier SKU.
    pub fn delete(&self, id: SupplierSkuId) -> Result<()> {
        self.ensure()?;
        self.db.supplier_skus().delete(id)
    }

    /// Bulk upsert supplier SKUs for a supplier, keyed by internal product.
    pub fn bulk_upsert(&self, supplier_id: Uuid, items: Vec<BulkSupplierSkuItem>) -> Result<u64> {
        self.ensure()?;
        self.db.supplier_skus().bulk_upsert(supplier_id, items)
    }
}
