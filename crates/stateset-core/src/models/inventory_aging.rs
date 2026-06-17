//! Inventory aging report
//!
//! Buckets on-hand cost layers by age (days since received) into standard
//! aging buckets — 0–30, 31–60, 61–90, and 90+ days — summing quantity and
//! value per bucket. This is a pure computed report (no persistence): callers
//! supply the cost layers and an as-of date.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductId;

/// A single cost layer feeding the aging report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgingCostLayer {
    /// Product, if known.
    pub product_id: Option<ProductId>,
    /// SKU.
    pub sku: String,
    /// Remaining quantity in this layer.
    pub quantity: Decimal,
    /// Unit cost of the layer.
    pub unit_cost: Decimal,
    /// Date the layer was received.
    pub received_at: NaiveDate,
}

impl AgingCostLayer {
    /// Extended value of the layer (`quantity × unit_cost`).
    #[must_use]
    pub fn value(&self) -> Decimal {
        self.quantity * self.unit_cost
    }

    /// Age in whole days as of the given date (clamped at zero for future
    /// receipts).
    #[must_use]
    pub fn age_days(&self, as_of: NaiveDate) -> i64 {
        (as_of - self.received_at).num_days().max(0)
    }
}

/// One aging bucket with summed quantity and value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAgingBucket {
    /// Human-readable label (e.g. `"0-30"`).
    pub label: String,
    /// Inclusive lower bound in days.
    pub min_days: i64,
    /// Inclusive upper bound in days, or `None` for the open-ended top bucket.
    pub max_days: Option<i64>,
    /// Total quantity in this bucket.
    pub quantity: Decimal,
    /// Total value in this bucket.
    pub value: Decimal,
}

impl InventoryAgingBucket {
    fn new(label: &str, min_days: i64, max_days: Option<i64>) -> Self {
        Self {
            label: label.to_string(),
            min_days,
            max_days,
            quantity: Decimal::ZERO,
            value: Decimal::ZERO,
        }
    }

    /// Whether an age (in days) falls in this bucket.
    #[must_use]
    pub const fn contains(&self, age_days: i64) -> bool {
        if age_days < self.min_days {
            return false;
        }
        match self.max_days {
            Some(max) => age_days <= max,
            None => true,
        }
    }
}

/// The computed inventory aging report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAgingReport {
    /// The as-of date used for aging.
    pub as_of: NaiveDate,
    /// Buckets, oldest-bound first.
    pub buckets: Vec<InventoryAgingBucket>,
    /// Total quantity across all layers.
    pub total_quantity: Decimal,
    /// Total value across all layers.
    pub total_value: Decimal,
}

/// Compute the standard inventory aging report (0–30 / 31–60 / 61–90 / 90+)
/// for the given cost layers as of `as_of`.
#[must_use]
pub fn compute_inventory_aging(
    layers: &[AgingCostLayer],
    as_of: NaiveDate,
) -> InventoryAgingReport {
    let mut buckets = vec![
        InventoryAgingBucket::new("0-30", 0, Some(30)),
        InventoryAgingBucket::new("31-60", 31, Some(60)),
        InventoryAgingBucket::new("61-90", 61, Some(90)),
        InventoryAgingBucket::new("90+", 91, None),
    ];
    let mut total_quantity = Decimal::ZERO;
    let mut total_value = Decimal::ZERO;

    for layer in layers {
        let age = layer.age_days(as_of);
        let value = layer.value();
        total_quantity += layer.quantity;
        total_value += value;
        if let Some(bucket) = buckets.iter_mut().find(|b| b.contains(age)) {
            bucket.quantity += layer.quantity;
            bucket.value += value;
        }
    }

    InventoryAgingReport { as_of, buckets, total_quantity, total_value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn layer(sku: &str, qty: Decimal, cost: Decimal, received: NaiveDate) -> AgingCostLayer {
        AgingCostLayer {
            product_id: None,
            sku: sku.into(),
            quantity: qty,
            unit_cost: cost,
            received_at: received,
        }
    }

    #[test]
    fn age_days_clamped() {
        let l = layer("a", dec!(1), dec!(1), day(2026, 7, 1));
        // future receipt → 0
        assert_eq!(l.age_days(day(2026, 6, 1)), 0);
        assert_eq!(l.age_days(day(2026, 7, 31)), 30);
    }

    #[test]
    fn buckets_assign_by_age() {
        let as_of = day(2026, 6, 30);
        let layers = vec![
            layer("fresh", dec!(10), dec!(2), day(2026, 6, 20)), // 10 days → 0-30
            layer("mid", dec!(5), dec!(4), day(2026, 5, 10)),    // 51 days → 31-60
            layer("old", dec!(2), dec!(10), day(2026, 1, 1)),    // ~180 days → 90+
        ];
        let report = compute_inventory_aging(&layers, as_of);
        assert_eq!(report.total_quantity, dec!(17));
        assert_eq!(report.total_value, dec!(20) + dec!(20) + dec!(20)); // 20,20,20

        let b0 = &report.buckets[0];
        assert_eq!(b0.label, "0-30");
        assert_eq!(b0.quantity, dec!(10));
        assert_eq!(b0.value, dec!(20));

        let b1 = &report.buckets[1];
        assert_eq!(b1.quantity, dec!(5));
        assert_eq!(b1.value, dec!(20));

        let b3 = &report.buckets[3];
        assert_eq!(b3.label, "90+");
        assert_eq!(b3.quantity, dec!(2));
        assert_eq!(b3.value, dec!(20));

        // empty middle bucket
        assert_eq!(report.buckets[2].quantity, dec!(0));
    }

    #[test]
    fn boundary_ages() {
        let as_of = day(2026, 6, 30);
        // exactly 30 days → 0-30 bucket
        let at_30 = layer("x", dec!(1), dec!(1), as_of - chrono::Duration::days(30));
        // exactly 31 days → 31-60 bucket
        let at_31 = layer("y", dec!(1), dec!(1), as_of - chrono::Duration::days(31));
        let report = compute_inventory_aging(&[at_30, at_31], as_of);
        assert_eq!(report.buckets[0].quantity, dec!(1));
        assert_eq!(report.buckets[1].quantity, dec!(1));
    }

    #[test]
    fn empty_report() {
        let report = compute_inventory_aging(&[], day(2026, 6, 30));
        assert_eq!(report.total_quantity, dec!(0));
        assert_eq!(report.buckets.len(), 4);
    }
}
