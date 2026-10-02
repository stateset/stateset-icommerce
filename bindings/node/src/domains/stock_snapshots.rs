//! Stock snapshots  (point-in-time inventory).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Stock snapshots  (point-in-time inventory)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureStockLineInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_on_hand: String,
    /// Exact decimal string
    pub quantity_available: String,
    pub location: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureStockSnapshotInput {
    pub label: Option<String>,
    pub lines: Vec<CaptureStockLineInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotFilterInput {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotLineOutput {
    pub id: String,
    pub stock_snapshot_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_on_hand: String,
    /// Exact decimal string
    pub quantity_available: String,
    pub location: Option<String>,
}

impl From<stateset_core::StockSnapshotLine> for StockSnapshotLineOutput {
    fn from(l: stateset_core::StockSnapshotLine) -> Self {
        Self {
            id: l.id.to_string(),
            stock_snapshot_id: l.stock_snapshot_id.to_string(),
            product_id: l.product_id.to_string(),
            sku: l.sku,
            quantity_on_hand: l.quantity_on_hand.to_string(),
            quantity_available: l.quantity_available.to_string(),
            location: l.location,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotOutput {
    pub id: String,
    pub label: Option<String>,
    pub total_skus: String,
    /// Exact decimal string
    pub total_units: String,
    pub lines: Vec<StockSnapshotLineOutput>,
    pub captured_at: String,
}

impl From<stateset_core::StockSnapshot> for StockSnapshotOutput {
    fn from(s: stateset_core::StockSnapshot) -> Self {
        Self {
            id: s.id.to_string(),
            label: s.label,
            total_skus: s.total_skus.to_string(),
            total_units: s.total_units.to_string(),
            lines: s.lines.into_iter().map(Into::into).collect(),
            captured_at: s.captured_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct StockSnapshots {
    pub(crate) commerce: Handle,
}

#[napi]
impl StockSnapshots {
    /// Whether the stock-snapshots backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.stock_snapshots().is_supported())
    }

    /// Capture a new snapshot; totals are computed from the supplied lines.
    #[napi]
    pub async fn capture(&self, input: CaptureStockSnapshotInput) -> Result<StockSnapshotOutput> {
        let commerce = self.commerce.get()?;
        let lines = input
            .lines
            .into_iter()
            .map(|l| -> Result<stateset_core::CaptureStockLine> {
                Ok(stateset_core::CaptureStockLine {
                    product_id: parse_uuid_str(&l.product_id, "product_id")?.into(),
                    sku: l.sku,
                    quantity_on_hand: parse_decimal_str(&l.quantity_on_hand, "quantity_on_hand")?,
                    quantity_available: parse_decimal_str(
                        &l.quantity_available,
                        "quantity_available",
                    )?,
                    location: l.location,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let snapshot = commerce
            .stock_snapshots()
            .capture(stateset_core::CaptureStockSnapshot { label: input.label, lines })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to capture stock snapshot", e))?;
        Ok(snapshot.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<StockSnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "stock_snapshot")?;
        let snapshot = commerce
            .stock_snapshots()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get stock snapshot", e))?;
        Ok(snapshot.map(Into::into))
    }

    /// Most recent snapshot, if any.
    #[napi]
    pub async fn latest(&self) -> Result<Option<StockSnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let snapshot = commerce
            .stock_snapshots()
            .latest()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get latest stock snapshot", e))?;
        Ok(snapshot.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<StockSnapshotFilterInput>,
    ) -> Result<Vec<StockSnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::StockSnapshotFilter::default, |f| {
            stateset_core::StockSnapshotFilter { limit: f.limit, offset: f.offset }
        });
        let snapshots = commerce
            .stock_snapshots()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list stock snapshots", e))?;
        Ok(snapshots.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "stock_snapshot")?;
        commerce
            .stock_snapshots()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete stock snapshot", e))
    }
}
