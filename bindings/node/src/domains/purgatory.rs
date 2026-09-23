//! Purgatory (order ingestion staging).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Purgatory (order ingestion staging)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IngestLineItemInput {
    pub external_sku: String,
    /// Exact decimal string
    pub quantity: String,
    pub product_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IngestOrderInput {
    pub channel_id: Option<String>,
    pub external_order_id: String,
    pub external_status: Option<String>,
    /// JSON string
    pub metadata: Option<String>,
    pub items: Vec<IngestLineItemInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct MapPurgatoryLineInput {
    pub product_id: Option<String>,
    pub ignore_item: Option<bool>,
    pub non_physical: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryFilterInput {
    pub channel_id: Option<String>,
    pub is_posted: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryLineItemOutput {
    pub id: String,
    pub purgatory_order_id: String,
    pub external_sku: String,
    pub product_id: Option<String>,
    /// Exact decimal string
    pub quantity: String,
    pub ignore_item: bool,
    pub non_physical: bool,
    pub is_resolved: bool,
}

impl From<stateset_core::PurgatoryLineItem> for PurgatoryLineItemOutput {
    fn from(l: stateset_core::PurgatoryLineItem) -> Self {
        let is_resolved = l.is_resolved();
        Self {
            id: l.id.to_string(),
            purgatory_order_id: l.purgatory_order_id.to_string(),
            external_sku: l.external_sku,
            product_id: l.product_id.map(|id| id.to_string()),
            quantity: l.quantity.to_string(),
            ignore_item: l.ignore_item,
            non_physical: l.non_physical,
            is_resolved,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryOrderOutput {
    pub id: String,
    pub channel_id: Option<String>,
    pub external_order_id: String,
    pub external_status: Option<String>,
    pub is_posted: bool,
    pub hold_reason: Option<String>,
    /// JSON string
    pub metadata: String,
    pub items: Vec<PurgatoryLineItemOutput>,
    pub is_ready_to_post: bool,
    pub unresolved_count: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PurgatoryOrder> for PurgatoryOrderOutput {
    fn from(o: stateset_core::PurgatoryOrder) -> Self {
        let is_ready_to_post = o.is_ready_to_post();
        let unresolved_count = o.unresolved_count().to_string();
        Self {
            id: o.id.to_string(),
            channel_id: o.channel_id.map(|id| id.to_string()),
            external_order_id: o.external_order_id,
            external_status: o.external_status,
            is_posted: o.is_posted,
            hold_reason: o.hold_reason,
            metadata: serde_json::to_string(&o.metadata).unwrap_or_else(|_| "null".to_string()),
            items: o.items.into_iter().map(Into::into).collect(),
            is_ready_to_post,
            unresolved_count,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Purgatory {
    pub(crate) commerce: Handle,
}

#[napi]
impl Purgatory {
    /// Whether the purgatory backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.purgatory().is_supported())
    }

    /// Ingest an external order into purgatory.
    #[napi]
    pub async fn ingest(&self, input: IngestOrderInput) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.get()?;
        let metadata = match input.metadata {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| wrap(ErrCode::Validation, "Invalid metadata JSON", e))?,
            None => serde_json::Value::Null,
        };
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::IngestLineItem> {
                Ok(stateset_core::IngestLineItem {
                    external_sku: i.external_sku,
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                    product_id: parse_optional_uuid(i.product_id, "product_id")?.map(Into::into),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let order = commerce
            .purgatory()
            .ingest(stateset_core::IngestOrder {
                channel_id: parse_optional_uuid(input.channel_id, "channel_id")?.map(Into::into),
                external_order_id: input.external_order_id,
                external_status: input.external_status,
                metadata,
                items,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to ingest purgatory order", e))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PurgatoryOrderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let order = commerce
            .purgatory()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get purgatory order", e))?;
        Ok(order.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PurgatoryFilterInput>,
    ) -> Result<Vec<PurgatoryOrderOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PurgatoryFilter::default()),
            |f| -> Result<stateset_core::PurgatoryFilter> {
                Ok(stateset_core::PurgatoryFilter {
                    channel_id: parse_optional_uuid(f.channel_id, "channel_id")?.map(Into::into),
                    is_posted: f.is_posted,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let orders = commerce
            .purgatory()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list purgatory orders", e))?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Map a staged line to a product and/or toggle its flags.
    #[napi]
    pub async fn map_line(
        &self,
        id: String,
        line_id: String,
        input: MapPurgatoryLineInput,
    ) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let line_uuid = parse_uuid_str(&line_id, "purgatory_line_item")?;
        let order = commerce
            .purgatory()
            .map_line(
                uuid.into(),
                line_uuid.into(),
                stateset_core::MapPurgatoryLine {
                    product_id: parse_optional_uuid(input.product_id, "product_id")?
                        .map(Into::into),
                    ignore_item: input.ignore_item,
                    non_physical: input.non_physical,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to map purgatory line", e))?;
        Ok(order.into())
    }

    /// Post the order out of purgatory.
    #[napi]
    pub async fn post(&self, id: String) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let order = commerce
            .purgatory()
            .post(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to post purgatory order", e))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        commerce
            .purgatory()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete purgatory order", e))
    }
}
