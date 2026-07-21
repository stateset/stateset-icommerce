//! Fixed asset register operations
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::{Commerce, CreateFixedAsset};
//!
//! let commerce = Commerce::new("./store.db")?;
//! let asset = commerce.fixed_assets().create(input)?;
//! let asset = commerce.fixed_assets().place_in_service(asset.id, date)?;
//! let schedule = commerce.fixed_assets().generate_schedule(asset.id)?;
//! let asset = commerce.fixed_assets().post_depreciation(asset.id, 1)?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{
    CreateFixedAsset, DepreciationSchedule, FixedAsset, FixedAssetFilter, Result, UpdateFixedAsset,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Fixed asset register operations.
pub struct FixedAssets {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for FixedAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixedAssets").finish_non_exhaustive()
    }
}

impl FixedAssets {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    /// Whether fixed assets are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::FixedAssets)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::FixedAssets)
    }

    /// Create a new fixed asset.
    pub fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        self.ensure()?;
        self.db.fixed_assets().create(input)
    }

    /// Get a fixed asset by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        self.ensure()?;
        self.db.fixed_assets().get(id)
    }

    /// List fixed assets with optional filtering.
    pub fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        self.ensure()?;
        self.db.fixed_assets().list(filter)
    }

    /// Update a fixed asset (partial).
    pub fn update(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        self.ensure()?;
        self.db.fixed_assets().update(id, input)
    }

    /// Place a draft asset in service.
    pub fn place_in_service(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        self.ensure()?;
        let asset = self.db.fixed_assets().place_in_service(id, date)?;
        #[cfg(feature = "events")]
        self.emit(CommerceEvent::FixedAssetPlacedInService {
            asset_id: asset.id,
            asset_number: asset.asset_number.clone(),
            in_service_date: asset.in_service_date.unwrap_or(date),
            acquisition_cost: asset.acquisition_cost,
            timestamp: Utc::now(),
        });
        Ok(asset)
    }

    /// Dispose of an asset for the given proceeds, recording gain/loss.
    pub fn dispose(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.ensure()?;
        let asset = self.db.fixed_assets().dispose(id, date, proceeds, notes)?;
        #[cfg(feature = "events")]
        self.emit(CommerceEvent::FixedAssetDisposed {
            asset_id: asset.id,
            asset_number: asset.asset_number.clone(),
            disposal_date: asset.disposal.as_ref().map_or(date, |d| d.disposal_date),
            proceeds,
            gain_loss: asset.disposal.as_ref().map_or(rust_decimal::Decimal::ZERO, |d| d.gain_loss),
            timestamp: Utc::now(),
        });
        Ok(asset)
    }

    /// Write off an asset (disposal with zero proceeds).
    pub fn write_off(
        &self,
        id: Uuid,
        date: NaiveDate,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.ensure()?;
        let asset = self.db.fixed_assets().write_off(id, date, notes)?;
        #[cfg(feature = "events")]
        self.emit(CommerceEvent::FixedAssetWrittenOff {
            asset_id: asset.id,
            asset_number: asset.asset_number.clone(),
            write_off_date: asset.disposal.as_ref().map_or(date, |d| d.disposal_date),
            loss: asset
                .disposal
                .as_ref()
                .map_or(rust_decimal::Decimal::ZERO, |d| d.gain_loss.abs()),
            timestamp: Utc::now(),
        });
        Ok(asset)
    }

    /// Generate and persist the depreciation schedule for an asset.
    pub fn generate_schedule(&self, id: Uuid) -> Result<DepreciationSchedule> {
        self.ensure()?;
        self.db.fixed_assets().generate_schedule(id)
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    pub fn get_schedule(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        self.ensure()?;
        self.db.fixed_assets().get_schedule(id)
    }

    /// Post the next `periods` scheduled depreciation entries.
    ///
    /// If the active GL auto-posting configuration has `auto_post_depreciation`
    /// enabled, a balanced journal entry is also created and posted (debit
    /// depreciation expense, credit accumulated depreciation), using the
    /// asset's configured GL accounts or, as a fallback, the first active
    /// posting accounts with the matching account sub-types.
    pub fn post_depreciation(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        self.ensure()?;
        #[cfg(feature = "events")]
        let previous_accumulated = self
            .db
            .fixed_assets()
            .get(id)?
            .map(|a| a.accumulated_depreciation)
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let asset = self.db.fixed_assets().post_depreciation(id, periods)?;
        #[cfg(feature = "events")]
        self.emit(CommerceEvent::DepreciationPosted {
            asset_id: asset.id,
            asset_number: asset.asset_number.clone(),
            periods,
            amount: asset.accumulated_depreciation - previous_accumulated,
            accumulated_depreciation: asset.accumulated_depreciation,
            timestamp: Utc::now(),
        });
        Ok(asset)
    }
}
