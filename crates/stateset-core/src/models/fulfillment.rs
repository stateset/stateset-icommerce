//! Fulfillment domain models
//!
//! Models for pick, pack, and ship operations in warehouse fulfillment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{FulfillmentId, OrderId, OrderItemId, ShipmentId};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Fulfillment Types
// ============================================================================

/// A wave groups multiple orders for efficient picking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: FulfillmentId,
    pub wave_number: String,
    pub warehouse_id: i32,
    pub status: WaveStatus,
    pub order_count: i32,
    pub pick_count: i32,
    pub completed_pick_count: i32,
    pub priority: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A pick task for retrieving items from warehouse locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickTask {
    pub id: Uuid,
    pub wave_id: Option<FulfillmentId>,
    pub order_id: OrderId,
    pub order_item_id: OrderItemId,
    pub warehouse_id: i32,
    pub status: PickStatus,
    pub sku: String,
    pub product_name: Option<String>,
    pub source_location_id: i32,
    pub source_location_code: Option<String>,
    pub quantity_requested: Decimal,
    pub quantity_picked: Decimal,
    pub quantity_short: Decimal,
    pub lot_id: Option<Uuid>,
    pub serial_number: Option<String>,
    pub assigned_to: Option<String>,
    pub priority: i32,
    pub pick_sequence: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A pack task for packaging picked items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackTask {
    pub id: Uuid,
    pub order_id: OrderId,
    pub shipment_id: Option<ShipmentId>,
    pub status: PackStatus,
    pub carton_count: i32,
    pub total_weight_kg: Option<Decimal>,
    pub assigned_to: Option<String>,
    pub packing_station: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A carton/package within a pack task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Carton {
    pub id: Uuid,
    pub pack_task_id: Uuid,
    pub carton_number: String,
    pub package_type: PackageType,
    pub weight_kg: Option<Decimal>,
    pub length_cm: Option<Decimal>,
    pub width_cm: Option<Decimal>,
    pub height_cm: Option<Decimal>,
    pub tracking_number: Option<String>,
    pub label_printed: bool,
    pub created_at: DateTime<Utc>,
}

/// Item in a carton.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartonItem {
    pub id: Uuid,
    pub carton_id: Uuid,
    pub sku: String,
    pub quantity: Decimal,
    pub lot_id: Option<Uuid>,
    pub serial_number: Option<String>,
}

/// A ship task for final shipping handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipTask {
    pub id: Uuid,
    pub order_id: OrderId,
    pub shipment_id: ShipmentId,
    pub pack_task_id: Uuid,
    pub status: ShipStatus,
    pub carrier: Option<String>,
    pub service_level: Option<String>,
    pub tracking_number: Option<String>,
    pub label_url: Option<String>,
    pub shipping_cost: Option<Decimal>,
    pub assigned_to: Option<String>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Enums
// ============================================================================

/// Status of a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WaveStatus {
    #[default]
    Draft,
    Released,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for WaveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaveStatus::Draft => write!(f, "draft"),
            WaveStatus::Released => write!(f, "released"),
            WaveStatus::InProgress => write!(f, "in_progress"),
            WaveStatus::Completed => write!(f, "completed"),
            WaveStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for WaveStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(WaveStatus::Draft),
            "released" => Ok(WaveStatus::Released),
            "in_progress" | "inprogress" => Ok(WaveStatus::InProgress),
            "completed" => Ok(WaveStatus::Completed),
            "cancelled" | "canceled" => Ok(WaveStatus::Cancelled),
            _ => Err(format!("Unknown wave status: {}", s)),
        }
    }
}

/// Status of a pick task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PickStatus {
    #[default]
    Pending,
    Assigned,
    InProgress,
    Completed,
    Short,
    Cancelled,
}

impl std::fmt::Display for PickStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PickStatus::Pending => write!(f, "pending"),
            PickStatus::Assigned => write!(f, "assigned"),
            PickStatus::InProgress => write!(f, "in_progress"),
            PickStatus::Completed => write!(f, "completed"),
            PickStatus::Short => write!(f, "short"),
            PickStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for PickStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(PickStatus::Pending),
            "assigned" => Ok(PickStatus::Assigned),
            "in_progress" | "inprogress" => Ok(PickStatus::InProgress),
            "completed" => Ok(PickStatus::Completed),
            "short" => Ok(PickStatus::Short),
            "cancelled" | "canceled" => Ok(PickStatus::Cancelled),
            _ => Err(format!("Unknown pick status: {}", s)),
        }
    }
}

/// Status of a pack task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PackStatus {
    #[default]
    Pending,
    Assigned,
    ReadyToPack,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for PackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackStatus::Pending => write!(f, "pending"),
            PackStatus::Assigned => write!(f, "assigned"),
            PackStatus::ReadyToPack => write!(f, "ready_to_pack"),
            PackStatus::InProgress => write!(f, "in_progress"),
            PackStatus::Completed => write!(f, "completed"),
            PackStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for PackStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(PackStatus::Pending),
            "assigned" => Ok(PackStatus::Assigned),
            "ready_to_pack" | "readytopack" => Ok(PackStatus::ReadyToPack),
            "in_progress" | "inprogress" => Ok(PackStatus::InProgress),
            "completed" => Ok(PackStatus::Completed),
            "cancelled" | "canceled" => Ok(PackStatus::Cancelled),
            _ => Err(format!("Unknown pack status: {}", s)),
        }
    }
}

/// Status of a ship task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ShipStatus {
    #[default]
    Pending,
    ReadyToShip,
    LabelPrinted,
    Shipped,
    Cancelled,
}

impl std::fmt::Display for ShipStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShipStatus::Pending => write!(f, "pending"),
            ShipStatus::ReadyToShip => write!(f, "ready_to_ship"),
            ShipStatus::LabelPrinted => write!(f, "label_printed"),
            ShipStatus::Shipped => write!(f, "shipped"),
            ShipStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for ShipStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(ShipStatus::Pending),
            "ready_to_ship" | "readytoship" => Ok(ShipStatus::ReadyToShip),
            "label_printed" | "labelprinted" => Ok(ShipStatus::LabelPrinted),
            "shipped" => Ok(ShipStatus::Shipped),
            "cancelled" | "canceled" => Ok(ShipStatus::Cancelled),
            _ => Err(format!("Unknown ship status: {}", s)),
        }
    }
}

/// Package type for cartons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PackageType {
    #[default]
    Box,
    Envelope,
    Tube,
    Pallet,
    Custom,
}

impl std::fmt::Display for PackageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageType::Box => write!(f, "box"),
            PackageType::Envelope => write!(f, "envelope"),
            PackageType::Tube => write!(f, "tube"),
            PackageType::Pallet => write!(f, "pallet"),
            PackageType::Custom => write!(f, "custom"),
        }
    }
}

impl FromStr for PackageType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "box" => Ok(PackageType::Box),
            "envelope" => Ok(PackageType::Envelope),
            "tube" => Ok(PackageType::Tube),
            "pallet" => Ok(PackageType::Pallet),
            "custom" => Ok(PackageType::Custom),
            _ => Err(format!("Unknown package type: {}", s)),
        }
    }
}

/// Type of wave for order grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WaveType {
    /// Process orders in batches
    #[default]
    Batch,
    /// Priority orders processed first
    Priority,
    /// Zone-based wave planning
    Zone,
    /// Single order waves
    Single,
}

impl std::fmt::Display for WaveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaveType::Batch => write!(f, "batch"),
            WaveType::Priority => write!(f, "priority"),
            WaveType::Zone => write!(f, "zone"),
            WaveType::Single => write!(f, "single"),
        }
    }
}

impl FromStr for WaveType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "batch" => Ok(WaveType::Batch),
            "priority" => Ok(WaveType::Priority),
            "zone" => Ok(WaveType::Zone),
            "single" | "single_order" | "singleorder" => Ok(WaveType::Single),
            _ => Err(format!("Unknown wave type: {}", s)),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a wave.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateWave {
    pub warehouse_id: i32,
    pub order_ids: Vec<OrderId>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

/// Input for creating a pick task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePickTask {
    pub wave_id: Option<FulfillmentId>,
    pub order_id: OrderId,
    pub order_item_id: OrderItemId,
    pub warehouse_id: i32,
    pub sku: String,
    pub product_name: Option<String>,
    pub source_location_id: i32,
    pub quantity_requested: Decimal,
    pub lot_id: Option<Uuid>,
    pub serial_number: Option<String>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
}

/// Input for completing a pick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletePick {
    pub pick_id: Uuid,
    pub quantity_picked: Decimal,
    pub quantity_short: Option<Decimal>,
    pub short_reason: Option<String>,
    pub lot_id: Option<Uuid>,
    pub serial_number: Option<String>,
    pub completed_by: Option<String>,
}

/// Input for creating a pack task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePackTask {
    pub order_id: OrderId,
    pub notes: Option<String>,
}

/// Input for adding a carton.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddCarton {
    pub pack_task_id: Uuid,
    pub package_type: PackageType,
    pub weight_kg: Option<Decimal>,
    pub length_cm: Option<Decimal>,
    pub width_cm: Option<Decimal>,
    pub height_cm: Option<Decimal>,
}

/// Input for adding item to carton.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCartonItem {
    pub carton_id: Uuid,
    pub sku: String,
    pub quantity: Decimal,
    pub lot_id: Option<Uuid>,
    pub serial_number: Option<String>,
}

/// Input for creating a ship task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShipTask {
    pub order_id: OrderId,
    pub shipment_id: ShipmentId,
    pub pack_task_id: Uuid,
    pub carrier: Option<String>,
    pub service_level: Option<String>,
    pub notes: Option<String>,
}

/// Input for completing shipping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteShip {
    pub ship_task_id: Uuid,
    pub tracking_number: String,
    pub shipping_cost: Option<Decimal>,
    pub shipped_by: Option<String>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing waves.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WaveFilter {
    pub warehouse_id: Option<i32>,
    pub status: Option<WaveStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing pick tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PickTaskFilter {
    pub warehouse_id: Option<i32>,
    pub wave_id: Option<FulfillmentId>,
    pub order_id: Option<OrderId>,
    pub status: Option<PickStatus>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing pack tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackTaskFilter {
    pub order_id: Option<OrderId>,
    pub status: Option<PackStatus>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing ship tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShipTaskFilter {
    pub order_id: Option<OrderId>,
    pub status: Option<ShipStatus>,
    pub carrier: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a wave number.
pub fn generate_wave_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..4].to_uppercase();
    format!("WV-{}-{}", timestamp, random)
}

/// Generate a carton number.
pub fn generate_carton_number() -> String {
    let timestamp = chrono::Utc::now().format("%H%M%S").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..4].to_uppercase();
    format!("CTN-{}-{}", timestamp, random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_status_from_str() {
        assert_eq!(WaveStatus::from_str("released").unwrap(), WaveStatus::Released);
        assert_eq!(WaveStatus::from_str("InProgress").unwrap(), WaveStatus::InProgress);
        assert!(WaveStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_pick_status_from_str() {
        assert_eq!(PickStatus::from_str("assigned").unwrap(), PickStatus::Assigned);
        assert_eq!(PickStatus::from_str("canceled").unwrap(), PickStatus::Cancelled);
        assert!(PickStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_pack_status_from_str() {
        assert_eq!(PackStatus::from_str("assigned").unwrap(), PackStatus::Assigned);
        assert_eq!(PackStatus::from_str("readytopack").unwrap(), PackStatus::ReadyToPack);
        assert!(PackStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_ship_status_from_str() {
        assert_eq!(ShipStatus::from_str("labelprinted").unwrap(), ShipStatus::LabelPrinted);
        assert_eq!(ShipStatus::from_str("ready_to_ship").unwrap(), ShipStatus::ReadyToShip);
        assert!(ShipStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_package_type_from_str() {
        assert_eq!(PackageType::from_str("box").unwrap(), PackageType::Box);
        assert!(PackageType::from_str("nope").is_err());
    }

    #[test]
    fn test_wave_type_from_str() {
        assert_eq!(WaveType::from_str("single_order").unwrap(), WaveType::Single);
        assert!(WaveType::from_str("nope").is_err());
    }
}
