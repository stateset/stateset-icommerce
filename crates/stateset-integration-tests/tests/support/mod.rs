//! Helpers shared by the invariant harnesses (`invariants.rs` — the books —
//! and `cross_entity_invariants.rs` — the order lifecycle across entities).
//!
//! Each integration-test file is its own crate, so a helper one harness does
//! not use would trip `dead_code`; the allow keeps the module shareable.
#![allow(dead_code)]

use rust_decimal::Decimal;

/// Minor units for the harness currency (USD).
pub(crate) const MONEY_SCALE: u32 = 2;

/// No money value may carry more decimal places than the currency allows.
pub(crate) fn money_scale(what: &str, value: Decimal) -> Result<(), String> {
    if value.normalize().scale() > MONEY_SCALE {
        return Err(format!("MONEY SCALE: {what} = {value} has more than {MONEY_SCALE} decimals"));
    }
    Ok(())
}

/// `pct`% of `amount`, rounded to the currency scale.
pub(crate) fn pct_of(amount: Decimal, pct: u8) -> Decimal {
    (amount * Decimal::from(pct) / Decimal::from(100)).round_dp(MONEY_SCALE)
}

/// Case count: the first of `vars` that parses, else `PROPTEST_CASES`, else
/// `default`.
pub(crate) fn cases_from_env(vars: &[&str], default: u32) -> u32 {
    vars.iter()
        .chain(std::iter::once(&"PROPTEST_CASES"))
        .find_map(|var| std::env::var(var).ok().and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

/// Render a caught panic payload.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "non-string panic".into())
}
