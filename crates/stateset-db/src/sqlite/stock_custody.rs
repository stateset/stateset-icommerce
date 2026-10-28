//! Physical custody from picking through carrier handoff.
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{OptionalExtension, Transaction};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, MovementType, PickTask};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn pick(tx: &Transaction<'_>, pick: &PickTask, now: &str) -> DbResult<()> {
    if pick.quantity_picked <= Decimal::ZERO {
        return Ok(());
    }
    let order_id: Uuid = pick.order_id.into();
    let order_item_id: Uuid = pick.order_item_id.into();
    let source = pick.source_location_id;
    let lot_key = pick.lot_id.map(|id| id.to_string()).unwrap_or_default();
    let staging = super::stock_move::role_location(tx, pick.warehouse_id, "staging", now)?;
    if staging == source {
        return Err(invalid("Already staged stock cannot be picked again"));
    }
    if pick.serial_number.is_some() && pick.quantity_picked != Decimal::ONE {
        return Err(invalid("A serialized pick must contain exactly one unit"));
    }
    let reservations = {
        let mut stmt = tx.prepare("SELECT r.id, r.quantity FROM inventory_reservations r JOIN inventory_items i ON i.id = r.item_id WHERE i.sku = ?1 AND r.location_id = ?2 AND r.status IN ('pending', 'confirmed', 'allocated') AND r.reference_type = 'order' AND r.reference_id = ?3 AND (r.order_item_id = ?4 OR r.order_item_id IS NULL) AND (r.expires_at IS NULL OR r.expires_at > ?5) ORDER BY r.id")?;
        let rows = stmt.query_map(
            rusqlite::params![
                &pick.sku,
                pick.warehouse_id,
                order_id.to_string(),
                (order_item_id).to_string(),
                now
            ],
            |r| {
                Ok((
                    super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                ))
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut unassigned = pick.quantity_picked;
    for (reservation, reserved) in reservations {
        if unassigned.is_zero() {
            break;
        }
        let slices = {
            let mut stmt = tx.prepare(
                "SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1",
            )?;
            let rows = stmt.query_map(rusqlite::params![(reservation).to_string()], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mapped: Decimal = slices.into_iter().sum();
        if mapped > reserved {
            return Err(invalid("Physical allocation exceeds its reservation"));
        }
        let mut local = tx.query_row("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 AND location_id = ?2 AND lot_id = ?3", rusqlite::params![(reservation).to_string(), source, &lot_key], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
        // Existing aggregate-only reservations gain explicit physical custody here.
        let adopt = (reserved - mapped).min((unassigned - local).max(Decimal::ZERO));
        if adopt > Decimal::ZERO {
            super::stock_move::hold(tx, source, &pick.sku, pick.lot_id, adopt, reservation, now)?;
            local += adopt;
            tx.execute("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity", rusqlite::params![(reservation).to_string(), source, &lot_key, (local).to_string()])?;
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
        )?;
        tx.execute("UPDATE inventory_reservation_stock SET quantity = ?1 WHERE reservation_id = ?2 AND location_id = ?3 AND lot_id = ?4", rusqlite::params![(local-take).to_string(), (reservation).to_string(), source, &lot_key])?;
        let staged = tx.query_row("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 AND location_id = ?2 AND lot_id = ?3", rusqlite::params![(reservation).to_string(), staging, &lot_key], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
        tx.execute("INSERT INTO inventory_reservation_stock (reservation_id, location_id, lot_id, quantity) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(reservation_id, location_id, lot_id) DO UPDATE SET quantity = excluded.quantity", rusqlite::params![(reservation).to_string(), staging, &lot_key, (staged+take).to_string()])?;
        tx.execute("INSERT INTO pick_stock_reservations (pick_id, reservation_id, quantity) VALUES (?1, ?2, ?3)", rusqlite::params![(pick.id).to_string(), (reservation).to_string(), (take).to_string()])?;
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
        )?;
    }
    tx.execute("INSERT INTO pick_stock_custody (pick_id, staging_location_id, quantity) VALUES (?1, ?2, ?3)", rusqlite::params![(pick.id).to_string(), staging, (pick.quantity_picked).to_string()])?;
    if let Some(serial) = &pick.serial_number {
        tx.execute("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, from_location_id, to_location_id, performed_by, created_at) SELECT ?1, id, 'reserved', 'pick_task', ?2, status, 'reserved', current_location_id, ?3, ?4, ?5 FROM serial_numbers WHERE serial = ?6", rusqlite::params![(Uuid::new_v4()).to_string(), (pick.id).to_string(), staging, pick.assigned_to.as_deref(), now, serial])?;
        tx.execute("UPDATE serial_numbers SET current_location_id = ?1, status = 'reserved', updated_at = ?2 WHERE serial = ?3", rusqlite::params![staging, now, serial])?;
    }
    Ok(())
}
pub(super) fn ship(
    tx: &Transaction<'_>,
    ship_id: Uuid,
    order_id: Uuid,
    pack_id: Uuid,
    performed_by: Option<&str>,
    now: &str,
) -> DbResult<()> {
    let pack_order = tx.query_row(
        "SELECT order_id FROM pack_tasks WHERE id = ?1",
        [pack_id.to_string()],
        |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "pack_task", "order_id"),
    )?;
    let already_shipped: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM ship_tasks WHERE pack_task_id = ?1 AND status = 'shipped' AND id != ?2)", rusqlite::params![pack_id.to_string(), ship_id.to_string()], |r| r.get(0))?;
    if pack_order != order_id || already_shipped {
        return Err(invalid("Pack task belongs to another order or has already shipped"));
    }
    let items = {
        let mut stmt = tx.prepare("SELECT ci.sku, ci.quantity, ci.lot_id, ci.serial_number FROM carton_items ci JOIN cartons c ON c.id = ci.carton_id WHERE c.pack_task_id = ?1 ORDER BY ci.id")?;
        let rows = stmt.query_map(rusqlite::params![(pack_id).to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_uuid_opt_row(r.get(2)?, "stock", "id")?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (sku, quantity, lot, serial) in items {
        if quantity <= Decimal::ZERO || (serial.is_some() && quantity != Decimal::ONE) {
            return Err(invalid("Carton quantities must be positive, with one unit per serial"));
        }
        let lot_key = lot.map(|id| id.to_string()).unwrap_or_default();
        let picks = {
            let mut stmt = tx.prepare("SELECT p.id, c.staging_location_id, c.quantity, c.quantity_shipped FROM pick_stock_custody c JOIN pick_tasks p ON p.id = c.pick_id WHERE p.order_id = ?1 AND p.sku = ?2 AND COALESCE(p.lot_id, '') = ?3 AND ((p.serial_number = ?4) OR (p.serial_number IS NULL AND ?4 IS NULL)) ORDER BY p.id")?;
            let rows = stmt.query_map(
                rusqlite::params![(order_id).to_string(), &sku, &lot_key, &serial],
                |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        r.get::<_, i32>(1)?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                    ))
                },
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut remaining = quantity;
        for (pick, staging, picked, shipped) in picks {
            if remaining.is_zero() {
                break;
            }
            let take = remaining.min(picked - shipped);
            if take <= Decimal::ZERO {
                continue;
            }
            super::stock_guard::ensure_sellable(tx, &sku, lot, serial.as_deref(), Some(staging))
                .map_err(super::bins::smuggle)?;
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
            )?;
            let links = {
                let mut stmt = tx.prepare("SELECT reservation_id, quantity, quantity_shipped FROM pick_stock_reservations WHERE pick_id = ?1 ORDER BY reservation_id")?;
                let rows = stmt.query_map(rusqlite::params![(pick).to_string()], |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let mut consume = take;
            for (reservation, assigned, used) in links {
                let part = consume.min(assigned - used);
                if part <= Decimal::ZERO {
                    continue;
                }
                let (reserved, status) = tx.query_row(
                    "SELECT quantity, status FROM inventory_reservations WHERE id = ?1",
                    rusqlite::params![(reservation).to_string()],
                    |r| {
                        Ok((
                            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                            r.get::<_, String>(1)?,
                        ))
                    },
                )?;
                if !matches!(status.as_str(), "pending" | "confirmed" | "allocated")
                    || reserved < part
                {
                    return Err(invalid(
                        "Picked reservation was changed; reconcile custody before shipment",
                    ));
                }
                let slice = tx.query_row("SELECT quantity FROM inventory_reservation_stock WHERE reservation_id = ?1 AND location_id = ?2 AND lot_id = ?3", rusqlite::params![(reservation).to_string(), staging, &lot_key], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity"))?;
                if slice < part {
                    return Err(invalid("Staged reservation is insufficient"));
                }
                tx.execute("UPDATE inventory_reservation_stock SET quantity = ?1 WHERE reservation_id = ?2 AND location_id = ?3 AND lot_id = ?4", rusqlite::params![(slice-part).to_string(), (reservation).to_string(), staging, &lot_key])?;
                let next_status = if reserved == part { "fulfilled" } else { status.as_str() };
                let next_quantity = if reserved == part { reserved } else { reserved - part };
                tx.execute(
                    "UPDATE inventory_reservations SET quantity = ?1, status = ?2 WHERE id = ?3",
                    rusqlite::params![
                        (next_quantity).to_string(),
                        next_status,
                        (reservation).to_string()
                    ],
                )?;
                tx.execute("UPDATE pick_stock_reservations SET quantity_shipped = ?1 WHERE pick_id = ?2 AND reservation_id = ?3", rusqlite::params![(used+part).to_string(), (pick).to_string(), (reservation).to_string()])?;
                consume -= part;
            }
            tx.execute(
                "UPDATE pick_stock_custody SET quantity_shipped = ?1 WHERE pick_id = ?2",
                rusqlite::params![(shipped + take).to_string(), (pick).to_string()],
            )?;
            remaining -= take;
        }
        if !remaining.is_zero() {
            return Err(invalid("Shipment exceeds matching picked stock in staging"));
        }
        if let Some(serial) = serial {
            tx.execute("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, from_location_id, performed_by, created_at) SELECT ?1, id, 'shipped', 'ship_task', ?2, status, 'shipped', current_location_id, ?3, ?4 FROM serial_numbers WHERE serial = ?5", rusqlite::params![(Uuid::new_v4()).to_string(), (ship_id).to_string(), performed_by, now, &serial])?;
            tx.execute("UPDATE serial_numbers SET status = 'shipped', current_location_id = NULL, sold_at = COALESCE(sold_at, ?1), updated_at = ?1 WHERE serial = ?2", rusqlite::params![now, &serial])?;
            tx.execute("UPDATE serial_reservations SET released_at = ?1, active_key = NULL WHERE serial_id = (SELECT id FROM serial_numbers WHERE serial = ?2) AND released_at IS NULL", rusqlite::params![now, serial])?;
        }
    }
    Ok(())
}
