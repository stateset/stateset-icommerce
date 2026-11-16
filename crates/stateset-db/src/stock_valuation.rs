//! Exact aggregate of authoritative per-SKU carrying values.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CostMethod, InventoryValuation, Result, SkuCostSummary};
pub(crate) fn report(
    rows: Vec<SkuCostSummary>,
    fallback: CostMethod,
) -> Result<InventoryValuation> {
    let overflow = || CommerceError::ValidationError("Inventory valuation overflow".into());
    let mut total_quantity = Decimal::ZERO;
    let mut total_value = Decimal::ZERO;
    for row in rows {
        total_quantity = total_quantity.checked_add(row.quantity_on_hand).ok_or_else(overflow)?;
        total_value = total_value.checked_add(row.total_value).ok_or_else(overflow)?;
    }
    let average_unit_cost = if total_quantity.is_zero() {
        Decimal::ZERO
    } else {
        total_value.checked_div(total_quantity).ok_or_else(overflow)?
    };
    Ok(InventoryValuation {
        total_quantity,
        total_value,
        average_unit_cost,
        valuation_method: fallback,
        as_of_date: chrono::Utc::now(),
    })
}
