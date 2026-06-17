//! Purgatory operations (order ingestion staging).

use stateset_core::{
    IngestOrder, MapPurgatoryLine, PurgatoryFilter, PurgatoryLineItemId, PurgatoryOrder,
    PurgatoryOrderId, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Purgatory (order ingestion staging) operations.
pub struct Purgatory {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Purgatory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Purgatory").finish_non_exhaustive()
    }
}

impl Purgatory {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether purgatory is supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Purgatory)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Purgatory)
    }

    /// Ingest an order into purgatory.
    pub fn ingest(&self, input: IngestOrder) -> Result<PurgatoryOrder> {
        self.ensure()?;
        self.db.purgatory().ingest(input)
    }

    /// Get a purgatory order by ID.
    pub fn get(&self, id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>> {
        self.ensure()?;
        self.db.purgatory().get(id)
    }

    /// List purgatory orders with optional filtering.
    pub fn list(&self, filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>> {
        self.ensure()?;
        self.db.purgatory().list(filter)
    }

    /// Map a line to a product and/or toggle its flags.
    pub fn map_line(
        &self,
        id: PurgatoryOrderId,
        line_id: PurgatoryLineItemId,
        input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder> {
        self.ensure()?;
        self.db.purgatory().map_line(id, line_id, input)
    }

    /// Post the order out of purgatory.
    pub fn post(&self, id: PurgatoryOrderId) -> Result<PurgatoryOrder> {
        self.ensure()?;
        self.db.purgatory().post(id)
    }

    /// Delete a purgatory order.
    pub fn delete(&self, id: PurgatoryOrderId) -> Result<()> {
        self.ensure()?;
        self.db.purgatory().delete(id)
    }
}
