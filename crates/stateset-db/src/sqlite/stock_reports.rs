//! Historical owned-stock aging reconstructed from dated cost-layer changes.
use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, InventoryAgingReport};
type DbResult<T> = Result<T, rusqlite::Error>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
fn mul(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| invalid("Inventory aging overflow"))
}
pub(super) fn aging(tx: &rusqlite::Connection, as_of: NaiveDate) -> DbResult<InventoryAgingReport> {
    let end = as_of.succ_opt().ok_or_else(|| invalid("Invalid as-of date"))?.to_string();
    let base = super::ledger_fx::base_currency(tx)?;
    let mut movements = {
        let mut stmt = tx.prepare("SELECT i.sku, t.quantity FROM inventory_transactions t JOIN inventory_items i ON i.id = t.item_id WHERE t.transaction_type NOT IN ('allocation','deallocation','transfer') AND julianday(t.created_at) < julianday(?1) ORDER BY t.id")?;
        let rows = stmt.query_map(rusqlite::params![&end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let losses = {
        let mut stmt = tx.prepare("SELECT t.sku,t.quantity FROM inventory_movements t WHERE t.reference_type = 'transfer_shortage' AND julianday(t.created_at) < julianday(?1)")?;
        let rows = stmt.query_map(rusqlite::params![&end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    movements.extend(losses);
    let stored = {
        let mut stmt = tx.prepare("SELECT l.id,l.sku,substr(CAST(l.layer_date AS TEXT),1,10),l.quantity,l.unit_cost,CAST(l.base_total_cost AS TEXT),l.base_currency,c.currency FROM cost_layers l LEFT JOIN item_costs c ON c.sku = l.sku WHERE julianday(l.created_at) < julianday(?1) AND substr(CAST(l.layer_date AS TEXT),1,10) <= ?2 ORDER BY l.layer_date,l.id")?;
        let rows = stmt.query_map(rusqlite::params![&end, &as_of.to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(4)?, "stock", "quantity")?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
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
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
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
    let mut changes={ let mut stmt = tx.prepare("SELECT t.layer_id,t.quantity_delta,t.value_delta,t.currency FROM stock_cost_layer_changes t WHERE julianday(t.created_at) < julianday(?1)")?; let rows = stmt.query_map(rusqlite::params![&end], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, r.get::<_, String>(3)?)))?; rows.collect::<rusqlite::Result<Vec<_>>>()? }.into_iter().map(|(id,qty,value,currency)|{if currency != base.as_str(){Err(invalid("Reconcile inventory history currency"))}else{Ok((id,-qty,-value))}}).collect::<DbResult<Vec<_>>>()?;
    let manual = {
        let mut stmt = tx.prepare("SELECT t.layer_id,t.quantity FROM cost_transactions t WHERE t.layer_id IS NOT NULL AND t.transaction_type = 'issue' AND julianday(t.created_at) < julianday(?1)")?;
        let rows = stmt.query_map(rusqlite::params![&end], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
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
    crate::stock_reports::aging(as_of, movements, layers, changes).map_err(super::bins::smuggle)
}
