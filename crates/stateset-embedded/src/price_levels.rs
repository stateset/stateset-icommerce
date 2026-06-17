//! Price level operations (B2B pricing tiers).

use rust_decimal::Decimal;
use stateset_core::{
    CreatePriceLevel, PriceLevel, PriceLevelEntry, PriceLevelFilter, PriceLevelId, ProductId,
    Result, UpdatePriceLevel,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Price level operations.
pub struct PriceLevels {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for PriceLevels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriceLevels").finish_non_exhaustive()
    }
}

impl PriceLevels {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether price levels are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::PriceLevels)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::PriceLevels)
    }

    /// Create a new price level.
    pub fn create(&self, input: CreatePriceLevel) -> Result<PriceLevel> {
        self.ensure()?;
        self.db.price_levels().create(input)
    }

    /// Get a price level by ID.
    pub fn get(&self, id: PriceLevelId) -> Result<Option<PriceLevel>> {
        self.ensure()?;
        self.db.price_levels().get(id)
    }

    /// Update a price level.
    pub fn update(&self, id: PriceLevelId, input: UpdatePriceLevel) -> Result<PriceLevel> {
        self.ensure()?;
        self.db.price_levels().update(id, input)
    }

    /// List price levels with optional filtering.
    pub fn list(&self, filter: PriceLevelFilter) -> Result<Vec<PriceLevel>> {
        self.ensure()?;
        self.db.price_levels().list(filter)
    }

    /// Delete a price level and its entries.
    pub fn delete(&self, id: PriceLevelId) -> Result<()> {
        self.ensure()?;
        self.db.price_levels().delete(id)
    }

    /// Upsert a per-product fixed price entry.
    pub fn set_entry(
        &self,
        id: PriceLevelId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceLevelEntry> {
        self.ensure()?;
        self.db.price_levels().set_entry(id, product_id, price)
    }

    /// Remove a per-product entry.
    pub fn delete_entry(&self, id: PriceLevelId, product_id: ProductId) -> Result<()> {
        self.ensure()?;
        self.db.price_levels().delete_entry(id, product_id)
    }

    /// List per-product entries for a level.
    pub fn list_entries(&self, id: PriceLevelId) -> Result<Vec<PriceLevelEntry>> {
        self.ensure()?;
        self.db.price_levels().list_entries(id)
    }
}
