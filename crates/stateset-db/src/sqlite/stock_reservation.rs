//! Physical allocations owned by inventory reservations.
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{OptionalExtension, Transaction};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, MovementType};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn allocate(
    tx: &Transaction<'_>,
    reservation: Uuid,
    sku: &str,
    warehouse: i32,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    if quantity <= Decimal::ZERO {
        return Ok(());
    }
    super::stock_placement::materialize(tx, warehouse, sku, now)?;
    let (reference_type, reference_id) = tx.query_row(
        "SELECT reference_type, reference_id FROM inventory_reservations WHERE id = ?1",
        rusqlite::params![(reservation).to_string()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    let preferred = if reference_type == "work_order" {
        let id = Uuid::parse_str(&reference_id)
            .map_err(|_| invalid("Invalid work order reservation reference"))?;
        Some(tx.query_row(
            "SELECT source_location_id FROM manufacturing_work_orders WHERE id = ?1",
            rusqlite::params![(id).to_string()],
            |r| r.get::<_, i32>(0),
        )?)
    } else {
        None
    };
    let rows = {
        let mut stmt = tx.prepare("SELECT li.location_id, li.lot_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = ?1 AND li.sku = ?2 AND l.is_active = 1 AND l.is_pickable = 1 ORDER BY li.location_id, li.lot_id")?;
        let rows = stmt.query_map(rusqlite::params![warehouse, sku], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = quantity;
    for (location, key, on_hand, reserved) in rows {
        if remaining.is_zero() {
            break;
        }
        let lot = if key.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&key).map_err(|_| invalid("Invalid stock lot"))?)
        };
        if preferred.is_some_and(|value| value != location)
            || (preferred.is_some() && lot.is_some())
        {
            continue;
        }
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        match super::stock_guard::ensure_sellable(tx, sku, lot, None, Some(location)) {
            Ok(()) => {}
            Err(CommerceError::Conflict(_)) | Err(CommerceError::NotPermitted(_)) => continue,
            Err(error) => return Err(super::bins::smuggle(error)),
        }
        super::stock_move::hold(tx, location, sku, lot, take, reservation, now)?;
        let old = tx.query_row("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 AND location_id = ?2 AND lot_id = ?3", rusqlite::params![(reservation).to_string(), location, &key], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
        tx.execute("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity", rusqlite::params![(reservation).to_string(), location, &key, (old+take).to_string()])?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient eligible physical stock for reservation"));
    }
    Ok(())
}
pub(super) fn release(tx: &Transaction<'_>, reservation: Uuid, now: &str) -> DbResult<()> {
    let sku = tx.query_row("SELECT i.sku FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE r.id = ?1", rusqlite::params![(reservation).to_string()], |r| r.get::<_, String>(0))?;
    let rows = {
        let mut stmt = tx.prepare("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 ORDER BY location_id, lot_id")?;
        let rows = stmt.query_map(rusqlite::params![(reservation).to_string()], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (location, key, quantity) in rows {
        if quantity.is_zero() {
            continue;
        }
        let lot = if key.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&key).map_err(|_| invalid("Invalid stock lot"))?)
        };
        super::stock_move::hold(tx, location, &sku, lot, -quantity, reservation, now)?;
        tx.execute("UPDATE inventory_reservation_stock SET quantity = '0' WHERE reservation_id = ?1 AND location_id = ?2 AND lot_id = ?3", rusqlite::params![(reservation).to_string(), location, &key])?;
    }
    Ok(())
}
pub(super) fn consume(
    tx: &Transaction<'_>,
    reservation: Uuid,
    quantity: Decimal,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    let (sku, warehouse, reserved) = tx.query_row("SELECT i.sku, r.location_id, r.quantity FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE r.id = ?1", rusqlite::params![(reservation).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?)))?;
    let mapped: Decimal = {
        let mut stmt = tx.prepare(
            "SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![(reservation).to_string()], |r| {
            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    }
    .into_iter()
    .sum();
    if mapped > reserved {
        return Err(invalid("Physical allocation exceeds its reservation"));
    }
    if mapped < reserved {
        allocate(tx, reservation, &sku, warehouse, reserved - mapped, now)?;
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 ORDER BY location_id, lot_id")?;
        let rows = stmt.query_map(rusqlite::params![(reservation).to_string()], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = quantity;
    for (location, key, allocated) in rows {
        let take = remaining.min(allocated);
        if take <= Decimal::ZERO {
            continue;
        }
        let lot = if key.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&key).map_err(|_| invalid("Invalid stock lot"))?)
        };
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
        )?;
        tx.execute("UPDATE inventory_reservation_stock SET quantity = ?1 WHERE reservation_id = ?2 AND location_id = ?3 AND lot_id = ?4", rusqlite::params![(allocated-take).to_string(), (reservation).to_string(), location, &key])?;
        let links = {
            let mut stmt = tx.prepare("SELECT r.pick_id, r.quantity, r.quantity_shipped, c.quantity_shipped FROM pick_stock_reservations r JOIN pick_stock_custody c ON c.pick_id = r.pick_id WHERE r.reservation_id = ?1 AND c.staging_location_id = ?2 ORDER BY r.pick_id")?;
            let rows =
                stmt.query_map(rusqlite::params![(reservation).to_string(), location], |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                    ))
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut linked = take;
        for (pick, assigned, shipped, custody_shipped) in links {
            let part = linked.min(assigned - shipped);
            if part <= Decimal::ZERO {
                continue;
            }
            tx.execute("UPDATE pick_stock_reservations SET quantity_shipped = ?1 WHERE pick_id = ?2 AND reservation_id = ?3", rusqlite::params![(shipped+part).to_string(), (pick).to_string(), (reservation).to_string()])?;
            tx.execute(
                "UPDATE pick_stock_custody SET quantity_shipped = ?1 WHERE pick_id = ?2",
                rusqlite::params![(custody_shipped + part).to_string(), (pick).to_string()],
            )?;
            linked -= part;
        }
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Reservation has insufficient physical stock"));
    }
    Ok(())
}

pub(super) fn split(
    tx: &Transaction<'_>,
    reservation: Uuid,
    confirmed: Uuid,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    let _ = now;
    let rows = {
        let mut stmt = tx.prepare("SELECT location_id, lot_id, quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 ORDER BY location_id, lot_id")?;
        let rows = stmt.query_map(rusqlite::params![(reservation).to_string()], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = quantity;
    for (location, key, allocated) in rows {
        let take = allocated.min(remaining);
        if take <= Decimal::ZERO {
            continue;
        }
        tx.execute("UPDATE inventory_reservation_stock SET quantity = ?1 WHERE reservation_id = ?2 AND location_id = ?3 AND lot_id = ?4", rusqlite::params![(allocated-take).to_string(), (reservation).to_string(), location, &key])?;
        tx.execute("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES (?1, ?2, ?3, ?4)", rusqlite::params![(confirmed).to_string(), location, &key, (take).to_string()])?;
        let links = {
            let mut stmt = tx.prepare("SELECT r.pick_id, r.quantity, r.quantity_shipped FROM pick_stock_reservations r JOIN pick_stock_custody c ON c.pick_id = r.pick_id WHERE r.reservation_id = ?1 AND c.staging_location_id = ?2 ORDER BY r.pick_id")?;
            let rows =
                stmt.query_map(rusqlite::params![(reservation).to_string(), location], |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                    ))
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut linked = take;
        for (pick, assigned, shipped) in links {
            let part = linked.min(assigned - shipped);
            if part <= Decimal::ZERO {
                continue;
            }
            tx.execute("UPDATE pick_stock_reservations SET quantity = ?1 WHERE pick_id = ?2 AND reservation_id = ?3", rusqlite::params![(assigned-part).to_string(), (pick).to_string(), (reservation).to_string()])?;
            tx.execute("INSERT INTO pick_stock_reservations (pick_id, reservation_id, quantity) VALUES (?1, ?2, ?3)", rusqlite::params![(pick).to_string(), (confirmed).to_string(), (part).to_string()])?;
            linked -= part;
        }
        remaining -= take;
    }
    Ok(())
}
