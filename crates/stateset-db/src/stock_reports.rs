//! Reconcile a stored cost-layer view against physical inventory movements.
use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, InventoryAgingReport, Result};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) type Layer = (Uuid, String, NaiveDate, Decimal, Decimal);
fn invalid(message: impl Into<String>) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn add(left: Decimal, right: Decimal) -> Result<Decimal> {
    left.checked_add(right).ok_or_else(|| invalid("Inventory report decimal overflow"))
}

pub(crate) fn aging(
    as_of: NaiveDate,
    movements: Vec<(String, Decimal)>,
    layers: Vec<Layer>,
    issues: Vec<(Uuid, Decimal, Decimal)>,
) -> Result<InventoryAgingReport> {
    let mut stock = BTreeMap::new();
    for (sku, quantity) in movements {
        let total = stock.entry(sku).or_insert(Decimal::ZERO);
        *total = add(*total, quantity)?;
    }
    let mut consumed = BTreeMap::new();
    for (id, quantity, value) in issues {
        let total = consumed.entry(id).or_insert((Decimal::ZERO, Decimal::ZERO));
        total.0 = add(total.0, quantity)?;
        total.1 = add(total.1, value)?;
    }
    let mut costed = BTreeMap::new();
    let mut report = stateset_core::compute_inventory_aging(&[], as_of);
    for (id, sku, received_at, quantity, value) in layers {
        let (used_qty, used_value) = consumed.get(&id).copied().unwrap_or_default();
        let remaining = add(quantity, -used_qty)?;
        let value = add(value, -used_value)?;
        if remaining < Decimal::ZERO
            || value < Decimal::ZERO
            || remaining.is_zero() && !value.is_zero()
        {
            return Err(invalid(format!(
                "Invalid stored cost layer for {sku}; reconcile before reporting"
            )));
        }
        let total = costed.entry(sku).or_insert(Decimal::ZERO);
        *total = add(*total, remaining)?;
        report.total_quantity = add(report.total_quantity, remaining)?;
        report.total_value = add(report.total_value, value)?;
        let age = (as_of - received_at).num_days();
        let bucket = report
            .buckets
            .iter_mut()
            .find(|b| b.contains(age))
            .ok_or_else(|| invalid("Cost layer is dated after the report"))?;
        bucket.quantity = add(bucket.quantity, remaining)?;
        bucket.value = add(bucket.value, value)?;
    }
    for sku in stock.keys().chain(costed.keys()) {
        let physical = stock.get(sku).copied().unwrap_or_default();
        let valued = costed.get(sku).copied().unwrap_or_default();
        if physical < Decimal::ZERO || physical != valued {
            return Err(invalid(format!(
                "Stock/cost-layer mismatch for {sku} as of {as_of}: {physical} owned units, {valued} costed units; reconcile before reporting"
            )));
        }
    }
    Ok(report)
}
