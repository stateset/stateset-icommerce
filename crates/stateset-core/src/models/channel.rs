//! Sales / fulfillment channel domain models
//!
//! A channel represents an external integration point through which orders
//! flow in (sales channels) and/or out (fulfillment channels). Channels own
//! the SKU mappings that translate between an external system's identifiers
//! and internal product SKUs, and can be locked to block
//! external API mutations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{ChannelId, ProductId, WarehouseId};
use strum::{Display, EnumString};

/// The kind of channel, which determines the direction of order flow.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ChannelType {
    /// Orders are ingested from this channel (e.g. Shopify, Amazon).
    #[default]
    SalesChannel,
    /// Orders are pushed to this channel for fulfillment (e.g. a 3PL/WMS).
    FulfillmentChannel,
    /// The channel handles both ingestion and fulfillment.
    EndToEndChannel,
}

/// Lifecycle status of a channel.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ChannelStatus {
    /// Channel is active and processing orders.
    #[default]
    Active,
    /// Channel is configured but paused.
    Paused,
    /// Channel has been soft-deleted.
    Deleted,
}

/// A sales / fulfillment channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel ID.
    pub id: ChannelId,
    /// Human-readable channel name.
    pub name: String,
    /// What kind of channel this is. Cannot be changed once set.
    pub channel_type: ChannelType,
    /// The external application/integration backing this channel (e.g. "shopify").
    pub integration: Option<String>,
    /// Lifecycle status.
    pub status: ChannelStatus,
    /// When `true`, external API mutations (`update`, `delete`, product sync)
    /// are rejected. Reads are always permitted.
    pub api_locked: bool,
    /// Default warehouse group / warehouse used when routing this channel's orders.
    pub default_warehouse_id: Option<WarehouseId>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Arbitrary metadata.
    pub metadata: serde_json::Value,
    /// When the channel was created.
    pub created_at: DateTime<Utc>,
    /// When the channel was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A mapping between a channel-specific SKU and an internal product SKU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelProductMapping {
    /// Channel this mapping belongs to.
    pub channel_id: ChannelId,
    /// The SKU as it appears in the external channel.
    pub channel_sku: String,
    /// The internal product this maps to.
    pub product_id: ProductId,
    /// The internal SKU this maps to.
    pub internal_sku: String,
    /// When the mapping was created.
    pub created_at: DateTime<Utc>,
    /// When the mapping was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannel {
    /// Channel name.
    pub name: String,
    /// Channel type. Required and immutable once set.
    pub channel_type: ChannelType,
    /// Backing integration, if any.
    pub integration: Option<String>,
    /// Default warehouse for routing.
    pub default_warehouse_id: Option<WarehouseId>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Input for updating a channel. Uses PATCH/merge semantics — omitted fields
/// are preserved. `channel_type` is intentionally absent; it cannot be changed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateChannel {
    /// Updated name.
    pub name: Option<String>,
    /// Updated integration.
    pub integration: Option<String>,
    /// Updated status.
    pub status: Option<ChannelStatus>,
    /// Updated default warehouse.
    pub default_warehouse_id: Option<WarehouseId>,
    /// Updated tags.
    pub tags: Option<Vec<String>>,
    /// Updated metadata.
    pub metadata: Option<serde_json::Value>,
}

/// A single item in a bulk channel-product sync request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelProductSyncItem {
    /// External channel SKU.
    pub channel_sku: String,
    /// Internal product to map to (required unless `delete` is `true`).
    pub product_id: Option<ProductId>,
    /// Internal SKU to map to.
    pub internal_sku: Option<String>,
    /// When `true`, remove the mapping for `channel_sku` instead of upserting.
    #[serde(default)]
    pub delete: bool,
}

/// Filter for listing channels.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelFilter {
    /// Filter by channel type.
    pub channel_type: Option<ChannelType>,
    /// Filter by status.
    pub status: Option<ChannelStatus>,
    /// Filter by backing integration.
    pub integration: Option<String>,
    /// Filter by lock state.
    pub api_locked: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

impl Channel {
    /// Returns `true` if external API mutations should be blocked.
    #[must_use]
    pub const fn is_mutation_blocked(&self) -> bool {
        self.api_locked
    }

    /// Returns `true` if this channel can ingest orders.
    #[must_use]
    pub const fn can_ingest(&self) -> bool {
        matches!(self.channel_type, ChannelType::SalesChannel | ChannelType::EndToEndChannel)
    }

    /// Returns `true` if this channel can fulfill orders.
    #[must_use]
    pub const fn can_fulfill(&self) -> bool {
        matches!(self.channel_type, ChannelType::FulfillmentChannel | ChannelType::EndToEndChannel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel(channel_type: ChannelType, api_locked: bool) -> Channel {
        Channel {
            id: ChannelId::new(),
            name: "Test Channel".to_string(),
            channel_type,
            integration: Some("shopify".to_string()),
            status: ChannelStatus::Active,
            api_locked,
            default_warehouse_id: None,
            tags: vec![],
            metadata: serde_json::Value::Null,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn sales_channel_can_ingest_but_not_fulfill() {
        let c = make_channel(ChannelType::SalesChannel, false);
        assert!(c.can_ingest());
        assert!(!c.can_fulfill());
    }

    #[test]
    fn fulfillment_channel_can_fulfill_but_not_ingest() {
        let c = make_channel(ChannelType::FulfillmentChannel, false);
        assert!(!c.can_ingest());
        assert!(c.can_fulfill());
    }

    #[test]
    fn end_to_end_channel_can_do_both() {
        let c = make_channel(ChannelType::EndToEndChannel, false);
        assert!(c.can_ingest());
        assert!(c.can_fulfill());
    }

    #[test]
    fn locked_channel_blocks_mutations() {
        assert!(make_channel(ChannelType::SalesChannel, true).is_mutation_blocked());
        assert!(!make_channel(ChannelType::SalesChannel, false).is_mutation_blocked());
    }

    #[test]
    fn channel_type_display_fromstr_roundtrip() {
        for t in [
            ChannelType::SalesChannel,
            ChannelType::FulfillmentChannel,
            ChannelType::EndToEndChannel,
        ] {
            let s = t.to_string();
            let parsed: ChannelType = s.parse().unwrap();
            assert_eq!(parsed, t, "round-trip failed for {s}");
        }
    }

    #[test]
    fn channel_type_default_is_sales() {
        assert_eq!(ChannelType::default(), ChannelType::SalesChannel);
    }

    #[test]
    fn sync_item_delete_defaults_false() {
        let json = r#"{"channel_sku":"EXT-1"}"#;
        let item: ChannelProductSyncItem = serde_json::from_str(json).unwrap();
        assert!(!item.delete);
        assert!(item.product_id.is_none());
    }
}
