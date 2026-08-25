//! Returns domain models

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, OrderId, OrderItemId, ReturnId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Return entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Return {
    pub id: ReturnId,
    pub order_id: OrderId,
    pub customer_id: CustomerId,
    pub status: ReturnStatus,
    pub reason: ReturnReason,
    pub reason_details: Option<String>,
    pub idempotency_key: Option<String>,
    pub refund_amount: Option<Decimal>,
    pub refund_method: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<ReturnItem>,
    pub notes: Option<String>,
    /// Version for optimistic locking
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Return line item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnItem {
    pub id: Uuid,
    pub return_id: ReturnId,
    pub order_item_id: OrderItemId,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub condition: ItemCondition,
    pub refund_amount: Decimal,
    /// Warehouse disposition recorded once the item is physically received.
    pub disposition: Option<ReturnDisposition>,
    pub disposition_at: Option<DateTime<Utc>>,
    pub disposition_by: Option<String>,
}

/// What the warehouse does with a received return item.
///
/// Stock effects (applied atomically with the disposition write):
/// - `Restock`: warehouse-level `on_hand += quantity`; into the returns (or
///   quarantine) bin when the warehouse has bins.
/// - `Quarantine`: when a quarantine bin exists, `on_hand += quantity` and
///   `allocated += quantity` (held, not sellable) at bin and warehouse level;
///   without bins nothing is touched.
/// - `Refurbish`, `Scrap`, `ReturnToVendor`: no stock change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReturnDisposition {
    Restock,
    Refurbish,
    Scrap,
    #[strum(serialize = "return_to_vendor", serialize = "returntovendor")]
    ReturnToVendor,
    Quarantine,
}

impl ReturnDisposition {
    /// Whether this disposition puts units back into warehouse stock.
    #[must_use]
    pub const fn affects_stock(self) -> bool {
        matches!(self, Self::Restock | Self::Quarantine)
    }
}

/// Input for recording a return item's disposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetReturnDisposition {
    pub disposition: ReturnDisposition,
    /// Warehouse receiving the stock (defaults to 1, the default location).
    pub warehouse_id: Option<i32>,
    /// Explicit target bin; when omitted a `returns` (Restock) or `quarantine`
    /// bin of the warehouse is used if one exists.
    pub bin_id: Option<i32>,
    pub disposition_by: Option<String>,
}

impl Default for SetReturnDisposition {
    fn default() -> Self {
        Self {
            disposition: ReturnDisposition::Restock,
            warehouse_id: None,
            bin_id: None,
            disposition_by: None,
        }
    }
}

/// Return status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReturnStatus {
    Requested,
    Approved,
    Rejected,
    #[strum(serialize = "in_transit", serialize = "intransit")]
    InTransit,
    Received,
    Inspecting,
    Completed,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
}

impl Default for ReturnStatus {
    fn default() -> Self {
        Self::Requested
    }
}

impl ReturnStatus {
    /// Check if a status transition is allowed.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match self {
            Self::Requested => matches!(next, Self::Approved | Self::Rejected | Self::Cancelled),
            Self::Approved => matches!(next, Self::InTransit | Self::Cancelled),
            Self::InTransit => matches!(next, Self::Received),
            Self::Received => matches!(next, Self::Inspecting),
            Self::Inspecting => matches!(next, Self::Completed | Self::Rejected),
            Self::Rejected | Self::Completed | Self::Cancelled => false,
        }
    }

    /// Returns true if this status is a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Completed | Self::Cancelled)
    }
}

/// Return reason enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReturnReason {
    Defective,
    #[strum(serialize = "wrong_item", serialize = "wrongitem")]
    WrongItem,
    #[strum(serialize = "not_as_described", serialize = "notasdescribed")]
    NotAsDescribed,
    #[strum(serialize = "changed_mind", serialize = "changedmind")]
    ChangedMind,
    #[strum(serialize = "better_price_found", serialize = "betterpricefound")]
    BetterPriceFound,
    #[strum(serialize = "no_longer_needed", serialize = "nolongerneeded")]
    NoLongerNeeded,
    Damaged,
    Other,
}

impl Default for ReturnReason {
    fn default() -> Self {
        Self::Other
    }
}

/// Item condition on return
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ItemCondition {
    New,
    Opened,
    Used,
    Damaged,
    Defective,
}

impl Default for ItemCondition {
    fn default() -> Self {
        Self::New
    }
}
/// Input for creating a return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReturn {
    pub order_id: OrderId,
    pub reason: ReturnReason,
    pub reason_details: Option<String>,
    pub idempotency_key: Option<String>,
    pub items: Vec<CreateReturnItem>,
    pub notes: Option<String>,
}

impl Default for CreateReturn {
    fn default() -> Self {
        Self {
            order_id: OrderId::from_uuid(Uuid::nil()),
            reason: ReturnReason::Other,
            reason_details: None,
            idempotency_key: None,
            items: vec![],
            notes: None,
        }
    }
}

/// Input for creating a return item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReturnItem {
    pub order_item_id: OrderItemId,
    pub quantity: i32,
    pub condition: Option<ItemCondition>,
}

impl Default for CreateReturnItem {
    fn default() -> Self {
        Self { order_item_id: OrderItemId::from_uuid(Uuid::nil()), quantity: 0, condition: None }
    }
}

impl Validate for CreateReturnItem {
    /// Validate a single return line item.
    ///
    /// Requires a non-nil order item reference and a positive return quantity:
    /// you cannot return zero or a negative number of units.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .uuid_not_nil("order_item_id", self.order_item_id.into_uuid())
            .positive_i32("quantity", self.quantity)
            .build()
    }
}

impl Validate for CreateReturn {
    /// Validate a return create request.
    ///
    /// Requires a non-nil order reference, at least one return item, and
    /// validates each item (non-positive quantities are rejected).
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .uuid_not_nil("order_id", self.order_id.into_uuid())
            .non_empty_list("items", &self.items)
            .build()?;

        for item in &self.items {
            item.validate()?;
        }

        Ok(())
    }
}

/// Input for updating a return
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateReturn {
    pub status: Option<ReturnStatus>,
    pub tracking_number: Option<String>,
    pub refund_amount: Option<Decimal>,
    pub refund_method: Option<String>,
    pub notes: Option<String>,
}

/// Return filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReturnFilter {
    pub order_id: Option<OrderId>,
    pub customer_id: Option<CustomerId>,
    pub status: Option<ReturnStatus>,
    pub reason: Option<ReturnReason>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

impl Return {
    /// Calculate total refund amount from items
    #[must_use]
    pub fn calculate_refund_total(&self) -> Decimal {
        self.items.iter().map(|item| item.refund_amount).sum()
    }

    /// Check if return can be approved
    #[must_use]
    pub fn can_approve(&self) -> bool {
        self.status == ReturnStatus::Requested
    }

    /// Check if return can be completed
    #[must_use]
    pub const fn can_complete(&self) -> bool {
        matches!(self.status, ReturnStatus::Received | ReturnStatus::Inspecting)
    }

    /// Check if refund is eligible based on reason
    #[must_use]
    pub const fn is_refund_eligible(&self) -> bool {
        matches!(
            self.reason,
            ReturnReason::Defective
                | ReturnReason::WrongItem
                | ReturnReason::NotAsDescribed
                | ReturnReason::Damaged
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_return_status_from_str() {
        assert_eq!(ReturnStatus::from_str("in_transit").unwrap(), ReturnStatus::InTransit);
        assert_eq!(ReturnStatus::from_str("intransit").unwrap(), ReturnStatus::InTransit);
        assert_eq!(ReturnStatus::from_str("canceled").unwrap(), ReturnStatus::Cancelled);
    }

    #[test]
    fn test_return_reason_from_str() {
        assert_eq!(ReturnReason::from_str("wrong_item").unwrap(), ReturnReason::WrongItem);
        assert_eq!(ReturnReason::from_str("wrongitem").unwrap(), ReturnReason::WrongItem);
        assert_eq!(ReturnReason::from_str("notasdescribed").unwrap(), ReturnReason::NotAsDescribed);
        assert_eq!(
            ReturnReason::from_str("no_longer_needed").unwrap(),
            ReturnReason::NoLongerNeeded
        );
    }

    #[test]
    fn test_item_condition_from_str() {
        assert_eq!(ItemCondition::from_str("opened").unwrap(), ItemCondition::Opened);
        assert_eq!(ItemCondition::from_str("damaged").unwrap(), ItemCondition::Damaged);
    }

    #[test]
    fn return_status_valid_transitions() {
        assert!(ReturnStatus::Requested.can_transition_to(ReturnStatus::Approved));
        assert!(ReturnStatus::Requested.can_transition_to(ReturnStatus::Rejected));
        assert!(ReturnStatus::Requested.can_transition_to(ReturnStatus::Cancelled));
        assert!(ReturnStatus::Approved.can_transition_to(ReturnStatus::InTransit));
        assert!(ReturnStatus::Approved.can_transition_to(ReturnStatus::Cancelled));
        assert!(ReturnStatus::InTransit.can_transition_to(ReturnStatus::Received));
        assert!(ReturnStatus::Received.can_transition_to(ReturnStatus::Inspecting));
        assert!(ReturnStatus::Inspecting.can_transition_to(ReturnStatus::Completed));
        assert!(ReturnStatus::Inspecting.can_transition_to(ReturnStatus::Rejected));
    }

    #[test]
    fn return_status_invalid_transitions() {
        assert!(!ReturnStatus::Requested.can_transition_to(ReturnStatus::Completed));
        assert!(!ReturnStatus::Requested.can_transition_to(ReturnStatus::InTransit));
        assert!(!ReturnStatus::Approved.can_transition_to(ReturnStatus::Completed));
        assert!(!ReturnStatus::InTransit.can_transition_to(ReturnStatus::Completed));
        assert!(!ReturnStatus::Completed.can_transition_to(ReturnStatus::Requested));
    }

    #[test]
    fn return_status_terminal_states() {
        assert!(ReturnStatus::Rejected.is_terminal());
        assert!(ReturnStatus::Completed.is_terminal());
        assert!(ReturnStatus::Cancelled.is_terminal());
        assert!(!ReturnStatus::Requested.is_terminal());
        assert!(!ReturnStatus::Approved.is_terminal());
        assert!(!ReturnStatus::InTransit.is_terminal());
        assert!(!ReturnStatus::Received.is_terminal());
        assert!(!ReturnStatus::Inspecting.is_terminal());
    }

    fn valid_return_item() -> CreateReturnItem {
        CreateReturnItem {
            order_item_id: OrderItemId::from_uuid(Uuid::new_v4()),
            quantity: 1,
            condition: None,
        }
    }

    #[test]
    fn create_return_item_rejects_non_positive_quantity() {
        for qty in [0, -1] {
            let item = CreateReturnItem { quantity: qty, ..valid_return_item() };
            let err = item.validate().expect_err("non-positive quantity must be rejected");
            assert!(
                matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "quantity")
            );
        }
    }

    #[test]
    fn create_return_item_accepts_positive_quantity() {
        assert!(valid_return_item().validate().is_ok());
    }

    #[test]
    fn create_return_rejects_empty_items() {
        let input = CreateReturn {
            order_id: OrderId::from_uuid(Uuid::new_v4()),
            items: vec![],
            ..Default::default()
        };
        let err = input.validate().expect_err("a return with no items must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "items")
        );
    }

    #[test]
    fn create_return_rejects_item_with_non_positive_quantity() {
        let input = CreateReturn {
            order_id: OrderId::from_uuid(Uuid::new_v4()),
            items: vec![CreateReturnItem { quantity: 0, ..valid_return_item() }],
            ..Default::default()
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_return_accepts_valid_request() {
        let input = CreateReturn {
            order_id: OrderId::from_uuid(Uuid::new_v4()),
            items: vec![valid_return_item()],
            ..Default::default()
        };
        assert!(input.validate().is_ok());
    }
}
