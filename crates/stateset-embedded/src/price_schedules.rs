//! Price schedule operations (time-bounded pricing).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use stateset_core::{
    CreatePriceSchedule, PriceSchedule, PriceScheduleEntry, PriceScheduleFilter, PriceScheduleId,
    ProductId, Result, UpdatePriceSchedule,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Price schedule operations.
pub struct PriceSchedules {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for PriceSchedules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriceSchedules").finish_non_exhaustive()
    }
}

impl PriceSchedules {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether price schedules are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::PriceSchedules)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::PriceSchedules)
    }

    /// Create a new price schedule.
    pub fn create(&self, input: CreatePriceSchedule) -> Result<PriceSchedule> {
        self.ensure()?;
        self.db.price_schedules().create(input)
    }

    /// Get a price schedule by ID.
    pub fn get(&self, id: PriceScheduleId) -> Result<Option<PriceSchedule>> {
        self.ensure()?;
        self.db.price_schedules().get(id)
    }

    /// Update a price schedule.
    pub fn update(&self, id: PriceScheduleId, input: UpdatePriceSchedule) -> Result<PriceSchedule> {
        self.ensure()?;
        self.db.price_schedules().update(id, input)
    }

    /// List price schedules with optional filtering.
    pub fn list(&self, filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>> {
        self.ensure()?;
        self.db.price_schedules().list(filter)
    }

    /// Delete a price schedule and its entries.
    pub fn delete(&self, id: PriceScheduleId) -> Result<()> {
        self.ensure()?;
        self.db.price_schedules().delete(id)
    }

    /// Upsert a per-product scheduled price.
    pub fn set_entry(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceScheduleEntry> {
        self.ensure()?;
        self.db.price_schedules().set_entry(id, product_id, price)
    }

    /// Remove a per-product entry.
    pub fn delete_entry(&self, id: PriceScheduleId, product_id: ProductId) -> Result<()> {
        self.ensure()?;
        self.db.price_schedules().delete_entry(id, product_id)
    }

    /// List per-product entries for a schedule.
    pub fn list_entries(&self, id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>> {
        self.ensure()?;
        self.db.price_schedules().list_entries(id)
    }

    /// Resolve the effective scheduled price for a product at an instant.
    pub fn resolve_price(
        &self,
        product_id: ProductId,
        at: DateTime<Utc>,
    ) -> Result<Option<Decimal>> {
        self.ensure()?;
        self.db.price_schedules().resolve_price(product_id, at)
    }
}
