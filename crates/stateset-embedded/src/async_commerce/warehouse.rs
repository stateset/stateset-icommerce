//! Warehouse accessors: warehouses/locations, receiving, fulfillment.

use super::*;

/// Async warehouse operations.
pub struct AsyncWarehouse {
    db: Arc<PostgresDatabase>,
}

impl AsyncWarehouse {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().create_warehouse_async(input).await
    }

    pub async fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse_async(id).await
    }

    pub async fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse_by_code_async(code).await
    }

    pub async fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().update_warehouse_async(id, input).await
    }

    pub async fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        self.db.warehouse().list_warehouses_async(filter).await
    }

    pub async fn delete_warehouse(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_warehouse_async(id).await
    }

    pub async fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        self.db.warehouse().count_warehouses_async(filter).await
    }

    pub async fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        self.db.warehouse().create_zone_async(input).await
    }

    pub async fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        self.db.warehouse().get_zone_async(id).await
    }

    pub async fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        self.db.warehouse().get_zones_async(warehouse_id).await
    }

    pub async fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        self.db.warehouse().update_zone_async(id, input).await
    }

    pub async fn delete_zone(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_zone_async(id).await
    }

    pub async fn create_location(&self, input: CreateLocation) -> Result<Location> {
        self.db.warehouse().create_location_async(input).await
    }

    pub async fn get_location(&self, id: i32) -> Result<Option<Location>> {
        self.db.warehouse().get_location_async(id).await
    }

    pub async fn get_location_by_code(
        &self,
        warehouse_id: i32,
        code: &str,
    ) -> Result<Option<Location>> {
        self.db.warehouse().get_location_by_code_async(warehouse_id, code).await
    }

    pub async fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        self.db.warehouse().update_location_async(id, input).await
    }

    pub async fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        self.db.warehouse().list_locations_async(filter).await
    }

    pub async fn delete_location(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_location_async(id).await
    }

    pub async fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        self.db.warehouse().count_locations_async(filter).await
    }

    pub async fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_for_warehouse_async(warehouse_id).await
    }

    pub async fn get_pickable_locations(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<Location>> {
        self.db.warehouse().get_pickable_locations_async(warehouse_id, sku).await
    }

    pub async fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_receivable_locations_async(warehouse_id).await
    }

    pub async fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_location_inventory_async(location_id).await
    }

    pub async fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_inventory_for_sku_async(warehouse_id, sku).await
    }

    pub async fn adjust_inventory(
        &self,
        input: AdjustLocationInventory,
    ) -> Result<LocationInventory> {
        self.db.warehouse().adjust_inventory_async(input).await
    }

    pub async fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        self.db.warehouse().move_inventory_async(input).await
    }

    pub async fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().list_location_inventory_async(filter).await
    }

    pub async fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        self.db.warehouse().get_movements_async(filter).await
    }

    pub async fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        self.db.warehouse().count_movements_async(filter).await
    }

    pub async fn create_locations_batch(
        &self,
        inputs: Vec<CreateLocation>,
    ) -> Result<BatchResult<Location>> {
        self.db.warehouse().create_locations_batch_async(inputs).await
    }

    pub async fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_batch_async(ids).await
    }

    /// Create a cycle count (draft) with its expected lines.
    pub async fn create_cycle_count(&self, input: CreateCycleCount) -> Result<CycleCount> {
        self.db.warehouse().create_cycle_count_async(input).await
    }

    /// Get a cycle count (with lines) by ID.
    pub async fn get_cycle_count(&self, id: Uuid) -> Result<Option<CycleCount>> {
        self.db.warehouse().get_cycle_count_async(id).await
    }

    /// List cycle counts matching the filter.
    pub async fn list_cycle_counts(&self, filter: CycleCountFilter) -> Result<Vec<CycleCount>> {
        self.db.warehouse().list_cycle_counts_async(filter).await
    }

    /// Start a draft cycle count (`draft` → `in_progress`).
    pub async fn start_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().start_cycle_count_async(id).await
    }

    /// Record physical counts against an in-progress cycle count.
    pub async fn record_cycle_counts(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount> {
        self.db.warehouse().record_cycle_counts_async(id, counts).await
    }

    /// Complete an in-progress cycle count, applying variance adjustments
    /// to `location_inventory`.
    pub async fn complete_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().complete_cycle_count_async(id).await
    }

    /// Cancel a draft or in-progress cycle count. No adjustments are applied.
    pub async fn cancel_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().cancel_cycle_count_async(id).await
    }
}

// ============================================================================
// Async Receiving
// ============================================================================

/// Async receiving operations.
pub struct AsyncReceiving {
    db: Arc<PostgresDatabase>,
}

impl AsyncReceiving {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        self.db.receiving().create_receipt_async(input).await
    }

    pub async fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt_async(id).await
    }

    pub async fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt_by_number_async(number).await
    }

    pub async fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        self.db.receiving().update_receipt_async(id, input).await
    }

    pub async fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        self.db.receiving().list_receipts_async(filter).await
    }

    pub async fn delete_receipt(&self, id: Uuid) -> Result<()> {
        self.db.receiving().delete_receipt_async(id).await
    }

    pub async fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().start_receiving_async(id).await
    }

    pub async fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        self.db.receiving().receive_items_async(input).await
    }

    pub async fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().complete_receiving_async(id).await
    }

    pub async fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().cancel_receipt_async(id).await
    }

    pub async fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        self.db.receiving().get_receipt_items_async(receipt_id).await
    }

    pub async fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        self.db.receiving().count_receipts_async(filter).await
    }

    pub async fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        self.db.receiving().create_put_away_async(input).await
    }

    pub async fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        self.db.receiving().get_put_away_async(id).await
    }

    pub async fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        self.db.receiving().list_put_aways_async(filter).await
    }

    pub async fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        self.db.receiving().assign_put_away_async(id, assigned_to).await
    }

    pub async fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().start_put_away_async(id).await
    }

    pub async fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        self.db.receiving().complete_put_away_async(input).await
    }

    pub async fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().cancel_put_away_async(id).await
    }

    pub async fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.db.receiving().get_pending_put_aways_async(receipt_id).await
    }

    pub async fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        self.db.receiving().count_put_aways_async(filter).await
    }

    pub async fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        self.db.receiving().create_receipt_from_po_async(po_id, warehouse_id).await
    }

    pub async fn create_receipts_batch(
        &self,
        inputs: Vec<CreateReceipt>,
    ) -> Result<BatchResult<Receipt>> {
        self.db.receiving().create_receipts_batch_async(inputs).await
    }

    pub async fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        self.db.receiving().get_receipts_batch_async(ids).await
    }
}

// ============================================================================
// Async Fulfillment
// ============================================================================

/// Async fulfillment operations.
pub struct AsyncFulfillment {
    db: Arc<PostgresDatabase>,
}

impl AsyncFulfillment {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        self.db.fulfillment().create_wave_async(input).await
    }

    pub async fn get_wave(&self, id: Uuid) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave_async(id).await
    }

    pub async fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave_by_number_async(number).await
    }

    pub async fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        self.db.fulfillment().list_waves_async(filter).await
    }

    pub async fn release_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().release_wave_async(id).await
    }

    pub async fn complete_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().complete_wave_async(id).await
    }

    pub async fn cancel_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().cancel_wave_async(id).await
    }

    pub async fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        self.db.fulfillment().get_wave_orders_async(wave_id).await
    }

    pub async fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        self.db.fulfillment().count_waves_async(filter).await
    }

    pub async fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        self.db.fulfillment().create_pick_async(input).await
    }

    pub async fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        self.db.fulfillment().get_pick_async(id).await
    }

    pub async fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        self.db.fulfillment().list_picks_async(filter).await
    }

    pub async fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        self.db.fulfillment().assign_pick_async(id, assigned_to).await
    }

    pub async fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().start_pick_async(id).await
    }

    pub async fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        self.db.fulfillment().complete_pick_async(input).await
    }

    pub async fn report_short(
        &self,
        id: Uuid,
        short_qty: Decimal,
        reason: &str,
    ) -> Result<PickTask> {
        self.db.fulfillment().report_short_async(id, short_qty, reason).await
    }

    pub async fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().cancel_pick_async(id).await
    }

    pub async fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_order_async(order_id).await
    }

    pub async fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_wave_async(wave_id).await
    }

    pub async fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_picks_async(filter).await
    }

    pub async fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        self.db.fulfillment().create_pack_async(input).await
    }

    pub async fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        self.db.fulfillment().get_pack_async(id).await
    }

    pub async fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        self.db.fulfillment().list_packs_async(filter).await
    }

    pub async fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        self.db.fulfillment().assign_pack_async(id, assigned_to).await
    }

    pub async fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().start_pack_async(id).await
    }

    pub async fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().complete_pack_async(id).await
    }

    pub async fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        self.db.fulfillment().add_carton_async(input).await
    }

    pub async fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        self.db.fulfillment().add_carton_item_async(input).await
    }

    pub async fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        self.db.fulfillment().get_cartons_async(pack_task_id).await
    }

    pub async fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        self.db.fulfillment().get_carton_items_async(carton_id).await
    }

    pub async fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        self.db.fulfillment().mark_label_printed_async(carton_id).await
    }

    pub async fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().cancel_pack_async(id).await
    }

    pub async fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_packs_async(filter).await
    }

    pub async fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        self.db.fulfillment().create_ship_async(input).await
    }

    pub async fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        self.db.fulfillment().get_ship_async(id).await
    }

    pub async fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        self.db.fulfillment().list_ships_async(filter).await
    }

    pub async fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        self.db.fulfillment().assign_ship_async(id, assigned_to).await
    }

    pub async fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        self.db.fulfillment().print_label_async(id, label_url).await
    }

    pub async fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        self.db.fulfillment().complete_ship_async(input).await
    }

    pub async fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        self.db.fulfillment().cancel_ship_async(id).await
    }

    pub async fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_ships_async(filter).await
    }

    pub async fn create_picks_for_order(
        &self,
        order_id: Uuid,
        warehouse_id: i32,
    ) -> Result<Vec<PickTask>> {
        self.db.fulfillment().create_picks_for_order_async(order_id, warehouse_id).await
    }

    pub async fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_pack_async(order_id).await
    }

    pub async fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_ship_async(order_id).await
    }

    pub async fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        self.db.fulfillment().create_waves_batch_async(inputs).await
    }

    pub async fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_batch_async(ids).await
    }
}

// ============================================================================
// Async Accounts Payable
// ============================================================================
