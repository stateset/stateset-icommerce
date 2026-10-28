//! Operational inventory costs and journals belong to the physical movement transaction.
use super::general_ledger::PgGeneralLedgerRepository as Ledger;
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateJournalEntry, CurrencyCode, MovementType};
use uuid::Uuid;
type DbResult<T> = Result<T, CommerceError>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
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
pub(super) async fn post(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    movement: &StockMove<'_>,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<()> {
    // Internal placement and owned transit retain their carrying value.
    if movement.reference_type == Some("lot_relabel")
        || movement.from.is_some() && movement.to.is_some()
        || movement.lot_effect == LotEffect::PlacementOnly
    {
        return Ok(());
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("stock_cost:{}", movement.sku))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let config = Ledger::get_auto_posting_config_tx(tx).await?;
    let base = super::ledger_fx::base_currency(tx).await?;
    let date = now.date_naive().to_string();
    let item = sqlx::query_as::<_, (String, Decimal, Decimal, String)>("SELECT cost_method, standard_cost, average_cost, currency FROM item_costs WHERE sku = $1 FOR UPDATE").bind(movement.sku).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    let mut receipt = None;
    if movement.reference_type == Some("put_away") {
        let id =
            movement.reference_id.ok_or_else(|| invalid("Receipt movement needs its put-away"))?;
        let (amount, currency) = sqlx::query_as::<_, (Option<String>, Option<String>)>("SELECT CAST(i.unit_cost AS TEXT), p.currency FROM put_aways a JOIN receipt_items i ON i.id = a.receipt_item_id LEFT JOIN purchase_order_items pi ON pi.id = i.po_line_id LEFT JOIN purchase_orders p ON p.id = pi.purchase_order_id WHERE a.id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
        if let Some(amount) = amount {
            receipt = Some((decimal(&amount)?, currency));
        }
    }
    if receipt.is_none() && movement.to.is_some() {
        if let Some(lot) = movement.lot_id {
            let cost = sqlx::query_scalar::<_, Option<String>>(
                "SELECT CAST(cost_per_unit AS TEXT) FROM lots WHERE id = $1",
            )
            .bind(lot)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
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
    let item_rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
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
        let (reference, owner) = sqlx::query_as::<_, (String, String)>(
            "SELECT reference_type, reference_id FROM inventory_reservations WHERE id = $1",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
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
    let layers = sqlx::query_as::<_, (Uuid, Decimal, Decimal, Option<String>, Option<String>, String, Option<Uuid>)>("SELECT id, remaining_quantity, unit_cost, CAST(base_remaining_cost AS TEXT), base_currency, substr(CAST(layer_date AS TEXT),1,10), lot_id FROM cost_layers WHERE sku = $1 ORDER BY layer_date, created_at, id FOR UPDATE").bind(movement.sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &layer.5).await?;
            mul(mul(layer.1, layer.2)?, rate)?
        };
        if value < Decimal::ZERO {
            return Err(invalid("Negative inventory carrying cost"));
        }
        old_qty = add(old_qty, layer.1)?;
        old_value = add(old_value, value)?;
        sqlx::query(
            "UPDATE cost_layers SET base_remaining_cost = $1, base_currency = $2 WHERE id = $3",
        )
        .bind(value)
        .bind(base.as_str())
        .bind(layer.0)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
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
        let receipt_rate = super::ledger_fx::rate_as_of(tx, receipt_currency, base, &date).await?;
        actual = mul(mul(raw, movement.quantity)?, receipt_rate)?;
        if kind == "receipt" {
            settlement_currency = receipt_currency;
            settlement_rate = receipt_rate;
        }
        if kind == "work_order_output" {
            let id = work_order.ok_or_else(|| invalid("Missing manufacturing source"))?;
            let (target, completed) = sqlx::query_as::<_, (Decimal, Decimal)>("SELECT quantity_to_build, quantity_completed FROM manufacturing_work_orders WHERE id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
            let completed = add(completed, movement.quantity)?;
            let materials = sqlx::query_as::<_, (String, Decimal, Decimal)>("SELECT component_sku, reserved_quantity, consumed_quantity FROM manufacturing_work_order_materials WHERE work_order_id = $1 ORDER BY component_sku").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
            let mut cumulative = Decimal::ZERO;
            for (sku, required, consumed) in materials {
                let costs = sqlx::query_scalar::<_, Decimal>("SELECT t.total_cost FROM cost_transactions t JOIN stock_cost_events e ON e.movement_id = t.id WHERE e.work_order_id = $1 AND e.kind = 'work_order_issue' AND t.sku = $2").bind(id).bind(&sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
            let previous = sqlx::query_scalar::<_, Decimal>("SELECT carrying_cost FROM stock_cost_events WHERE work_order_id = $1 AND kind = 'work_order_output'").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
            actual = add(cumulative, -previous.into_iter().try_fold(Decimal::ZERO, add)?)?;
            if actual < Decimal::ZERO {
                return Err(invalid("Manufacturing output exceeds its allocated material costs"));
            }
        }
        // Restocked returns reinstate the shipment's historical cost, never today's price.
        if kind == "return" {
            let id =
                movement.reference_id.ok_or_else(|| invalid("Restock needs its return item"))?;
            let order = sqlx::query_scalar::<_, Uuid>("SELECT r.order_id FROM return_items i JOIN returns r ON r.id = i.return_id WHERE i.id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
            let shipped = sqlx::query_as::<_, (Decimal, Decimal)>("SELECT t.quantity, t.total_cost FROM cost_transactions t JOIN stock_cost_events e ON e.movement_id = t.id JOIN inventory_movements m ON m.id = e.movement_id LEFT JOIN inventory_reservations r ON m.reference_type = 'inventory_reservation' AND CAST(r.id AS TEXT) = CAST(m.reference_id AS TEXT) LEFT JOIN ship_tasks s ON m.reference_type = 'ship_task' AND CAST(s.id AS TEXT) = CAST(m.reference_id AS TEXT) WHERE e.kind = 'issue' AND t.sku = $1 AND (r.reference_id = $2 OR CAST(s.order_id AS TEXT) = $2 OR m.reference_type = 'order' AND CAST(m.reference_id AS TEXT) = $2)").bind(movement.sku).bind(order.to_string()).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
            let (quantity, value) = shipped
                .into_iter()
                .try_fold((Decimal::ZERO, Decimal::ZERO), |(q, v), (a, b)| {
                    Ok::<_, CommerceError>((add(q, a)?, add(v, b)?))
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
        sqlx::query("INSERT INTO cost_layers (id,sku,layer_date,quantity,remaining_quantity,unit_cost,total_cost,source_type,source_id,lot_id,location_id,created_at,base_total_cost,base_remaining_cost,base_currency) VALUES ($1,$2,$3,$4,$4,$5,$6,$7,$1,$8,$9,$3,$10,$10,$11)").bind(movement.id).bind(movement.sku).bind(now).bind(movement.quantity).bind(unit_cost).bind(div(carrying,item_rate)?).bind(source).bind(movement.lot_id).bind(movement.to).bind(carrying).bind(base.as_str()).execute(tx.as_mut()).await.map_err(map_db_error)?;
        let average =
            div(div(add(old_value, carrying)?, add(old_qty, movement.quantity)?)?, item_rate)?;
        sqlx::query("INSERT INTO item_costs (id,sku,cost_method,standard_cost,average_cost,last_cost,currency,effective_date,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8,$8) ON CONFLICT(sku) DO UPDATE SET average_cost = excluded.average_cost, last_cost = excluded.last_cost, updated_at = excluded.updated_at").bind(Uuid::new_v4()).bind(movement.sku).bind(&method).bind(standard).bind(average).bind(div(div(actual,movement.quantity)?,item_rate)?).bind(currency.as_str()).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
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
            sqlx::query("UPDATE cost_layers SET remaining_quantity = $1, base_remaining_cost = $2, base_currency = $3 WHERE id = $4").bind(qty-take).bind(value-cost).bind(base.as_str()).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
            change(tx, id, Some(movement.id), -take, -cost, base, now).await?;
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
            let remaining_layers = sqlx::query_as::<_, (Uuid, Decimal)>("SELECT id, remaining_quantity FROM cost_layers WHERE sku = $1 ORDER BY layer_date, id").bind(movement.sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
                let previous = sqlx::query_scalar::<_, Decimal>(
                    "SELECT base_remaining_cost FROM cost_layers WHERE id = $1",
                )
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                sqlx::query("UPDATE cost_layers SET base_remaining_cost = $1, base_currency = $2 WHERE id = $3").bind(cumulative-allocated).bind(base.as_str()).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
                change(
                    tx,
                    id,
                    Some(movement.id),
                    Decimal::ZERO,
                    cumulative - allocated - previous,
                    base,
                    now,
                )
                .await?;
                allocated = cumulative;
            }
        }
        actual = carrying;
    }
    sqlx::query("INSERT INTO stock_cost_events (movement_id,kind,currency,carrying_cost,actual_cost,work_order_id) VALUES ($1,$2,$3,$4,$5,$6)").bind(movement.id).bind(kind).bind(base.as_str()).bind(carrying).bind(actual).bind(work_order).execute(tx.as_mut()).await.map_err(map_db_error)?;
    sqlx::query("UPDATE stock_cost_events SET settlement_currency = $1,settlement_rate = $2 WHERE movement_id = $3").bind(settlement_currency.as_str()).bind(settlement_rate).bind(movement.id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    let transaction_type = if movement.to.is_some() { "receipt" } else { "issue" };
    sqlx::query("INSERT INTO cost_transactions (id,sku,transaction_type,quantity,unit_cost,total_cost,reference_type,reference_id,notes,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(movement.id).bind(movement.sku).bind(transaction_type).bind(movement.quantity).bind(div(carrying,movement.quantity)?).bind(carrying).bind(reference_type).bind(reference_id).bind(movement.reason).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    if let Some(config) = config {
        let lines = crate::ledger_posting::stock_lines(&config, kind, carrying, actual)?;
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
            )
            .await?;
        }
    }
    Ok(())
}

fn currency(value: &str) -> DbResult<CurrencyCode> {
    value.parse().map_err(|_| invalid("Invalid cost currency"))
}
async fn change(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    movement: Option<Uuid>,
    quantity: Decimal,
    value: Decimal,
    currency: CurrencyCode,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<()> {
    sqlx::query("INSERT INTO stock_cost_layer_changes (id,layer_id,movement_id,quantity_delta,value_delta,currency,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7)").bind(Uuid::new_v4()).bind(id).bind(movement).bind(quantity).bind(value).bind(currency.as_str()).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
pub(super) async fn relabel(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source: Uuid,
    target: Uuid,
    quantity: Decimal,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<()> {
    let base = super::ledger_fx::base_currency(tx).await?;
    let layers=sqlx::query_as::<_, (Uuid, String, Decimal, Decimal, Option<String>, String, String, Option<i32>, String, Option<Uuid>)>("SELECT id,sku,remaining_quantity,unit_cost,CAST(base_remaining_cost AS TEXT),substr(CAST(layer_date AS TEXT),1,10),CAST(layer_date AS TEXT),location_id,source_type,source_id FROM cost_layers WHERE lot_id = $1 ORDER BY layer_date,id FOR UPDATE").bind(source).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
            let code =
                sqlx::query_scalar::<_, String>("SELECT currency FROM item_costs WHERE sku = $1")
                    .bind(&sku)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            let currency = currency(code.as_deref().unwrap_or(base.as_str()))?;
            let rate = super::ledger_fx::rate_as_of(tx, currency, base, &date).await?;
            mul(mul(qty, unit)?, rate)?
        };
        let cost = if take == qty { value } else { mul(div(value, qty)?, take)? };
        change(tx, id, None, -take, -cost, base, now).await?;
        sqlx::query("UPDATE cost_layers SET remaining_quantity = $1, base_remaining_cost = $2, base_currency = $3 WHERE id = $4").bind(qty-take).bind(value-cost).bind(base.as_str()).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
        let layer_date: chrono::DateTime<chrono::Utc> = layer_date
            .parse()
            .or_else(|_| {
                chrono::DateTime::parse_from_str(&layer_date, "%Y-%m-%d %H:%M:%S%.f%#z")
                    .map(|date| date.with_timezone(&chrono::Utc))
            })
            .map_err(|_| invalid("Invalid layer date"))?;
        sqlx::query("INSERT INTO cost_layers (id,sku,layer_date,quantity,remaining_quantity,unit_cost,total_cost,source_type,source_id,lot_id,location_id,created_at,base_total_cost,base_remaining_cost,base_currency) VALUES ($1,$2,$3,$4,$4,$5,$6,$7,$8,$9,$10,$11,$12,$12,$13)").bind(Uuid::new_v4()).bind(&sku).bind(layer_date).bind(take).bind(unit).bind(mul(take,unit)?).bind(&kind).bind(source_id).bind(target).bind(location).bind(now).bind(cost).bind(base.as_str()).execute(tx.as_mut()).await.map_err(map_db_error)?;
        remaining -= take;
    }
    if remaining > Decimal::ZERO {
        return Err(invalid("Lot cost layers are incomplete; reconcile before split or merge"));
    }
    Ok(())
}

pub(super) async fn guard_master(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &stateset_core::SetItemCost,
    allow_revalue: bool,
) -> DbResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("stock_cost:{}", input.sku))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let current = sqlx::query_as::<_, (String, Decimal, String)>(
        "SELECT cost_method,standard_cost,currency FROM item_costs WHERE sku = $1 FOR UPDATE",
    )
    .bind(&input.sku)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if let Some((method, standard, currency)) = current {
        let layers = sqlx::query_scalar::<_, Decimal>(
            "SELECT remaining_quantity FROM cost_layers WHERE sku = $1",
        )
        .bind(&input.sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let physical=sqlx::query_scalar::<_, Decimal>("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1").bind(&input.sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let stocked = layers.iter().chain(physical.iter()).any(|q| *q > Decimal::ZERO);
        if input.currency.is_some_and(|value| value.as_str() != currency) {
            let history = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM cost_transactions WHERE sku = $1 LIMIT 1",
            )
            .bind(&input.sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;
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
pub(super) async fn revalue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    sku: &str,
    new_cost: Decimal,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<()> {
    let summary =
        super::stock_valuation::summaries(tx, Some(sku), stateset_core::CostMethod::Average)
            .await?
            .into_iter()
            .next();
    let Some(summary) = summary else {
        return Ok(());
    };
    if summary.quantity_on_hand.is_zero() {
        return Ok(());
    }
    let base = super::ledger_fx::base_currency(tx).await?;
    let date = now.date_naive().to_string();
    let code = sqlx::query_scalar::<_, String>("SELECT currency FROM item_costs WHERE sku = $1")
        .bind(sku)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let rate = super::ledger_fx::rate_as_of(tx, currency(&code)?, base, &date).await?;
    let new_unit = mul(new_cost, rate)?;
    let layers=sqlx::query_as::<_, (Uuid, Decimal, Decimal, Option<String>, String)>("SELECT id,remaining_quantity,unit_cost,CAST(base_remaining_cost AS TEXT),substr(CAST(layer_date AS TEXT),1,10) FROM cost_layers WHERE sku = $1 ORDER BY id FOR UPDATE").bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
            let rate =
                super::ledger_fx::rate_as_of(tx, currency(&code)?, base, &layer_date).await?;
            mul(mul(qty, unit)?, rate)?
        };
        let next = mul(qty, new_unit)?;
        let difference = next - previous;
        change(tx, layer, None, Decimal::ZERO, difference, base, now).await?;
        sqlx::query(
            "UPDATE cost_layers SET base_remaining_cost = $1, base_currency = $2 WHERE id = $3",
        )
        .bind(next)
        .bind(base.as_str())
        .bind(layer)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        delta = add(delta, difference)?;
    }
    sqlx::query("INSERT INTO cost_transactions (id,sku,transaction_type,quantity,unit_cost,total_cost,reference_type,reference_id,created_at,currency) VALUES ($1,$2,'revaluation',$3,$4,$5,'cost_adjustment',$1,$6,$7)").bind(id).bind(sku).bind(summary.quantity_on_hand).bind(div(delta,summary.quantity_on_hand)?).bind(delta).bind(now).bind(base.as_str()).execute(tx.as_mut()).await.map_err(map_db_error)?;
    sqlx::query("UPDATE item_costs SET average_cost = $1 WHERE sku = $2")
        .bind(new_cost)
        .bind(sku)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    if let Some(config) = Ledger::get_auto_posting_config_tx(tx).await? {
        if let Some(lines) = crate::ledger_posting::inventory_lines(&config, "revaluation", delta)?
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
            )
            .await?;
        }
    }
    Ok(())
}
