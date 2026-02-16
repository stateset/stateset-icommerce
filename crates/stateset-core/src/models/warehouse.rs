//! Warehouse and Location domain models
//!
//! This module provides models for warehouse management including:
//! - Warehouse definitions
//! - Location hierarchy (zones, aisles, racks, bins)
//! - Location inventory tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Enums
// ============================================================================

/// Type of warehouse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WarehouseType {
    /// Main distribution center
    #[default]
    Distribution,
    /// Manufacturing facility
    Manufacturing,
    /// Retail store with inventory
    Retail,
    /// Third-party logistics provider
    ThirdParty,
    /// Consignment warehouse
    Consignment,
    /// Returns processing center
    Returns,
}

impl std::fmt::Display for WarehouseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Distribution => write!(f, "distribution"),
            Self::Manufacturing => write!(f, "manufacturing"),
            Self::Retail => write!(f, "retail"),
            Self::ThirdParty => write!(f, "third_party"),
            Self::Consignment => write!(f, "consignment"),
            Self::Returns => write!(f, "returns"),
        }
    }
}

impl std::str::FromStr for WarehouseType {
    type Err = crate::CommerceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "distribution" => Ok(Self::Distribution),
            "manufacturing" => Ok(Self::Manufacturing),
            "retail" => Ok(Self::Retail),
            "third_party" | "thirdparty" => Ok(Self::ThirdParty),
            "consignment" => Ok(Self::Consignment),
            "returns" => Ok(Self::Returns),
            _ => {
                Err(crate::CommerceError::ValidationError(format!("Invalid warehouse type: {}", s)))
            }
        }
    }
}

/// Type of location within a warehouse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LocationType {
    /// Bulk storage location
    #[default]
    Bulk,
    /// Picking location for order fulfillment
    Pick,
    /// Staging area for orders
    Staging,
    /// Receiving dock/area
    Receiving,
    /// Shipping dock/area
    Shipping,
    /// Quarantine area for quality holds
    Quarantine,
    /// Returns processing area
    Returns,
    /// Production/manufacturing area
    Production,
    /// Packing station
    Packing,
    /// Cross-docking area
    CrossDock,
}

impl std::fmt::Display for LocationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bulk => write!(f, "bulk"),
            Self::Pick => write!(f, "pick"),
            Self::Staging => write!(f, "staging"),
            Self::Receiving => write!(f, "receiving"),
            Self::Shipping => write!(f, "shipping"),
            Self::Quarantine => write!(f, "quarantine"),
            Self::Returns => write!(f, "returns"),
            Self::Production => write!(f, "production"),
            Self::Packing => write!(f, "packing"),
            Self::CrossDock => write!(f, "cross_dock"),
        }
    }
}

impl std::str::FromStr for LocationType {
    type Err = crate::CommerceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bulk" => Ok(Self::Bulk),
            "pick" => Ok(Self::Pick),
            "staging" => Ok(Self::Staging),
            "receiving" => Ok(Self::Receiving),
            "shipping" => Ok(Self::Shipping),
            "quarantine" => Ok(Self::Quarantine),
            "returns" => Ok(Self::Returns),
            "production" => Ok(Self::Production),
            "packing" => Ok(Self::Packing),
            "cross_dock" | "crossdock" => Ok(Self::CrossDock),
            _ => {
                Err(crate::CommerceError::ValidationError(format!("Invalid location type: {}", s)))
            }
        }
    }
}

// ============================================================================
// Address (embedded)
// ============================================================================

/// Physical address for a warehouse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct WarehouseAddress {
    pub street1: String,
    pub street2: Option<String>,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
}

// ============================================================================
// Warehouse
// ============================================================================

/// A warehouse or distribution center
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Warehouse {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub warehouse_type: WarehouseType,
    pub address: WarehouseAddress,
    pub timezone: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a warehouse
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CreateWarehouse {
    pub code: String,
    pub name: String,
    pub warehouse_type: WarehouseType,
    pub address: WarehouseAddress,
    pub timezone: Option<String>,
}

/// Input for updating a warehouse
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UpdateWarehouse {
    pub name: Option<String>,
    pub warehouse_type: Option<WarehouseType>,
    pub address: Option<WarehouseAddress>,
    pub timezone: Option<String>,
    pub is_active: Option<bool>,
}

/// Filter for listing warehouses
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WarehouseFilter {
    pub warehouse_type: Option<WarehouseType>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Location
// ============================================================================

/// A location within a warehouse (zone/aisle/rack/bin)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Location {
    pub id: i32,
    pub warehouse_id: i32,
    pub code: String,
    pub location_type: LocationType,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub level: Option<String>,
    pub bin: Option<String>,
    pub max_weight_kg: Option<Decimal>,
    pub max_volume_m3: Option<Decimal>,
    pub current_weight_kg: Option<Decimal>,
    pub current_volume_m3: Option<Decimal>,
    pub is_pickable: bool,
    pub is_receivable: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a location
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CreateLocation {
    pub warehouse_id: i32,
    pub code: Option<String>,
    pub location_type: LocationType,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub level: Option<String>,
    pub bin: Option<String>,
    pub max_weight_kg: Option<Decimal>,
    pub max_volume_m3: Option<Decimal>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
}

/// Input for updating a location
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UpdateLocation {
    pub location_type: Option<LocationType>,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub level: Option<String>,
    pub bin: Option<String>,
    pub max_weight_kg: Option<Decimal>,
    pub max_volume_m3: Option<Decimal>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
    pub is_active: Option<bool>,
}

/// Filter for listing locations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationFilter {
    pub warehouse_id: Option<i32>,
    pub location_type: Option<LocationType>,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
    pub is_active: Option<bool>,
    pub has_capacity: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Location Inventory
// ============================================================================

/// Inventory at a specific location
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationInventory {
    pub location_id: i32,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub quantity_on_hand: Decimal,
    pub quantity_reserved: Decimal,
    pub quantity_available: Decimal,
    pub updated_at: DateTime<Utc>,
}

/// Input for adjusting location inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AdjustLocationInventory {
    pub location_id: i32,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub quantity: Decimal,
    pub reason: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub performed_by: Option<String>,
}

/// Input for moving inventory between locations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MoveInventory {
    pub from_location_id: i32,
    pub to_location_id: i32,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub quantity: Decimal,
    pub reason: Option<String>,
    pub performed_by: Option<String>,
}

/// Filter for location inventory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationInventoryFilter {
    pub location_id: Option<i32>,
    pub warehouse_id: Option<i32>,
    pub sku: Option<String>,
    pub lot_id: Option<Uuid>,
    pub has_quantity: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Location Movement
// ============================================================================

/// Record of inventory movement between locations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationMovement {
    pub id: Uuid,
    pub movement_type: MovementType,
    pub from_location_id: Option<i32>,
    pub to_location_id: Option<i32>,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub quantity: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub reason: Option<String>,
    pub performed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of inventory movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MovementType {
    /// Received into warehouse
    Receipt,
    /// Moved between locations
    Transfer,
    /// Picked for order
    Pick,
    /// Adjustment (count, damage, etc.)
    Adjustment,
    /// Shipped out
    Shipment,
    /// Returned to stock
    Return,
}

impl std::fmt::Display for MovementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Receipt => write!(f, "receipt"),
            Self::Transfer => write!(f, "transfer"),
            Self::Pick => write!(f, "pick"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Shipment => write!(f, "shipment"),
            Self::Return => write!(f, "return"),
        }
    }
}

impl std::str::FromStr for MovementType {
    type Err = crate::CommerceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "receipt" => Ok(Self::Receipt),
            "transfer" => Ok(Self::Transfer),
            "pick" => Ok(Self::Pick),
            "adjustment" => Ok(Self::Adjustment),
            "shipment" => Ok(Self::Shipment),
            "return" => Ok(Self::Return),
            _ => {
                Err(crate::CommerceError::ValidationError(format!("Invalid movement type: {}", s)))
            }
        }
    }
}

/// Filter for inventory movements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MovementFilter {
    pub warehouse_id: Option<i32>,
    pub location_id: Option<i32>,
    pub sku: Option<String>,
    pub lot_id: Option<Uuid>,
    pub movement_type: Option<MovementType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Zone
// ============================================================================

/// A zone within a warehouse (grouping of locations)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Zone {
    pub id: i32,
    pub warehouse_id: i32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a zone
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CreateZone {
    pub warehouse_id: i32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

/// Input for updating a zone
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UpdateZone {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// ============================================================================
// Type Aliases for API compatibility
// ============================================================================

/// Alias for CreateLocation for API convenience
pub type CreateWarehouseLocation = CreateLocation;
