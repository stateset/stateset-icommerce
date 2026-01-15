//! Serial Number management operations
//!
//! Comprehensive serial number tracking supporting:
//! - Individual unit tracking via unique serial numbers
//! - Full lifecycle management (production to sale to return)
//! - Serial reservations and ownership transfers
//! - Complete audit trail of all serial events
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateSerialNumber};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a serial number for a high-value item
//! let serial = commerce.serials().create(CreateSerialNumber {
//!     serial: Some("SN-2025-ABC123".into()),
//!     sku: "LAPTOP-PRO-15".into(),
//!     ..Default::default()
//! })?;
//!
//! println!("Created serial {}", serial.serial);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    BatchResult, ChangeSerialStatus, CreateSerialNumber, CreateSerialNumbersBulk,
    MoveSerial, ReserveSerialNumber, Result, SerialFilter, SerialHistory,
    SerialHistoryFilter, SerialLookupResult, SerialNumber, SerialReservation,
    SerialValidation, TransferSerialOwnership, UpdateSerialNumber,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Serial number management interface.
pub struct Serials {
    db: Arc<dyn Database>,
}

impl Serials {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Basic CRUD
    // ========================================================================

    /// Create a serial number.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSerialNumber};
    /// use chrono::Utc;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let serial = commerce.serials().create(CreateSerialNumber {
    ///     serial: Some("SN-12345".into()),
    ///     sku: "WIDGET-001".into(),
    ///     lot_number: Some("LOT-2025-001".into()),
    ///     manufactured_at: Some(Utc::now()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().create(input)
    }

    /// Create multiple serial numbers in bulk.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSerialNumbersBulk};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Generate 100 serial numbers with prefix
    /// let serials = commerce.serials().create_bulk(CreateSerialNumbersBulk {
    ///     sku: "WIDGET-001".into(),
    ///     quantity: 100,
    ///     prefix: Some("WGT".into()),
    ///     lot_number: Some("LOT-2025-001".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created {} serial numbers", serials.len());
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>> {
        self.db.serials().create_bulk(input)
    }

    /// Get a serial by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        self.db.serials().get(id)
    }

    /// Get a serial by its serial number string.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// if let Some(serial) = commerce.serials().get_by_serial("SN-12345")? {
    ///     println!("Serial {} is currently {}", serial.serial, serial.status);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>> {
        self.db.serials().get_by_serial(serial)
    }

    /// List serials with optional filtering.
    pub fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        self.db.serials().list(filter)
    }

    /// Update a serial number.
    pub fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().update(id, input)
    }

    /// Delete a serial (only if never used).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.serials().delete(id)
    }

    // ========================================================================
    // Status Management
    // ========================================================================

    /// Change serial status with full tracking.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ChangeSerialStatus, SerialStatus};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.serials().change_status(ChangeSerialStatus {
    ///     serial_id: Uuid::new_v4(),
    ///     new_status: SerialStatus::InService,
    ///     reference_type: Some("repair_order".into()),
    ///     reference_id: Some(Uuid::new_v4()),
    ///     notes: Some("Sent for repair".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        self.db.serials().change_status(input)
    }

    /// Mark a serial as sold.
    pub fn mark_sold(&self, id: Uuid, customer_id: Uuid, order_id: Option<Uuid>) -> Result<SerialNumber> {
        self.db.serials().mark_sold(id, customer_id, order_id)
    }

    /// Mark a serial as shipped.
    pub fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_shipped(id, shipment_id)
    }

    /// Mark a serial as returned.
    pub fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_returned(id, return_id)
    }

    /// Activate a serial (e.g., for warranty start).
    pub fn activate(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().activate(id)
    }

    /// Quarantine a serial.
    pub fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().quarantine(id, reason)
    }

    /// Release a serial from quarantine.
    pub fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().release_quarantine(id)
    }

    /// Scrap a serial.
    pub fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().scrap(id, reason)
    }

    // ========================================================================
    // Reservations
    // ========================================================================

    /// Reserve a serial for an order or other purpose.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ReserveSerialNumber};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let reservation = commerce.serials().reserve(ReserveSerialNumber {
    ///     serial_id: Uuid::new_v4(),
    ///     reference_type: "order".into(),
    ///     reference_id: Uuid::new_v4(),
    ///     reserved_by: Some("sales_user".into()),
    ///     expires_in_seconds: Some(3600), // 1 hour
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Reservation created, expires at {:?}", reservation.expires_at);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        self.db.serials().reserve(input)
    }

    /// Release a reservation.
    pub fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().release_reservation(reservation_id)
    }

    /// Confirm a reservation (finalize the allocation).
    pub fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().confirm_reservation(reservation_id)
    }

    // ========================================================================
    // Location & Ownership
    // ========================================================================

    /// Move a serial to a new location.
    pub fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber> {
        self.db.serials().move_serial(input)
    }

    /// Transfer ownership of a serial.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, TransferSerialOwnership};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.serials().transfer_ownership(TransferSerialOwnership {
    ///     serial_id: Uuid::new_v4(),
    ///     new_owner_id: Uuid::new_v4(),
    ///     new_owner_type: "customer".into(),
    ///     notes: Some("Warranty transfer requested".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber> {
        self.db.serials().transfer_ownership(input)
    }

    // ========================================================================
    // History & Lookup
    // ========================================================================

    /// Get serial history.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SerialHistoryFilter};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let history = commerce.serials().get_history(
    ///     Uuid::new_v4(),
    ///     SerialHistoryFilter {
    ///         limit: Some(50),
    ///         ..Default::default()
    ///     },
    /// )?;
    ///
    /// for event in history {
    ///     println!("{}: {} -> {}", event.event_type, event.from_status, event.to_status);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_history(&self, serial_id: Uuid, filter: SerialHistoryFilter) -> Result<Vec<SerialHistory>> {
        self.db.serials().get_history(serial_id, filter)
    }

    /// Full serial lookup with related data.
    ///
    /// Returns the serial along with lot info, warranty status, and recent history.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// if let Some(result) = commerce.serials().lookup("SN-12345")? {
    ///     println!("Serial: {}", result.serial.serial);
    ///     println!("SKU: {}", result.serial.sku);
    ///     println!("Status: {}", result.serial.status);
    ///     if let Some(warranty) = result.warranty_status {
    ///         println!("Warranty active: {}", warranty.is_active);
    ///     }
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        self.db.serials().lookup(serial)
    }

    /// Validate a serial number.
    ///
    /// Returns validation info without the full serial data.
    pub fn validate(&self, serial: &str) -> Result<SerialValidation> {
        self.db.serials().validate(serial)
    }

    // ========================================================================
    // Queries
    // ========================================================================

    /// Get available serials for a SKU.
    pub fn get_available(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_available_for_sku(sku, limit)
    }

    /// Get serials for a lot.
    pub fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_lot(lot_id)
    }

    /// Get serials owned by a customer.
    pub fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_customer(customer_id)
    }

    /// Count serials matching filter.
    pub fn count(&self, filter: SerialFilter) -> Result<u64> {
        self.db.serials().count(filter)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple serials with partial success handling.
    pub fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> Result<BatchResult<SerialNumber>> {
        self.db.serials().create_batch(inputs)
    }

    /// Get multiple serials by ID.
    pub fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_batch(ids)
    }

    /// Get multiple serials by serial string.
    pub fn get_batch_by_serial(&self, serials: Vec<String>) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_batch_by_serial(serials)
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Check if a serial is available for sale.
    pub fn is_available(&self, serial: &str) -> Result<bool> {
        if let Some(s) = self.get_by_serial(serial)? {
            Ok(s.is_available())
        } else {
            Ok(false)
        }
    }

    /// Check if a serial can be shipped.
    pub fn can_ship(&self, serial: &str) -> Result<bool> {
        if let Some(s) = self.get_by_serial(serial)? {
            Ok(s.can_ship())
        } else {
            Ok(false)
        }
    }
}
