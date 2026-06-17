//! Sales-by-channel report
//!
//! Aggregates sales records into per-channel revenue/units/order-count rows over
//! an inclusive date range. Pure computed report (no persistence): callers
//! supply the records and the window.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A single sales record feeding the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesRecord {
    /// Channel the sale was attributed to (e.g. `shopify`, `wholesale`).
    pub channel: String,
    /// Order revenue.
    pub revenue: Decimal,
    /// Units sold.
    pub units: Decimal,
    /// Order date.
    pub order_date: NaiveDate,
}

/// Aggregated sales for a single channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSalesRow {
    /// Channel name.
    pub channel: String,
    /// Number of orders.
    pub order_count: u64,
    /// Total revenue.
    pub total_revenue: Decimal,
    /// Total units.
    pub total_units: Decimal,
}

/// The computed sales-by-channel report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesByChannelReport {
    /// Inclusive window start.
    pub from: NaiveDate,
    /// Inclusive window end.
    pub to: NaiveDate,
    /// Per-channel rows, highest revenue first.
    pub rows: Vec<ChannelSalesRow>,
    /// Total orders across all channels in range.
    pub total_orders: u64,
    /// Total revenue across all channels in range.
    pub total_revenue: Decimal,
    /// Total units across all channels in range.
    pub total_units: Decimal,
}

/// Compute the sales-by-channel report for records whose `order_date` falls
/// within `[from, to]` (inclusive). Rows are sorted by revenue descending,
/// then channel name for stable ordering.
#[must_use]
pub fn compute_sales_by_channel(
    records: &[SalesRecord],
    from: NaiveDate,
    to: NaiveDate,
) -> SalesByChannelReport {
    use std::collections::BTreeMap;

    // BTreeMap keeps a deterministic base order by channel name.
    let mut by_channel: BTreeMap<String, ChannelSalesRow> = BTreeMap::new();
    let mut total_orders = 0u64;
    let mut total_revenue = Decimal::ZERO;
    let mut total_units = Decimal::ZERO;

    for rec in records {
        if rec.order_date < from || rec.order_date > to {
            continue;
        }
        total_orders += 1;
        total_revenue += rec.revenue;
        total_units += rec.units;
        let row = by_channel.entry(rec.channel.clone()).or_insert_with(|| ChannelSalesRow {
            channel: rec.channel.clone(),
            order_count: 0,
            total_revenue: Decimal::ZERO,
            total_units: Decimal::ZERO,
        });
        row.order_count += 1;
        row.total_revenue += rec.revenue;
        row.total_units += rec.units;
    }

    let mut rows: Vec<ChannelSalesRow> = by_channel.into_values().collect();
    // Sort by revenue desc; channel name (already the BTreeMap order) breaks ties.
    rows.sort_by(|a, b| b.total_revenue.cmp(&a.total_revenue).then(a.channel.cmp(&b.channel)));

    SalesByChannelReport { from, to, rows, total_orders, total_revenue, total_units }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn rec(channel: &str, revenue: Decimal, units: Decimal, date: NaiveDate) -> SalesRecord {
        SalesRecord { channel: channel.into(), revenue, units, order_date: date }
    }

    #[test]
    fn groups_and_totals() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let records = vec![
            rec("shopify", dec!(100), dec!(2), day(2026, 6, 5)),
            rec("shopify", dec!(50), dec!(1), day(2026, 6, 10)),
            rec("wholesale", dec!(500), dec!(20), day(2026, 6, 15)),
        ];
        let report = compute_sales_by_channel(&records, from, to);
        assert_eq!(report.total_orders, 3);
        assert_eq!(report.total_revenue, dec!(650));
        assert_eq!(report.total_units, dec!(23));
        // wholesale has highest revenue → first row
        assert_eq!(report.rows[0].channel, "wholesale");
        assert_eq!(report.rows[0].total_revenue, dec!(500));
        // shopify aggregated across 2 orders
        assert_eq!(report.rows[1].channel, "shopify");
        assert_eq!(report.rows[1].order_count, 2);
        assert_eq!(report.rows[1].total_revenue, dec!(150));
    }

    #[test]
    fn date_range_filters() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let records = vec![
            rec("shopify", dec!(100), dec!(1), day(2026, 5, 31)), // before
            rec("shopify", dec!(200), dec!(2), day(2026, 6, 15)), // in
            rec("shopify", dec!(300), dec!(3), day(2026, 7, 1)),  // after
        ];
        let report = compute_sales_by_channel(&records, from, to);
        assert_eq!(report.total_orders, 1);
        assert_eq!(report.total_revenue, dec!(200));
    }

    #[test]
    fn boundaries_inclusive() {
        let from = day(2026, 6, 1);
        let to = day(2026, 6, 30);
        let records = vec![rec("a", dec!(1), dec!(1), from), rec("a", dec!(1), dec!(1), to)];
        let report = compute_sales_by_channel(&records, from, to);
        assert_eq!(report.total_orders, 2);
    }

    #[test]
    fn empty_report() {
        let report = compute_sales_by_channel(&[], day(2026, 6, 1), day(2026, 6, 30));
        assert_eq!(report.total_orders, 0);
        assert!(report.rows.is_empty());
    }
}
