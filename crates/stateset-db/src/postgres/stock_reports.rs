//! Historical owned-stock aging reconstructed from dated cost-layer changes.
use super::map_db_error;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, InventoryAgingReport};
use uuid::Uuid;
type DbResult<T> = Result<T, CommerceError>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn mul(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| invalid("Inventory aging overflow"))
}
pub(super) async fn aging(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    as_of: NaiveDate,
) -> DbResult<InventoryAgingReport> {
    let end = as_of
        .succ_opt()
        .ok_or_else(|| invalid("Invalid as-of date"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| invalid("Invalid as-of date"))?
        .and_utc();
    let base = super::ledger_fx::base_currency(tx).await?;
    let mut movements=sqlx::query_as::<_, (String, Decimal)>("SELECT i.sku, t.quantity FROM inventory_transactions t JOIN inventory_items i ON i.id = t.item_id WHERE t.transaction_type NOT IN ('allocation','deallocation','transfer') AND t.created_at < $1 ORDER BY t.id").bind(end).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let losses=sqlx::query_as::<_, (String, Decimal)>("SELECT t.sku,t.quantity FROM inventory_movements t WHERE t.reference_type = 'transfer_shortage' AND t.created_at < $1").bind(end).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    movements.extend(losses);
    let stored=sqlx::query_as::<_, (Uuid, String, String, Decimal, Decimal, Option<String>, Option<String>, Option<String>)>("SELECT l.id,l.sku,substr(CAST(l.layer_date AS TEXT),1,10),l.quantity,l.unit_cost,CAST(l.base_total_cost AS TEXT),l.base_currency,c.currency FROM cost_layers l LEFT JOIN item_costs c ON c.sku = l.sku WHERE l.created_at < $1 AND substr(CAST(l.layer_date AS TEXT),1,10) <= $2 ORDER BY l.layer_date,l.id").bind(end).bind(as_of.to_string()).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut layers = Vec::new();
    for (id, sku, date, qty, unit, saved, denomination, currency) in stored {
        if denomination.as_deref().is_some_and(|code| code != base.as_str()) {
            return Err(invalid("Reconcile inventory functional currency"));
        }
        let value = if let Some(saved) = saved {
            saved.parse().map_err(|_| invalid("Invalid layer carrying value"))?
        } else {
            let currency = currency
                .as_deref()
                .unwrap_or(base.as_str())
                .parse()
                .map_err(|_| invalid("Invalid item cost currency"))?;
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
            mul(mul(qty, unit)?, rate)?
        };
        layers.push((
            id,
            sku,
            date.parse::<NaiveDate>().map_err(|_| invalid("Invalid layer date"))?,
            qty,
            value,
        ));
    }
    let mut changes=sqlx::query_as::<_, (Uuid, Decimal, Decimal, String)>("SELECT t.layer_id,t.quantity_delta,t.value_delta,t.currency FROM stock_cost_layer_changes t WHERE t.created_at < $1").bind(end).fetch_all(tx.as_mut()).await.map_err(map_db_error)?.into_iter().map(|(id,qty,value,currency)|{if currency != base.as_str(){Err(invalid("Reconcile inventory history currency"))}else{Ok((id,-qty,-value))}}).collect::<DbResult<Vec<_>>>()?;
    let manual=sqlx::query_as::<_, (Uuid, Decimal)>("SELECT t.layer_id,t.quantity FROM cost_transactions t WHERE t.layer_id IS NOT NULL AND t.transaction_type = 'issue' AND t.created_at < $1").bind(end).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    for (id, qty) in manual {
        let layer = layers
            .iter()
            .find(|l| l.0 == id)
            .ok_or_else(|| invalid("Missing cost layer for historical issue"))?;
        let unit = layer
            .4
            .checked_div(layer.3)
            .ok_or_else(|| invalid("Invalid historical layer quantity"))?;
        changes.push((id, qty, mul(qty, unit)?));
    }
    crate::stock_reports::aging(as_of, movements, layers, changes)
}
