//! Purgatory (order ingestion staging) domain models
//!
//! When an order is ingested from a sales channel it lands in "purgatory" — a
//! staging area where it is **non-posted**: not yet committed to inventory and
//! accounting. Each line must be resolved (mapped to a product, or flagged as
//! ignored / non-physical) before the order can be posted. Sits downstream of
//! [`Channel`](crate::Channel).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{ChannelId, ProductId, PurgatoryLineItemId, PurgatoryOrderId};

/// A single ingested order line awaiting resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgatoryLineItem {
    /// Unique line ID.
    pub id: PurgatoryLineItemId,
    /// Owning purgatory order.
    pub purgatory_order_id: PurgatoryOrderId,
    /// SKU as received from the external channel.
    pub external_sku: String,
    /// Internal product this line has been mapped to, if any.
    pub product_id: Option<ProductId>,
    /// Quantity ordered.
    pub quantity: Decimal,
    /// When `true`, the line is intentionally excluded (e.g. fees, notes).
    pub ignore_item: bool,
    /// When `true`, the line is non-physical (no inventory impact).
    pub non_physical: bool,
}

impl PurgatoryLineItem {
    /// Whether this line is resolved — either mapped to a product, ignored,
    /// or flagged non-physical.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        self.product_id.is_some() || self.ignore_item || self.non_physical
    }
}

/// An ingested-but-not-yet-posted order in the staging area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgatoryOrder {
    /// Unique purgatory order ID.
    pub id: PurgatoryOrderId,
    /// Channel the order was ingested from, if known.
    pub channel_id: Option<ChannelId>,
    /// External order identifier (e.g. Shopify order number).
    pub external_order_id: String,
    /// External order status string.
    pub external_status: Option<String>,
    /// Whether the order has been posted into inventory/accounting.
    pub is_posted: bool,
    /// Reason the order is held (set when posting is blocked).
    pub hold_reason: Option<String>,
    /// Raw ingested payload / metadata.
    pub metadata: serde_json::Value,
    /// Line items.
    pub items: Vec<PurgatoryLineItem>,
    /// When the order was ingested.
    pub created_at: DateTime<Utc>,
    /// When the order was last updated.
    pub updated_at: DateTime<Utc>,
}

impl PurgatoryOrder {
    /// Whether every line is resolved, so the order can be posted.
    #[must_use]
    pub fn is_ready_to_post(&self) -> bool {
        !self.items.is_empty() && self.items.iter().all(PurgatoryLineItem::is_resolved)
    }

    /// Count of unresolved lines.
    #[must_use]
    pub fn unresolved_count(&self) -> usize {
        self.items.iter().filter(|i| !i.is_resolved()).count()
    }
}

/// A line on an ingest request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestLineItem {
    /// External SKU.
    pub external_sku: String,
    /// Quantity.
    pub quantity: Decimal,
    /// Optional pre-resolved product mapping.
    pub product_id: Option<ProductId>,
}

/// Input for ingesting an order into purgatory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOrder {
    /// Source channel, if known.
    pub channel_id: Option<ChannelId>,
    /// External order identifier.
    pub external_order_id: String,
    /// External status.
    pub external_status: Option<String>,
    /// Raw payload / metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Line items (at least one required).
    pub items: Vec<IngestLineItem>,
}

/// Input for mapping / flagging a purgatory line.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MapPurgatoryLine {
    /// Product to map the line to.
    pub product_id: Option<ProductId>,
    /// Set the ignore flag.
    pub ignore_item: Option<bool>,
    /// Set the non-physical flag.
    pub non_physical: Option<bool>,
}

/// Filter for listing purgatory orders.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PurgatoryFilter {
    /// Filter by channel.
    pub channel_id: Option<ChannelId>,
    /// Filter by posted state (purgatory is typically `is_posted = false`).
    pub is_posted: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn line(product: Option<ProductId>, ignore: bool, non_physical: bool) -> PurgatoryLineItem {
        PurgatoryLineItem {
            id: PurgatoryLineItemId::new(),
            purgatory_order_id: PurgatoryOrderId::new(),
            external_sku: "EXT-1".into(),
            product_id: product,
            quantity: dec!(1),
            ignore_item: ignore,
            non_physical,
        }
    }

    fn order(items: Vec<PurgatoryLineItem>) -> PurgatoryOrder {
        PurgatoryOrder {
            id: PurgatoryOrderId::new(),
            channel_id: None,
            external_order_id: "SHOP-1001".into(),
            external_status: Some("paid".into()),
            is_posted: false,
            hold_reason: None,
            metadata: serde_json::Value::Null,
            items,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn line_resolution_rules() {
        assert!(line(Some(ProductId::new()), false, false).is_resolved());
        assert!(line(None, true, false).is_resolved());
        assert!(line(None, false, true).is_resolved());
        assert!(!line(None, false, false).is_resolved());
    }

    #[test]
    fn ready_to_post_requires_all_resolved() {
        let ready =
            order(vec![line(Some(ProductId::new()), false, false), line(None, true, false)]);
        assert!(ready.is_ready_to_post());
        assert_eq!(ready.unresolved_count(), 0);

        let not_ready =
            order(vec![line(Some(ProductId::new()), false, false), line(None, false, false)]);
        assert!(!not_ready.is_ready_to_post());
        assert_eq!(not_ready.unresolved_count(), 1);
    }

    #[test]
    fn empty_order_not_ready() {
        assert!(!order(vec![]).is_ready_to_post());
    }
}
