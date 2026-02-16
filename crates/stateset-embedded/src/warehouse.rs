//! Warehouse and Location management operations
//!
//! Comprehensive warehouse management system supporting:
//! - Warehouse definitions and configuration
//! - Location hierarchy (zones, aisles, racks, bins)
//! - Location inventory tracking
//! - Inventory movements and transfers
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateWarehouse, CreateLocation, WarehouseType, LocationType};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a warehouse
//! let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
//!     code: "WH-001".into(),
//!     name: "Main Distribution Center".into(),
//!     warehouse_type: WarehouseType::Distribution,
//!     ..Default::default()
//! })?;
//!
//! // Create a location within the warehouse
//! let location = commerce.warehouse().create_location(CreateLocation {
//!     warehouse_id: warehouse.id,
//!     location_type: LocationType::Pick,
//!     zone: Some("A".into()),
//!     aisle: Some("01".into()),
//!     rack: Some("02".into()),
//!     bin: Some("03".into()),
//!     ..Default::default()
//! })?;
//!
//! println!("Created location {} in warehouse {}", location.code, warehouse.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    AdjustLocationInventory, BatchResult, CreateLocation, CreateWarehouse, CreateZone, Location,
    LocationFilter, LocationInventory, LocationInventoryFilter, LocationMovement, MoveInventory,
    MovementFilter, Result, UpdateLocation, UpdateWarehouse, UpdateZone, Warehouse,
    WarehouseFilter, Zone,
};
use stateset_db::Database;
use std::sync::Arc;

/// Warehouse and Location management interface.
pub struct WarehouseOps {
    db: Arc<dyn Database>,
}

impl WarehouseOps {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Warehouse Operations
    // ========================================================================

    /// Create a new warehouse.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWarehouse, WarehouseType, WarehouseAddress};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
    ///     code: "DC-EAST".into(),
    ///     name: "East Coast Distribution Center".into(),
    ///     warehouse_type: WarehouseType::Distribution,
    ///     address: WarehouseAddress {
    ///         street1: "123 Logistics Way".into(),
    ///         city: "Newark".into(),
    ///         state: "NJ".into(),
    ///         postal_code: "07102".into(),
    ///         country: "US".into(),
    ///         ..Default::default()
    ///     },
    ///     timezone: Some("America/New_York".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().create_warehouse(input)
    }

    /// Get a warehouse by ID.
    pub fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse(id)
    }

    /// Get a warehouse by code.
    pub fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse_by_code(code)
    }

    /// Update a warehouse.
    pub fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().update_warehouse(id, input)
    }

    /// List warehouses with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, WarehouseFilter, WarehouseType};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all active distribution centers
    /// let warehouses = commerce.warehouse().list_warehouses(WarehouseFilter {
    ///     warehouse_type: Some(WarehouseType::Distribution),
    ///     is_active: Some(true),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        self.db.warehouse().list_warehouses(filter)
    }

    /// Delete a warehouse. Only warehouses without locations can be deleted.
    pub fn delete_warehouse(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_warehouse(id)
    }

    /// Count warehouses matching the filter.
    pub fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        self.db.warehouse().count_warehouses(filter)
    }

    // ========================================================================
    // Zone Operations
    // ========================================================================

    /// Create a new zone within a warehouse.
    ///
    /// Zones are logical groupings of locations (e.g., "Bulk Storage", "Pick Area", "Returns").
    pub fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        self.db.warehouse().create_zone(input)
    }

    /// Get a zone by ID.
    pub fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        self.db.warehouse().get_zone(id)
    }

    /// Get all zones in a warehouse.
    pub fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        self.db.warehouse().get_zones(warehouse_id)
    }

    /// Update a zone.
    pub fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        self.db.warehouse().update_zone(id, input)
    }

    /// Delete a zone.
    pub fn delete_zone(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_zone(id)
    }

    // ========================================================================
    // Location Operations
    // ========================================================================

    /// Create a new location within a warehouse.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateLocation, LocationType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Create a pick location with capacity constraints
    /// let location = commerce.warehouse().create_location(CreateLocation {
    ///     warehouse_id: 1,
    ///     location_type: LocationType::Pick,
    ///     zone: Some("A".into()),
    ///     aisle: Some("01".into()),
    ///     rack: Some("05".into()),
    ///     level: Some("2".into()),
    ///     bin: Some("03".into()),
    ///     max_weight_kg: Some(dec!(100)),
    ///     max_volume_m3: Some(dec!(0.5)),
    ///     is_pickable: Some(true),
    ///     is_receivable: Some(false),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created location: {}", location.code); // e.g., "A-01-05-2-03"
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_location(&self, input: CreateLocation) -> Result<Location> {
        self.db.warehouse().create_location(input)
    }

    /// Get a location by ID.
    pub fn get_location(&self, id: i32) -> Result<Option<Location>> {
        self.db.warehouse().get_location(id)
    }

    /// Get a location by code within a warehouse.
    pub fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>> {
        self.db.warehouse().get_location_by_code(warehouse_id, code)
    }

    /// Update a location.
    pub fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        self.db.warehouse().update_location(id, input)
    }

    /// List locations with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, LocationFilter, LocationType};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all pickable locations in zone A
    /// let locations = commerce.warehouse().list_locations(LocationFilter {
    ///     warehouse_id: Some(1),
    ///     zone: Some("A".into()),
    ///     is_pickable: Some(true),
    ///     is_active: Some(true),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        self.db.warehouse().list_locations(filter)
    }

    /// Delete a location. Only locations without inventory can be deleted.
    pub fn delete_location(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_location(id)
    }

    /// Count locations matching the filter.
    pub fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        self.db.warehouse().count_locations(filter)
    }

    /// Get all active locations for a warehouse.
    pub fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_for_warehouse(warehouse_id)
    }

    /// Get pickable locations with available inventory for a SKU.
    ///
    /// Returns locations that are marked as pickable and have available
    /// (non-reserved) inventory for the specified SKU.
    pub fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>> {
        self.db.warehouse().get_pickable_locations(warehouse_id, sku)
    }

    /// Get receivable locations for a warehouse.
    ///
    /// Returns locations where inventory can be received (e.g., receiving docks, staging areas).
    pub fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_receivable_locations(warehouse_id)
    }

    /// Create multiple locations in a batch.
    ///
    /// Returns a BatchResult with succeeded and failed operations.
    pub fn create_locations_batch(
        &self,
        inputs: Vec<CreateLocation>,
    ) -> Result<BatchResult<Location>> {
        self.db.warehouse().create_locations_batch(inputs)
    }

    /// Get multiple locations by ID.
    pub fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_batch(ids)
    }

    // ========================================================================
    // Location Inventory Operations
    // ========================================================================

    /// Get all inventory at a specific location.
    pub fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_location_inventory(location_id)
    }

    /// Get all locations with inventory for a SKU within a warehouse.
    pub fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_inventory_for_sku(warehouse_id, sku)
    }

    /// Adjust inventory at a location.
    ///
    /// Use positive quantities to add inventory, negative to remove.
    /// Creates a movement record for audit trail.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AdjustLocationInventory};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Add inventory from receiving
    /// let inventory = commerce.warehouse().adjust_inventory(AdjustLocationInventory {
    ///     location_id: 1,
    ///     sku: "PROD-001".into(),
    ///     lot_id: None,
    ///     quantity: dec!(100),
    ///     reason: "Receipt from PO-12345".into(),
    ///     reference_type: Some("purchase_order".into()),
    ///     reference_id: None,
    ///     performed_by: Some("warehouse_user".into()),
    /// })?;
    ///
    /// println!("New quantity on hand: {}", inventory.quantity_on_hand);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory> {
        self.db.warehouse().adjust_inventory(input)
    }

    /// Move inventory between locations.
    ///
    /// Validates that sufficient available (non-reserved) quantity exists
    /// at the source location. Creates movement records for both locations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, MoveInventory};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Move inventory from bulk to pick location
    /// let movement = commerce.warehouse().move_inventory(MoveInventory {
    ///     from_location_id: 1,  // Bulk storage
    ///     to_location_id: 2,    // Pick location
    ///     sku: "PROD-001".into(),
    ///     lot_id: None,
    ///     quantity: dec!(50),
    ///     reason: Some("Replenish pick location".into()),
    ///     performed_by: Some("forklift_operator".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        self.db.warehouse().move_inventory(input)
    }

    /// List location inventory with optional filtering.
    pub fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().list_location_inventory(filter)
    }

    /// Get total available quantity for a SKU across a warehouse.
    ///
    /// Sums available quantity (on_hand - reserved) across all locations.
    pub fn get_total_available(&self, warehouse_id: i32, sku: &str) -> Result<Decimal> {
        let inventory = self.db.warehouse().get_inventory_for_sku(warehouse_id, sku)?;
        Ok(inventory.iter().map(|i| i.quantity_available).sum())
    }

    /// Get total on-hand quantity for a SKU across a warehouse.
    pub fn get_total_on_hand(&self, warehouse_id: i32, sku: &str) -> Result<Decimal> {
        let inventory = self.db.warehouse().get_inventory_for_sku(warehouse_id, sku)?;
        Ok(inventory.iter().map(|i| i.quantity_on_hand).sum())
    }

    // ========================================================================
    // Movement Operations
    // ========================================================================

    /// Get inventory movements with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, MovementFilter, MovementType};
    /// use chrono::{Utc, Duration};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all transfers in the last 7 days
    /// let movements = commerce.warehouse().get_movements(MovementFilter {
    ///     warehouse_id: Some(1),
    ///     movement_type: Some(MovementType::Transfer),
    ///     from_date: Some(Utc::now() - Duration::days(7)),
    ///     limit: Some(100),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        self.db.warehouse().get_movements(filter)
    }

    /// Count movements matching the filter.
    pub fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        self.db.warehouse().count_movements(filter)
    }
}
