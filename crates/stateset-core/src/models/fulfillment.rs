//! Fulfillment domain models
//!
//! Models for pick, pack, and ship operations in warehouse fulfillment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Fulfillment Types
// ============================================================================

/// A wave groups multiple orders for efficient picking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: Uuid,
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
    pub wave_id: Option<Uuid>,
    pub order_id: Uuid,
    pub order_item_id: Uuid,
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
    pub order_id: Uuid,
    pub shipment_id: Option<Uuid>,
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
    pub order_id: Uuid,
    pub shipment_id: Uuid,
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
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(WaveStatus::Draft),
            "released" => Ok(WaveStatus::Released),
            "in_progress" => Ok(WaveStatus::InProgress),
            "completed" => Ok(WaveStatus::Completed),
            "cancelled" => Ok(WaveStatus::Cancelled),
            _ => Ok(WaveStatus::Draft),
        }
    }
}

/// Status of a pick task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PickStatus::Pending),
            "assigned" => Ok(PickStatus::Assigned),
            "in_progress" => Ok(PickStatus::InProgress),
            "completed" => Ok(PickStatus::Completed),
            "short" => Ok(PickStatus::Short),
            "cancelled" => Ok(PickStatus::Cancelled),
            _ => Ok(PickStatus::Pending),
        }
    }
}

/// Status of a pack task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackStatus {
    #[default]
    Pending,
    ReadyToPack,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for PackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackStatus::Pending => write!(f, "pending"),
            PackStatus::ReadyToPack => write!(f, "ready_to_pack"),
            PackStatus::InProgress => write!(f, "in_progress"),
            PackStatus::Completed => write!(f, "completed"),
            PackStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for PackStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PackStatus::Pending),
            "ready_to_pack" => Ok(PackStatus::ReadyToPack),
            "in_progress" => Ok(PackStatus::InProgress),
            "completed" => Ok(PackStatus::Completed),
            "cancelled" => Ok(PackStatus::Cancelled),
            _ => Ok(PackStatus::Pending),
        }
    }
}

/// Status of a ship task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ShipStatus::Pending),
            "ready_to_ship" => Ok(ShipStatus::ReadyToShip),
            "label_printed" => Ok(ShipStatus::LabelPrinted),
            "shipped" => Ok(ShipStatus::Shipped),
            "cancelled" => Ok(ShipStatus::Cancelled),
            _ => Ok(ShipStatus::Pending),
        }
    }
}

/// Package type for cartons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "box" => Ok(PackageType::Box),
            "envelope" => Ok(PackageType::Envelope),
            "tube" => Ok(PackageType::Tube),
            "pallet" => Ok(PackageType::Pallet),
            "custom" => Ok(PackageType::Custom),
            _ => Ok(PackageType::Box),
        }
    }
}

/// Type of wave for order grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "batch" => Ok(WaveType::Batch),
            "priority" => Ok(WaveType::Priority),
            "zone" => Ok(WaveType::Zone),
            "single" => Ok(WaveType::Single),
            _ => Ok(WaveType::Batch),
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
    pub order_ids: Vec<Uuid>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

/// Input for creating a pick task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePickTask {
    pub wave_id: Option<Uuid>,
    pub order_id: Uuid,
    pub order_item_id: Uuid,
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
    pub order_id: Uuid,
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
    pub order_id: Uuid,
    pub shipment_id: Uuid,
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
    pub wave_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub status: Option<PickStatus>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing pack tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackTaskFilter {
    pub order_id: Option<Uuid>,
    pub status: Option<PackStatus>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing ship tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShipTaskFilter {
    pub order_id: Option<Uuid>,
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
