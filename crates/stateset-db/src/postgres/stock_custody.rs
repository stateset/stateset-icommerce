//! Physical custody from picking through carrier handoff.
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, MovementType, PickTask};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn pick(
    tx: &mut Transaction<'_, Postgres>,
    pick: &PickTask,
    now: DateTime<Utc>,
) -> DbResult<()> {
    if pick.quantity_picked <= Decimal::ZERO {
        return Ok(());
    }
    let order_id: Uuid = pick.order_id.into();
    let order_item_id: Uuid = pick.order_item_id.into();
    let source = pick.source_location_id;
    let lot_key = pick.lot_id.unwrap_or_else(Uuid::nil);
    let staging =
        super::stock_move::role_location(tx.as_mut(), pick.warehouse_id, "staging", now).await?;
    if staging == source {
        return Err(invalid("Already staged stock cannot be picked again"));
    }
    if pick.serial_number.is_some() && pick.quantity_picked != Decimal::ONE {
        return Err(invalid("A serialized pick must contain exactly one unit"));
    }
    let reservations = sqlx::query_as::<_, (Uuid, Decimal)>("SELECT r.id, r.quantity FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE i.sku = $1 AND r.location_id = $2 AND r.status IN ('pending', 'confirmed', 'allocated') AND r.reference_type = 'order' AND r.reference_id = $3 AND (r.order_item_id = $4 OR r.order_item_id IS NULL) AND (r.expires_at IS NULL OR r.expires_at > $5) ORDER BY r.id FOR UPDATE OF r").bind(&pick.sku).bind(pick.warehouse_id).bind(order_id.to_string()).bind(order_item_id).bind(now).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut unassigned = pick.quantity_picked;
    for (reservation, reserved) in reservations {
        if unassigned.is_zero() {
            break;
        }
        let slices = sqlx::query_scalar::<_, Decimal>(
            "SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1",
        )
        .bind(reservation)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let mapped: Decimal = slices.into_iter().sum();
        if mapped > reserved {
            return Err(invalid("Physical allocation exceeds its reservation"));
        }
        let mut local = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1 AND location_id = $2 AND lot_id = $3").bind(reservation).bind(source).bind(lot_key).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
        // Existing aggregate-only reservations gain explicit physical custody here.
        let adopt = (reserved - mapped).min((unassigned - local).max(Decimal::ZERO));
        if adopt > Decimal::ZERO {
            super::stock_move::hold(
                tx.as_mut(),
                source,
                &pick.sku,
                pick.lot_id,
                adopt,
                reservation,
                now,
            )
            .await?;
            local += adopt;
            sqlx::query("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES ($1, $2, $3, $4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity").bind(reservation).bind(source).bind(lot_key).bind(local).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
        let take = local.min(unassigned);
        if take.is_zero() {
            continue;
        }
        super::stock_move::post(
            tx,
            &StockMove {
                id: Uuid::new_v4(),
                sku: &pick.sku,
                lot_id: pick.lot_id,
                from: Some(source),
                to: Some(staging),
                quantity: take,
                release_reserved: take,
                reserve_received: take,
                movement_type: MovementType::Pick,
                lot_effect: LotEffect::Track,
                check_sellable: true,
                reference_type: Some("pick_task"),
                reference_id: Some(pick.id),
                reference_label: None,
                reason: Some("picked into staging"),
                performed_by: pick.assigned_to.as_deref(),
            },
            now,
        )
        .await?;
        sqlx::query("UPDATE inventory_reservation_stock SET quantity = $1 WHERE reservation_id = $2 AND location_id = $3 AND lot_id = $4").bind(local-take).bind(reservation).bind(source).bind(lot_key).execute(tx.as_mut()).await.map_err(map_db_error)?;
        let staged = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1 AND location_id = $2 AND lot_id = $3").bind(reservation).bind(staging).bind(lot_key).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
        sqlx::query("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES ($1, $2, $3, $4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity").bind(reservation).bind(staging).bind(lot_key).bind(staged+take).execute(tx.as_mut()).await.map_err(map_db_error)?;
        sqlx::query("INSERT INTO pick_stock_reservations (pick_id, reservation_id, quantity) VALUES ($1, $2, $3)").bind(pick.id).bind(reservation).bind(take).execute(tx.as_mut()).await.map_err(map_db_error)?;
        unassigned -= take;
    }
    if unassigned > Decimal::ZERO {
        super::stock_move::post(
            tx,
            &StockMove {
                id: Uuid::new_v4(),
                sku: &pick.sku,
                lot_id: pick.lot_id,
                from: Some(source),
                to: Some(staging),
                quantity: unassigned,
                release_reserved: Decimal::ZERO,
                reserve_received: unassigned,
                movement_type: MovementType::Pick,
                lot_effect: LotEffect::Track,
                check_sellable: true,
                reference_type: Some("pick_task"),
                reference_id: Some(pick.id),
                reference_label: None,
                reason: Some("picked into staging"),
                performed_by: pick.assigned_to.as_deref(),
            },
            now,
        )
        .await?;
    }
    sqlx::query("INSERT INTO pick_stock_custody (pick_id, staging_location_id, quantity) VALUES ($1, $2, $3)").bind(pick.id).bind(staging).bind(pick.quantity_picked).execute(tx.as_mut()).await.map_err(map_db_error)?;
    if let Some(serial) = &pick.serial_number {
        sqlx::query("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, from_location_id, to_location_id, performed_by, created_at) SELECT $1, id, 'reserved', 'pick_task', $2, status, 'reserved', current_location_id, $3, $4, $5 FROM serial_numbers WHERE serial = $6").bind(Uuid::new_v4()).bind(pick.id).bind(staging).bind(pick.assigned_to.as_deref()).bind(now).bind(serial).execute(tx.as_mut()).await.map_err(map_db_error)?;
        sqlx::query("UPDATE serial_numbers SET current_location_id = $1, status = 'reserved', updated_at = $2 WHERE serial = $3").bind(staging).bind(now).bind(serial).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    Ok(())
}
pub(super) async fn ship(
    tx: &mut Transaction<'_, Postgres>,
    ship_id: Uuid,
    order_id: Uuid,
    pack_id: Uuid,
    performed_by: Option<&str>,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let pack_order: Uuid =
        sqlx::query_scalar("SELECT order_id FROM pack_tasks WHERE id = $1 FOR UPDATE")
            .bind(pack_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    let already_shipped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ship_tasks WHERE pack_task_id = $1 AND status = 'shipped' AND id != $2)").bind(pack_id).bind(ship_id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if pack_order != order_id || already_shipped {
        return Err(invalid("Pack task belongs to another order or has already shipped"));
    }
    // Generic order fulfillment holds reservations before touching pick custody.
    // Take the same locks in the same order before locking any staged picks.
    sqlx::query("SELECT id FROM inventory_reservations WHERE reference_type = 'order' AND reference_id = $1 ORDER BY item_id, location_id, id FOR UPDATE")
        .bind(order_id.to_string()).execute(tx.as_mut()).await.map_err(map_db_error)?;
    let items = sqlx::query_as::<_, (String, Decimal, Option<Uuid>, Option<String>)>("SELECT ci.sku, ci.quantity, ci.lot_id, ci.serial_number FROM carton_items ci JOIN cartons c ON c.id = ci.carton_id WHERE c.pack_task_id = $1 ORDER BY ci.id").bind(pack_id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    for (sku, quantity, lot, serial) in items {
        if quantity <= Decimal::ZERO || (serial.is_some() && quantity != Decimal::ONE) {
            return Err(invalid("Carton quantities must be positive, with one unit per serial"));
        }
        let lot_key = lot.unwrap_or_else(Uuid::nil);
        let picks = sqlx::query_as::<_, (Uuid, i32, Decimal, Decimal)>("SELECT p.id, c.staging_location_id, c.quantity, c.quantity_shipped FROM pick_stock_custody c JOIN pick_tasks p ON p.id = c.pick_id WHERE p.order_id = $1 AND p.sku = $2 AND COALESCE(p.lot_id, '00000000-0000-0000-0000-000000000000'::uuid) = $3 AND ((p.serial_number = $4) OR (p.serial_number IS NULL AND $4 IS NULL)) ORDER BY p.id FOR UPDATE OF p, c").bind(order_id).bind(&sku).bind(lot_key).bind(&serial).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut remaining = quantity;
        for (pick, staging, picked, shipped) in picks {
            if remaining.is_zero() {
                break;
            }
            let take = remaining.min(picked - shipped);
            if take <= Decimal::ZERO {
                continue;
            }
            super::stock_guard::ensure_sellable(
                tx.as_mut(),
                &sku,
                lot,
                serial.as_deref(),
                Some(staging),
            )
            .await?;
            super::stock_move::post(
                tx,
                &StockMove {
                    id: Uuid::new_v4(),
                    sku: &sku,
                    lot_id: lot,
                    from: Some(staging),
                    to: None,
                    quantity: take,
                    release_reserved: take,
                    reserve_received: Decimal::ZERO,
                    movement_type: MovementType::Shipment,
                    lot_effect: LotEffect::Track,
                    check_sellable: true,
                    reference_type: Some("ship_task"),
                    reference_id: Some(ship_id),
                    reference_label: None,
                    reason: Some("carrier handoff"),
                    performed_by,
                },
                now,
            )
            .await?;
            let links = sqlx::query_as::<_, (Uuid, Decimal, Decimal)>("SELECT reservation_id, quantity, quantity_shipped FROM pick_stock_reservations WHERE pick_id = $1 ORDER BY reservation_id FOR UPDATE").bind(pick).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
            let mut consume = take;
            for (reservation, assigned, used) in links {
                let part = consume.min(assigned - used);
                if part <= Decimal::ZERO {
                    continue;
                }
                let (reserved, status) = sqlx::query_as::<_, (Decimal, String)>(
                    "SELECT quantity, status FROM inventory_reservations WHERE id = $1 FOR UPDATE",
                )
                .bind(reservation)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                if !matches!(status.as_str(), "pending" | "confirmed" | "allocated")
                    || reserved < part
                {
                    return Err(invalid(
                        "Picked reservation was changed; reconcile custody before shipment",
                    ));
                }
                let slice = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1 AND location_id = $2 AND lot_id = $3").bind(reservation).bind(staging).bind(lot_key).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
                if slice < part {
                    return Err(invalid("Staged reservation is insufficient"));
                }
                sqlx::query("UPDATE inventory_reservation_stock SET quantity = $1 WHERE reservation_id = $2 AND location_id = $3 AND lot_id = $4").bind(slice-part).bind(reservation).bind(staging).bind(lot_key).execute(tx.as_mut()).await.map_err(map_db_error)?;
                let next_status = if reserved == part { "fulfilled" } else { status.as_str() };
                let next_quantity = if reserved == part { reserved } else { reserved - part };
                sqlx::query(
                    "UPDATE inventory_reservations SET quantity = $1, status = $2 WHERE id = $3",
                )
                .bind(next_quantity)
                .bind(next_status)
                .bind(reservation)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                sqlx::query("UPDATE pick_stock_reservations SET quantity_shipped = $1 WHERE pick_id = $2 AND reservation_id = $3").bind(used+part).bind(pick).bind(reservation).execute(tx.as_mut()).await.map_err(map_db_error)?;
                consume -= part;
            }
            sqlx::query("UPDATE pick_stock_custody SET quantity_shipped = $1 WHERE pick_id = $2")
                .bind(shipped + take)
                .bind(pick)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            remaining -= take;
        }
        if !remaining.is_zero() {
            return Err(invalid("Shipment exceeds matching picked stock in staging"));
        }
        if let Some(serial) = serial {
            sqlx::query("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, from_location_id, performed_by, created_at) SELECT $1, id, 'shipped', 'ship_task', $2, status, 'shipped', current_location_id, $3, $4 FROM serial_numbers WHERE serial = $5").bind(Uuid::new_v4()).bind(ship_id).bind(performed_by).bind(now).bind(&serial).execute(tx.as_mut()).await.map_err(map_db_error)?;
            sqlx::query("UPDATE serial_numbers SET status = 'shipped', current_location_id = NULL, sold_at = COALESCE(sold_at, $1), updated_at = $1 WHERE serial = $2").bind(now).bind(&serial).execute(tx.as_mut()).await.map_err(map_db_error)?;
            sqlx::query("UPDATE serial_reservations SET released_at = $1, active_key = NULL WHERE serial_id = (SELECT id FROM serial_numbers WHERE serial = $2) AND released_at IS NULL").bind(now).bind(&serial).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
    }
    Ok(())
}
