//! Owned inventory valuation from each SKU's stored costing method and carrying layers.
use super::map_db_error;
use chrono::Datelike;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CostMethod, SkuCostSummary};
type DbResult<T> = Result<T, CommerceError>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn add(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_add(b).ok_or_else(|| invalid("Inventory valuation overflow"))
}
fn mul(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| invalid("Inventory valuation overflow"))
}
async fn summaries_inner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    wanted: Option<&str>,
    fallback: CostMethod,
    allow_unvalued: bool,
) -> DbResult<Vec<SkuCostSummary>> {
    let base = super::ledger_fx::base_currency(tx).await?;
    let now = chrono::Utc::now();
    let date = now.date_naive().to_string();
    let year = now.year().to_string();
    let skus = sqlx::query_scalar::<_, String>(
        "SELECT sku FROM inventory_items WHERE (CAST($1 AS TEXT) IS NULL OR sku = $1) ORDER BY sku",
    )
    .bind(wanted)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let mut summaries = Vec::new();
    for sku in skus {
        let rows=sqlx::query_scalar::<_, Decimal>("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1").bind(&sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut quantity = rows.into_iter().try_fold(Decimal::ZERO, add)?;
        let transit=sqlx::query_as::<_, (Decimal, Decimal, Decimal)>("SELECT t.quantity_shipped, t.quantity_received, t.quantity_variance FROM transfer_order_items t JOIN transfer_orders o ON o.id = t.transfer_order_id WHERE t.sku = $1 AND o.source_location_id IS NOT NULL").bind(&sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        for (shipped, received, variance) in transit {
            let qty = shipped - received - variance;
            if qty < Decimal::ZERO {
                return Err(invalid("Invalid owned transit quantity"));
            }
            quantity = add(quantity, qty)?;
        }
        let item=sqlx::query_as::<_, (String, Decimal, Decimal, String)>("SELECT cost_method, standard_cost, average_cost, currency FROM item_costs WHERE sku = $1").bind(&sku).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
        let known = item.is_some();
        let (method, standard, average, currency) = item.unwrap_or_else(|| {
            (fallback.to_string(), Decimal::ZERO, Decimal::ZERO, base.to_string())
        });
        let currency = currency.parse().map_err(|_| invalid("Invalid costing currency"))?;
        let layers=sqlx::query_as::<_, (Decimal, Decimal, Option<String>, Option<String>, String)>("SELECT remaining_quantity, unit_cost, CAST(base_remaining_cost AS TEXT), base_currency, substr(CAST(layer_date AS TEXT),1,10) FROM cost_layers WHERE sku = $1").bind(&sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
                let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
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
        let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
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
        let variances=sqlx::query_as::<_, (Decimal, String)>("SELECT total_variance, substr(CAST(variance_date AS TEXT),1,10) FROM cost_variances WHERE sku = $1 AND substr(CAST(variance_date AS TEXT),1,4) = $2").bind(&sku).bind(&year).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut variance_ytd = Decimal::ZERO;
        for (amount, date) in variances {
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
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

pub(super) async fn summaries(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    wanted: Option<&str>,
    fallback: CostMethod,
) -> DbResult<Vec<SkuCostSummary>> {
    summaries_inner(tx, wanted, fallback, false).await
}

pub(super) async fn summaries_for_close(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> DbResult<Vec<SkuCostSummary>> {
    summaries_inner(tx, None, CostMethod::Average, true).await
}
