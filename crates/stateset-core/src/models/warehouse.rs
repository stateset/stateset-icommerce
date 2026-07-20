//! Warehouse and Location domain models
//!
//! This module provides models for warehouse management including:
//! - Warehouse definitions
//! - Location hierarchy (zones, aisles, racks, bins)
//! - Location inventory tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Enums
// ============================================================================

/// Type of warehouse
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
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
    #[strum(serialize = "third_party", serialize = "thirdparty")]
    ThirdParty,
    /// Consignment warehouse
    Consignment,
    /// Returns processing center
    Returns,
}

/// Type of location within a warehouse
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
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
    #[strum(serialize = "cross_dock", serialize = "crossdock")]
    CrossDock,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Cycle count variance adjustment
    CycleCount,
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
            Self::CycleCount => write!(f, "cycle_count"),
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
            "cycle_count" => Ok(Self::CycleCount),
            _ => Err(crate::CommerceError::ValidationError(format!("Invalid movement type: {s}"))),
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

/// Alias for `CreateLocation` for API convenience
pub type CreateWarehouseLocation = CreateLocation;

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ============================================================================
    // WarehouseType
    // ============================================================================

    #[test]
    fn warehouse_type_display_from_str_round_trip() {
        for t in [
            WarehouseType::Distribution,
            WarehouseType::Manufacturing,
            WarehouseType::Retail,
            WarehouseType::ThirdParty,
            WarehouseType::Consignment,
            WarehouseType::Returns,
        ] {
            assert_eq!(WarehouseType::from_str(&t.to_string()), Ok(t));
        }
    }

    #[test]
    fn warehouse_type_third_party_aliases() {
        assert_eq!(WarehouseType::from_str("third_party"), Ok(WarehouseType::ThirdParty));
        assert_eq!(WarehouseType::from_str("thirdparty"), Ok(WarehouseType::ThirdParty));
        assert_eq!(WarehouseType::ThirdParty.to_string(), "third_party");
    }

    #[test]
    fn warehouse_type_case_insensitive_and_unknown() {
        assert_eq!(WarehouseType::from_str("RETAIL"), Ok(WarehouseType::Retail));
        assert!(WarehouseType::from_str("spaceport").is_err());
    }

    #[test]
    fn warehouse_type_default_is_distribution() {
        assert_eq!(WarehouseType::default(), WarehouseType::Distribution);
    }

    // ============================================================================
    // LocationType
    // ============================================================================

    #[test]
    fn location_type_display_from_str_round_trip() {
        for t in [
            LocationType::Bulk,
            LocationType::Pick,
            LocationType::Staging,
            LocationType::Receiving,
            LocationType::Shipping,
            LocationType::Quarantine,
            LocationType::Returns,
            LocationType::Production,
            LocationType::Packing,
            LocationType::CrossDock,
        ] {
            assert_eq!(LocationType::from_str(&t.to_string()), Ok(t));
        }
    }

    #[test]
    fn location_type_cross_dock_aliases() {
        assert_eq!(LocationType::from_str("cross_dock"), Ok(LocationType::CrossDock));
        assert_eq!(LocationType::from_str("crossdock"), Ok(LocationType::CrossDock));
        assert_eq!(LocationType::CrossDock.to_string(), "cross_dock");
    }

    #[test]
    fn location_type_default_is_bulk_and_unknown_errs() {
        assert_eq!(LocationType::default(), LocationType::Bulk);
        assert!(LocationType::from_str("void").is_err());
    }

    // ============================================================================
    // MovementType
    // ============================================================================

    #[test]
    fn movement_type_display_from_str_round_trip() {
        for t in [
            MovementType::Receipt,
            MovementType::Transfer,
            MovementType::Pick,
            MovementType::Adjustment,
            MovementType::Shipment,
            MovementType::Return,
            MovementType::CycleCount,
        ] {
            assert_eq!(MovementType::from_str(&t.to_string()).expect("round trip"), t);
        }
    }

    #[test]
    fn movement_type_from_str_case_insensitive() {
        assert_eq!(MovementType::from_str("RETURN").expect("parses"), MovementType::Return);
        assert_eq!(MovementType::from_str("Receipt").expect("parses"), MovementType::Receipt);
    }

    #[test]
    fn movement_type_from_str_invalid_is_validation_error() {
        let err = MovementType::from_str("teleport").expect_err("should fail");
        match err {
            crate::CommerceError::ValidationError(msg) => {
                assert!(msg.contains("teleport"), "message should include input: {msg}");
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    // ============================================================================
    // Serde representations
    // ============================================================================

    #[test]
    fn warehouse_type_serde_snake_case() {
        let json = serde_json::to_string(&WarehouseType::ThirdParty).expect("serialize");
        assert_eq!(json, "\"third_party\"");
        let back: WarehouseType = serde_json::from_str("\"third_party\"").expect("deserialize");
        assert_eq!(back, WarehouseType::ThirdParty);
    }

    #[test]
    fn movement_type_serde_snake_case() {
        let json = serde_json::to_string(&MovementType::Receipt).expect("serialize");
        assert_eq!(json, "\"receipt\"");
        let back: MovementType = serde_json::from_str("\"adjustment\"").expect("deserialize");
        assert_eq!(back, MovementType::Adjustment);
    }

    #[test]
    fn location_inventory_serde_round_trip() {
        let inv = LocationInventory {
            location_id: 7,
            sku: "SKU-1".to_string(),
            lot_id: None,
            quantity_on_hand: Decimal::from(10),
            quantity_reserved: Decimal::from(3),
            quantity_available: Decimal::from(7),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&inv).expect("serialize");
        let back: LocationInventory = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, inv);
    }

    // ============================================================================
    // Defaults
    // ============================================================================

    #[test]
    fn warehouse_address_default_is_empty() {
        let addr = WarehouseAddress::default();
        assert!(addr.street1.is_empty());
        assert!(addr.city.is_empty());
        assert!(addr.country.is_empty());
        assert_eq!(addr.street2, None);
        assert_eq!(addr.phone, None);
    }

    #[test]
    fn create_warehouse_default_uses_distribution() {
        let create = CreateWarehouse::default();
        assert_eq!(create.warehouse_type, WarehouseType::Distribution);
        assert!(create.code.is_empty());
        assert_eq!(create.timezone, None);
    }

    #[test]
    fn create_location_default_uses_bulk() {
        let create = CreateLocation::default();
        assert_eq!(create.location_type, LocationType::Bulk);
        assert_eq!(create.warehouse_id, 0);
        assert_eq!(create.is_pickable, None);
        assert_eq!(create.max_weight_kg, None);
    }

    #[test]
    fn filters_default_to_unset() {
        let f = LocationFilter::default();
        assert!(f.warehouse_id.is_none() && f.location_type.is_none() && f.limit.is_none());
        let mf = MovementFilter::default();
        assert!(mf.sku.is_none() && mf.movement_type.is_none() && mf.offset.is_none());
    }
}
