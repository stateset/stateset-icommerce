//! Transaction COGS (cost of goods sold) report
//!
//! Aggregates transaction lines into per-SKU revenue, COGS, and gross margin
//! over an inclusive date range, with roll-up totals and an overall gross
//! margin percentage. Pure computed report (no persistence).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductId;

/// A single transaction line feeding the COGS report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CogsLine {
    /// Product, if known.
    pub product_id: Option<ProductId>,
    /// SKU.
    pub sku: String,
    /// Quantity sold.
    pub quantity: Decimal,
    /// Revenue recognised for the line.
    pub revenue: Decimal,
    /// Cost of goods sold for the line.
    pub cost: Decimal,
    /// Transaction date.
    pub transaction_date: NaiveDate,
}

/// Per-SKU aggregated COGS row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CogsRow {
    /// SKU.
    pub sku: String,
    /// Total quantity.
    pub quantity: Decimal,
    /// Total revenue.
    pub revenue: Decimal,
    /// Total COGS.
    pub cogs: Decimal,
    /// Gross margin (revenue − cogs).
    pub gross_margin: Decimal,
}

/// The computed transaction-COGS report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCogsReport {
    /// Inclusive window start.
    pub from: NaiveDate,
    /// Inclusive window end.
    pub to: NaiveDate,
    /// Per-SKU rows, highest gross margin first.
    pub rows: Vec<CogsRow>,
    /// Total revenue.
    pub total_revenue: Decimal,
    /// Total COGS.
    pub total_cogs: Decimal,
    /// Total gross margin (revenue − cogs).
    pub gross_margin: Decimal,
    /// Overall gross margin percent (0 when revenue is zero), 2 dp.
    pub gross_margin_pct: Decimal,
}

/// Compute the gross-margin percentage for a revenue/margin pair, rounded to
/// two decimal places. Returns zero when revenue is zero.
#[must_use]
pub fn gross_margin_pct(revenue: Decimal, gross_margin: Decimal) -> Decimal {
    if revenue == Decimal::ZERO {
        return Decimal::ZERO;
    }
    (gross_margin / revenue * Decimal::from(100)).round_dp(2)
}

/// Compute the transaction COGS report for lines whose `transaction_date` falls
/// within `[from, to]` (inclusive). Rows are sorted by gross margin descending,
/// then SKU for stability.
#[must_use]
pub fn compute_transaction_cogs(
    lines: &[CogsLine],
    from: NaiveDate,
    to: NaiveDate,
) -> TransactionCogsReport {
    use std::collections::BTreeMap;

    let mut by_sku: BTreeMap<String, CogsRow> = BTreeMap::new();
    let mut total_revenue = Decimal::ZERO;
    let mut total_cogs = Decimal::ZERO;

    for line in lines {
        if line.transaction_date < from || line.transaction_date > to {
            continue;
        }
        total_revenue += line.revenue;
        total_cogs += line.cost;
        let row = by_sku.entry(line.sku.clone()).or_insert_with(|| CogsRow {
            sku: line.sku.clone(),
            quantity: Decimal::ZERO,
            revenue: Decimal::ZERO,
            cogs: Decimal::ZERO,
            gross_margin: Decimal::ZERO,
        });
        row.quantity += line.quantity;
        row.revenue += line.revenue;
        row.cogs += line.cost;
        row.gross_margin = row.revenue - row.cogs;
    }

    let mut rows: Vec<CogsRow> = by_sku.into_values().collect();
    rows.sort_by(|a, b| b.gross_margin.cmp(&a.gross_margin).then(a.sku.cmp(&b.sku)));

    let gross_margin = total_revenue - total_cogs;
    TransactionCogsReport {
        from,
        to,
        rows,
        total_revenue,
        total_cogs,
        gross_margin,
        gross_margin_pct: gross_margin_pct(total_revenue, gross_margin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn line(sku: &str, qty: Decimal, rev: Decimal, cost: Decimal, date: NaiveDate) -> CogsLine {
        CogsLine {
            product_id: None,
            sku: sku.into(),
            quantity: qty,
            revenue: rev,
            cost,
            transaction_date: date,
        }
    }

    #[test]
    fn margin_pct_helper() {
        assert_eq!(gross_margin_pct(dec!(100), dec!(40)), dec!(40.00));
        assert_eq!(gross_margin_pct(dec!(0), dec!(0)), dec!(0));
        assert_eq!(gross_margin_pct(dec!(3), dec!(1)), dec!(33.33));
    }

    #[test]
    fn aggregates_and_margins() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let lines = vec![
            line("a", dec!(2), dec!(100), dec!(60), day(2026, 6, 5)),
            line("a", dec!(1), dec!(50), dec!(30), day(2026, 6, 10)),
            line("b", dec!(5), dec!(500), dec!(100), day(2026, 6, 15)),
        ];
        let report = compute_transaction_cogs(&lines, from, to);
        assert_eq!(report.total_revenue, dec!(650));
        assert_eq!(report.total_cogs, dec!(190));
        assert_eq!(report.gross_margin, dec!(460));
        // b has higher margin (400) than a (60) → first row
        assert_eq!(report.rows[0].sku, "b");
        assert_eq!(report.rows[0].gross_margin, dec!(400));
        assert_eq!(report.rows[1].sku, "a");
        assert_eq!(report.rows[1].revenue, dec!(150));
        assert_eq!(report.rows[1].gross_margin, dec!(60));
    }

    #[test]
    fn date_range_filters() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let lines = vec![
            line("a", dec!(1), dec!(100), dec!(50), day(2026, 5, 31)),
            line("a", dec!(1), dec!(100), dec!(50), day(2026, 6, 15)),
        ];
        let report = compute_transaction_cogs(&lines, from, to);
        assert_eq!(report.total_revenue, dec!(100));
    }

    #[test]
    fn empty_report_zero_margin() {
        let report = compute_transaction_cogs(&[], day(2026, 6, 1), day(2026, 6, 30));
        assert_eq!(report.total_revenue, dec!(0));
        assert_eq!(report.gross_margin_pct, dec!(0));
        assert!(report.rows.is_empty());
    }
}
