//! Backorder Management domain models
//!
//! Models for tracking and managing backorders when inventory is unavailable.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Backorder Types
// ============================================================================

/// A backorder record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backorder {
    pub id: Uuid,
    pub backorder_number: String,
    pub order_id: Uuid,
    pub order_line_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub sku: String,
    pub quantity_ordered: Decimal,
    pub quantity_fulfilled: Decimal,
    pub quantity_remaining: Decimal,
    pub status: BackorderStatus,
    pub priority: BackorderPriority,
    pub expected_date: Option<DateTime<Utc>>,
    pub promised_date: Option<DateTime<Utc>>,
    pub source_location_id: Option<i32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A backorder fulfillment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackorderFulfillment {
    pub id: Uuid,
    pub backorder_id: Uuid,
    pub quantity: Decimal,
    pub source_type: FulfillmentSourceType,
    pub source_id: Option<Uuid>,
    pub notes: Option<String>,
    pub fulfilled_at: DateTime<Utc>,
    pub fulfilled_by: Option<String>,
}

/// Backorder allocation (reserved inventory for backorder).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackorderAllocation {
    pub id: Uuid,
    pub backorder_id: Uuid,
    pub sku: String,
    pub quantity: Decimal,
    pub location_id: Option<i32>,
    pub lot_id: Option<Uuid>,
    pub status: AllocationStatus,
    pub allocated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Enums
// ============================================================================

/// Backorder status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackorderStatus {
    #[default]
    Pending,
    PartiallyFulfilled,
    Allocated,
    ReadyToShip,
    Fulfilled,
    Cancelled,
}

impl std::fmt::Display for BackorderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackorderStatus::Pending => write!(f, "pending"),
            BackorderStatus::PartiallyFulfilled => write!(f, "partially_fulfilled"),
            BackorderStatus::Allocated => write!(f, "allocated"),
            BackorderStatus::ReadyToShip => write!(f, "ready_to_ship"),
            BackorderStatus::Fulfilled => write!(f, "fulfilled"),
            BackorderStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for BackorderStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(BackorderStatus::Pending),
            "partially_fulfilled" => Ok(BackorderStatus::PartiallyFulfilled),
            "allocated" => Ok(BackorderStatus::Allocated),
            "ready_to_ship" => Ok(BackorderStatus::ReadyToShip),
            "fulfilled" => Ok(BackorderStatus::Fulfilled),
            "cancelled" => Ok(BackorderStatus::Cancelled),
            _ => Ok(BackorderStatus::Pending),
        }
    }
}

/// Backorder priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackorderPriority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl std::fmt::Display for BackorderPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackorderPriority::Low => write!(f, "low"),
            BackorderPriority::Normal => write!(f, "normal"),
            BackorderPriority::High => write!(f, "high"),
            BackorderPriority::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for BackorderPriority {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(BackorderPriority::Low),
            "normal" => Ok(BackorderPriority::Normal),
            "high" => Ok(BackorderPriority::High),
            "critical" => Ok(BackorderPriority::Critical),
            _ => Ok(BackorderPriority::Normal),
        }
    }
}

/// Source type for fulfillment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentSourceType {
    #[default]
    Inventory,
    PurchaseOrder,
    Transfer,
    Production,
}

impl std::fmt::Display for FulfillmentSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FulfillmentSourceType::Inventory => write!(f, "inventory"),
            FulfillmentSourceType::PurchaseOrder => write!(f, "purchase_order"),
            FulfillmentSourceType::Transfer => write!(f, "transfer"),
            FulfillmentSourceType::Production => write!(f, "production"),
        }
    }
}

impl FromStr for FulfillmentSourceType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inventory" => Ok(FulfillmentSourceType::Inventory),
            "purchase_order" => Ok(FulfillmentSourceType::PurchaseOrder),
            "transfer" => Ok(FulfillmentSourceType::Transfer),
            "production" => Ok(FulfillmentSourceType::Production),
            _ => Ok(FulfillmentSourceType::Inventory),
        }
    }
}

/// Allocation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatus {
    #[default]
    Reserved,
    Confirmed,
    Released,
    Expired,
}

impl std::fmt::Display for AllocationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AllocationStatus::Reserved => write!(f, "reserved"),
            AllocationStatus::Confirmed => write!(f, "confirmed"),
            AllocationStatus::Released => write!(f, "released"),
            AllocationStatus::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for AllocationStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "reserved" => Ok(AllocationStatus::Reserved),
            "confirmed" => Ok(AllocationStatus::Confirmed),
            "released" => Ok(AllocationStatus::Released),
            "expired" => Ok(AllocationStatus::Expired),
            _ => Ok(AllocationStatus::Reserved),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a backorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackorder {
    pub order_id: Uuid,
    pub order_line_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub sku: String,
    pub quantity: Decimal,
    pub priority: Option<BackorderPriority>,
    pub expected_date: Option<DateTime<Utc>>,
    pub promised_date: Option<DateTime<Utc>>,
    pub source_location_id: Option<i32>,
    pub notes: Option<String>,
}

/// Input for updating a backorder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBackorder {
    pub priority: Option<BackorderPriority>,
    pub expected_date: Option<DateTime<Utc>>,
    pub promised_date: Option<DateTime<Utc>>,
    pub source_location_id: Option<i32>,
    pub notes: Option<String>,
}

/// Input for fulfilling a backorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillBackorder {
    pub backorder_id: Uuid,
    pub quantity: Decimal,
    pub source_type: FulfillmentSourceType,
    pub source_id: Option<Uuid>,
    pub notes: Option<String>,
    pub fulfilled_by: Option<String>,
}

/// Input for allocating inventory to a backorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocateBackorder {
    pub backorder_id: Uuid,
    pub quantity: Decimal,
    pub location_id: Option<i32>,
    pub lot_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing backorders.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackorderFilter {
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub sku: Option<String>,
    pub status: Option<BackorderStatus>,
    pub priority: Option<BackorderPriority>,
    pub expected_before: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Summary Types
// ============================================================================

/// Backorder summary by SKU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkuBackorderSummary {
    pub sku: String,
    pub total_quantity: Decimal,
    pub backorder_count: i32,
    pub oldest_date: Option<DateTime<Utc>>,
    pub earliest_expected: Option<DateTime<Utc>>,
}

/// Overall backorder summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackorderSummary {
    pub total_backorders: i32,
    pub total_quantity: Decimal,
    pub pending_count: i32,
    pub allocated_count: i32,
    pub critical_count: i32,
    pub overdue_count: i32,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a backorder number.
pub fn generate_backorder_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("BO-{}-{}", timestamp, random)
}
