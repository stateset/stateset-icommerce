//! Production Batches  (grouping manufacturing work orders).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Production Batches  (grouping manufacturing work orders)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductionBatchInput {
    pub name: String,
    pub vendor_id: Option<String>,
    /// Work order UUIDs to link at creation
    pub work_order_ids: Option<Vec<String>>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateProductionBatchInput {
    pub name: Option<String>,
    pub vendor_id: Option<String>,
    /// planned, in_progress, completed, cancelled
    #[napi(ts_type = "ProductionBatchStatus")]
    pub status: Option<String>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductionBatchFilterInput {
    /// planned, in_progress, completed, cancelled
    #[napi(ts_type = "ProductionBatchStatus")]
    pub status: Option<String>,
    pub vendor_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductionBatchOutput {
    pub id: String,
    pub name: String,
    /// planned, in_progress, completed, cancelled
    #[napi(ts_type = "ProductionBatchStatus")]
    pub status: String,
    pub vendor_id: Option<String>,
    pub work_order_ids: Vec<String>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ProductionBatch> for ProductionBatchOutput {
    fn from(b: stateset_core::ProductionBatch) -> Self {
        Self {
            id: b.id.to_string(),
            name: b.name,
            status: format!("{}", b.status),
            vendor_id: b.vendor_id.map(|id| id.to_string()),
            work_order_ids: b.work_order_ids.iter().map(ToString::to_string).collect(),
            notes: b.notes,
            scheduled_start: b.scheduled_start.map(|d| d.to_rfc3339()),
            scheduled_end: b.scheduled_end.map(|d| d.to_rfc3339()),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct ProductionBatches {
    pub(crate) commerce: Handle,
}

#[napi]
impl ProductionBatches {
    /// Whether the production-batches backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.production_batches().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateProductionBatchInput) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.get()?;
        let work_order_ids = input
            .work_order_ids
            .unwrap_or_default()
            .into_iter()
            .map(|s| parse_uuid_str(&s, "work_order"))
            .collect::<Result<Vec<_>>>()?;
        let batch = commerce
            .production_batches()
            .create(stateset_core::CreateProductionBatch {
                name: input.name,
                vendor_id: parse_optional_uuid(input.vendor_id, "vendor_id")?,
                work_order_ids,
                notes: input.notes,
                scheduled_start: parse_rfc3339_opt(input.scheduled_start, "scheduled_start")?,
                scheduled_end: parse_rfc3339_opt(input.scheduled_end, "scheduled_end")?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create production batch", e))?;
        Ok(batch.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ProductionBatchOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let batch = commerce
            .production_batches()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get production batch", e))?;
        Ok(batch.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateProductionBatchInput,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let status = input
            .status
            .map(|s| {
                s.parse::<stateset_core::ProductionBatchStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid production batch status"))
            })
            .transpose()?;
        let batch = commerce
            .production_batches()
            .update(
                uuid.into(),
                stateset_core::UpdateProductionBatch {
                    name: input.name,
                    vendor_id: parse_optional_uuid(input.vendor_id, "vendor_id")?,
                    status,
                    notes: input.notes,
                    scheduled_start: parse_rfc3339_opt(input.scheduled_start, "scheduled_start")?,
                    scheduled_end: parse_rfc3339_opt(input.scheduled_end, "scheduled_end")?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update production batch", e))?;
        Ok(batch.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ProductionBatchFilterInput>,
    ) -> Result<Vec<ProductionBatchOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ProductionBatchFilter::default()),
            |f| -> Result<stateset_core::ProductionBatchFilter> {
                Ok(stateset_core::ProductionBatchFilter {
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::ProductionBatchStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid production batch status")
                            })
                        })
                        .transpose()?,
                    vendor_id: parse_optional_uuid(f.vendor_id, "vendor_id")?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let batches = commerce
            .production_batches()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list production batches", e))?;
        Ok(batches.into_iter().map(Into::into).collect())
    }

    /// Delete a production batch.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "production batch")?;
        commerce
            .production_batches()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete production batch", e))?;
        Ok(())
    }

    /// Link work orders to a batch.
    #[napi]
    pub async fn add_work_orders(
        &self,
        id: String,
        work_order_ids: Vec<String>,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let work_order_ids = work_order_ids
            .into_iter()
            .map(|s| parse_uuid_str(&s, "work_order"))
            .collect::<Result<Vec<_>>>()?;
        let batch = commerce
            .production_batches()
            .add_work_orders(uuid.into(), work_order_ids)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add work orders", e))?;
        Ok(batch.into())
    }

    /// Remove a work order from a batch.
    #[napi]
    pub async fn remove_work_order(
        &self,
        id: String,
        work_order_id: String,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let work_order_uuid = parse_uuid_str(&work_order_id, "work_order")?;
        let batch = commerce
            .production_batches()
            .remove_work_order(uuid.into(), work_order_uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove work order", e))?;
        Ok(batch.into())
    }
}
