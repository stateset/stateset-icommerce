//! Inventory domain models

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
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

/// Reservation status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Allocated,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
    Released,
    Expired,
}

impl Default for ReservationStatus {
    fn default() -> Self {
        Self::Pending
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

/// Input for confirming all or part of an inventory reservation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmInventoryReservation {
    pub reservation_id: Uuid,
    /// Quantity to confirm. `None` confirms the full remaining reservation.
    pub quantity: Option<Decimal>,
}

/// Input for releasing an inventory reservation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInventoryReservation {
    pub reservation_id: Uuid,
}

/// Input for creating inventory item
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

impl Validate for CreateInventoryItem {
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .sku("sku", &self.sku)
            .required("name", &self.name)
            .non_negative("initial_quantity", self.initial_quantity.unwrap_or(Decimal::ZERO))
            .non_negative("reorder_point", self.reorder_point.unwrap_or(Decimal::ZERO))
            .non_negative("safety_stock", self.safety_stock.unwrap_or(Decimal::ZERO))
            .build()?;
        if self.location_id.is_some_and(|id| id <= 0) {
            return Err(crate::CommerceError::ValidationError(
                "location_id must be positive".into(),
            ));
        }
        if self.unit_of_measure.as_deref().is_some_and(|unit| unit.trim().is_empty()) {
            return Err(crate::CommerceError::ValidationError(
                "unit_of_measure cannot be empty".into(),
            ));
        }
        Ok(())
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
    #[must_use]
    pub fn needs_reorder(&self) -> bool {
        if let Some(reorder_point) = self.reorder_point {
            self.quantity_available < reorder_point
        } else {
            false
        }
    }

    /// Calculate available quantity
    #[must_use]
    pub fn calculate_available(&self) -> Decimal {
        self.quantity_on_hand - self.quantity_allocated
    }

    /// Check if requested quantity can be allocated
    #[must_use]
    pub fn can_allocate(&self, quantity: Decimal) -> bool {
        self.quantity_available >= quantity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn transaction_type_from_str() {
        assert_eq!(TransactionType::from_str("receipt").unwrap(), TransactionType::Receipt);
        assert!(TransactionType::from_str("unknown").is_err());
    }

    #[test]
    fn reservation_status_from_str() {
        assert_eq!(ReservationStatus::from_str("allocated").unwrap(), ReservationStatus::Allocated);
        assert!(ReservationStatus::from_str("unknown").is_err());
    }
}
