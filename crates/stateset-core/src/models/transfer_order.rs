//! Transfer order domain models
//!
//! A transfer order moves inventory from a source warehouse/location to a
//! destination warehouse/location. Unlike an ad-hoc `InventoryMovement`, a
//! transfer order is a tracked, multi-line document with a lifecycle
//! (draft → in transit → received).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{ProductId, TransferOrderId, TransferOrderItemId, WarehouseId};
use strum::{Display, EnumString};

/// Lifecycle status of a transfer order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum TransferOrderStatus {
    /// Created but not yet dispatched.
    #[default]
    Draft,
    /// Stock has been committed/picked at the source.
    Pending,
    /// Stock has left the source and is in transit.
    InTransit,
    /// Some lines have been received at the destination.
    PartiallyReceived,
    /// All lines received at the destination.
    Received,
    /// Cancelled before completion.
    Cancelled,
}

impl TransferOrderStatus {
    /// Whether the order is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Received | Self::Cancelled)
    }
}

/// A single line on a transfer order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOrderItem {
    /// Unique line ID.
    pub id: TransferOrderItemId,
    /// Owning transfer order.
    pub transfer_order_id: TransferOrderId,
    /// Product being transferred.
    pub product_id: ProductId,
    /// SKU snapshot.
    pub sku: String,
    /// Quantity requested to transfer.
    pub quantity: Decimal,
    /// Quantity actually shipped from the source.
    pub quantity_shipped: Decimal,
    /// Quantity received at the destination.
    pub quantity_received: Decimal,
}

impl TransferOrderItem {
    /// Outstanding quantity still to be received.
    #[must_use]
    pub fn quantity_outstanding(&self) -> Decimal {
        (self.quantity - self.quantity_received).max(Decimal::ZERO)
    }

    /// Whether this line has been fully received.
    #[must_use]
    pub fn is_fully_received(&self) -> bool {
        self.quantity_received >= self.quantity
    }
}

/// A transfer order moving stock between two warehouses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOrder {
    /// Unique transfer order ID.
    pub id: TransferOrderId,
    /// Human-readable transfer order number.
    pub number: String,
    /// Source warehouse.
    pub source_warehouse_id: WarehouseId,
    /// Destination warehouse.
    pub destination_warehouse_id: WarehouseId,
    /// Lifecycle status.
    pub status: TransferOrderStatus,
    /// Line items.
    pub items: Vec<TransferOrderItem>,
    /// Expected arrival date.
    pub expected_at: Option<DateTime<Utc>>,
    /// When stock was shipped from the source.
    pub shipped_at: Option<DateTime<Utc>>,
    /// When the transfer was fully received.
    pub received_at: Option<DateTime<Utc>>,
    /// Free-form notes.
    pub notes: Option<String>,
    /// When the order was created.
    pub created_at: DateTime<Utc>,
    /// When the order was last updated.
    pub updated_at: DateTime<Utc>,
}

impl TransferOrder {
    /// Total quantity requested across all lines.
    #[must_use]
    pub fn total_quantity(&self) -> Decimal {
        self.items.iter().map(|i| i.quantity).sum()
    }

    /// Total quantity received across all lines.
    #[must_use]
    pub fn total_received(&self) -> Decimal {
        self.items.iter().map(|i| i.quantity_received).sum()
    }

    /// Compute the status implied by current line receipts. Does not mutate.
    #[must_use]
    pub fn derive_receipt_status(&self) -> TransferOrderStatus {
        if self.status == TransferOrderStatus::Cancelled {
            return TransferOrderStatus::Cancelled;
        }
        let received = self.total_received();
        let total = self.total_quantity();
        if received <= Decimal::ZERO {
            self.status
        } else if received >= total {
            TransferOrderStatus::Received
        } else {
            TransferOrderStatus::PartiallyReceived
        }
    }
}

/// A line on a create-transfer-order request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferOrderItem {
    /// Product to transfer.
    pub product_id: ProductId,
    /// Quantity to transfer.
    pub quantity: Decimal,
}

/// Input for creating a transfer order. Source and destination must differ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferOrder {
    /// Source warehouse.
    pub source_warehouse_id: WarehouseId,
    /// Destination warehouse.
    pub destination_warehouse_id: WarehouseId,
    /// Line items (at least one required).
    pub items: Vec<CreateTransferOrderItem>,
    /// Expected arrival date.
    pub expected_at: Option<DateTime<Utc>>,
    /// Notes.
    pub notes: Option<String>,
}

/// Filter for listing transfer orders.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransferOrderFilter {
    /// Filter by status.
    pub status: Option<TransferOrderStatus>,
    /// Filter by source warehouse.
    pub source_warehouse_id: Option<WarehouseId>,
    /// Filter by destination warehouse.
    pub destination_warehouse_id: Option<WarehouseId>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_item(qty: Decimal, received: Decimal) -> TransferOrderItem {
        TransferOrderItem {
            id: TransferOrderItemId::new(),
            transfer_order_id: TransferOrderId::new(),
            product_id: ProductId::new(),
            sku: "SKU-1".to_string(),
            quantity: qty,
            quantity_shipped: qty,
            quantity_received: received,
        }
    }

    fn make_order(items: Vec<TransferOrderItem>, status: TransferOrderStatus) -> TransferOrder {
        TransferOrder {
            id: TransferOrderId::new(),
            number: "TO-1".to_string(),
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            status,
            items,
            expected_at: None,
            shipped_at: None,
            received_at: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn item_outstanding_and_fully_received() {
        let item = make_item(dec!(10), dec!(4));
        assert_eq!(item.quantity_outstanding(), dec!(6));
        assert!(!item.is_fully_received());

        let done = make_item(dec!(10), dec!(10));
        assert_eq!(done.quantity_outstanding(), dec!(0));
        assert!(done.is_fully_received());
    }

    #[test]
    fn item_outstanding_never_negative() {
        let over = make_item(dec!(10), dec!(12));
        assert_eq!(over.quantity_outstanding(), dec!(0));
    }

    #[test]
    fn totals_sum_across_lines() {
        let order = make_order(
            vec![make_item(dec!(10), dec!(2)), make_item(dec!(5), dec!(5))],
            TransferOrderStatus::InTransit,
        );
        assert_eq!(order.total_quantity(), dec!(15));
        assert_eq!(order.total_received(), dec!(7));
    }

    #[test]
    fn derive_status_partial() {
        let order = make_order(vec![make_item(dec!(10), dec!(2))], TransferOrderStatus::InTransit);
        assert_eq!(order.derive_receipt_status(), TransferOrderStatus::PartiallyReceived);
    }

    #[test]
    fn derive_status_full() {
        let order = make_order(vec![make_item(dec!(10), dec!(10))], TransferOrderStatus::InTransit);
        assert_eq!(order.derive_receipt_status(), TransferOrderStatus::Received);
    }

    #[test]
    fn derive_status_none_keeps_current() {
        let order = make_order(vec![make_item(dec!(10), dec!(0))], TransferOrderStatus::InTransit);
        assert_eq!(order.derive_receipt_status(), TransferOrderStatus::InTransit);
    }

    #[test]
    fn derive_status_cancelled_sticky() {
        let order = make_order(vec![make_item(dec!(10), dec!(10))], TransferOrderStatus::Cancelled);
        assert_eq!(order.derive_receipt_status(), TransferOrderStatus::Cancelled);
    }

    #[test]
    fn terminal_states() {
        assert!(TransferOrderStatus::Received.is_terminal());
        assert!(TransferOrderStatus::Cancelled.is_terminal());
        assert!(!TransferOrderStatus::InTransit.is_terminal());
    }

    #[test]
    fn status_roundtrip() {
        for s in [
            TransferOrderStatus::Draft,
            TransferOrderStatus::Pending,
            TransferOrderStatus::InTransit,
            TransferOrderStatus::PartiallyReceived,
            TransferOrderStatus::Received,
            TransferOrderStatus::Cancelled,
        ] {
            let parsed: TransferOrderStatus = s.to_string().parse().unwrap();
            assert_eq!(parsed, s);
        }
    }
}
