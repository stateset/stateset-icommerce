//! Exact arithmetic shared by both BOM rollup implementations.

use rust_decimal::Decimal;
use stateset_core::{CommerceError, Result};

pub(crate) fn overflow() -> CommerceError {
    CommerceError::ValidationError("BOM rollup exceeds decimal precision".into())
}

pub(crate) fn add_cost(total: &mut Decimal, quantity: Decimal, cost: Decimal) -> Result<()> {
    if cost < Decimal::ZERO {
        return Err(CommerceError::ValidationError("BOM component cost cannot be negative".into()));
    }
    *total = quantity
        .checked_mul(cost)
        .and_then(|value| total.checked_add(value))
        .ok_or_else(overflow)?;
    Ok(())
}
