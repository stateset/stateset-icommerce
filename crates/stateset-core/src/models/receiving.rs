//! Receiving domain models
//!
//! Models for managing goods receipt, ASN processing, and put-away operations.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Core Receiving Types
// ============================================================================

/// A goods receipt document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: Uuid,
    pub receipt_number: String,
    pub receipt_type: ReceiptType,
    pub status: ReceiptStatus,
    /// Reference to source document (PO, transfer, return)
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub supplier_id: Option<Uuid>,
    pub warehouse_id: i32,
    /// Carrier info from ASN
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// Expected arrival from ASN
    pub expected_date: Option<DateTime<Utc>>,
    pub received_date: Option<DateTime<Utc>>,
    pub completed_date: Option<DateTime<Utc>>,
    /// Total items expected
    pub expected_quantity: Decimal,
    /// Total items received
    pub received_quantity: Decimal,
    /// Items requiring inspection
    pub pending_inspection_quantity: Decimal,
    /// Items put away to locations
    pub put_away_quantity: Decimal,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A line item on a receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptItem {
    pub id: Uuid,
    pub receipt_id: Uuid,
    pub line_number: i32,
    pub sku: String,
    pub description: Option<String>,
    /// Reference to PO line if applicable
    pub po_line_id: Option<Uuid>,
    /// Expected from ASN/PO
    pub expected_quantity: Decimal,
    pub received_quantity: Decimal,
    pub rejected_quantity: Decimal,
    pub unit_cost: Option<Decimal>,
    /// Lot number assigned on receipt
    pub lot_number: Option<String>,
    /// Serial numbers received (comma-separated)
    pub serial_numbers: Option<String>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub status: ReceiptItemStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Put-away record for received items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutAway {
    pub id: Uuid,
    pub receipt_id: Uuid,
    pub receipt_item_id: Uuid,
    pub sku: String,
    pub from_location_id: Option<i32>,
    pub to_location_id: i32,
    pub quantity: Decimal,
    pub lot_id: Option<Uuid>,
    pub status: PutAwayStatus,
    pub assigned_to: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Enums
// ============================================================================

/// Type of receipt.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReceiptType {
    #[default]
    #[strum(serialize = "purchase_order", serialize = "purchaseorder", serialize = "po")]
    PurchaseOrder,
    Transfer,
    #[strum(serialize = "return", serialize = "returns")]
    Return,
    Adjustment,
    Production,
    Other,
}

/// Status of a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReceiptStatus {
    /// ASN received, awaiting goods
    #[default]
    Expected,
    /// Goods arrived, receiving in progress
    InProgress,
    /// All items received, pending put-away
    Received,
    /// Quality inspection in progress
    Inspecting,
    /// Put-away in progress
    PuttingAway,
    /// All items put away
    Completed,
    /// Receipt cancelled
    Cancelled,
}

impl ReceiptStatus {
    /// Whether a receipt in this status may still be cancelled.
    ///
    /// A receipt can only be cancelled before its goods are received — once it
    /// reaches `Received` (or a later inspecting/put-away/completed state, or is
    /// already cancelled) the goods are physically on hand, so it must not be
    /// cancelled. Both backends gate `cancel_receipt` on this so they agree.
    #[must_use]
    pub const fn can_cancel(self) -> bool {
        matches!(self, Self::Expected | Self::InProgress)
    }
}

impl FromStr for ReceiptStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "expected" => Ok(Self::Expected),
            "in_progress" | "inprogress" => Ok(Self::InProgress),
            "received" => Ok(Self::Received),
            "inspecting" => Ok(Self::Inspecting),
            "putting_away" | "puttingaway" => Ok(Self::PuttingAway),
            "completed" => Ok(Self::Completed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown receipt status: {s}")),
        }
    }
}

/// Status of a receipt line item.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReceiptItemStatus {
    #[default]
    Pending,
    #[strum(serialize = "partially_received", serialize = "partiallyreceived")]
    PartiallyReceived,
    Received,
    Inspecting,
    Rejected,
    #[strum(serialize = "put_away", serialize = "putaway")]
    PutAway,
}

/// Status of a put-away task.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PutAwayStatus {
    #[default]
    Pending,
    Assigned,
    #[strum(serialize = "in_progress", serialize = "inprogress")]
    InProgress,
    Completed,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a receipt.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateReceipt {
    pub receipt_number: Option<String>,
    pub receipt_type: ReceiptType,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub supplier_id: Option<Uuid>,
    pub warehouse_id: i32,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub expected_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub items: Vec<CreateReceiptItem>,
}

/// Input for creating a receipt item.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateReceiptItem {
    pub sku: String,
    pub description: Option<String>,
    pub po_line_id: Option<Uuid>,
    pub expected_quantity: Decimal,
    pub unit_cost: Option<Decimal>,
    pub lot_number: Option<String>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Input for receiving items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveItems {
    pub receipt_id: Uuid,
    pub items: Vec<ReceiveItemLine>,
    pub receiving_location_id: Option<i32>,
    pub received_by: Option<String>,
}

/// A line in a receive operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveItemLine {
    pub receipt_item_id: Uuid,
    pub quantity_received: Decimal,
    pub quantity_rejected: Option<Decimal>,
    pub rejection_reason: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Input for creating a put-away task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePutAway {
    pub receipt_id: Uuid,
    pub receipt_item_id: Uuid,
    pub sku: String,
    pub from_location_id: Option<i32>,
    pub to_location_id: i32,
    pub quantity: Decimal,
    pub lot_id: Option<Uuid>,
    pub assigned_to: Option<String>,
    pub notes: Option<String>,
}

/// Input for completing a put-away task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletePutAway {
    pub put_away_id: Uuid,
    pub actual_location_id: Option<i32>,
    pub completed_by: Option<String>,
    pub notes: Option<String>,
}

/// Input for updating a receipt.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateReceipt {
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub expected_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing receipts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReceiptFilter {
    pub warehouse_id: Option<i32>,
    pub receipt_type: Option<ReceiptType>,
    pub status: Option<ReceiptStatus>,
    pub supplier_id: Option<Uuid>,
    pub reference_id: Option<Uuid>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing put-aways.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PutAwayFilter {
    pub receipt_id: Option<Uuid>,
    pub warehouse_id: Option<i32>,
    pub status: Option<PutAwayStatus>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Type Aliases for API compatibility
// ============================================================================

/// Alias for `CreateReceiptItem` for API convenience
pub type CreateReceiptLine = CreateReceiptItem;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a receipt number.
#[must_use]
pub fn generate_receipt_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("RCV-{timestamp}-{random}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn receipt_type_from_str() {
        assert_eq!(ReceiptType::from_str("po").unwrap(), ReceiptType::PurchaseOrder);
        assert!(ReceiptType::from_str("unknown").is_err());
    }

    #[test]
    fn receipt_status_from_str() {
        assert_eq!(ReceiptStatus::from_str("inprogress").unwrap(), ReceiptStatus::InProgress);
        assert!(ReceiptStatus::from_str("unknown").is_err());
    }

    #[test]
    fn receipt_item_status_from_str() {
        assert_eq!(
            ReceiptItemStatus::from_str("partiallyreceived").unwrap(),
            ReceiptItemStatus::PartiallyReceived
        );
        assert!(ReceiptItemStatus::from_str("unknown").is_err());
    }

    #[test]
    fn put_away_status_from_str() {
        assert_eq!(PutAwayStatus::from_str("inprogress").unwrap(), PutAwayStatus::InProgress);
        assert_eq!(PutAwayStatus::from_str("canceled").unwrap(), PutAwayStatus::Cancelled);
        assert!(PutAwayStatus::from_str("unknown").is_err());
    }
}
