//! Operational inventory costs and journals belong to the physical movement transaction.
use super::general_ledger::SqliteGeneralLedgerRepository as Ledger;
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateJournalEntry, CurrencyCode, MovementType};
use uuid::Uuid;
type DbResult<T> = Result<T, rusqlite::Error>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
fn decimal(value: &str) -> DbResult<Decimal> {
    value.parse().map_err(|_| invalid("Invalid inventory cost"))
}
fn add(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_add(b).ok_or_else(|| invalid("Inventory cost overflow"))
}
fn mul(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| invalid("Inventory cost overflow"))
}
fn div(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_div(b).ok_or_else(|| invalid("Invalid inventory cost divisor"))
}
pub(super) fn post(tx: &rusqlite::Connection, movement: &StockMove<'_>, now: &str) -> DbResult<()> {
    // Internal placement and owned transit retain their carrying value.
    if movement.reference_type == Some("lot_relabel")
        || movement.from.is_some() && movement.to.is_some()
        || movement.lot_effect == LotEffect::PlacementOnly
    {
        return Ok(());
    }
    let config = Ledger::get_auto_posting_config_with_conn(tx)?;
    let base = super::ledger_fx::base_currency(tx)?;
    let date = now.get(..10).ok_or_else(|| invalid("Invalid movement date"))?.to_string();
    let item = tx.query_row("SELECT cost_method, standard_cost, average_cost, currency FROM item_costs WHERE sku = ?1", rusqlite::params![movement.sku], |r| Ok((r.get::<_, String>(0)?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, r.get::<_, String>(3)?))).optional()?;
    let mut receipt = None;
    if movement.reference_type == Some("put_away") {
        let id =
            movement.reference_id.ok_or_else(|| invalid("Receipt movement needs its put-away"))?;
        let (amount, currency) = tx.query_row("SELECT CAST(i.unit_cost AS TEXT), p.currency FROM put_aways a JOIN receipt_items i ON i.id = a.receipt_item_id LEFT JOIN purchase_order_items pi ON pi.id = i.po_line_id LEFT JOIN purchase_orders p ON p.id = pi.purchase_order_id WHERE a.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)))?;
        if let Some(amount) = amount {
            receipt = Some((decimal(&amount)?, currency));
        }
    }
    if receipt.is_none() && movement.to.is_some() {
        if let Some(lot) = movement.lot_id {
            let cost = tx.query_row(
                "SELECT CAST(cost_per_unit AS TEXT) FROM lots WHERE id = ?1",
                rusqlite::params![(lot).to_string()],
                |r| r.get::<_, Option<String>>(0),
            )?;
            if let Some(cost) = cost {
                receipt = Some((decimal(&cost)?, None));
            }
        }
    }
    if item.is_none() && receipt.is_none() && movement.reference_type != Some("work_order_output") {
        if config.is_some() {
            return Err(invalid("Set an inventory cost or receipt unit cost before posting stock"));
        }
        return Ok(());
    }
    let (method, standard, average, currency) =
        item.unwrap_or_else(|| ("average".into(), Decimal::ZERO, Decimal::ZERO, base.to_string()));
    let currency: CurrencyCode =
        currency.parse().map_err(|_| invalid("Invalid item cost currency"))?;
    let item_rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
    let mut kind = if movement.to.is_some() {
        if movement.movement_type == MovementType::Return {
            "return"
        } else if movement.movement_type == MovementType::Receipt {
            "receipt"
        } else {
            "adjustment_in"
        }
    } else if movement.movement_type == MovementType::Shipment {
        "issue"
    } else {
        "adjustment_out"
    };
    if movement.reference_type == Some("vendor_return") {
        kind = "vendor_return";
    }
    let mut work_order = None;
    if movement.reference_type == Some("inventory_reservation") {
        let id = movement.reference_id.ok_or_else(|| invalid("Missing reservation"))?;
        let (reference, owner) = tx.query_row(
            "SELECT reference_type, reference_id FROM inventory_reservations WHERE id = ?1",
            rusqlite::params![(id).to_string()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )?;
        if reference == "work_order" {
            kind = "work_order_issue";
            work_order =
                Some(owner.parse::<Uuid>().map_err(|_| invalid("Invalid work order reservation"))?);
        }
    }
    if movement.reference_type == Some("work_order_output") {
        kind = "work_order_output";
        work_order = movement.reference_id;
    }
    let layers = {
        let mut stmt = tx.prepare("SELECT id, remaining_quantity, unit_cost, CAST(base_remaining_cost AS TEXT), base_currency, substr(CAST(layer_date AS TEXT),1,10), lot_id FROM cost_layers WHERE sku = ?1 ORDER BY layer_date, created_at, id")?;
        let rows = stmt.query_map(rusqlite::params![movement.sku], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                super::parse_uuid_opt_row(r.get(6)?, "stock", "id")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut layers = layers.into_iter().filter(|l| l.1 > Decimal::ZERO).collect::<Vec<_>>();
    let mut old_qty = Decimal::ZERO;
    let mut old_value = Decimal::ZERO;
    for layer in &mut layers {
        if layer.4.as_deref().is_some_and(|code| code != base.as_str()) {
            return Err(invalid("Cost layer functional currency requires reconciliation"));
        }
        let value = if let Some(value) = &layer.3 {
            decimal(value)?
        } else {
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &layer.5)?;
            mul(mul(layer.1, layer.2)?, rate)?
        };
        if value < Decimal::ZERO {
            return Err(invalid("Negative inventory carrying cost"));
        }
        old_qty = add(old_qty, layer.1)?;
        old_value = add(old_value, value)?;
        tx.execute(
            "UPDATE cost_layers SET base_remaining_cost = ?1, base_currency = ?2 WHERE id = ?3",
            rusqlite::params![(value).to_string(), base.as_str(), (layer.0).to_string()],
        )?;
        layer.3 = Some(value.to_string());
    }
    let mut settlement_currency = base;
    let mut settlement_rate = Decimal::ONE;
    let mut actual;
    let carrying;
    let reference_id = work_order.or(movement.reference_id);
    let reference_type = if work_order.is_some() { Some(kind) } else { movement.reference_type };
    if movement.to.is_some() {
        let default = if method == "standard" {
            standard
        } else if old_qty > Decimal::ZERO {
            div(div(old_value, old_qty)?, item_rate)?
        } else {
            average.max(standard)
        };
        let (raw, receipt_currency) = receipt.unwrap_or((default, None));
        if raw < Decimal::ZERO {
            return Err(invalid("Receipt cost cannot be negative"));
        }
        let receipt_currency: CurrencyCode = receipt_currency
            .as_deref()
            .unwrap_or(currency.as_str())
            .parse()
            .map_err(|_| invalid("Invalid receipt currency"))?;
        let receipt_rate = super::ledger_fx::rate_as_of(tx, receipt_currency, base, &date)?;
        actual = mul(mul(raw, movement.quantity)?, receipt_rate)?;
        if kind == "receipt" {
            settlement_currency = receipt_currency;
            settlement_rate = receipt_rate;
        }
        if kind == "work_order_output" {
            let id = work_order.ok_or_else(|| invalid("Missing manufacturing source"))?;
            let (target, completed) = tx.query_row("SELECT quantity_to_build, quantity_completed FROM manufacturing_work_orders WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?)))?;
            let completed = add(completed, movement.quantity)?;
            let materials = {
                let mut stmt = tx.prepare("SELECT component_sku, reserved_quantity, consumed_quantity FROM manufacturing_work_order_materials WHERE work_order_id = ?1 ORDER BY component_sku")?;
                let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let mut cumulative = Decimal::ZERO;
            for (sku, required, consumed) in materials {
                let costs = {
                    let mut stmt = tx.prepare("SELECT t.total_cost FROM cost_transactions t JOIN stock_cost_events e ON e.movement_id = t.id WHERE e.work_order_id = ?1 AND e.kind = 'work_order_issue' AND t.sku = ?2")?;
                    let rows = stmt.query_map(rusqlite::params![(id).to_string(), &sku], |r| {
                        super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
                    })?;
                    rows.collect::<rusqlite::Result<Vec<_>>>()?
                };
                let total = costs.into_iter().try_fold(Decimal::ZERO, add)?;
                if consumed <= Decimal::ZERO && required > Decimal::ZERO {
                    return Err(invalid("Manufacturing material costs are not recorded"));
                }
                let required_for_output = if completed == target {
                    required
                } else {
                    div(mul(required, completed)?, target)?
                };
                cumulative = add(
                    cumulative,
                    if required_for_output == consumed {
                        total
                    } else {
                        mul(div(total, consumed)?, required_for_output)?
                    },
                )?;
            }
            let previous = {
                let mut stmt = tx.prepare("SELECT carrying_cost FROM stock_cost_events WHERE work_order_id = ?1 AND kind = 'work_order_output'")?;
                let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
                    super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            actual = add(cumulative, -previous.into_iter().try_fold(Decimal::ZERO, add)?)?;
            if actual < Decimal::ZERO {
                return Err(invalid("Manufacturing output exceeds its allocated material costs"));
            }
        }
        // Restocked returns reinstate the shipment's historical cost, never today's price.
        if kind == "return" {
            let id =
                movement.reference_id.ok_or_else(|| invalid("Restock needs its return item"))?;
            let order = tx.query_row("SELECT r.order_id FROM return_items i JOIN returns r ON r.id = i.return_id WHERE i.id = ?1", rusqlite::params![(id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"))?;
            let shipped = {
                let mut stmt = tx.prepare("SELECT t.quantity, t.total_cost FROM cost_transactions t JOIN stock_cost_events e ON e.movement_id = t.id JOIN inventory_movements m ON m.id = e.movement_id LEFT JOIN inventory_reservations r ON m.reference_type = 'inventory_reservation' AND CAST(r.id AS TEXT) = m.reference_id LEFT JOIN ship_tasks s ON m.reference_type = 'ship_task' AND CAST(s.id AS TEXT) = m.reference_id WHERE e.kind = 'issue' AND t.sku = ?1 AND (r.reference_id = ?2 OR CAST(s.order_id AS TEXT) = ?2 OR m.reference_type = 'order' AND m.reference_id = ?2)")?;
                let rows =
                    stmt.query_map(rusqlite::params![movement.sku, &order.to_string()], |r| {
                        Ok((
                            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                            super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        ))
                    })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let (quantity, value) = shipped
                .into_iter()
                .try_fold((Decimal::ZERO, Decimal::ZERO), |(q, v), (a, b)| {
                    Ok::<_, rusqlite::Error>((add(q, a)?, add(v, b)?))
                })?;
            if quantity < movement.quantity {
                return Err(invalid(
                    "Original shipment cost is missing; reconcile before restocking",
                ));
            }
            actual = mul(div(value, quantity)?, movement.quantity)?;
        }
        carrying = if method == "standard" && !matches!(kind, "return" | "work_order_output") {
            mul(mul(standard, movement.quantity)?, item_rate)?
        } else {
            actual
        };
        let unit_cost = div(div(carrying, movement.quantity)?, item_rate)?;
        let source = if kind == "work_order_output" {
            "production"
        } else if kind == "receipt" {
            "purchase"
        } else {
            "adjustment"
        };
        tx.execute("INSERT INTO cost_layers (id,sku,layer_date,quantity,remaining_quantity,unit_cost,total_cost,source_type,source_id,lot_id,location_id,created_at,base_total_cost,base_remaining_cost,base_currency) VALUES (?1,?2,?3,?4,?4,?5,?6,?7,?1,?8,?9,?3,?10,?10,?11)", rusqlite::params![(movement.id).to_string(), movement.sku, now, (movement.quantity).to_string(), (unit_cost).to_string(), (div(carrying,item_rate)?).to_string(), source, (movement.lot_id).map(|id| id.to_string()), movement.to, (carrying).to_string(), base.as_str()])?;
        let average =
            div(div(add(old_value, carrying)?, add(old_qty, movement.quantity)?)?, item_rate)?;
        tx.execute("INSERT INTO item_costs (id,sku,cost_method,standard_cost,average_cost,last_cost,currency,effective_date,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8,?8) ON CONFLICT(sku) DO UPDATE SET average_cost = excluded.average_cost, last_cost = excluded.last_cost, updated_at = excluded.updated_at", rusqlite::params![(Uuid::new_v4()).to_string(), movement.sku, &method, (standard).to_string(), (average).to_string(), (div(div(actual,movement.quantity)?,item_rate)?).to_string(), currency.as_str(), now])?;
    } else {
        if old_qty < movement.quantity {
            return Err(invalid(
                "Inventory cost layers are incomplete; reconcile opening stock before issuing",
            ));
        }
        if method == "lifo" {
            layers.reverse();
        }
        if method == "specific" {
            if movement.lot_id.is_none() {
                return Err(invalid("Specific costing requires an explicit lot"));
            }
            layers.retain(|layer| layer.6 == movement.lot_id);
        }
        let mut remaining = movement.quantity;
        let mut issued = Decimal::ZERO;
        for (id, qty, unit_cost, value, _, _, _) in layers {
            if remaining.is_zero() {
                break;
            }
            let take = remaining.min(qty);
            let value = decimal(value.as_deref().ok_or_else(|| invalid("Missing layer cost"))?)?;
            let cost = if take == qty { value } else { mul(div(value, qty)?, take)? };
            tx.execute("UPDATE cost_layers SET remaining_quantity = ?1, base_remaining_cost = ?2, base_currency = ?3 WHERE id = ?4", rusqlite::params![(qty-take).to_string(), (value-cost).to_string(), base.as_str(), (id).to_string()])?;
            change(tx, id, Some(movement.id), -take, -cost, base, now)?;
            remaining -= take;
            issued = add(issued, cost)?;
            let _ = unit_cost;
        }
        if remaining > Decimal::ZERO {
            return Err(invalid("Insufficient specific cost layers for this lot"));
        }
        carrying = if method == "average" {
            if old_qty == movement.quantity {
                old_value
            } else {
                mul(div(old_value, old_qty)?, movement.quantity)?
            }
        } else {
            issued
        };
        // Average costing spreads remaining book value over all surviving layers.
        if method == "average" {
            let remaining_layers = {
                let mut stmt = tx.prepare("SELECT id, remaining_quantity FROM cost_layers WHERE sku = ?1 ORDER BY layer_date, id")?;
                let rows = stmt.query_map(rusqlite::params![movement.sku], |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let mut allocated = Decimal::ZERO;
            let mut seen = Decimal::ZERO;
            let remaining_qty = old_qty - movement.quantity;
            let remaining_value = old_value - carrying;
            for (id, qty) in remaining_layers {
                if qty <= Decimal::ZERO {
                    continue;
                }
                seen = add(seen, qty)?;
                let cumulative = if seen == remaining_qty {
                    remaining_value
                } else {
                    mul(div(remaining_value, remaining_qty)?, seen)?
                };
                let previous = tx.query_row(
                    "SELECT base_remaining_cost FROM cost_layers WHERE id = ?1",
                    rusqlite::params![(id).to_string()],
                    |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity"),
                )?;
                tx.execute("UPDATE cost_layers SET base_remaining_cost = ?1, base_currency = ?2 WHERE id = ?3", rusqlite::params![(cumulative-allocated).to_string(), base.as_str(), (id).to_string()])?;
                change(
                    tx,
                    id,
                    Some(movement.id),
                    Decimal::ZERO,
                    cumulative - allocated - previous,
                    base,
                    now,
                )?;
                allocated = cumulative;
            }
        }
        actual = carrying;
    }
    tx.execute("INSERT INTO stock_cost_events (movement_id,kind,currency,carrying_cost,actual_cost,work_order_id) VALUES (?1,?2,?3,?4,?5,?6)", rusqlite::params![(movement.id).to_string(), kind, base.as_str(), (carrying).to_string(), (actual).to_string(), (work_order).map(|id| id.to_string())])?;
    tx.execute("UPDATE stock_cost_events SET settlement_currency = ?1,settlement_rate = ?2 WHERE movement_id = ?3", rusqlite::params![settlement_currency.as_str(), (settlement_rate).to_string(), (movement.id).to_string()])?;
    let transaction_type = if movement.to.is_some() { "receipt" } else { "issue" };
    tx.execute("INSERT INTO cost_transactions (id,sku,transaction_type,quantity,unit_cost,total_cost,reference_type,reference_id,notes,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", rusqlite::params![(movement.id).to_string(), movement.sku, transaction_type, (movement.quantity).to_string(), (div(carrying,movement.quantity)?).to_string(), (carrying).to_string(), reference_type, (reference_id).map(|id| id.to_string()), movement.reason, now])?;
    if let Some(config) = config {
        let lines = crate::ledger_posting::stock_lines(&config, kind, carrying, actual)
            .map_err(super::bins::smuggle)?;
        if !lines.is_empty() {
            Ledger::create_posted_entry_with_conn(
                tx,
                &CreateJournalEntry {
                    entry_date: date.parse().map_err(|_| invalid("Invalid stock date"))?,
                    entry_type: None,
                    description: format!("Stock {kind}: {}", movement.sku),
                    lines,
                    source_document_type: Some("cost_transaction".into()),
                    source_document_id: Some(movement.id),
                    auto_post: Some(true),
                    fx: None,
                },
                "system",
            )?;
        }
    }
    Ok(())
}

fn currency(value: &str) -> DbResult<CurrencyCode> {
    value.parse().map_err(|_| invalid("Invalid cost currency"))
}
fn change(
    tx: &rusqlite::Connection,
    id: Uuid,
    movement: Option<Uuid>,
    quantity: Decimal,
    value: Decimal,
    currency: CurrencyCode,
    now: &str,
) -> DbResult<()> {
    tx.execute("INSERT INTO stock_cost_layer_changes (id,layer_id,movement_id,quantity_delta,value_delta,currency,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)", rusqlite::params![(Uuid::new_v4()).to_string(), (id).to_string(), (movement).map(|id| id.to_string()), (quantity).to_string(), (value).to_string(), currency.as_str(), now])?;
    Ok(())
}
pub(super) fn relabel(
    tx: &rusqlite::Connection,
    source: Uuid,
    target: Uuid,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    let base = super::ledger_fx::base_currency(tx)?;
    let layers = {
        let mut stmt = tx.prepare("SELECT id,sku,remaining_quantity,unit_cost,CAST(base_remaining_cost AS TEXT),substr(CAST(layer_date AS TEXT),1,10),CAST(layer_date AS TEXT),location_id,source_type,source_id FROM cost_layers WHERE lot_id = ?1 ORDER BY layer_date,id")?;
        let rows = stmt.query_map(rusqlite::params![(source).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<i32>>(7)?,
                r.get::<_, String>(8)?,
                super::parse_uuid_opt_row(r.get(9)?, "stock", "id")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if layers.is_empty() {
        return Ok(());
    }
    let mut remaining = quantity;
    for (id, sku, qty, unit, saved, date, layer_date, location, kind, source_id) in layers {
        if qty <= Decimal::ZERO || remaining.is_zero() {
            continue;
        }
        let take = remaining.min(qty);
        let value = if let Some(saved) = saved {
            decimal(&saved)?
        } else {
            let code = tx
                .query_row(
                    "SELECT currency FROM item_costs WHERE sku = ?1",
                    rusqlite::params![&sku],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;
            let currency = currency(code.as_deref().unwrap_or(base.as_str()))?;
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date)?;
            mul(mul(qty, unit)?, rate)?
        };
        let cost = if take == qty { value } else { mul(div(value, qty)?, take)? };
        change(tx, id, None, -take, -cost, base, now)?;
        tx.execute("UPDATE cost_layers SET remaining_quantity = ?1, base_remaining_cost = ?2, base_currency = ?3 WHERE id = ?4", rusqlite::params![(qty-take).to_string(), (value-cost).to_string(), base.as_str(), (id).to_string()])?;

        tx.execute("INSERT INTO cost_layers (id,sku,layer_date,quantity,remaining_quantity,unit_cost,total_cost,source_type,source_id,lot_id,location_id,created_at,base_total_cost,base_remaining_cost,base_currency) VALUES (?1,?2,?3,?4,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12,?13)", rusqlite::params![(Uuid::new_v4()).to_string(), &sku, layer_date, (take).to_string(), (unit).to_string(), (mul(take,unit)?).to_string(), &kind, (source_id).map(|id| id.to_string()), (target).to_string(), location, now, (cost).to_string(), base.as_str()])?;
        remaining -= take;
    }
    if remaining > Decimal::ZERO {
        return Err(invalid("Lot cost layers are incomplete; reconcile before split or merge"));
    }
    Ok(())
}

pub(super) fn guard_master(
    tx: &rusqlite::Connection,
    input: &stateset_core::SetItemCost,
    allow_revalue: bool,
) -> DbResult<()> {
    let current = tx
        .query_row(
            "SELECT cost_method,standard_cost,currency FROM item_costs WHERE sku = ?1",
            rusqlite::params![&input.sku],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    if let Some((method, standard, currency)) = current {
        let layers = {
            let mut stmt =
                tx.prepare("SELECT remaining_quantity FROM cost_layers WHERE sku = ?1")?;
            let rows = stmt.query_map(rusqlite::params![&input.sku], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let physical = {
            let mut stmt = tx.prepare("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = ?1")?;
            let rows = stmt.query_map(rusqlite::params![&input.sku], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let stocked = layers.iter().chain(physical.iter()).any(|q| *q > Decimal::ZERO);
        if input.currency.is_some_and(|value| value.as_str() != currency) {
            let history = tx
                .query_row(
                    "SELECT id FROM cost_transactions WHERE sku = ?1 LIMIT 1",
                    rusqlite::params![&input.sku],
                    |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
                )
                .optional()?;
            if stocked || !layers.is_empty() || history.is_some() {
                return Err(invalid("Cost currency is frozen once inventory cost history exists"));
            }
        }
        if stocked
            && (input.cost_method.is_some_and(|value| value.to_string() != method)
                || !allow_revalue
                    && method == "standard"
                    && input.standard_cost.is_some_and(|value| value != standard))
        {
            return Err(invalid(
                "Reconcile stock before changing its costing method; use an approved adjustment to change standard inventory value",
            ));
        }
    }
    Ok(())
}
pub(super) fn revalue(
    tx: &rusqlite::Connection,
    id: Uuid,
    sku: &str,
    new_cost: Decimal,
    now: &str,
) -> DbResult<()> {
    let summary =
        super::stock_valuation::summaries(tx, Some(sku), stateset_core::CostMethod::Average)?
            .into_iter()
            .next();
    let Some(summary) = summary else {
        return Ok(());
    };
    if summary.quantity_on_hand.is_zero() {
        return Ok(());
    }
    let base = super::ledger_fx::base_currency(tx)?;
    let date = now[..10].to_string();
    let code = tx.query_row(
        "SELECT currency FROM item_costs WHERE sku = ?1",
        rusqlite::params![sku],
        |r| r.get::<_, String>(0),
    )?;
    let rate = super::ledger_fx::rate_as_of(tx, currency(&code)?, base, &date)?;
    let new_unit = mul(new_cost, rate)?;
    let layers = {
        let mut stmt = tx.prepare("SELECT id,remaining_quantity,unit_cost,CAST(base_remaining_cost AS TEXT),substr(CAST(layer_date AS TEXT),1,10) FROM cost_layers WHERE sku = ?1 ORDER BY id")?;
        let rows = stmt.query_map(rusqlite::params![sku], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if layers.is_empty() {
        return Err(invalid("Reconcile opening cost layers before revaluing stock"));
    }
    let mut delta = Decimal::ZERO;
    for (layer, qty, unit, saved, layer_date) in layers {
        if qty.is_zero() {
            continue;
        }
        let previous = if let Some(saved) = saved {
            decimal(&saved)?
        } else {
            let rate = super::ledger_fx::rate_as_of(tx, currency(&code)?, base, &layer_date)?;
            mul(mul(qty, unit)?, rate)?
        };
        let next = mul(qty, new_unit)?;
        let difference = next - previous;
        change(tx, layer, None, Decimal::ZERO, difference, base, now)?;
        tx.execute(
            "UPDATE cost_layers SET base_remaining_cost = ?1, base_currency = ?2 WHERE id = ?3",
            rusqlite::params![(next).to_string(), base.as_str(), (layer).to_string()],
        )?;
        delta = add(delta, difference)?;
    }
    tx.execute("INSERT INTO cost_transactions (id,sku,transaction_type,quantity,unit_cost,total_cost,reference_type,reference_id,created_at,currency) VALUES (?1,?2,'revaluation',?3,?4,?5,'cost_adjustment',?1,?6,?7)", rusqlite::params![(id).to_string(), sku, (summary.quantity_on_hand).to_string(), (div(delta,summary.quantity_on_hand)?).to_string(), (delta).to_string(), now, base.as_str()])?;
    tx.execute(
        "UPDATE item_costs SET average_cost = ?1 WHERE sku = ?2",
        rusqlite::params![(new_cost).to_string(), sku],
    )?;
    if let Some(config) = Ledger::get_auto_posting_config_with_conn(tx)? {
        if let Some(lines) = crate::ledger_posting::inventory_lines(&config, "revaluation", delta)
            .map_err(super::bins::smuggle)?
        {
            Ledger::create_posted_entry_with_conn(
                tx,
                &CreateJournalEntry {
                    entry_date: date.parse().map_err(|_| invalid("Invalid adjustment date"))?,
                    entry_type: None,
                    description: format!("Inventory cost adjustment {id}"),
                    lines,
                    source_document_type: Some("cost_transaction".into()),
                    source_document_id: Some(id),
                    auto_post: Some(true),
                    fx: None,
                },
                "system",
            )?;
        }
    }
    Ok(())
}
