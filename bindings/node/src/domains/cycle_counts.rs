//! Cycle Counts  (quantities cross as exact decimal strings).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Cycle Counts  (quantities cross as exact decimal strings)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCycleCountLineInput {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub expected_quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCycleCountInput {
    pub warehouse_id: i32,
    /// Optional single location scope; omit to count across the warehouse.
    pub location_id: Option<i32>,
    /// RFC 3339 timestamp
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CreateCycleCountLineInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordCycleCountLineInput {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub counted_quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountFilterInput {
    pub warehouse_id: Option<i32>,
    pub location_id: Option<i32>,
    /// draft, in_progress, completed, cancelled
    #[napi(ts_type = "CycleCountStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountLineOutput {
    pub id: String,
    pub cycle_count_id: String,
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub expected_quantity: String,
    /// Exact decimal string
    pub counted_quantity: Option<String>,
    /// Exact decimal string: counted_quantity - expected_quantity
    pub variance: Option<String>,
}

impl From<stateset_core::CycleCountLine> for CycleCountLineOutput {
    fn from(l: stateset_core::CycleCountLine) -> Self {
        Self {
            id: l.id.to_string(),
            cycle_count_id: l.cycle_count_id.to_string(),
            sku: l.sku,
            lot_id: l.lot_id.map(|id| id.to_string()),
            expected_quantity: l.expected_quantity.to_string(),
            counted_quantity: l.counted_quantity.map(|d| d.to_string()),
            variance: l.variance.map(|d| d.to_string()),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountOutput {
    pub id: String,
    pub warehouse_id: i32,
    pub location_id: Option<i32>,
    /// draft, in_progress, completed, cancelled
    #[napi(ts_type = "CycleCountStatus")]
    pub status: String,
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CycleCountLineOutput>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

impl From<stateset_core::CycleCount> for CycleCountOutput {
    fn from(c: stateset_core::CycleCount) -> Self {
        Self {
            id: c.id.to_string(),
            warehouse_id: c.warehouse_id,
            location_id: c.location_id,
            status: format!("{}", c.status),
            scheduled_date: c.scheduled_date.map(|d| d.to_rfc3339()),
            counted_by: c.counted_by,
            lines: c.lines.into_iter().map(Into::into).collect(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
            completed_at: c.completed_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[napi]
pub struct CycleCounts {
    pub(crate) commerce: Handle,
}

#[napi]
impl CycleCounts {
    /// Create a cycle count (draft) with its expected lines.
    #[napi]
    pub async fn create(&self, input: CreateCycleCountInput) -> Result<CycleCountOutput> {
        let commerce = self.commerce.get()?;
        let scheduled_date = match input.scheduled_date.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        coded(ErrCode::Validation, "Invalid scheduled_date RFC 3339 timestamp")
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let lines = input
            .lines
            .into_iter()
            .map(|l| -> Result<stateset_core::CreateCycleCountLine> {
                Ok(stateset_core::CreateCycleCountLine {
                    sku: l.sku,
                    lot_id: parse_optional_uuid(l.lot_id, "lot_id")?,
                    expected_quantity: parse_decimal_str(
                        &l.expected_quantity,
                        "expected_quantity",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .warehouse()
            .create_cycle_count(stateset_core::CreateCycleCount {
                warehouse_id: input.warehouse_id,
                location_id: input.location_id,
                scheduled_date,
                counted_by: input.counted_by,
                lines,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create cycle count", e))?;
        Ok(count.into())
    }

    /// Get a cycle count (with lines) by ID.
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CycleCountOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .get_cycle_count(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get cycle count", e))?;
        Ok(count.map(Into::into))
    }

    /// List cycle counts matching the filter.
    #[napi]
    pub async fn list(
        &self,
        filter: Option<CycleCountFilterInput>,
    ) -> Result<Vec<CycleCountOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::CycleCountFilter::default()),
            |f| -> Result<stateset_core::CycleCountFilter> {
                Ok(stateset_core::CycleCountFilter {
                    warehouse_id: f.warehouse_id,
                    location_id: f.location_id,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::CycleCountStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid cycle count status")
                            })
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let counts = commerce
            .warehouse()
            .list_cycle_counts(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list cycle counts", e))?;
        Ok(counts.into_iter().map(Into::into).collect())
    }

    /// Start a draft cycle count (draft -> in_progress).
    #[napi]
    pub async fn start(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .start_cycle_count(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to start cycle count", e))?;
        Ok(count.into())
    }

    /// Record physical counts against an in-progress cycle count.
    #[napi]
    pub async fn record_counts(
        &self,
        id: String,
        counts: Vec<RecordCycleCountLineInput>,
    ) -> Result<CycleCountOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let counts = counts
            .into_iter()
            .map(|c| -> Result<stateset_core::RecordCycleCountLine> {
                Ok(stateset_core::RecordCycleCountLine {
                    sku: c.sku,
                    lot_id: parse_optional_uuid(c.lot_id, "lot_id")?,
                    counted_quantity: parse_decimal_str(&c.counted_quantity, "counted_quantity")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .warehouse()
            .record_cycle_counts(uuid, counts)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to record cycle counts", e))?;
        Ok(count.into())
    }

    /// Complete an in-progress cycle count, applying variance adjustments.
    #[napi]
    pub async fn complete(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .complete_cycle_count(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete cycle count", e))?;
        Ok(count.into())
    }

    /// Cancel a draft or in-progress cycle count. No adjustments are applied.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .cancel_cycle_count(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel cycle count", e))?;
        Ok(count.into())
    }
}
