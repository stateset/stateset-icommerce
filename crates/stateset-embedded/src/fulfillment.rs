//! Fulfillment (Pick/Pack/Ship) operations
//!
//! Comprehensive outbound warehouse operations supporting:
//! - Wave planning for efficient picking
//! - Pick task management
//! - Pack/carton management
//! - Ship task and handoff
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateWave, PickTaskFilter, PickStatus};
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a wave from orders
//! let wave = commerce.fulfillment().create_wave(CreateWave {
//!     warehouse_id: 1,
//!     order_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
//!     priority: Some(1),
//!     ..Default::default()
//! })?;
//!
//! // Release wave for picking
//! let wave = commerce.fulfillment().release_wave(wave.id)?;
//!
//! // Get pending picks for the wave
//! let picks = commerce.fulfillment().list_picks(PickTaskFilter {
//!     wave_id: Some(wave.id),
//!     status: Some(PickStatus::Pending),
//!     ..Default::default()
//! })?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    AddCarton, AddCartonItem, BatchResult, Carton, CartonItem, CompletePick, CompleteShip,
    CreatePackTask, CreatePickTask, CreateShipTask, CreateWave, FulfillmentRepository,
    PackTask, PackTaskFilter, PickTask, PickTaskFilter, Result,
    ShipTask, ShipTaskFilter, Wave, WaveFilter,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Fulfillment (pick/pack/ship) management interface.
pub struct Fulfillment {
    db: Arc<dyn Database>,
}

impl Fulfillment {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Wave Operations
    // ========================================================================

    /// Create a wave grouping multiple orders for efficient picking.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWave};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let wave = commerce.fulfillment().create_wave(CreateWave {
    ///     warehouse_id: 1,
    ///     order_ids: vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
    ///     priority: Some(1),
    ///     notes: Some("Priority batch for same-day shipping".into()),
    ///     created_by: Some("warehouse_manager".into()),
    /// })?;
    ///
    /// println!("Created wave {} with {} orders", wave.wave_number, wave.order_count);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        self.db.fulfillment().create_wave(input)
    }

    /// Get a wave by ID.
    pub fn get_wave(&self, id: Uuid) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave(id)
    }

    /// Get a wave by wave number.
    pub fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave_by_number(number)
    }

    /// List waves with optional filtering.
    pub fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        self.db.fulfillment().list_waves(filter)
    }

    /// Release a wave for picking (draft -> released).
    ///
    /// Once released, pick tasks become available for warehouse workers.
    pub fn release_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().release_wave(id)
    }

    /// Complete a wave (all picks finished).
    pub fn complete_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().complete_wave(id)
    }

    /// Cancel a wave.
    pub fn cancel_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().cancel_wave(id)
    }

    /// Get order IDs in a wave.
    pub fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        self.db.fulfillment().get_wave_orders(wave_id)
    }

    /// Count waves matching the filter.
    pub fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        self.db.fulfillment().count_waves(filter)
    }

    // ========================================================================
    // Pick Operations
    // ========================================================================

    /// Create a pick task.
    pub fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        self.db.fulfillment().create_pick(input)
    }

    /// Get a pick task by ID.
    pub fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        self.db.fulfillment().get_pick(id)
    }

    /// List pick tasks with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, PickTaskFilter, PickStatus};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all pending picks for a warehouse
    /// let picks = commerce.fulfillment().list_picks(PickTaskFilter {
    ///     warehouse_id: Some(1),
    ///     status: Some(PickStatus::Pending),
    ///     limit: Some(50),
    ///     ..Default::default()
    /// })?;
    ///
    /// for pick in picks {
    ///     println!("Pick {} of {} from location {}",
    ///         pick.quantity_requested, pick.sku, pick.source_location_id);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        self.db.fulfillment().list_picks(filter)
    }

    /// Assign a pick task to a user.
    pub fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        self.db.fulfillment().assign_pick(id, assigned_to)
    }

    /// Start a pick task.
    pub fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().start_pick(id)
    }

    /// Complete a pick task.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CompletePick};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Complete a pick with full quantity
    /// let pick = commerce.fulfillment().complete_pick(CompletePick {
    ///     pick_id: Uuid::new_v4(),
    ///     quantity_picked: dec!(10),
    ///     quantity_short: None,
    ///     short_reason: None,
    ///     lot_id: None,
    ///     serial_number: None,
    ///     completed_by: Some("picker_01".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        self.db.fulfillment().complete_pick(input)
    }

    /// Report a short pick (less than requested quantity available).
    pub fn report_short(&self, id: Uuid, short_qty: Decimal, reason: &str) -> Result<PickTask> {
        self.db.fulfillment().report_short(id, short_qty, reason)
    }

    /// Cancel a pick task.
    pub fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().cancel_pick(id)
    }

    /// Get all picks for an order.
    pub fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_order(order_id)
    }

    /// Get all picks in a wave.
    pub fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_wave(wave_id)
    }

    /// Count pick tasks matching the filter.
    pub fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_picks(filter)
    }

    // ========================================================================
    // Pack Operations
    // ========================================================================

    /// Create a pack task for an order.
    pub fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        self.db.fulfillment().create_pack(input)
    }

    /// Get a pack task by ID.
    pub fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        self.db.fulfillment().get_pack(id)
    }

    /// List pack tasks with optional filtering.
    pub fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        self.db.fulfillment().list_packs(filter)
    }

    /// Assign a pack task to a user.
    pub fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        self.db.fulfillment().assign_pack(id, assigned_to)
    }

    /// Start packing.
    pub fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().start_pack(id)
    }

    /// Complete packing.
    pub fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().complete_pack(id)
    }

    /// Add a carton to a pack task.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddCarton, PackageType};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let carton = commerce.fulfillment().add_carton(AddCarton {
    ///     pack_task_id: Uuid::new_v4(),
    ///     package_type: PackageType::Box,
    ///     weight_kg: Some(dec!(2.5)),
    ///     length_cm: Some(dec!(30)),
    ///     width_cm: Some(dec!(20)),
    ///     height_cm: Some(dec!(15)),
    /// })?;
    ///
    /// println!("Created carton {}", carton.carton_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        self.db.fulfillment().add_carton(input)
    }

    /// Add an item to a carton.
    pub fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        self.db.fulfillment().add_carton_item(input)
    }

    /// Get all cartons for a pack task.
    pub fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        self.db.fulfillment().get_cartons(pack_task_id)
    }

    /// Get items in a carton.
    pub fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        self.db.fulfillment().get_carton_items(carton_id)
    }

    /// Mark a carton's label as printed.
    pub fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        self.db.fulfillment().mark_label_printed(carton_id)
    }

    /// Cancel a pack task.
    pub fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().cancel_pack(id)
    }

    /// Count pack tasks matching the filter.
    pub fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_packs(filter)
    }

    // ========================================================================
    // Ship Operations
    // ========================================================================

    /// Create a ship task.
    pub fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        self.db.fulfillment().create_ship(input)
    }

    /// Get a ship task by ID.
    pub fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        self.db.fulfillment().get_ship(id)
    }

    /// List ship tasks with optional filtering.
    pub fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        self.db.fulfillment().list_ships(filter)
    }

    /// Assign a ship task to a user.
    pub fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        self.db.fulfillment().assign_ship(id, assigned_to)
    }

    /// Record label printing for a ship task.
    pub fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        self.db.fulfillment().print_label(id, label_url)
    }

    /// Complete shipping with tracking number.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CompleteShip};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let ship = commerce.fulfillment().complete_ship(CompleteShip {
    ///     ship_task_id: Uuid::new_v4(),
    ///     tracking_number: "1Z999AA10123456784".into(),
    ///     shipping_cost: Some(dec!(12.50)),
    ///     shipped_by: Some("shipping_clerk".into()),
    /// })?;
    ///
    /// println!("Shipped with tracking: {}", ship.tracking_number.unwrap());
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        self.db.fulfillment().complete_ship(input)
    }

    /// Cancel a ship task.
    pub fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        self.db.fulfillment().cancel_ship(id)
    }

    /// Count ship tasks matching the filter.
    pub fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_ships(filter)
    }

    // ========================================================================
    // Workflow Helpers
    // ========================================================================

    /// Create pick tasks for all items in an order.
    ///
    /// Automatically finds pickable locations for each item.
    pub fn create_picks_for_order(&self, order_id: Uuid, warehouse_id: i32) -> Result<Vec<PickTask>> {
        self.db.fulfillment().create_picks_for_order(order_id, warehouse_id)
    }

    /// Check if all picks for an order are complete (ready to pack).
    pub fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_pack(order_id)
    }

    /// Check if packing is complete for an order (ready to ship).
    pub fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_ship(order_id)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple waves in a batch.
    pub fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        self.db.fulfillment().create_waves_batch(inputs)
    }

    /// Get multiple picks by ID.
    pub fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_batch(ids)
    }
}
