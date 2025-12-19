//! Inventory domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inventory item (SKU master record)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Inventory balance at a location
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryBalance {
    pub id: i64,
    pub item_id: i64,
    pub location_id: i32,
    pub quantity_on_hand: Decimal,
    pub quantity_allocated: Decimal,
    pub quantity_available: Decimal,
    pub reorder_point: Option<Decimal>,
    pub safety_stock: Option<Decimal>,
    pub version: i32,
    pub last_counted_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Inventory transaction (audit trail)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryTransaction {
    pub id: i64,
    pub item_id: i64,
    pub location_id: i32,
    pub transaction_type: TransactionType,
    pub quantity: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub reason: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Inventory reservation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryReservation {
    pub id: Uuid,
    pub item_id: i64,
    pub location_id: i32,
    pub quantity: Decimal,
    pub status: ReservationStatus,
    pub reference_type: String,
    pub reference_id: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Transaction type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Receipt,
    Shipment,
    Adjustment,
    Transfer,
    Return,
    Allocation,
    Deallocation,
    CycleCount,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Receipt => write!(f, "receipt"),
            Self::Shipment => write!(f, "shipment"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Transfer => write!(f, "transfer"),
            Self::Return => write!(f, "return"),
            Self::Allocation => write!(f, "allocation"),
            Self::Deallocation => write!(f, "deallocation"),
            Self::CycleCount => write!(f, "cycle_count"),
        }
    }
}

/// Reservation status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Allocated,
    Cancelled,
    Released,
    Expired,
}

impl Default for ReservationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for ReservationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Allocated => write!(f, "allocated"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Released => write!(f, "released"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Input for adjusting inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustInventory {
    pub sku: String,
    pub location_id: Option<i32>,
    pub quantity: Decimal,
    pub reason: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
}

/// Input for reserving inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveInventory {
    pub sku: String,
    pub location_id: Option<i32>,
    pub quantity: Decimal,
    pub reference_type: String,
    pub reference_id: String,
    pub expires_in_seconds: Option<i64>,
}

/// Input for creating inventory item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInventoryItem {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: Option<String>,
    pub initial_quantity: Option<Decimal>,
    pub location_id: Option<i32>,
    pub reorder_point: Option<Decimal>,
    pub safety_stock: Option<Decimal>,
}

impl Default for CreateInventoryItem {
    fn default() -> Self {
        Self {
            sku: String::new(),
            name: String::new(),
            description: None,
            unit_of_measure: None,
            initial_quantity: None,
            location_id: None,
            reorder_point: None,
            safety_stock: None,
        }
    }
}

/// Stock level summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockLevel {
    pub sku: String,
    pub name: String,
    pub total_on_hand: Decimal,
    pub total_allocated: Decimal,
    pub total_available: Decimal,
    pub locations: Vec<LocationStock>,
}

/// Stock at a specific location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationStock {
    pub location_id: i32,
    pub location_name: Option<String>,
    pub on_hand: Decimal,
    pub allocated: Decimal,
    pub available: Decimal,
}

/// Inventory filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryFilter {
    pub sku: Option<String>,
    pub location_id: Option<i32>,
    pub below_reorder_point: Option<bool>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl InventoryBalance {
    /// Check if stock is below reorder point
    pub fn needs_reorder(&self) -> bool {
        if let Some(reorder_point) = self.reorder_point {
            self.quantity_available < reorder_point
        } else {
            false
        }
    }

    /// Calculate available quantity
    pub fn calculate_available(&self) -> Decimal {
        self.quantity_on_hand - self.quantity_allocated
    }

    /// Check if requested quantity can be allocated
    pub fn can_allocate(&self, quantity: Decimal) -> bool {
        self.quantity_available >= quantity
    }
}
