//! Physical allocations owned by inventory reservations.
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, MovementType};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn allocate(
    tx: &mut Transaction<'_, Postgres>,
    reservation: Uuid,
    sku: &str,
    warehouse: i32,
    quantity: Decimal,
    now: DateTime<Utc>,
) -> DbResult<()> {
    if quantity <= Decimal::ZERO {
        return Ok(());
    }
    super::stock_placement::materialize(tx, warehouse, sku, now).await?;
    let (reference_type, reference_id) = sqlx::query_as::<_, (String, String)>(
        "SELECT reference_type, reference_id FROM inventory_reservations WHERE id = $1",
    )
    .bind(reservation)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let preferred = if reference_type == "work_order" {
        let id = Uuid::parse_str(&reference_id)
            .map_err(|_| invalid("Invalid work order reservation reference"))?;
        Some(
            sqlx::query_scalar::<_, i32>(
                "SELECT source_location_id FROM manufacturing_work_orders WHERE id = $1",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?,
        )
    } else {
        None
    };
    let rows = sqlx::query_as::<_, (i32, Uuid, Decimal, Decimal)>("SELECT li.location_id, li.lot_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = $1 AND li.sku = $2 AND l.is_active = TRUE AND l.is_pickable = TRUE ORDER BY li.location_id, li.lot_id").bind(warehouse).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut remaining = quantity;
    for (location, key, on_hand, reserved) in rows {
        if remaining.is_zero() {
            break;
        }
        let lot = (!key.is_nil()).then_some(key);
        if preferred.is_some_and(|value| value != location)
            || (preferred.is_some() && lot.is_some())
        {
            continue;
        }
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        match super::stock_guard::ensure_sellable(tx.as_mut(), sku, lot, None, Some(location)).await
        {
            Ok(()) => {}
            Err(CommerceError::Conflict(_)) | Err(CommerceError::NotPermitted(_)) => continue,
            Err(error) => return Err(error),
        }
        super::stock_move::hold(tx.as_mut(), location, sku, lot, take, reservation, now).await?;
        let old = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1 AND location_id = $2 AND lot_id = $3").bind(reservation).bind(location).bind(key).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
        sqlx::query("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES ($1, $2, $3, $4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity").bind(reservation).bind(location).bind(key).bind(old+take).execute(tx.as_mut()).await.map_err(map_db_error)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient eligible physical stock for reservation"));
    }
    Ok(())
}
pub(super) async fn release(
    tx: &mut Transaction<'_, Postgres>,
    reservation: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let sku = sqlx::query_scalar::<_, String>("SELECT i.sku FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE r.id = $1").bind(reservation).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    let rows = sqlx::query_as::<_, (i32, Uuid, Decimal)>("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = $1 ORDER BY location_id, lot_id").bind(reservation).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    for (location, key, quantity) in rows {
        if quantity.is_zero() {
            continue;
        }
        let lot = (!key.is_nil()).then_some(key);
        super::stock_move::hold(tx.as_mut(), location, &sku, lot, -quantity, reservation, now)
            .await?;
        sqlx::query("UPDATE inventory_reservation_stock SET quantity = '0' WHERE reservation_id = $1 AND location_id = $2 AND lot_id = $3").bind(reservation).bind(location).bind(key).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    Ok(())
}
pub(super) async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    reservation: Uuid,
    quantity: Decimal,
    reason: &str,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let (sku, warehouse, reserved) = sqlx::query_as::<_, (String, i32, Decimal)>("SELECT i.sku, r.location_id, r.quantity FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE r.id = $1").bind(reservation).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    let mapped: Decimal = sqlx::query_scalar::<_, Decimal>(
        "SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = $1",
    )
    .bind(reservation)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .into_iter()
    .sum();
    if mapped > reserved {
        return Err(invalid("Physical allocation exceeds its reservation"));
    }
    if mapped < reserved {
        allocate(tx, reservation, &sku, warehouse, reserved - mapped, now).await?;
    }
    let rows = sqlx::query_as::<_, (i32, Uuid, Decimal)>("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = $1 ORDER BY location_id, lot_id").bind(reservation).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut remaining = quantity;
    for (location, key, allocated) in rows {
        let take = remaining.min(allocated);
        if take <= Decimal::ZERO {
            continue;
        }
        let lot = (!key.is_nil()).then_some(key);
        super::stock_move::post_physical(
            tx,
            &StockMove {
                id: Uuid::new_v4(),
                sku: &sku,
                lot_id: lot,
                from: Some(location),
                to: None,
                quantity: take,
                release_reserved: take,
                reserve_received: Decimal::ZERO,
                movement_type: MovementType::Shipment,
                lot_effect: LotEffect::Track,
                check_sellable: true,
                reference_type: Some("inventory_reservation"),
                reference_id: Some(reservation),
                reference_label: None,
                reason: Some(reason),
                performed_by: None,
            },
            now,
        )
        .await?;
        sqlx::query("UPDATE inventory_reservation_stock SET quantity = $1 WHERE reservation_id = $2 AND location_id = $3 AND lot_id = $4").bind(allocated-take).bind(reservation).bind(location).bind(key).execute(tx.as_mut()).await.map_err(map_db_error)?;
        let links = sqlx::query_as::<_, (Uuid, Decimal, Decimal, Decimal)>("SELECT r.pick_id, r.quantity, r.quantity_shipped, c.quantity_shipped FROM pick_stock_reservations r JOIN pick_stock_custody c ON c.pick_id = r.pick_id WHERE r.reservation_id = $1 AND c.staging_location_id = $2 ORDER BY r.pick_id").bind(reservation).bind(location).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut linked = take;
        for (pick, assigned, shipped, custody_shipped) in links {
            let part = linked.min(assigned - shipped);
            if part <= Decimal::ZERO {
                continue;
            }
            sqlx::query("UPDATE pick_stock_reservations SET quantity_shipped = $1 WHERE pick_id = $2 AND reservation_id = $3").bind(shipped+part).bind(pick).bind(reservation).execute(tx.as_mut()).await.map_err(map_db_error)?;
            sqlx::query("UPDATE pick_stock_custody SET quantity_shipped = $1 WHERE pick_id = $2")
                .bind(custody_shipped + part)
                .bind(pick)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            linked -= part;
        }
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Reservation has insufficient physical stock"));
    }
    Ok(())
}

pub(super) async fn split(
    tx: &mut Transaction<'_, Postgres>,
    reservation: Uuid,
    confirmed: Uuid,
    quantity: Decimal,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let rows = sqlx::query_as::<_, (i32, Uuid, Decimal)>("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = $1 ORDER BY location_id, lot_id").bind(reservation).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut remaining = quantity;
    for (location, key, allocated) in rows {
        let take = allocated.min(remaining);
        if take <= Decimal::ZERO {
            continue;
        }
        sqlx::query("UPDATE inventory_reservation_stock SET quantity = $1 WHERE reservation_id = $2 AND location_id = $3 AND lot_id = $4").bind(allocated-take).bind(reservation).bind(location).bind(key).execute(tx.as_mut()).await.map_err(map_db_error)?;
        sqlx::query("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES ($1, $2, $3, $4)").bind(confirmed).bind(location).bind(key).bind(take).execute(tx.as_mut()).await.map_err(map_db_error)?;
        let links = sqlx::query_as::<_, (Uuid, Decimal, Decimal)>("SELECT r.pick_id, r.quantity, r.quantity_shipped FROM pick_stock_reservations r JOIN pick_stock_custody c ON c.pick_id = r.pick_id WHERE r.reservation_id = $1 AND c.staging_location_id = $2 ORDER BY r.pick_id").bind(reservation).bind(location).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut linked = take;
        for (pick, assigned, shipped) in links {
            let part = linked.min(assigned - shipped);
            if part <= Decimal::ZERO {
                continue;
            }
            sqlx::query("UPDATE pick_stock_reservations SET quantity = $1 WHERE pick_id = $2 AND reservation_id = $3").bind(assigned-part).bind(pick).bind(reservation).execute(tx.as_mut()).await.map_err(map_db_error)?;
            sqlx::query("INSERT INTO pick_stock_reservations (pick_id, reservation_id, quantity) VALUES ($1, $2, $3)").bind(pick).bind(confirmed).bind(part).execute(tx.as_mut()).await.map_err(map_db_error)?;
            linked -= part;
        }
        remaining -= take;
    }
    Ok(())
}
