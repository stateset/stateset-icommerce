//! Consumption report
//!
//! Aggregates inventory consumption events (components used in production,
//! stock drawn down, etc.) into per-SKU totals and an average daily consumption
//! rate over an inclusive date window. The daily rate supports reorder/runway
//! planning. Pure computed report (no persistence).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductId;

/// A single consumption event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionRecord {
    /// Product, if known.
    pub product_id: Option<ProductId>,
    /// SKU.
    pub sku: String,
    /// Quantity consumed.
    pub quantity: Decimal,
    /// Date consumed.
    pub consumed_at: NaiveDate,
}

/// Per-SKU aggregated consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionRow {
    /// SKU.
    pub sku: String,
    /// Total quantity consumed in the window.
    pub total_quantity: Decimal,
    /// Number of consumption events.
    pub event_count: u64,
    /// Average daily consumption (`total_quantity / days`), 4 dp.
    pub avg_daily: Decimal,
}

/// The computed consumption report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionReport {
    /// Inclusive window start.
    pub from: NaiveDate,
    /// Inclusive window end.
    pub to: NaiveDate,
    /// Number of days in the window (inclusive, at least 1).
    pub days: i64,
    /// Per-SKU rows, highest total consumption first.
    pub rows: Vec<ConsumptionRow>,
    /// Total quantity consumed across all SKUs.
    pub total_quantity: Decimal,
}

/// Inclusive day count for a window (at least 1).
#[must_use]
pub fn window_days(from: NaiveDate, to: NaiveDate) -> i64 {
    ((to - from).num_days() + 1).max(1)
}

/// Compute the consumption report for records whose `consumed_at` falls within
/// `[from, to]` (inclusive). Rows are sorted by total quantity descending, then
/// SKU. The average daily rate divides by the inclusive day count.
#[must_use]
pub fn compute_consumption(
    records: &[ConsumptionRecord],
    from: NaiveDate,
    to: NaiveDate,
) -> ConsumptionReport {
    use std::collections::BTreeMap;

    let days = window_days(from, to);
    let days_dec = Decimal::from(days);
    let mut by_sku: BTreeMap<String, (Decimal, u64)> = BTreeMap::new();
    let mut total_quantity = Decimal::ZERO;

    for rec in records {
        if rec.consumed_at < from || rec.consumed_at > to {
            continue;
        }
        total_quantity += rec.quantity;
        let entry = by_sku.entry(rec.sku.clone()).or_insert((Decimal::ZERO, 0));
        entry.0 += rec.quantity;
        entry.1 += 1;
    }

    let mut rows: Vec<ConsumptionRow> = by_sku
        .into_iter()
        .map(|(sku, (total, count))| ConsumptionRow {
            sku,
            total_quantity: total,
            event_count: count,
            avg_daily: (total / days_dec).round_dp(4),
        })
        .collect();
    rows.sort_by(|a, b| b.total_quantity.cmp(&a.total_quantity).then(a.sku.cmp(&b.sku)));

    ConsumptionReport { from, to, days, rows, total_quantity }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn rec(sku: &str, qty: Decimal, date: NaiveDate) -> ConsumptionRecord {
        ConsumptionRecord { product_id: None, sku: sku.into(), quantity: qty, consumed_at: date }
    }

    #[test]
    fn window_days_inclusive() {
        assert_eq!(window_days(day(2026, 6, 1), day(2026, 6, 30)), 30);
        assert_eq!(window_days(day(2026, 6, 1), day(2026, 6, 1)), 1);
    }

    #[test]
    fn aggregates_and_rates() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 10); // 10 days
        let records = vec![
            rec("a", dec!(20), day(2026, 6, 2)),
            rec("a", dec!(30), day(2026, 6, 5)),
            rec("b", dec!(5), day(2026, 6, 3)),
        ];
        let report = compute_consumption(&records, from, to);
        assert_eq!(report.days, 10);
        assert_eq!(report.total_quantity, dec!(55));
        // a: 50 total over 10 days → 5/day, 2 events; highest → first
        assert_eq!(report.rows[0].sku, "a");
        assert_eq!(report.rows[0].total_quantity, dec!(50));
        assert_eq!(report.rows[0].event_count, 2);
        assert_eq!(report.rows[0].avg_daily, dec!(5.0000));
        // b: 5 over 10 days → 0.5/day
        assert_eq!(report.rows[1].avg_daily, dec!(0.5000));
    }

    #[test]
    fn date_range_filters() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let records = vec![
            rec("a", dec!(10), day(2026, 5, 31)), // before
            rec("a", dec!(10), day(2026, 6, 15)), // in
            rec("a", dec!(10), day(2026, 7, 1)),  // after
        ];
        let report = compute_consumption(&records, from, to);
        assert_eq!(report.total_quantity, dec!(10));
    }

    #[test]
    fn empty_report() {
        let report = compute_consumption(&[], day(2026, 6, 1), day(2026, 6, 30));
        assert_eq!(report.total_quantity, dec!(0));
        assert!(report.rows.is_empty());
        assert_eq!(report.days, 30);
    }
}
