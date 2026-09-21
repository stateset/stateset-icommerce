//! Channels  (sales / fulfillment channels).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Channels  (sales / fulfillment channels)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateChannelInput {
    pub name: String,
    /// sales_channel, fulfillment_channel, end_to_end_channel
    #[napi(ts_type = "ChannelType")]
    pub channel_type: String,
    pub integration: Option<String>,
    pub default_warehouse_id: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateChannelInput {
    pub name: Option<String>,
    pub integration: Option<String>,
    /// active, paused, deleted
    #[napi(ts_type = "ChannelStatus")]
    pub status: Option<String>,
    pub default_warehouse_id: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelFilterInput {
    /// sales_channel, fulfillment_channel, end_to_end_channel
    #[napi(ts_type = "ChannelType")]
    pub channel_type: Option<String>,
    /// active, paused, deleted
    #[napi(ts_type = "ChannelStatus")]
    pub status: Option<String>,
    pub integration: Option<String>,
    pub api_locked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelProductSyncItemInput {
    pub channel_sku: String,
    pub product_id: Option<String>,
    pub internal_sku: Option<String>,
    /// When true, remove the mapping instead of upserting it
    pub delete: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelOutput {
    pub id: String,
    pub name: String,
    /// sales_channel, fulfillment_channel, end_to_end_channel
    #[napi(ts_type = "ChannelType")]
    pub channel_type: String,
    pub integration: Option<String>,
    /// active, paused, deleted
    #[napi(ts_type = "ChannelStatus")]
    pub status: String,
    pub api_locked: bool,
    pub default_warehouse_id: Option<String>,
    pub tags: Vec<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Channel> for ChannelOutput {
    fn from(c: stateset_core::Channel) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            channel_type: c.channel_type.to_string(),
            integration: c.integration,
            status: c.status.to_string(),
            api_locked: c.api_locked,
            default_warehouse_id: c.default_warehouse_id.map(|w| w.to_string()),
            tags: c.tags,
            metadata: c.metadata.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelProductMappingOutput {
    pub channel_id: String,
    pub channel_sku: String,
    pub product_id: String,
    pub internal_sku: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ChannelProductMapping> for ChannelProductMappingOutput {
    fn from(m: stateset_core::ChannelProductMapping) -> Self {
        Self {
            channel_id: m.channel_id.to_string(),
            channel_sku: m.channel_sku,
            product_id: m.product_id.to_string(),
            internal_sku: m.internal_sku,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_channel_type(s: &str) -> Result<stateset_core::ChannelType> {
    s.parse::<stateset_core::ChannelType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid channel type: {s}")))
}

pub(crate) fn parse_channel_status(s: &str) -> Result<stateset_core::ChannelStatus> {
    s.parse::<stateset_core::ChannelStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid channel status: {s}")))
}

#[napi]
pub struct Channels {
    pub(crate) commerce: Handle,
}

#[napi]
impl Channels {
    /// Whether the channels backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.channels().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateChannelInput) -> Result<ChannelOutput> {
        let commerce = self.commerce.get()?;
        let channel = commerce
            .channels()
            .create(stateset_core::CreateChannel {
                name: input.name,
                channel_type: parse_channel_type(&input.channel_type)?,
                integration: input.integration,
                default_warehouse_id: parse_optional_uuid(
                    input.default_warehouse_id,
                    "default_warehouse_id",
                )?
                .map(Into::into),
                tags: input.tags.unwrap_or_default(),
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create channel", e))?;
        Ok(channel.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ChannelOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get channel", e))?;
        Ok(channel.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateChannelInput) -> Result<ChannelOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .update(
                uuid.into(),
                stateset_core::UpdateChannel {
                    name: input.name,
                    integration: input.integration,
                    status: input.status.as_deref().map(parse_channel_status).transpose()?,
                    default_warehouse_id: parse_optional_uuid(
                        input.default_warehouse_id,
                        "default_warehouse_id",
                    )?
                    .map(Into::into),
                    tags: input.tags,
                    metadata: input.metadata.map(Some).map(parse_metadata_json).transpose()?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update channel", e))?;
        Ok(channel.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<ChannelFilterInput>) -> Result<Vec<ChannelOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ChannelFilter::default()),
            |f| -> Result<stateset_core::ChannelFilter> {
                Ok(stateset_core::ChannelFilter {
                    channel_type: f.channel_type.as_deref().map(parse_channel_type).transpose()?,
                    status: f.status.as_deref().map(parse_channel_status).transpose()?,
                    integration: f.integration,
                    api_locked: f.api_locked,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let channels = commerce
            .channels()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list channels", e))?;
        Ok(channels.into_iter().map(Into::into).collect())
    }

    /// Soft-delete a channel.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        commerce
            .channels()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete channel", e))
    }

    /// Lock or unlock a channel against external mutations.
    #[napi]
    pub async fn set_lock(&self, id: String, locked: bool) -> Result<ChannelOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .set_lock(uuid.into(), locked)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set channel lock", e))?;
        Ok(channel.into())
    }

    /// Bulk upsert/delete channel SKU mappings. Returns the affected count.
    #[napi]
    pub async fn sync_products(
        &self,
        id: String,
        items: Vec<ChannelProductSyncItemInput>,
    ) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        let items = items
            .into_iter()
            .map(|i| -> Result<stateset_core::ChannelProductSyncItem> {
                Ok(stateset_core::ChannelProductSyncItem {
                    channel_sku: i.channel_sku,
                    product_id: parse_optional_uuid(i.product_id, "product_id")?.map(Into::into),
                    internal_sku: i.internal_sku,
                    delete: i.delete.unwrap_or(false),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .channels()
            .sync_products(uuid.into(), items)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to sync channel products", e))?;
        u32::try_from(count).map_err(|_| coded(ErrCode::Validation, "Sync count overflowed u32"))
    }

    /// List a channel's SKU mappings.
    #[napi]
    pub async fn list_product_mappings(
        &self,
        id: String,
    ) -> Result<Vec<ChannelProductMappingOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "channel")?;
        let mappings = commerce
            .channels()
            .list_product_mappings(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list channel product mappings", e))?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }
}
