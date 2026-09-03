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
    /// Lot the returned units were restored to (recorded at disposition).
    #[serde(default)]
    pub lot_id: Option<Uuid>,
    /// Serial numbers physically received with this line (recorded at
    /// disposition; each is transitioned through `returned` to the state the
    /// disposition implies).
    #[serde(default)]
    pub serial_ids: Vec<Uuid>,
}

impl ReturnItem {
    /// Whether this item's disposition put units back into warehouse stock.
    #[must_use]
    pub fn has_stock_effect(&self) -> bool {
        self.disposition.is_some_and(ReturnDisposition::affects_stock)
    }
}

/// What the warehouse does with a received return item.
///
/// Stock effects (applied atomically with the disposition write):
/// - `Restock`: warehouse-level `on_hand += quantity`; into the returns (or
///   quarantine) bin when the warehouse has bins.
/// - `Quarantine`: warehouse-level `on_hand += quantity` and
///   `allocated += quantity` (held, not sellable); mirrored into the
///   quarantine bin when the warehouse has one. Without bins the hold is
///   still recorded at warehouse level so received units never vanish.
/// - `Refurbish`, `Scrap`, `ReturnToVendor`: no stock change.
///
/// Serial / lot effects (when the disposition names them): every serial is
/// first marked `returned` (owner cleared) and then moved to
/// [`Self::serial_target`]; a lot regains the units on `Restock`
/// (`quantity_remaining`) and `Quarantine` (`quantity_remaining` +
/// `quantity_quarantined`).
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

    /// Whether this disposition restores units to their lot.
    #[must_use]
    pub const fn restores_lot(self) -> bool {
        matches!(self, Self::Restock | Self::Quarantine)
    }

    /// The serial status a serial received under this disposition ends in
    /// (after passing through `returned`). `None` leaves it `returned`.
    #[must_use]
    pub const fn serial_target(self) -> Option<crate::SerialStatus> {
        match self {
            Self::Restock => Some(crate::SerialStatus::Available),
            Self::Quarantine => Some(crate::SerialStatus::Quarantined),
            Self::Scrap => Some(crate::SerialStatus::Scrapped),
            Self::Refurbish => Some(crate::SerialStatus::InService),
            Self::ReturnToVendor => None,
        }
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
    /// Lot the units belong to; on `Restock` / `Quarantine` its on-hand is
    /// restored in the same transaction. Must carry the item's SKU.
    #[serde(default)]
    pub lot_id: Option<Uuid>,
    /// Serial numbers physically received. When non-empty the count must equal
    /// the item quantity and every serial must carry the item's SKU; each is
    /// marked `returned` and then moved to the disposition's target status.
    #[serde(default)]
    pub serial_ids: Vec<Uuid>,
}

impl Default for SetReturnDisposition {
    fn default() -> Self {
        Self {
            disposition: ReturnDisposition::Restock,
            warehouse_id: None,
            bin_id: None,
            disposition_by: None,
            lot_id: None,
            serial_ids: Vec::new(),
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
    ///
    /// This is the pure state-machine edge; the repositories layer two more
    /// guards on top of it inside the transition transaction:
    /// - `Rejected` / `Cancelled` are refused once any item has been
    ///   dispositioned at all — restocked, quarantined, refurbished, scrapped
    ///   or returned to vendor. The units have either re-entered stock or been
    ///   destroyed; releasing the return's claim on the order line would let
    ///   the same units be returned (and refunded) a second time;
    /// - `Completed` requires every item to be dispositioned unless the caller
    ///   explicitly writes the rest off (`UpdateReturn::write_off_undispositioned`).
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match self {
            Self::Requested => matches!(next, Self::Approved | Self::Rejected | Self::Cancelled),
            Self::Approved => matches!(next, Self::InTransit | Self::Cancelled),
            Self::InTransit => matches!(next, Self::Received),
            // Inspection is optional: a return may complete directly when no
            // disposition workflow is required.
            Self::Received => matches!(next, Self::Inspecting | Self::Completed),
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
    /// Refund total. Must be non-negative and at most the sum of the line
    /// refund amounts; immutable once the return is terminal.
    pub refund_amount: Option<Decimal>,
    /// How the refund is settled. `None` or [`REFUND_METHOD_ORIGINAL_PAYMENT`]
    /// creates payment refunds against the order's captured payments on
    /// completion; any other value (store credit, exchange, ...) is recorded
    /// and capped but settled outside the payments ledger.
    pub refund_method: Option<String>,
    pub notes: Option<String>,
    /// Complete the return even though some received items have no
    /// disposition. The undispositioned units are written off: they are
    /// neither restocked nor tracked, and the completion event records
    /// `undispositioned_units`. Off by default so received goods cannot
    /// silently vanish.
    #[serde(default)]
    pub write_off_undispositioned: bool,
}

/// `refund_method` value meaning "refund to the payments that captured the
/// order" (also the meaning of `None`).
pub const REFUND_METHOD_ORIGINAL_PAYMENT: &str = "original_payment";

impl UpdateReturn {
    /// Whether `method` settles through the payments ledger (payment refunds
    /// are created on completion) rather than out of band.
    #[must_use]
    pub fn refund_method_uses_payments(method: Option<&str>) -> bool {
        match method {
            None => true,
            Some(m) => {
                let m = m.trim().to_ascii_lowercase();
                m.is_empty()
                    || m == REFUND_METHOD_ORIGINAL_PAYMENT
                    || m == "original"
                    || m == "payment"
                    || m == "card"
            }
        }
    }
}

/// Kernel command payload for a return state-machine transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionReturn {
    pub return_id: ReturnId,
    pub status: ReturnStatus,
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

    /// Check that moving this return to `next` is allowed, including the
    /// guards layered on the pure state machine (see
    /// [`ReturnStatus::can_transition_to`]). Every backend and the kernel
    /// executor route status writes through this so the rules cannot drift.
    ///
    /// Errors: `ValidationError` for an illegal edge, `Conflict` for
    /// rejecting/cancelling after any item was dispositioned, and
    /// `NotPermitted` for completing with undispositioned items when
    /// `write_off_undispositioned` is false.
    pub fn check_transition(
        &self,
        next: ReturnStatus,
        write_off_undispositioned: bool,
    ) -> Result<()> {
        use crate::CommerceError;
        if !self.status.can_transition_to(next) {
            return Err(CommerceError::ValidationError(format!(
                "Invalid return status transition from {} to {next}",
                self.status
            )));
        }
        if next == self.status {
            return Ok(());
        }
        if matches!(next, ReturnStatus::Rejected | ReturnStatus::Cancelled) {
            // ANY disposition pins the return, not just the stock-affecting
            // ones: scrapped and returned-to-vendor goods are physically gone,
            // so releasing the claim on the order line would let the same units
            // be returned — and refunded — a second time.
            let dispositioned: i64 = self
                .items
                .iter()
                .filter(|item| item.disposition.is_some())
                .map(|item| i64::from(item.quantity))
                .sum();
            if dispositioned > 0 {
                return Err(CommerceError::Conflict(format!(
                    "Return {} cannot be {next}: {dispositioned} unit(s) were already \
                     dispositioned (restocked, quarantined, scrapped, refurbished or returned \
                     to vendor) and the return must keep its claim on the order line",
                    self.id
                )));
            }
        }
        if next == ReturnStatus::Completed && !write_off_undispositioned {
            let pending = self.undispositioned_items().count();
            if pending > 0 {
                return Err(CommerceError::NotPermitted(format!(
                    "Return {} cannot be completed: {pending} item(s) have no disposition; \
                     disposition them or set write_off_undispositioned",
                    self.id
                )));
            }
        }
        Ok(())
    }

    /// Items that have not been dispositioned yet.
    pub fn undispositioned_items(&self) -> impl Iterator<Item = &ReturnItem> {
        self.items.iter().filter(|item| item.disposition.is_none())
    }

    /// Whether any item's disposition put units back into stock.
    #[must_use]
    pub fn has_stock_disposition(&self) -> bool {
        self.items.iter().any(ReturnItem::has_stock_effect)
    }

    /// Whether any item has been dispositioned at all (stock-affecting or not).
    #[must_use]
    pub fn has_any_disposition(&self) -> bool {
        self.items.iter().any(|item| item.disposition.is_some())
    }

    /// Check that this return may be hard-deleted.
    ///
    /// A return is a *claim* on its order line: [`Self::check_transition`] and
    /// the repositories' over-return guard count the units of every
    /// non-terminal return against what remains returnable. Deleting the row
    /// silently drops that claim, so a completed, restocked, refunded return
    /// could be deleted and the same units returned and refunded again while
    /// the restocked stock stayed on the shelf.
    ///
    /// Deletion is therefore allowed only in the early, no-effect window —
    /// `Requested` and `Approved`, and only while no item carries a
    /// disposition:
    ///
    /// - `InTransit` / `Received` / `Inspecting`: the goods are in motion or in
    ///   the building; the return is the only record of them.
    /// - `Completed`: stock, serial, lot and refund effects have been applied.
    /// - `Rejected` / `Cancelled`: terminal audit records of a decision.
    ///
    /// Anything outside the window must be cancelled (while it still can be) or
    /// left in place; there is no soft-delete.
    ///
    /// Errors: `NotPermitted` with the blocking status/disposition.
    pub fn check_deletable(&self) -> Result<()> {
        use crate::CommerceError;
        if !matches!(self.status, ReturnStatus::Requested | ReturnStatus::Approved) {
            return Err(CommerceError::NotPermitted(format!(
                "Return {} is {}; only returns still in the early no-effect window \
                 (requested, approved) may be deleted — deleting a later one would free its \
                 claim on the order line and let the same units be returned and refunded again",
                self.id, self.status
            )));
        }
        if self.has_any_disposition() {
            return Err(CommerceError::NotPermitted(format!(
                "Return {} cannot be deleted: at least one item has been dispositioned",
                self.id
            )));
        }
        Ok(())
    }

    /// Sum of the line refund amounts: the cap for `refund_amount`.
    #[must_use]
    pub fn max_refund(&self) -> Decimal {
        self.calculate_refund_total()
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
    use crate::SerialStatus;
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
        assert!(ReturnStatus::Received.can_transition_to(ReturnStatus::Completed));
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

    #[test]
    fn refund_method_classification() {
        assert!(UpdateReturn::refund_method_uses_payments(None));
        assert!(UpdateReturn::refund_method_uses_payments(Some("original_payment")));
        assert!(UpdateReturn::refund_method_uses_payments(Some(" Original_Payment ")));
        assert!(!UpdateReturn::refund_method_uses_payments(Some("store_credit")));
        assert!(!UpdateReturn::refund_method_uses_payments(Some("exchange")));
    }

    #[test]
    fn disposition_serial_targets() {
        assert_eq!(ReturnDisposition::Restock.serial_target(), Some(SerialStatus::Available));
        assert_eq!(ReturnDisposition::Quarantine.serial_target(), Some(SerialStatus::Quarantined));
        assert_eq!(ReturnDisposition::Scrap.serial_target(), Some(SerialStatus::Scrapped));
        assert_eq!(ReturnDisposition::ReturnToVendor.serial_target(), None);
        assert!(ReturnDisposition::Restock.restores_lot());
        assert!(!ReturnDisposition::Scrap.restores_lot());
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

    fn return_with(status: ReturnStatus, disposition: Option<ReturnDisposition>) -> Return {
        let id = ReturnId::new();
        Return {
            id,
            order_id: OrderId::from_uuid(Uuid::new_v4()),
            customer_id: CustomerId::from_uuid(Uuid::new_v4()),
            status,
            reason: ReturnReason::Damaged,
            reason_details: None,
            idempotency_key: None,
            refund_amount: None,
            refund_method: None,
            tracking_number: None,
            items: vec![ReturnItem {
                id: Uuid::new_v4(),
                return_id: id,
                order_item_id: OrderItemId::from_uuid(Uuid::new_v4()),
                sku: "SKU-1".into(),
                name: "Widget".into(),
                quantity: 2,
                condition: ItemCondition::Damaged,
                refund_amount: Decimal::ZERO,
                disposition,
                disposition_at: None,
                disposition_by: None,
                lot_id: None,
                serial_ids: Vec::new(),
            }],
            notes: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Scrap / return-to-vendor destroy the goods without a stock effect;
    /// rejecting after them used to be legal and released the order-line claim.
    #[test]
    fn reject_and_cancel_are_refused_after_any_disposition() {
        for disposition in [
            ReturnDisposition::Restock,
            ReturnDisposition::Quarantine,
            ReturnDisposition::Scrap,
            ReturnDisposition::ReturnToVendor,
            ReturnDisposition::Refurbish,
        ] {
            let ret = return_with(ReturnStatus::Inspecting, Some(disposition));
            let err = ret
                .check_transition(ReturnStatus::Rejected, false)
                .expect_err("reject after a disposition");
            assert!(matches!(err, crate::CommerceError::Conflict(_)), "{disposition}: got {err:?}");
            assert!(ret.has_any_disposition());
        }
        let clean = return_with(ReturnStatus::Inspecting, None);
        assert!(clean.check_transition(ReturnStatus::Rejected, false).is_ok());
        assert!(!clean.has_any_disposition());
    }

    #[test]
    fn only_requested_and_approved_returns_are_deletable() {
        for status in [ReturnStatus::Requested, ReturnStatus::Approved] {
            assert!(return_with(status, None).check_deletable().is_ok(), "{status}");
        }
        for status in [
            ReturnStatus::InTransit,
            ReturnStatus::Received,
            ReturnStatus::Inspecting,
            ReturnStatus::Completed,
            ReturnStatus::Rejected,
            ReturnStatus::Cancelled,
        ] {
            let err = return_with(status, None).check_deletable().expect_err("not deletable");
            assert!(matches!(err, crate::CommerceError::NotPermitted(_)), "{status}: {err:?}");
        }
        // A disposition inside the window (data drift) also blocks the delete.
        let err = return_with(ReturnStatus::Approved, Some(ReturnDisposition::Scrap))
            .check_deletable()
            .expect_err("dispositioned return is not deletable");
        assert!(matches!(err, crate::CommerceError::NotPermitted(_)), "got {err:?}");
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
