//! Warehouse, receiving, transfer-order, and inbound-shipment repositories.

use super::*;

// ============================================================================
// Warehouse Repository
// ============================================================================

/// Warehouse management repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait WarehouseRepository: Send + Sync {
    // Warehouse operations
    /// Create a new warehouse
    fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse>;

    /// Get warehouse by ID
    fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>>;

    /// Get warehouse by code
    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>>;

    /// Update a warehouse
    fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse>;

    /// List warehouses with filter
    fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>>;

    /// Delete a warehouse (only if empty)
    fn delete_warehouse(&self, id: i32) -> Result<()>;

    /// Count warehouses
    fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64>;

    // Zone operations
    /// Create a zone
    fn create_zone(&self, input: CreateZone) -> Result<Zone>;

    /// Get zone by ID
    fn get_zone(&self, id: i32) -> Result<Option<Zone>>;

    /// Get zones for warehouse
    fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>>;

    /// Update a zone
    fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone>;

    /// Delete a zone
    fn delete_zone(&self, id: i32) -> Result<()>;

    // Location operations
    /// Create a location
    fn create_location(&self, input: CreateLocation) -> Result<Location>;

    /// Get location by ID
    fn get_location(&self, id: i32) -> Result<Option<Location>>;

    /// Get location by code
    fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>>;

    /// Update a location
    fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location>;

    /// List locations with filter
    fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>>;

    /// Delete a location (only if empty)
    fn delete_location(&self, id: i32) -> Result<()>;

    /// Count locations
    fn count_locations(&self, filter: LocationFilter) -> Result<u64>;

    /// Get locations for warehouse
    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>>;

    /// Get pickable locations for SKU
    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>>;

    /// Get receivable locations
    fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>>;

    // Location inventory operations
    /// Get inventory at location
    fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>>;

    /// Get inventory for SKU across locations
    fn get_inventory_for_sku(&self, warehouse_id: i32, sku: &str)
    -> Result<Vec<LocationInventory>>;

    /// Adjust inventory at location
    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory>;

    /// Move inventory between locations
    fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement>;

    /// Get location inventory by filter
    fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>>;

    // Movement operations
    /// Get inventory movements
    fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>>;

    /// Count movements
    fn count_movements(&self, filter: MovementFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple locations
    fn create_locations_batch(&self, inputs: Vec<CreateLocation>) -> Result<BatchResult<Location>>;

    /// Get multiple locations by ID
    fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>>;

    // Cycle count operations
    /// Create a cycle count (draft) with its expected lines
    fn create_cycle_count(&self, input: CreateCycleCount) -> Result<CycleCount>;

    /// Get a cycle count (with lines) by ID
    fn get_cycle_count(&self, id: Uuid) -> Result<Option<CycleCount>>;

    /// List cycle counts (with lines) matching the filter
    fn list_cycle_counts(&self, filter: CycleCountFilter) -> Result<Vec<CycleCount>>;

    /// Start a draft cycle count (draft → `in_progress`)
    fn start_cycle_count(&self, id: Uuid) -> Result<CycleCount>;

    /// Record physical counts against an in-progress cycle count
    fn record_cycle_counts(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount>;

    /// Complete an in-progress cycle count: computes variances and applies
    /// inventory adjustments (recorded as `cycle_count` movements)
    fn complete_cycle_count(&self, id: Uuid) -> Result<CycleCount>;

    /// Cancel a draft or in-progress cycle count
    fn cancel_cycle_count(&self, id: Uuid) -> Result<CycleCount>;
}

// ============================================================================
// Receiving Repository
// ============================================================================

/// Receiving/Goods receipt repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ReceivingRepository: Send + Sync {
    // Receipt operations
    /// Create a new receipt
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt>;

    /// Get receipt by ID
    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>>;

    /// Get receipt by receipt number
    fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>>;

    /// Update a receipt
    fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt>;

    /// List receipts with filter
    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>>;

    /// Delete a receipt (only if not started)
    fn delete_receipt(&self, id: Uuid) -> Result<()>;

    /// Start receiving (transition to `in_progress`)
    fn start_receiving(&self, id: Uuid) -> Result<Receipt>;

    /// Receive items on a receipt
    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt>;

    /// Complete receiving (all items received)
    fn complete_receiving(&self, id: Uuid) -> Result<Receipt>;

    /// Cancel a receipt
    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt>;

    /// Get receipt items
    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>>;

    /// Count receipts
    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64>;

    // Put-away operations
    /// Create a put-away task
    fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway>;

    /// Get put-away by ID
    fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>>;

    /// List put-aways with filter
    fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>>;

    /// Assign put-away to user
    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway>;

    /// Start put-away
    fn start_put_away(&self, id: Uuid) -> Result<PutAway>;

    /// Complete put-away
    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway>;

    /// Cancel put-away
    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway>;

    /// Get pending put-aways for receipt
    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>>;

    /// Count put-aways
    fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64>;

    // Integration with PO
    /// Create receipt from purchase order
    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt>;

    // Batch operations
    /// Create multiple receipts
    fn create_receipts_batch(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>>;

    /// Get multiple receipts by ID
    fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>>;
}

/// Transfer order repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransferOrderRepository: Send + Sync {
    /// Create a new transfer order.
    fn create(&self, input: CreateTransferOrder) -> Result<TransferOrder>;

    /// Get a transfer order by ID (with line items).
    fn get(&self, id: TransferOrderId) -> Result<Option<TransferOrder>>;

    /// List transfer orders with filter.
    fn list(&self, filter: TransferOrderFilter) -> Result<Vec<TransferOrder>>;

    /// Mark a transfer order as shipped from the source.
    fn ship(&self, id: TransferOrderId) -> Result<TransferOrder>;

    /// Receive quantities at the destination for a single line.
    fn receive_line(
        &self,
        id: TransferOrderId,
        item_id: TransferOrderItemId,
        quantity: rust_decimal::Decimal,
    ) -> Result<TransferOrder>;

    /// Cancel a transfer order.
    fn cancel(&self, id: TransferOrderId) -> Result<TransferOrder>;
}

/// Inbound shipment (ASN) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait InboundShipmentRepository: Send + Sync {
    /// Create a new inbound shipment.
    fn create(&self, input: CreateInboundShipment) -> Result<InboundShipment>;

    /// Get an inbound shipment by ID (with line items).
    fn get(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>>;

    /// List inbound shipments with filter.
    fn list(&self, filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>>;

    /// Mark a shipment as in transit.
    fn mark_in_transit(&self, id: InboundShipmentId) -> Result<InboundShipment>;

    /// Mark a shipment as arrived at the warehouse.
    fn mark_arrived(&self, id: InboundShipmentId) -> Result<InboundShipment>;

    /// Receive a quantity against a single line, advancing the shipment status.
    fn receive_line(
        &self,
        id: InboundShipmentId,
        item_id: InboundShipmentItemId,
        quantity: rust_decimal::Decimal,
    ) -> Result<InboundShipment>;

    /// Cancel an inbound shipment.
    fn cancel(&self, id: InboundShipmentId) -> Result<InboundShipment>;
}
