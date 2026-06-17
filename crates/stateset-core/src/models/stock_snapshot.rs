//! Stock snapshot domain models
//!
//! A point-in-time capture of on-hand and available inventory per SKU, taken
//! for valuation, reconciliation, or export (JSON/CSV/Excel). Distinct from the
//! operational [`TopologySnapshot`](crate::TopologySnapshot), which captures
//! landscape counts rather than per-SKU stock.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{ProductId, StockSnapshotId, StockSnapshotLineId};

/// A single per-SKU line within a stock snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSnapshotLine {
    /// Unique line ID.
    pub id: StockSnapshotLineId,
    /// Owning snapshot.
    pub stock_snapshot_id: StockSnapshotId,
    /// Product.
    pub product_id: ProductId,
    /// SKU snapshot.
    pub sku: String,
    /// On-hand quantity at capture time.
    pub quantity_on_hand: Decimal,
    /// Available (unreserved) quantity at capture time.
    pub quantity_available: Decimal,
    /// Optional location label.
    pub location: Option<String>,
}

/// A point-in-time inventory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSnapshot {
    /// Unique snapshot ID.
    pub id: StockSnapshotId,
    /// Optional human-readable label (e.g. "EOM June 2026").
    pub label: Option<String>,
    /// Number of distinct SKUs captured.
    pub total_skus: u64,
    /// Total on-hand units across all lines.
    pub total_units: Decimal,
    /// Lines.
    pub lines: Vec<StockSnapshotLine>,
    /// When the snapshot was captured.
    pub captured_at: DateTime<Utc>,
}

impl StockSnapshot {
    /// Total available units across all lines.
    #[must_use]
    pub fn total_available(&self) -> Decimal {
        self.lines.iter().map(|l| l.quantity_available).sum()
    }
}

/// A line on a capture request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStockLine {
    /// Product.
    pub product_id: ProductId,
    /// SKU.
    pub sku: String,
    /// On-hand quantity.
    pub quantity_on_hand: Decimal,
    /// Available quantity.
    pub quantity_available: Decimal,
    /// Optional location.
    pub location: Option<String>,
}

/// Input for capturing a stock snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStockSnapshot {
    /// Optional label.
    pub label: Option<String>,
    /// Lines (at least one required).
    pub lines: Vec<CaptureStockLine>,
}

impl CaptureStockSnapshot {
    /// Number of distinct SKUs in the capture.
    #[must_use]
    pub fn total_skus(&self) -> u64 {
        self.lines.len() as u64
    }

    /// Total on-hand units in the capture.
    #[must_use]
    pub fn total_units(&self) -> Decimal {
        self.lines.iter().map(|l| l.quantity_on_hand).sum()
    }
}

/// Filter for listing stock snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StockSnapshotFilter {
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn capture() -> CaptureStockSnapshot {
        CaptureStockSnapshot {
            label: Some("EOM".into()),
            lines: vec![
                CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "SKU-1".into(),
                    quantity_on_hand: dec!(10),
                    quantity_available: dec!(8),
                    location: Some("MAIN".into()),
                },
                CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "SKU-2".into(),
                    quantity_on_hand: dec!(5),
                    quantity_available: dec!(5),
                    location: None,
                },
            ],
        }
    }

    #[test]
    fn capture_totals() {
        let c = capture();
        assert_eq!(c.total_skus(), 2);
        assert_eq!(c.total_units(), dec!(15));
    }

    #[test]
    fn snapshot_total_available() {
        let c = capture();
        let snapshot = StockSnapshot {
            id: StockSnapshotId::new(),
            label: c.label.clone(),
            total_skus: c.total_skus(),
            total_units: c.total_units(),
            lines: c
                .lines
                .iter()
                .map(|l| StockSnapshotLine {
                    id: StockSnapshotLineId::new(),
                    stock_snapshot_id: StockSnapshotId::new(),
                    product_id: l.product_id,
                    sku: l.sku.clone(),
                    quantity_on_hand: l.quantity_on_hand,
                    quantity_available: l.quantity_available,
                    location: l.location.clone(),
                })
                .collect(),
            captured_at: Utc::now(),
        };
        assert_eq!(snapshot.total_available(), dec!(13));
        assert_eq!(snapshot.total_units, dec!(15));
    }
}
