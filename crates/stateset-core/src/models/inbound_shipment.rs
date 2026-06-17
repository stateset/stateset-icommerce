//! Inbound shipment domain models
//!
//! An inbound shipment (advance ship notice / ASN) represents goods in transit
//! from a supplier toward a receiving warehouse. It carries expected line
//! quantities and a lifecycle (pending → in transit → arrived → received) that
//! sits upstream of a receiving report. Distinct from outbound `Shipment`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{InboundShipmentId, InboundShipmentItemId, ProductId, WarehouseId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of an inbound shipment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum InboundShipmentStatus {
    /// Announced but not yet shipped by the supplier.
    #[default]
    Pending,
    /// Shipped and in transit.
    InTransit,
    /// Physically arrived at the warehouse, awaiting receipt.
    Arrived,
    /// Partially received into stock.
    PartiallyReceived,
    /// Fully received.
    Received,
    /// Cancelled.
    Cancelled,
}

impl InboundShipmentStatus {
    /// Whether the shipment is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Received | Self::Cancelled)
    }
}

/// A single expected line on an inbound shipment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundShipmentItem {
    /// Unique line ID.
    pub id: InboundShipmentItemId,
    /// Owning inbound shipment.
    pub inbound_shipment_id: InboundShipmentId,
    /// Product expected.
    pub product_id: ProductId,
    /// SKU snapshot.
    pub sku: String,
    /// Quantity expected.
    pub quantity_expected: Decimal,
    /// Quantity received so far.
    pub quantity_received: Decimal,
}

impl InboundShipmentItem {
    /// Outstanding quantity still to be received (never negative).
    #[must_use]
    pub fn quantity_outstanding(&self) -> Decimal {
        (self.quantity_expected - self.quantity_received).max(Decimal::ZERO)
    }
}

/// Goods in transit from a supplier to a warehouse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundShipment {
    /// Unique inbound shipment ID.
    pub id: InboundShipmentId,
    /// Human-readable shipment number.
    pub number: String,
    /// Supplier sending the goods.
    pub supplier_id: Uuid,
    /// Originating purchase order, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Destination warehouse, if known.
    pub warehouse_id: Option<WarehouseId>,
    /// Carrier name.
    pub carrier: Option<String>,
    /// Tracking number.
    pub tracking_number: Option<String>,
    /// Lifecycle status.
    pub status: InboundShipmentStatus,
    /// Line items.
    pub items: Vec<InboundShipmentItem>,
    /// Expected arrival date.
    pub expected_at: Option<DateTime<Utc>>,
    /// When fully received.
    pub received_at: Option<DateTime<Utc>>,
    /// Notes.
    pub notes: Option<String>,
    /// When the shipment was created.
    pub created_at: DateTime<Utc>,
    /// When the shipment was last updated.
    pub updated_at: DateTime<Utc>,
}

impl InboundShipment {
    /// Total expected quantity across all lines.
    #[must_use]
    pub fn total_expected(&self) -> Decimal {
        self.items.iter().map(|i| i.quantity_expected).sum()
    }

    /// Total received quantity across all lines.
    #[must_use]
    pub fn total_received(&self) -> Decimal {
        self.items.iter().map(|i| i.quantity_received).sum()
    }

    /// Status implied by current receipts (cancelled stays cancelled).
    #[must_use]
    pub fn derive_receipt_status(&self) -> InboundShipmentStatus {
        if self.status == InboundShipmentStatus::Cancelled {
            return InboundShipmentStatus::Cancelled;
        }
        let received = self.total_received();
        if received <= Decimal::ZERO {
            self.status
        } else if received >= self.total_expected() {
            InboundShipmentStatus::Received
        } else {
            InboundShipmentStatus::PartiallyReceived
        }
    }
}

/// A line on a create-inbound-shipment request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInboundShipmentItem {
    /// Product expected.
    pub product_id: ProductId,
    /// SKU.
    pub sku: String,
    /// Quantity expected.
    pub quantity_expected: Decimal,
}

/// Input for creating an inbound shipment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInboundShipment {
    /// Supplier sending the goods.
    pub supplier_id: Uuid,
    /// Originating purchase order, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Destination warehouse.
    pub warehouse_id: Option<WarehouseId>,
    /// Carrier.
    pub carrier: Option<String>,
    /// Tracking number.
    pub tracking_number: Option<String>,
    /// Expected arrival.
    pub expected_at: Option<DateTime<Utc>>,
    /// Line items (at least one required).
    pub items: Vec<CreateInboundShipmentItem>,
    /// Notes.
    pub notes: Option<String>,
}

/// Filter for listing inbound shipments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboundShipmentFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by warehouse.
    pub warehouse_id: Option<WarehouseId>,
    /// Filter by status.
    pub status: Option<InboundShipmentStatus>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_item(expected: Decimal, received: Decimal) -> InboundShipmentItem {
        InboundShipmentItem {
            id: InboundShipmentItemId::new(),
            inbound_shipment_id: InboundShipmentId::new(),
            product_id: ProductId::new(),
            sku: "SKU-1".into(),
            quantity_expected: expected,
            quantity_received: received,
        }
    }

    fn make(items: Vec<InboundShipmentItem>, status: InboundShipmentStatus) -> InboundShipment {
        InboundShipment {
            id: InboundShipmentId::new(),
            number: "ASN-1".into(),
            supplier_id: Uuid::nil(),
            purchase_order_id: None,
            warehouse_id: None,
            carrier: Some("DHL".into()),
            tracking_number: Some("1Z999".into()),
            status,
            items,
            expected_at: None,
            received_at: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn outstanding_never_negative() {
        assert_eq!(make_item(dec!(10), dec!(4)).quantity_outstanding(), dec!(6));
        assert_eq!(make_item(dec!(10), dec!(12)).quantity_outstanding(), dec!(0));
    }

    #[test]
    fn totals_sum_lines() {
        let s = make(
            vec![make_item(dec!(10), dec!(2)), make_item(dec!(5), dec!(5))],
            InboundShipmentStatus::InTransit,
        );
        assert_eq!(s.total_expected(), dec!(15));
        assert_eq!(s.total_received(), dec!(7));
    }

    #[test]
    fn derive_status_progression() {
        let none = make(vec![make_item(dec!(10), dec!(0))], InboundShipmentStatus::Arrived);
        assert_eq!(none.derive_receipt_status(), InboundShipmentStatus::Arrived);
        let partial = make(vec![make_item(dec!(10), dec!(4))], InboundShipmentStatus::Arrived);
        assert_eq!(partial.derive_receipt_status(), InboundShipmentStatus::PartiallyReceived);
        let full = make(vec![make_item(dec!(10), dec!(10))], InboundShipmentStatus::Arrived);
        assert_eq!(full.derive_receipt_status(), InboundShipmentStatus::Received);
        let cancelled = make(vec![make_item(dec!(10), dec!(10))], InboundShipmentStatus::Cancelled);
        assert_eq!(cancelled.derive_receipt_status(), InboundShipmentStatus::Cancelled);
    }

    #[test]
    fn terminal_states() {
        assert!(InboundShipmentStatus::Received.is_terminal());
        assert!(InboundShipmentStatus::Cancelled.is_terminal());
        assert!(!InboundShipmentStatus::InTransit.is_terminal());
    }

    #[test]
    fn status_roundtrip() {
        for s in [
            InboundShipmentStatus::Pending,
            InboundShipmentStatus::InTransit,
            InboundShipmentStatus::Arrived,
            InboundShipmentStatus::PartiallyReceived,
            InboundShipmentStatus::Received,
            InboundShipmentStatus::Cancelled,
        ] {
            assert_eq!(s.to_string().parse::<InboundShipmentStatus>().unwrap(), s);
        }
    }
}
