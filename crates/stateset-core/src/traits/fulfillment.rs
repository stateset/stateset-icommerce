//! Shipment, fulfillment (pick/pack/ship/wave), shipping-zone, and print-station repositories.

use super::*;

/// Shipment repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ShipmentRepository: Send + Sync {
    /// Create a new shipment
    fn create(&self, input: CreateShipment) -> Result<Shipment>;

    /// Get shipment by ID
    fn get(&self, id: ShipmentId) -> Result<Option<Shipment>>;

    /// Get shipment by shipment number
    fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>>;

    /// Get shipment by tracking number
    fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>>;

    /// Update a shipment
    fn update(&self, id: ShipmentId, input: UpdateShipment) -> Result<Shipment>;

    /// List shipments with filter
    fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>>;

    /// Get shipments for an order
    fn for_order(&self, order_id: OrderId) -> Result<Vec<Shipment>>;

    /// Delete a shipment (cancel if not shipped)
    fn delete(&self, id: ShipmentId) -> Result<()>;

    // Status transitions
    /// Mark shipment as processing
    fn mark_processing(&self, id: ShipmentId) -> Result<Shipment>;

    /// Mark shipment as ready to ship
    fn mark_ready(&self, id: ShipmentId) -> Result<Shipment>;

    /// Mark shipment as shipped with tracking number
    fn ship(&self, id: ShipmentId, tracking_number: Option<String>) -> Result<Shipment>;

    /// Mark shipment as in transit
    fn mark_in_transit(&self, id: ShipmentId) -> Result<Shipment>;

    /// Mark shipment as out for delivery
    fn mark_out_for_delivery(&self, id: ShipmentId) -> Result<Shipment>;

    /// Mark shipment as delivered
    fn mark_delivered(&self, id: ShipmentId) -> Result<Shipment>;

    /// Mark shipment as failed delivery
    fn mark_failed(&self, id: ShipmentId) -> Result<Shipment>;

    /// Put shipment on hold
    fn hold(&self, id: ShipmentId) -> Result<Shipment>;

    /// Cancel shipment
    fn cancel(&self, id: ShipmentId) -> Result<Shipment>;

    // Item operations
    /// Add item to shipment
    fn add_item(&self, shipment_id: ShipmentId, item: CreateShipmentItem) -> Result<ShipmentItem>;

    /// Remove item from shipment
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items in shipment
    fn get_items(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentItem>>;

    // Event/tracking operations
    /// Add tracking event
    fn add_event(&self, shipment_id: ShipmentId, event: AddShipmentEvent) -> Result<ShipmentEvent>;

    /// Get tracking events for shipment
    fn get_events(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentEvent>>;

    /// Count shipments matching filter
    fn count(&self, filter: ShipmentFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple shipments - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>>;

    /// Create multiple shipments - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>>;

    /// Update multiple shipments - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(ShipmentId, UpdateShipment)>,
    ) -> Result<BatchResult<Shipment>>;

    /// Update multiple shipments - atomic (all-or-nothing)
    fn update_batch_atomic(
        &self,
        updates: Vec<(ShipmentId, UpdateShipment)>,
    ) -> Result<Vec<Shipment>>;

    /// Delete multiple shipments - partial success allowed
    fn delete_batch(&self, ids: Vec<ShipmentId>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple shipments - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<ShipmentId>) -> Result<()>;

    /// Get multiple shipments by ID
    fn get_batch(&self, ids: Vec<ShipmentId>) -> Result<Vec<Shipment>>;
}

// ============================================================================
// Fulfillment Repository
// ============================================================================

/// Fulfillment (pick/pack/ship) repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait FulfillmentRepository: Send + Sync {
    // Wave operations
    /// Create a wave from orders
    fn create_wave(&self, input: CreateWave) -> Result<Wave>;

    /// Get wave by ID
    fn get_wave(&self, id: FulfillmentId) -> Result<Option<Wave>>;

    /// Get wave by number
    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>>;

    /// List waves with filter
    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>>;

    /// Release wave for picking
    fn release_wave(&self, id: FulfillmentId) -> Result<Wave>;

    /// Complete a wave
    fn complete_wave(&self, id: FulfillmentId) -> Result<Wave>;

    /// Cancel a wave
    fn cancel_wave(&self, id: FulfillmentId) -> Result<Wave>;

    /// Get orders in a wave
    fn get_wave_orders(&self, wave_id: FulfillmentId) -> Result<Vec<OrderId>>;

    /// Count waves
    fn count_waves(&self, filter: WaveFilter) -> Result<u64>;

    // Pick operations
    /// Create a pick task
    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask>;

    /// Get pick task by ID
    fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>>;

    /// List pick tasks with filter
    fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>>;

    /// Assign pick to user
    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask>;

    /// Start a pick
    fn start_pick(&self, id: Uuid) -> Result<PickTask>;

    /// Complete a pick
    fn complete_pick(&self, input: CompletePick) -> Result<PickTask>;

    /// Report short pick
    fn report_short(
        &self,
        id: Uuid,
        short_qty: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<PickTask>;

    /// Cancel a pick
    fn cancel_pick(&self, id: Uuid) -> Result<PickTask>;

    /// Get picks for order
    fn get_picks_for_order(&self, order_id: OrderId) -> Result<Vec<PickTask>>;

    /// Get picks for wave
    fn get_picks_for_wave(&self, wave_id: FulfillmentId) -> Result<Vec<PickTask>>;

    /// Count picks
    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64>;

    // Pack operations
    /// Create a pack task
    fn create_pack(&self, input: CreatePackTask) -> Result<PackTask>;

    /// Get pack task by ID
    fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>>;

    /// List pack tasks with filter
    fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>>;

    /// Assign pack to user
    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask>;

    /// Start packing
    fn start_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Complete packing
    fn complete_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Add carton to pack task
    fn add_carton(&self, input: AddCarton) -> Result<Carton>;

    /// Add item to carton
    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem>;

    /// Get cartons for pack task
    fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>>;

    /// Get items in carton
    fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>>;

    /// Mark carton label printed
    fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton>;

    /// Cancel pack task
    fn cancel_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Count packs
    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64>;

    // Ship operations
    /// Create a ship task
    fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask>;

    /// Get ship task by ID
    fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>>;

    /// List ship tasks with filter
    fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>>;

    /// Assign ship to user
    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask>;

    /// Print shipping label
    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask>;

    /// Complete shipping
    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask>;

    /// Cancel ship task
    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask>;

    /// Count ships
    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64>;

    // Workflow helpers
    /// Create picks for an order
    fn create_picks_for_order(&self, order_id: OrderId, warehouse_id: i32)
    -> Result<Vec<PickTask>>;

    /// Check if order is ready to pack
    fn is_order_ready_to_pack(&self, order_id: OrderId) -> Result<bool>;

    /// Check if order is ready to ship
    fn is_order_ready_to_ship(&self, order_id: OrderId) -> Result<bool>;

    // Batch operations
    /// Create multiple waves
    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>>;

    /// Get multiple picks by ID
    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>>;
}

/// Shipping zone repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ShippingZoneRepository: Send + Sync {
    /// Create a new shipping zone
    fn create(&self, input: CreateShippingZone) -> Result<ShippingZone>;

    /// Get shipping zone by ID
    fn get(&self, id: ShippingZoneId) -> Result<Option<ShippingZone>>;

    /// Update a shipping zone
    fn update(&self, id: ShippingZoneId, input: UpdateShippingZone) -> Result<ShippingZone>;

    /// List shipping zones with filter
    fn list(&self, filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>>;

    /// Delete a shipping zone
    fn delete(&self, id: ShippingZoneId) -> Result<()>;

    /// Find zones matching a destination
    fn find_matching_zones(
        &self,
        country: &str,
        region: Option<&str>,
        postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>>;
}

/// Zone shipping method repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ZoneShippingMethodRepository: Send + Sync {
    /// Create a shipping method in a zone
    fn create(&self, input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod>;

    /// Get shipping method by ID
    fn get(&self, id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>>;

    /// List shipping methods with filter
    fn list(&self, filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>>;

    /// Delete a shipping method
    fn delete(&self, id: ShippingMethodId) -> Result<()>;

    /// Calculate rates for a destination
    fn calculate_rates(&self, request: ZoneShippingRateRequest) -> Result<Vec<ZoneShippingRate>>;
}

/// Print station / print job repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PrintStationRepository: Send + Sync {
    /// Pair a new print station, returning the station and its one-time token.
    fn pair(&self, input: CreatePrintStation) -> Result<PairStationResult>;

    /// List paired stations (most recently paired first).
    fn list_stations(&self) -> Result<Vec<PrintStation>>;

    /// Get a station by ID.
    fn get_station(&self, id: PrintStationId) -> Result<Option<PrintStation>>;

    /// Revoke a station's token.
    fn revoke_station(&self, id: PrintStationId) -> Result<PrintStation>;

    /// Enqueue a print job to a station. Errors if the station is revoked.
    fn enqueue_job(&self, station_id: PrintStationId, input: EnqueuePrintJob) -> Result<PrintJob>;

    /// Pick up the next queued job for a station (agent long-poll), marking it
    /// picked up and updating the station's last-seen time. Returns `None` when
    /// the queue is empty.
    fn next_job(&self, station_id: PrintStationId) -> Result<Option<PrintJob>>;

    /// Mark a job printed (`success = true`) or failed.
    fn complete_job(&self, job_id: PrintJobId, success: bool) -> Result<PrintJob>;

    /// List jobs for a station.
    fn list_jobs(
        &self,
        station_id: PrintStationId,
        filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>>;
}
