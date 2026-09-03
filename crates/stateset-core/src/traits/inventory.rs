//! Inventory, lot, serial-number, and stock-snapshot repositories.

use super::*;

/// Inventory repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait InventoryRepository: Send + Sync {
    /// Create a new inventory item
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem>;

    /// Get inventory item by ID
    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>>;

    /// Get inventory item by SKU
    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>>;

    /// Get stock level for SKU (aggregated across locations)
    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>>;

    /// Get balance at specific location
    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>>;

    /// Adjust inventory quantity
    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction>;

    /// Reserve inventory
    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation>;

    /// Get a reservation by ID
    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<InventoryReservation>>;

    /// Release reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm reservation (convert to allocation)
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// List reservations by reference (e.g., order id)
    fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>>;

    /// List inventory items with filter
    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>>;

    /// Get items below reorder point
    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>>;

    /// Record transaction
    fn record_transaction(&self, transaction: InventoryTransaction)
    -> Result<InventoryTransaction>;

    /// Get transaction history
    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>>;

    // === Batch Operations ===

    /// Create multiple inventory items - partial success allowed
    fn create_item_batch(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<BatchResult<InventoryItem>>;

    /// Create multiple inventory items - atomic (all-or-nothing)
    fn create_item_batch_atomic(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<Vec<InventoryItem>>;

    /// Adjust multiple inventory quantities - partial success allowed
    fn adjust_batch(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<BatchResult<InventoryTransaction>>;

    /// Adjust multiple inventory quantities - atomic (all-or-nothing)
    fn adjust_batch_atomic(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<Vec<InventoryTransaction>>;

    /// Get multiple inventory items by ID
    fn get_item_batch(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>>;

    /// Get stock levels for multiple SKUs
    fn get_stock_batch(&self, skus: Vec<String>) -> Result<Vec<StockLevel>>;
}

// ============================================================================
// Lot Repository
// ============================================================================

/// Lot/Batch tracking repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait LotRepository: Send + Sync {
    /// Create a new lot
    fn create(&self, input: CreateLot) -> Result<Lot>;

    /// Get lot by ID
    fn get(&self, id: Uuid) -> Result<Option<Lot>>;

    /// Get lot by lot number
    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>>;

    /// Update a lot
    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot>;

    /// List lots with filter
    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>>;

    /// Delete a lot (only if no transactions)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Adjust lot quantity
    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction>;

    /// Consume from a lot
    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction>;

    /// Reserve quantity from a lot
    fn reserve(&self, input: ReserveLot) -> Result<Uuid>;

    /// Release a reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm a reservation (convert to consumption)
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction>;

    /// Transfer lot between locations
    fn transfer(&self, input: TransferLot) -> Result<LotTransaction>;

    /// Split a lot into two
    fn split(&self, input: SplitLot) -> Result<Lot>;

    /// Merge multiple lots into one
    fn merge(&self, input: MergeLots) -> Result<Lot>;

    /// Quarantine a lot
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot>;

    /// Release from quarantine
    fn release_quarantine(&self, id: Uuid) -> Result<Lot>;

    /// Get lot transactions
    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>>;

    /// Get lot quantity at a location (None if no location record exists)
    fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<rust_decimal::Decimal>>;

    /// Get all locations for a lot
    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>>;

    // Certificate operations
    /// Add certificate to lot
    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate>;

    /// Get certificates for lot
    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>>;

    /// Delete certificate
    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()>;

    // Queries
    /// Get expiring lots
    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>>;

    /// Get expired lots
    fn get_expired_lots(&self) -> Result<Vec<Lot>>;

    /// Sweep `Active` lots whose `expiration_date` is before `now` into
    /// `Expired`, returning how many were flipped. Idempotent; each call is a
    /// single status-conditional UPDATE. Consumption paths refuse expired lots
    /// regardless of whether this sweeper has run.
    fn expire_lots(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64>;

    /// Sweep lot reservations that expired before `now` without being
    /// confirmed or released: close each one and hand its units back to the
    /// lot (and to the linked inventory balance). Returns the number of
    /// reservations released. Idempotent; `reserve` and `confirm_reservation`
    /// also expire stale reservations lazily on the lot they touch, so the
    /// sweeper only has to catch lots nobody is looking at.
    fn release_expired_reservations(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64>;

    /// Get lots with available quantity for SKU
    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>>;

    /// Trace lot (upstream and downstream)
    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult>;

    /// Count lots
    fn count(&self, filter: LotFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple lots
    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>>;

    /// Get multiple lots by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>>;
}

// ============================================================================
// Serial Number Repository
// ============================================================================

/// Serial number management repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait SerialRepository: Send + Sync {
    /// Create a serial number
    fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber>;

    /// Create multiple serial numbers in bulk
    fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>>;

    /// Get serial by ID
    fn get(&self, id: Uuid) -> Result<Option<SerialNumber>>;

    /// Get serial by serial number string
    fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>>;

    /// Update a serial number
    fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber>;

    /// List serials with filter
    fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>>;

    /// Delete a serial (only if never used)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Change serial status with history tracking
    fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber>;

    /// Reserve a serial
    fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation>;

    /// Release a reservation, returning the serial to `Available`.
    ///
    /// Allowed while the reservation still holds the unit — i.e. the serial is
    /// `Reserved` — **including after `confirm_reservation`**: confirmation is
    /// a commitment on the row, not a movement of the unit, so an order
    /// cancelled after confirmation but before `mark_shipped` / `mark_sold`
    /// can still hand the serial back. Once the unit has shipped or sold the
    /// reservation is consumed and release is refused with `Conflict`.
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm reservation
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Get a reservation by id
    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<SerialReservation>>;

    /// Sweep reservations that expired before `now` without being confirmed,
    /// released or consumed: close each one and return its serial to
    /// `Available`. Returns the number of serials returned to stock.
    fn release_expired_reservations(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64>;

    /// Move serial to new location
    fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber>;

    /// Transfer ownership
    fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber>;

    /// Mark as sold
    fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber>;

    /// Mark as shipped
    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber>;

    /// Mark as returned
    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber>;

    /// Activate serial (e.g., for warranty)
    fn activate(&self, id: Uuid) -> Result<SerialNumber>;

    /// Quarantine serial
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber>;

    /// Release from quarantine
    fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber>;

    /// Quarantine every `Available`/`Reserved` serial in a lot (open
    /// reservations are closed). Serials in other statuses are left alone.
    /// Returns the number of serials quarantined.
    fn quarantine_for_lot(&self, lot_id: Uuid, reason: &str) -> Result<u64>;

    /// Return every `Quarantined` serial in a lot to `Available`. Returns the
    /// number of serials released.
    fn release_quarantine_for_lot(&self, lot_id: Uuid) -> Result<u64>;

    /// Scrap serial
    fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber>;

    // History operations
    /// Get serial history
    fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>>;

    /// Get full serial lookup with related data
    fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>>;

    /// Validate serial number
    fn validate(&self, serial: &str) -> Result<SerialValidation>;

    // Queries
    /// Get available serials for SKU
    fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>>;

    /// Get serials for lot
    fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>>;

    /// Get serials for customer
    fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>>;

    /// Count serials
    fn count(&self, filter: SerialFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple serials
    fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> Result<BatchResult<SerialNumber>>;

    /// Get multiple serials by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>>;

    /// Get multiple serials by serial string
    fn get_batch_by_serial(&self, serials: Vec<String>) -> Result<Vec<SerialNumber>>;
}

/// Stock snapshot repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StockSnapshotRepository: Send + Sync {
    /// Capture a new stock snapshot (totals are computed from the lines).
    fn capture(&self, input: CaptureStockSnapshot) -> Result<StockSnapshot>;

    /// Get a snapshot by ID (with lines).
    fn get(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>>;

    /// Get the most recent snapshot, if any.
    fn latest(&self) -> Result<Option<StockSnapshot>>;

    /// List snapshots (header-level, most recent first).
    fn list(&self, filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>>;

    /// Delete a snapshot.
    fn delete(&self, id: StockSnapshotId) -> Result<()>;
}
