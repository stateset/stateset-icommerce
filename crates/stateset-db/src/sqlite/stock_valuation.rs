//! Owned inventory valuation from each SKU's stored costing method and carrying layers.
use chrono::Datelike;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CostMethod, SkuCostSummary};
type DbResult<T> = Result<T, rusqlite::Error>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
fn add(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_add(b).ok_or_else(|| invalid("Inventory valuation overflow"))
}
fn mul(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| invalid("Inventory valuation overflow"))
}
fn summaries_inner(
    tx: &rusqlite::Connection,
    wanted: Option<&str>,
    fallback: CostMethod,
    allow_unvalued: bool,
) -> DbResult<Vec<SkuCostSummary>> {
    let base = super::ledger_fx::base_currency(tx)?;
    let now = chrono::Utc::now();
    let date = now.date_naive().to_string();
    let year = now.year().to_string();
    let skus = {
        let mut stmt = tx.prepare(
            "SELECT sku FROM inventory_items WHERE (?1 IS NULL OR sku = ?1) ORDER BY sku",
        )?;
        let rows = stmt.query_map(rusqlite::params![wanted], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut summaries = Vec::new();
    for sku in skus {
        let rows = {
            let mut stmt = tx.prepare("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = ?1")?;
            let rows = stmt.query_map(rusqlite::params![&sku], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut quantity = rows.into_iter().try_fold(Decimal::ZERO, add)?;
        let transit = {
            let mut stmt = tx.prepare("SELECT t.quantity_shipped, t.quantity_received, t.quantity_variance FROM transfer_order_items t JOIN transfer_orders o ON o.id = t.transfer_order_id WHERE t.sku = ?1 AND o.source_location_id IS NOT NULL")?;
            let rows = stmt.query_map(rusqlite::params![&sku], |r| {
                Ok((
                    super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                    super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (shipped, received, variance) in transit {
            let qty = shipped - received - variance;
            if qty < Decimal::ZERO {
                return Err(invalid("Invalid owned transit quantity"));
            }
            quantity = add(quantity, qty)?;
        }
        let item=tx.query_row("SELECT cost_method, standard_cost, average_cost, currency FROM item_costs WHERE sku = ?1", rusqlite::params![&sku], |r| Ok((r.get::<_, String>(0)?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, r.get::<_, String>(3)?))).optional()?;
        let known = item.is_some();
        let (method, standard, average, currency) = item.unwrap_or_else(|| {
            (fallback.to_string(), Decimal::ZERO, Decimal::ZERO, base.to_string())
        });
        let currency = currency.parse().map_err(|_| invalid("Invalid costing currency"))?;
        let layers = {
            let mut stmt = tx.prepare("SELECT remaining_quantity, unit_cost, CAST(base_remaining_cost AS TEXT), base_currency, substr(CAST(layer_date AS TEXT),1,10) FROM cost_layers WHERE sku = ?1")?;
            let rows = stmt.query_map(rusqlite::params![&sku], |r| {
                Ok((
                    super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let layered = !layers.is_empty();
        let mut costed = Decimal::ZERO;
        let mut value = Decimal::ZERO;
        for (qty, unit, saved, denomination, date) in layers {
            if qty < Decimal::ZERO || unit < Decimal::ZERO {
                return Err(invalid("Invalid cost layer"));
            }
            if denomination.as_deref().is_some_and(|code| code != base.as_str()) {
                return Err(invalid("Reconcile cost layer functional currency"));
            }
            let remaining = if let Some(saved) = saved {
                saved.parse::<Decimal>().map_err(|_| invalid("Invalid layer carrying value"))?
            } else {
                let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
                mul(mul(qty, unit)?, rate)?
            };
            if remaining < Decimal::ZERO || qty.is_zero() && !remaining.is_zero() {
                return Err(invalid("Cost layer quantity and carrying value disagree"));
            }
            costed = add(costed, qty)?;
            value = add(value, remaining)?;
        }
        if layered && costed != quantity {
            return Err(invalid(&format!(
                "Stock/cost-layer mismatch for {sku}: {quantity} owned units, {costed} costed units; reconcile before valuation"
            )));
        }
        let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
        let standard_cost = mul(standard, rate)?;
        let average_cost = if quantity > Decimal::ZERO && layered {
            value.checked_div(quantity).ok_or_else(|| invalid("Invalid average inventory cost"))?
        } else {
            mul(average, rate)?
        };
        if !layered && quantity != Decimal::ZERO {
            if (!known || matches!(method.as_str(), "fifo" | "lifo" | "specific"))
                && !allow_unvalued
            {
                return Err(invalid(&format!("Missing inventory cost layers for {sku}")));
            }
            value = if !known || matches!(method.as_str(), "fifo" | "lifo" | "specific") {
                Decimal::ZERO
            } else {
                mul(quantity, if method == "standard" { standard_cost } else { average_cost })?
            };
        }
        let variances = {
            let mut stmt = tx.prepare("SELECT total_variance, substr(CAST(variance_date AS TEXT),1,10) FROM cost_variances WHERE sku = ?1 AND substr(CAST(variance_date AS TEXT),1,4) = ?2")?;
            let rows = stmt.query_map(rusqlite::params![&sku, &year], |r| {
                Ok((
                    super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                    r.get::<_, String>(1)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut variance_ytd = Decimal::ZERO;
        for (amount, date) in variances {
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
            variance_ytd = add(variance_ytd, mul(amount, rate)?)?;
        }
        summaries.push(SkuCostSummary {
            sku,
            quantity_on_hand: quantity,
            standard_cost,
            average_cost,
            total_value: value,
            variance_ytd,
        });
    }
    Ok(summaries)
}

pub(super) fn summaries(
    tx: &rusqlite::Connection,
    wanted: Option<&str>,
    fallback: CostMethod,
) -> DbResult<Vec<SkuCostSummary>> {
    summaries_inner(tx, wanted, fallback, false)
}

pub(super) fn summaries_for_close(tx: &rusqlite::Connection) -> DbResult<Vec<SkuCostSummary>> {
    summaries_inner(tx, None, CostMethod::Average, true)
}
