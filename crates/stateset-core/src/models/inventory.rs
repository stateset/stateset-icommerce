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

/// Reservation status enumeration.
///
/// A reservation **holds stock** (it is counted in
/// `inventory_balances.quantity_allocated`) while it is [`Pending`],
/// [`Confirmed`] or [`Allocated`]; see [`ReservationStatus::holds_stock`].
/// Every other status is terminal and has already handed its units back
/// (`Released`, `Cancelled`, `Expired`) or consumed them (`Fulfilled`: both
/// `quantity_on_hand` and `quantity_allocated` were decremented).
///
/// [`Pending`]: ReservationStatus::Pending
/// [`Confirmed`]: ReservationStatus::Confirmed
/// [`Allocated`]: ReservationStatus::Allocated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReservationStatus {
    /// Open hold on stock (the only status the engine writes on creation).
    Pending,
    /// Committed hold (order shipped / allocation confirmed); still counted
    /// in `quantity_allocated` until fulfilled or released.
    Confirmed,
    /// Legacy synonym of [`Self::Pending`] kept for rows written by earlier
    /// releases; the engine never writes it, but reads/expires it like
    /// `pending`.
    Allocated,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
    Released,
    Expired,
    /// The reserved units were consumed (shipped / backorder fulfilled):
    /// on-hand and allocated were both decremented by the reservation.
    Fulfilled,
}

impl ReservationStatus {
    /// Whether a reservation in this status is still counted in
    /// `quantity_allocated` (and therefore reduces `quantity_available`).
    #[must_use]
    pub const fn holds_stock(self) -> bool {
        matches!(self, Self::Pending | Self::Confirmed | Self::Allocated)
    }
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

impl Validate for AdjustInventory {
    /// An adjustment needs a well-formed SKU, a non-zero quantity (a zero
    /// adjustment would write a meaningless ledger row) and a non-blank
    /// reason (the ledger row is the audit trail for the stock movement).
    fn validate(&self) -> Result<()> {
        crate::validate_sku(&self.sku)?;
        if self.quantity.is_zero() {
            return Err(crate::CommerceError::ValidationError(
                "Adjustment quantity cannot be zero".into(),
            ));
        }
        if self.reason.trim().is_empty() {
            return Err(crate::CommerceError::ValidationError(
                "Adjustment reason cannot be empty".into(),
            ));
        }
        if self.location_id.is_some_and(|id| id <= 0) {
            return Err(crate::CommerceError::ValidationError(
                "location_id must be positive".into(),
            ));
        }
        Ok(())
    }
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
    /// The level below which the balance needs reordering: `reorder_point`
    /// raised by `safety_stock` (the buffer that must stay untouched), or
    /// `None` when no reorder point is set. Both backends' `get_reorder_needed`
    /// use exactly this threshold.
    #[must_use]
    pub fn reorder_threshold(&self) -> Option<Decimal> {
        self.reorder_point.map(|point| point + self.safety_stock.unwrap_or(Decimal::ZERO))
    }

    /// Check if stock is below the reorder threshold (reorder point plus
    /// safety stock).
    #[must_use]
    pub fn needs_reorder(&self) -> bool {
        self.reorder_threshold().is_some_and(|threshold| self.quantity_available < threshold)
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
        assert_eq!(ReservationStatus::from_str("fulfilled").unwrap(), ReservationStatus::Fulfilled);
        assert!(ReservationStatus::from_str("unknown").is_err());
    }

    #[test]
    fn reservation_status_holds_stock_only_while_open() {
        assert!(ReservationStatus::Pending.holds_stock());
        assert!(ReservationStatus::Confirmed.holds_stock());
        assert!(ReservationStatus::Allocated.holds_stock());
        assert!(!ReservationStatus::Released.holds_stock());
        assert!(!ReservationStatus::Cancelled.holds_stock());
        assert!(!ReservationStatus::Expired.holds_stock());
        assert!(!ReservationStatus::Fulfilled.holds_stock());
    }

    fn adjust(quantity: Decimal, reason: &str) -> AdjustInventory {
        AdjustInventory {
            sku: "SKU-1".into(),
            location_id: None,
            quantity,
            reason: reason.into(),
            reference_type: None,
            reference_id: None,
        }
    }

    #[test]
    fn adjust_inventory_validation() {
        assert!(adjust(Decimal::ONE, "cycle count").validate().is_ok());
        assert!(adjust(Decimal::ZERO, "cycle count").validate().is_err());
        assert!(adjust(Decimal::ONE, "   ").validate().is_err());
        let mut bad_sku = adjust(Decimal::ONE, "ok");
        bad_sku.sku = String::new();
        assert!(bad_sku.validate().is_err());
        let mut bad_location = adjust(Decimal::ONE, "ok");
        bad_location.location_id = Some(0);
        assert!(bad_location.validate().is_err());
    }

    #[test]
    fn reorder_threshold_includes_safety_stock() {
        let balance = InventoryBalance {
            id: 1,
            item_id: 1,
            location_id: 1,
            quantity_on_hand: Decimal::from(12),
            quantity_allocated: Decimal::ZERO,
            quantity_available: Decimal::from(12),
            reorder_point: Some(Decimal::from(10)),
            safety_stock: Some(Decimal::from(5)),
            version: 1,
            last_counted_at: None,
            updated_at: Utc::now(),
        };
        assert_eq!(balance.reorder_threshold(), Some(Decimal::from(15)));
        assert!(balance.needs_reorder());
        let no_point = InventoryBalance { reorder_point: None, ..balance };
        assert_eq!(no_point.reorder_threshold(), None);
        assert!(!no_point.needs_reorder());
    }
}
