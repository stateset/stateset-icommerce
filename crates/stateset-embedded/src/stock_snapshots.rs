//! Stock snapshot operations (point-in-time inventory).

use stateset_core::{
    CaptureStockSnapshot, Result, StockSnapshot, StockSnapshotFilter, StockSnapshotId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Stock snapshot operations.
pub struct StockSnapshots {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for StockSnapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StockSnapshots").finish_non_exhaustive()
    }
}

impl StockSnapshots {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether stock snapshots are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::StockSnapshots)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::StockSnapshots)
    }

    /// Inventory aging from stored cost layers, reconciled to stock movements.
    pub fn inventory_aging(
        &self,
        as_of: chrono::NaiveDate,
    ) -> Result<stateset_core::InventoryAgingReport> {
        self.ensure()?;
        self.db.stock_snapshots().inventory_aging(as_of)
    }

    /// Capture a new stock snapshot from the stored inventory balances.
    pub fn capture(&self, input: CaptureStockSnapshot) -> Result<StockSnapshot> {
        self.ensure()?;
        self.db.stock_snapshots().capture(input)
    }

    /// Get a snapshot by ID.
    pub fn get(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        self.ensure()?;
        self.db.stock_snapshots().get(id)
    }

    /// Get the most recent snapshot.
    pub fn latest(&self) -> Result<Option<StockSnapshot>> {
        self.ensure()?;
        self.db.stock_snapshots().latest()
    }

    /// List snapshots (header-level) with optional pagination.
    pub fn list(&self, filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>> {
        self.ensure()?;
        self.db.stock_snapshots().list(filter)
    }

    /// Delete a snapshot.
    pub fn delete(&self, id: StockSnapshotId) -> Result<()> {
        self.ensure()?;
        self.db.stock_snapshots().delete(id)
    }
}
