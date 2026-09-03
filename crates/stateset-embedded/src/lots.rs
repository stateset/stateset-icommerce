//! Lot/Batch tracking operations
//!
//! Comprehensive lot management system supporting:
//! - Lot creation and lifecycle management
//! - Lot transactions (consumption, adjustment, transfer)
//! - Certificate management (COA, COC, MSDS)
//! - Forward and backward traceability
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateLot};
//! use chrono::{Utc, Duration};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a lot for received materials
//! let lot = commerce.lots().create(CreateLot {
//!     lot_number: Some("LOT-2025-001".into()),
//!     sku: "RAW-MAT-001".into(),
//!     quantity_produced: dec!(1000),
//!     expiration_date: Some(Utc::now() + Duration::days(365)),
//!     ..Default::default()
//! })?;
//!
//! println!("Created lot {}", lot.lot_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    AddLotCertificate, AdjustLot, ConsumeLot, CreateLot, Lot, LotCertificate, LotFilter,
    LotGenealogyLink, LotLocation, LotStatus, LotTransaction, MergeLots, ReserveLot, Result,
    SplitLot, TraceabilityResult, TransferLot, UpdateLot,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Lot/Batch tracking management interface.
pub struct Lots {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Lots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lots").finish_non_exhaustive()
    }
}

impl Lots {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Basic CRUD
    // ========================================================================

    /// Create a new lot.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateLot};
    /// use chrono::{Utc, Duration};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let lot = commerce.lots().create(CreateLot {
    ///     lot_number: Some("BATCH-001".into()),
    ///     sku: "PROD-001".into(),
    ///     quantity_produced: dec!(500),
    ///     production_date: Some(Utc::now()),
    ///     expiration_date: Some(Utc::now() + Duration::days(180)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateLot) -> Result<Lot> {
        self.db.lots().create(input)
    }

    /// Get a lot by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        self.db.lots().get(id)
    }

    /// Get a lot by lot number.
    pub fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        self.db.lots().get_by_number(lot_number)
    }

    /// List lots with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, LotFilter, LotStatus};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all active lots for a SKU
    /// let lots = commerce.lots().list(LotFilter {
    ///     sku: Some("PROD-001".into()),
    ///     status: Some(LotStatus::Active),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        self.db.lots().list(filter)
    }

    /// Update a lot.
    pub fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        self.db.lots().update(id, input)
    }

    /// Delete a lot (only if unused).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.lots().delete(id)
    }

    // ========================================================================
    // Status Management
    // ========================================================================

    /// Quarantine a lot (prevent usage).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.lots().quarantine(Uuid::new_v4(), "Quality issue detected")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        self.db.lots().quarantine(id, reason)
    }

    /// Release a lot from quarantine.
    pub fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        self.db.lots().release_quarantine(id)
    }

    // ========================================================================
    // Inventory Operations
    // ========================================================================

    /// Adjust lot quantity (positive or negative).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, AdjustLot};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Remove 10 units due to damage
    /// commerce.lots().adjust(AdjustLot {
    ///     lot_id: Uuid::new_v4(),
    ///     quantity: dec!(-10),
    ///     reason: "Damaged in storage".into(),
    ///     performed_by: Some("warehouse_user".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        self.db.lots().adjust(input)
    }

    /// Consume quantity from a lot.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, ConsumeLot};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.lots().consume(ConsumeLot {
    ///     lot_id: Uuid::new_v4(),
    ///     quantity: dec!(25),
    ///     reference_type: "work_order".into(),
    ///     reference_id: Uuid::new_v4(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        self.db.lots().consume(input)
    }

    /// Reserve quantity in a lot.
    ///
    /// Returns the reservation ID which can be used to release or confirm the reservation.
    pub fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        self.db.lots().reserve(input)
    }

    /// Release a reservation (cancel it without consuming).
    pub fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.lots().release_reservation(reservation_id)
    }

    /// Confirm a reservation (convert to actual consumption).
    pub fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        self.db.lots().confirm_reservation(reservation_id)
    }

    /// Transfer lot to a different location.
    pub fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        self.db.lots().transfer(input)
    }

    /// Split a lot into two.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, SplitLot};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Split 100 units into a new lot
    /// let new_lot = commerce.lots().split(SplitLot {
    ///     source_lot_id: Uuid::new_v4(),
    ///     new_lot_number: Some("LOT-2025-001B".into()),
    ///     quantity: dec!(100),
    ///     reason: Some("Customer allocation".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn split(&self, input: SplitLot) -> Result<Lot> {
        self.db.lots().split(input)
    }

    /// Merge multiple lots into one.
    pub fn merge(&self, input: MergeLots) -> Result<Lot> {
        self.db.lots().merge(input)
    }

    // ========================================================================
    // Certificates
    // ========================================================================

    /// Add a certificate to a lot.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, AddLotCertificate, CertificateType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.lots().add_certificate(AddLotCertificate {
    ///     lot_id: Uuid::new_v4(),
    ///     certificate_type: CertificateType::Coa,
    ///     document_url: Some("https://storage.example.com/certs/coa-123.pdf".into()),
    ///     issued_by: Some("Quality Lab Inc.".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        self.db.lots().add_certificate(input)
    }

    /// Get certificates for a lot.
    pub fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        self.db.lots().get_certificates(lot_id)
    }

    /// Remove a certificate from a lot.
    pub fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        self.db.lots().delete_certificate(certificate_id)
    }

    // ========================================================================
    // Locations
    // ========================================================================

    /// Get lot quantities by location.
    pub fn get_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        self.db.lots().get_lot_locations(lot_id)
    }

    /// Get quantity at a specific location.
    pub fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<Decimal>> {
        self.db.lots().get_quantity_at_location(lot_id, location_id)
    }

    // ========================================================================
    // Genealogy
    // ========================================================================

    /// The lots this lot was derived from: the parent of a `split`, or every
    /// source of a `merge`. Empty for a lot created by a receipt.
    ///
    /// A merged lot can only carry one supplier / work order / purchase order
    /// on its own row, so this is how you recover the rest.
    pub fn get_parents(&self, lot_id: Uuid) -> Result<Vec<LotGenealogyLink>> {
        self.db.lots().get_lot_parents(lot_id)
    }

    /// The lots derived from this lot: split children, and the merge target it
    /// was consumed into.
    pub fn get_children(&self, lot_id: Uuid) -> Result<Vec<LotGenealogyLink>> {
        self.db.lots().get_lot_children(lot_id)
    }

    // ========================================================================
    // Transactions
    // ========================================================================

    /// Get transaction history for a lot.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let transactions = commerce.lots().get_transactions(
    ///     Uuid::new_v4(),
    ///     100,  // limit
    /// )?;
    ///
    /// for tx in transactions {
    ///     println!("{:?}: {} units", tx.transaction_type, tx.quantity);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        self.db.lots().get_transactions(lot_id, limit)
    }

    // ========================================================================
    // Traceability
    // ========================================================================

    /// Get full bidirectional traceability (upstream and downstream).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let trace = commerce.lots().trace(Uuid::new_v4())?;
    ///
    /// println!("Upstream sources: {} nodes", trace.upstream.len());
    /// println!("Downstream destinations: {} nodes", trace.downstream.len());
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        self.db.lots().trace(lot_id)
    }

    // ========================================================================
    // Queries
    // ========================================================================

    /// Get lots expiring within a number of days.
    pub fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        self.db.lots().get_expiring_lots(days)
    }

    /// Get already expired lots.
    pub fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        self.db.lots().get_expired_lots()
    }

    /// Sweep `Active` lots whose `expiration_date` has passed into `Expired`,
    /// returning how many were flipped. Idempotent — run it from a scheduler.
    ///
    /// Consumption paths (`consume`, `reserve`, `confirm_reservation`, FEFO
    /// picking) refuse expired lots regardless of whether this has run; the
    /// sweeper only makes the status column agree with the calendar.
    pub fn expire_lots(&self) -> Result<u64> {
        self.db.lots().expire_lots(chrono::Utc::now())
    }

    /// Sweep lot reservations that expired before `now` without being
    /// confirmed or released, handing their units back to the lot (and the
    /// linked inventory balance). Returns how many were released. Idempotent;
    /// `reserve` / `confirm_reservation` also expire stale reservations lazily
    /// on the lot they touch, so this only has to catch lots nobody touches.
    /// Schedule it together with [`Self::expire_lots`] and
    /// `Serials::release_expired_reservations` (e.g. via
    /// `stateset_jobs::TraceabilitySweepJob`).
    pub fn release_expired_reservations(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64> {
        self.db.lots().release_expired_reservations(now)
    }

    /// Get lots with available quantity for a SKU.
    ///
    /// Returns lots in FEFO order (soonest `expiration_date` first, unexpiring
    /// lots last, oldest first within a tie). Expired, non-active and fully
    /// reserved/quarantined lots are excluded.
    pub fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        self.db.lots().get_available_lots_for_sku(sku)
    }

    /// Count lots matching filter.
    pub fn count(&self, filter: LotFilter) -> Result<u64> {
        self.db.lots().count(filter)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple lots at once.
    pub fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<stateset_core::BatchResult<Lot>> {
        self.db.lots().create_batch(inputs)
    }

    /// Get multiple lots by ID.
    pub fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        self.db.lots().get_batch(ids)
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Get all active lots for a SKU.
    pub fn get_active_lots(&self, sku: &str) -> Result<Vec<Lot>> {
        self.list(LotFilter {
            sku: Some(sku.to_string()),
            status: Some(LotStatus::Active),
            ..Default::default()
        })
    }

    /// Get all quarantined lots.
    pub fn get_quarantined(&self) -> Result<Vec<Lot>> {
        self.list(LotFilter { status: Some(LotStatus::Quarantine), ..Default::default() })
    }
}
