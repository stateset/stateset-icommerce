//! Lot stock postings and ownership of physical reservations/quarantine.
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{Connection, OptionalExtension};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, Lot, MovementType};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn ensure_unheld(tx: &Connection, lot: &Lot, location: i32, now: &str) -> DbResult<()> {
    let held = tx.query_row("SELECT COUNT(*) FROM quality_holds WHERE sku = ?1 AND released_at IS NULL AND (expires_at IS NULL OR julianday(expires_at) > julianday(?4)) AND (lot_number IS NULL OR lot_number = ?2) AND (location_id IS NULL OR location_id = ?3)", rusqlite::params![&lot.sku, &lot.lot_number, location, now], |r| r.get::<_, i32>(0))?;
    if held > 0 {
        return Err(invalid("Lot stock is blocked by an active quality hold"));
    }
    Ok(())
}
pub(super) fn slices(
    tx: &Connection,
    lot: &Lot,
    warehouse: Option<i32>,
    now: &str,
) -> DbResult<Vec<(i32, Decimal, Decimal)>> {
    let _ = now;
    Ok({
        let mut stmt = tx.prepare("SELECT li.location_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE li.lot_id = ?1 AND li.sku = ?2 AND (?3 IS NULL OR l.warehouse_id = ?3) ORDER BY l.warehouse_id, li.location_id")?;
        let rows =
            stmt.query_map(rusqlite::params![(lot.id).to_string(), &lot.sku, warehouse], |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                    super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                ))
            })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    })
}
fn movement<'a>(lot: &'a Lot, quantity: Decimal, reason: &'a str) -> StockMove<'a> {
    StockMove {
        id: Uuid::new_v4(),
        sku: &lot.sku,
        lot_id: Some(lot.id),
        from: None,
        to: None,
        quantity,
        release_reserved: Decimal::ZERO,
        reserve_received: Decimal::ZERO,
        movement_type: MovementType::Adjustment,
        lot_effect: LotEffect::MetadataRecorded,
        check_sellable: false,
        reference_type: Some("lot"),
        reference_id: Some(lot.id),
        reference_label: None,
        reason: Some(reason),
        performed_by: None,
    }
}
pub(super) fn change(
    tx: &Connection,
    lot: &Lot,
    warehouse: Option<i32>,
    delta: Decimal,
    placement_recorded: bool,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    if delta.is_zero() {
        return Ok(());
    }
    let mut posting = movement(lot, delta.abs(), reason);
    if placement_recorded {
        posting.lot_effect = LotEffect::AlreadyRecorded;
        posting.movement_type = MovementType::Receipt;
    }
    if delta > Decimal::ZERO {
        // An unplaced lot can be registered ahead of receiving. Registration
        // alone is metadata; adjustments must identify a stock warehouse.
        if warehouse.is_none() && placement_recorded {
            return Ok(());
        }
        let warehouse = match warehouse {
            Some(warehouse) => warehouse,
            None => {
                let placements = {
                    let mut statement = tx.prepare("SELECT location_id FROM lot_locations WHERE lot_id = ?1 ORDER BY location_id")?;
                    let rows =
                        statement.query_map([lot.id.to_string()], |row| row.get::<_, i32>(0))?;
                    rows.collect::<rusqlite::Result<Vec<_>>>()?
                };
                if placements.len() != 1 {
                    return Err(invalid(
                        "A positive lot adjustment requires an unambiguous warehouse location",
                    ));
                }
                placements[0]
            }
        };
        super::stock_placement::ensure_warehouse(tx, warehouse, now)?;
        posting.to = Some(super::stock_move::role_location(tx, warehouse, "unassigned", now)?);
        super::stock_move::post(tx, &posting, now)?;
        return Ok(());
    }
    let mut remaining = -delta;
    for (location, on_hand, reserved) in slices(tx, lot, warehouse, now)? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now)?;
        posting.id = Uuid::new_v4();
        posting.from = Some(location);
        posting.quantity = take;
        super::stock_move::post(tx, &posting, now)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid(
            "Insufficient unreserved physical lot stock; receive or reconcile the lot first",
        ));
    }
    Ok(())
}
pub(super) fn record_hold(
    tx: &Connection,
    lot: Uuid,
    owner: Uuid,
    location: i32,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    let _ = now;
    let existing = tx.query_row("SELECT quantity FROM lot_stock_holds WHERE owner_id = ?1 AND lot_id = ?2 AND location_id = ?3", rusqlite::params![(owner).to_string(), (lot).to_string(), location], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
    let total =
        existing.checked_add(quantity).ok_or_else(|| invalid("Lot hold quantity overflow"))?;
    if total < Decimal::ZERO {
        return Err(invalid("Lot hold ownership is inconsistent"));
    }
    tx.execute("INSERT INTO lot_stock_holds (owner_id, lot_id, location_id, quantity) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(owner_id, lot_id, location_id) DO UPDATE SET quantity = excluded.quantity", rusqlite::params![(owner).to_string(), (lot).to_string(), location, (total).to_string()])?;
    Ok(())
}
pub(super) fn allocate(
    tx: &Connection,
    lot: &Lot,
    owner: Uuid,
    quantity: Decimal,
    check_quality: bool,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    let mut remaining = quantity;
    let stock = slices(tx, lot, None, now)?;
    if !check_quality && stock.is_empty() && unplaced(tx, lot.id)? {
        // A lot registered ahead of receipt can be blocked before it arrives.
        return Ok(());
    }
    for (location, on_hand, reserved) in stock {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        if check_quality {
            ensure_unheld(tx, lot, location, now)?;
        }
        super::stock_move::allocate_recorded_lot(
            tx, location, &lot.sku, lot.id, take, reason, now,
        )?;
        record_hold(tx, lot.id, owner, location, take, now)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid(
            "Insufficient physical lot stock for allocation; receive or reconcile the lot first",
        ));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_hold(
    tx: &Connection,
    lot: &Lot,
    owner: Uuid,
    quantity: Decimal,
    consume: bool,
    quarantine: bool,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    let rows = {
        let mut stmt = tx.prepare("SELECT location_id, quantity FROM lot_stock_holds WHERE owner_id = ?1 AND lot_id = ?2 ORDER BY location_id")?;
        let rows =
            stmt.query_map(rusqlite::params![(owner).to_string(), (lot.id).to_string()], |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                ))
            })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let owned: Decimal = rows.iter().map(|row| row.1).sum();
    if owner == lot.id && !consume && rows.is_empty() && unplaced(tx, lot.id)? {
        return Ok(());
    }
    if owned != quantity {
        return Err(invalid(
            "Lot reservation does not match its physical custody; reconcile before posting",
        ));
    }
    for (location, take) in rows {
        if take <= Decimal::ZERO {
            continue;
        }
        if consume {
            ensure_unheld(tx, lot, location, now)?;
            let mut posting = movement(lot, take, reason);
            posting.from = Some(location);
            posting.release_reserved = take;
            posting.movement_type = MovementType::Shipment;
            super::stock_move::post(tx, &posting, now)?;
        } else if quarantine {
            record_hold(tx, lot.id, lot.id, location, take, now)?;
        } else {
            super::stock_move::allocate_recorded_lot(
                tx, location, &lot.sku, lot.id, -take, reason, now,
            )?;
        }
        record_hold(tx, lot.id, owner, location, -take, now)?;
    }
    Ok(())
}

fn unplaced(tx: &Connection, lot: Uuid) -> DbResult<bool> {
    tx.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM lot_locations WHERE lot_id = ?1)",
        [lot.to_string()],
        |row| row.get(0),
    )
}
pub(super) fn transfer(
    tx: &Connection,
    lot: &Lot,
    from_warehouse: i32,
    to_warehouse: i32,
    quantity: Decimal,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    if from_warehouse == to_warehouse {
        return Err(invalid("Lot transfer requires distinct warehouses"));
    }
    super::stock_placement::ensure_warehouse(tx, to_warehouse, now)?;
    let destination = super::stock_move::role_location(tx, to_warehouse, "unassigned", now)?;
    let mut remaining = quantity;
    for (location, on_hand, reserved) in slices(tx, lot, Some(from_warehouse), now)? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now)?;
        let mut posting = movement(lot, take, reason);
        posting.from = Some(location);
        posting.to = Some(destination);
        posting.movement_type = MovementType::Transfer;
        super::stock_move::post(tx, &posting, now)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient quantity at source location for lot transfer"));
    }
    Ok(())
}
pub(super) fn relabel(
    tx: &Connection,
    lot: &Lot,
    target: &Lot,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    super::stock_cost::relabel(tx, lot.id, target.id, quantity, now)?;
    let mut remaining = quantity;
    for (location, on_hand, reserved) in slices(tx, lot, None, now)? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now)?;
        let mut outgoing = movement(lot, take, "Lot genealogy: source");
        outgoing.reference_type = Some("lot_relabel");
        outgoing.from = Some(location);
        super::stock_move::post(tx, &outgoing, now)?;
        let mut incoming = movement(target, take, "Lot genealogy: destination");
        incoming.reference_type = Some("lot_relabel");
        incoming.to = Some(location);
        super::stock_move::post(tx, &incoming, now)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient unreserved physical stock for lot split or merge"));
    }
    Ok(())
}
