//! One transactional posting path for shelf, bin, warehouse and lot stock.
use super::{bins::smuggle, parse_decimal_row, parse_decimal_strict};
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, WarehouseBin};
use std::collections::BTreeMap;
use uuid::Uuid;

type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: impl Into<String>) -> rusqlite::Error {
    smuggle(CommerceError::ValidationError(message.into()))
}
fn add(a: Decimal, b: Decimal) -> DbResult<Decimal> {
    a.checked_add(b).ok_or_else(|| invalid("Stock quantity overflow"))
}
fn decimal(value: &str) -> DbResult<Decimal> {
    parse_decimal_strict(value, "stock", "quantity").map_err(smuggle)
}

pub(super) fn warehouse_of(conn: &Connection, location: i32) -> DbResult<i32> {
    let active = conn.query_row("SELECT l.warehouse_id FROM locations l JOIN warehouses w ON w.id = l.warehouse_id WHERE l.id = ?1 AND l.is_active = 1 AND w.is_active = 1", [location], |r| r.get(0)).optional()?;
    if let Some(warehouse) = active {
        return Ok(warehouse);
    }
    // A deleted location is missing, not inactive.
    let exists: bool =
        conn.query_row("SELECT EXISTS (SELECT 1 FROM locations WHERE id = ?1)", [location], |r| {
            r.get(0)
        })?;
    Err(if exists {
        invalid("Stock location and its warehouse must be active")
    } else {
        smuggle(CommerceError::NotFound)
    })
}

/// Materialize a bin view from the location's existing ledger, without adding
/// any stock to its warehouse. This is a projection, not another receipt.
pub(super) fn bin_for_location(
    conn: &Connection,
    location: i32,
    now: &str,
) -> DbResult<WarehouseBin> {
    if let Some(bin) = conn.query_row("SELECT b.* FROM warehouse_bins b JOIN stock_location_bins m ON m.bin_id = b.id WHERE m.location_id = ?1", [location], super::bins::row_to_bin).optional()? { return Ok(bin); }
    let warehouse = warehouse_of(conn, location)?;
    let kind: String =
        conn.query_row("SELECT location_type FROM locations WHERE id = ?1", [location], |r| {
            r.get(0)
        })?;
    let kind = match kind.as_str() {
        "pick" | "receiving" | "staging" | "quarantine" | "returns" => kind,
        _ => "bulk".into(),
    };
    let code = format!("__stock_location_{location}");
    conn.execute("INSERT INTO warehouse_bins (warehouse_id, code, bin_type, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)", params![warehouse, code, kind, now])?;
    let id = conn.last_insert_rowid() as i32;
    conn.execute(
        "INSERT INTO stock_location_bins (location_id, bin_id) VALUES (?1, ?2)",
        params![location, id],
    )?;
    let mut totals: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT sku, quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = ?1 ORDER BY sku, lot_id")?;
    let rows = stmt.query_map([location], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    for row in rows {
        let (sku, on_hand, reserved) = row?;
        let total = totals.entry(sku).or_default();
        total.0 = add(total.0, decimal(&on_hand)?)?;
        total.1 = add(total.1, decimal(&reserved)?)?;
    }
    for (sku, (on_hand, reserved)) in totals {
        conn.execute("INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, sku, on_hand.to_string(), reserved.to_string(), now])?;
    }
    conn.query_row("SELECT * FROM warehouse_bins WHERE id = ?1", [id], super::bins::row_to_bin)
}

/// Give legacy bin-only stock an explicit physical location. A bin's old
/// quantities are copied as the starting projection; aggregate stock is unchanged.
pub(super) fn location_for_bin(conn: &Connection, bin: i32, now: &str) -> DbResult<i32> {
    if let Some(location) = conn
        .query_row("SELECT location_id FROM stock_location_bins WHERE bin_id = ?1", [bin], |r| {
            r.get(0)
        })
        .optional()?
    {
        return Ok(location);
    }
    let bin = conn.query_row(
        "SELECT * FROM warehouse_bins WHERE id = ?1",
        [bin],
        super::bins::row_to_bin,
    )?;
    if !bin.is_active {
        return Err(invalid("Stock bin is inactive"));
    }
    let kind = bin.bin_type.to_string();
    let pickable = !matches!(kind.as_str(), "staging" | "quarantine");
    conn.execute("INSERT INTO locations (warehouse_id, code, location_type, is_pickable, is_receivable, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, 1, ?5, ?5)", params![bin.warehouse_id, format!("__stock_bin_{}", bin.id), kind, pickable, now])?;
    let location = conn.last_insert_rowid() as i32;
    conn.execute(
        "INSERT INTO stock_location_bins (location_id, bin_id) VALUES (?1, ?2)",
        params![location, bin.id],
    )?;
    conn.execute("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) SELECT ?1, sku, '', quantity_on_hand, quantity_allocated, ?2 FROM inventory_bin_levels WHERE bin_id = ?3", params![location, now, bin.id])?;
    Ok(location)
}

pub(super) fn role_location(
    conn: &Connection,
    warehouse: i32,
    role: &str,
    now: &str,
) -> DbResult<i32> {
    if !matches!(role, "unassigned" | "staging") {
        return Err(invalid("Unknown stock location role"));
    }
    if let Some(location) = conn.query_row("SELECT location_id FROM stock_warehouse_locations WHERE warehouse_id = ?1 AND role = ?2", params![warehouse, role], |r| r.get(0)).optional()? { return Ok(location); }
    let kind = if role == "staging" { "staging" } else { "bulk" };
    conn.execute("INSERT INTO locations (warehouse_id, code, location_type, is_pickable, is_receivable, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, 1, ?5, ?5)", params![warehouse, format!("__stock_{role}"), kind, role != "staging", now])?;
    let location = conn.last_insert_rowid() as i32;
    conn.execute("INSERT INTO stock_warehouse_locations (warehouse_id, role, location_id) VALUES (?1, ?2, ?3)", params![warehouse, role, location])?;
    bin_for_location(conn, location, now)?;
    Ok(location)
}

fn shelf_delta(
    conn: &Connection,
    location: i32,
    movement: &StockMove<'_>,
    on_hand_delta: Decimal,
    reserved_delta: Decimal,
    now: &str,
) -> DbResult<i32> {
    let bin = bin_for_location(conn, location, now)?;
    if !bin.is_active {
        return Err(invalid("Stock bin is inactive"));
    }
    let lot_key = movement.lot_id.map(|id| id.to_string()).unwrap_or_default();
    let current: (Decimal, Decimal) = conn.query_row("SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = ?1 AND sku = ?2 AND lot_id = ?3", params![location, movement.sku, lot_key], |r| Ok((parse_decimal_row(&r.get::<_, String>(0)?, "stock", "on_hand")?, parse_decimal_row(&r.get::<_, String>(1)?, "stock", "reserved")?))).optional()?.unwrap_or_default();
    let on_hand = add(current.0, on_hand_delta)?;
    let reserved = add(current.1, reserved_delta)?;
    if on_hand < Decimal::ZERO || reserved < Decimal::ZERO || on_hand < reserved {
        return Err(smuggle(CommerceError::InsufficientStock {
            sku: movement.sku.into(),
            requested: on_hand_delta.abs().to_string(),
            available: (current.0 - current.1).to_string(),
        }));
    }
    conn.execute("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(location_id, sku, lot_id) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, quantity_reserved = excluded.quantity_reserved, updated_at = excluded.updated_at", params![location, movement.sku, lot_key, on_hand.to_string(), reserved.to_string(), now])?;
    let current: (Decimal, Decimal) = conn.query_row("SELECT quantity_on_hand, quantity_allocated FROM inventory_bin_levels WHERE bin_id = ?1 AND sku = ?2", params![bin.id, movement.sku], |r| Ok((parse_decimal_row(&r.get::<_, String>(0)?, "bin", "on_hand")?, parse_decimal_row(&r.get::<_, String>(1)?, "bin", "allocated")?))).optional()?.unwrap_or_default();
    let mut physical = (Decimal::ZERO, Decimal::ZERO);
    let mut statement = conn.prepare("SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = ?1 AND sku = ?2")?;
    let rows = statement.query_map(params![location, movement.sku], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (on_hand, reserved) = row?;
        physical.0 = add(physical.0, decimal(&on_hand)?)?;
        physical.1 = add(physical.1, decimal(&reserved)?)?;
    }
    if physical != (add(current.0, on_hand_delta)?, add(current.1, reserved_delta)?) {
        return Err(invalid(
            "Bin stock disagrees with its physical location; reconcile before posting",
        ));
    }
    let bin_on_hand = add(current.0, on_hand_delta)?;
    let bin_reserved = add(current.1, reserved_delta)?;
    if bin_reserved < Decimal::ZERO || bin_on_hand < bin_reserved {
        return Err(invalid(
            "Bin stock disagrees with its physical location; reconcile before posting",
        ));
    }
    if bin.capacity.is_some_and(|capacity| bin_on_hand > capacity) {
        return Err(invalid("Bin capacity exceeded"));
    }
    conn.execute("INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(bin_id, sku) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, quantity_allocated = excluded.quantity_allocated, updated_at = excluded.updated_at", params![bin.id, movement.sku, bin_on_hand.to_string(), bin_reserved.to_string(), now])?;
    Ok(bin.id)
}

fn warehouse_delta(
    conn: &Connection,
    warehouse: i32,
    movement: &StockMove<'_>,
    delta: Decimal,
    allocation: Decimal,
    now: &str,
) -> DbResult<()> {
    let (code, name): (String, String) =
        conn.query_row("SELECT code, name FROM warehouses WHERE id = ?1", [warehouse], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    conn.execute(
        "INSERT OR IGNORE INTO inventory_locations (id, code, name) VALUES (?1, ?2, ?3)",
        params![warehouse, code, name],
    )?;
    conn.execute("INSERT OR IGNORE INTO inventory_items (sku, name, created_at, updated_at) VALUES (?1, ?1, ?2, ?2)", params![movement.sku, now])?;
    let item: i64 =
        conn.query_row("SELECT id FROM inventory_items WHERE sku = ?1", [movement.sku], |r| {
            r.get(0)
        })?;
    let current: (Decimal, Decimal) = conn.query_row("SELECT quantity_on_hand, quantity_allocated FROM inventory_balances WHERE item_id = ?1 AND location_id = ?2", params![item, warehouse], |r| Ok((parse_decimal_row(&r.get::<_, String>(0)?, "balance", "on_hand")?, parse_decimal_row(&r.get::<_, String>(1)?, "balance", "allocated")?))).optional()?.unwrap_or_default();
    let on_hand = add(current.0, delta)?;
    let reserved = add(current.1, allocation)?;
    if reserved < Decimal::ZERO || on_hand < reserved {
        return Err(invalid(
            "Warehouse stock is insufficient or disagrees with physical stock; reconcile before posting",
        ));
    }
    conn.execute("INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(item_id, location_id) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, quantity_allocated = excluded.quantity_allocated, quantity_available = excluded.quantity_available, version = inventory_balances.version + 1, updated_at = excluded.updated_at", params![item, warehouse, on_hand.to_string(), reserved.to_string(), (on_hand - reserved).to_string(), now])?;
    let kind = if delta.is_zero() {
        if allocation >= Decimal::ZERO { "allocation" } else { "deallocation" }
    } else {
        match movement.movement_type {
            stateset_core::MovementType::Receipt => "receipt",
            stateset_core::MovementType::Shipment => "shipment",
            stateset_core::MovementType::Transfer => "transfer",
            _ => "adjustment",
        }
    };
    conn.execute("INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![item, warehouse, kind, if delta.is_zero() { allocation } else { delta }.to_string(), movement.reference_type, movement.reference_text(), movement.reason, now])?;
    Ok(())
}

fn lot_effect(
    conn: &Connection,
    movement: &StockMove<'_>,
    warehouses: &BTreeMap<i32, (Decimal, Decimal)>,
    now: &str,
) -> DbResult<()> {
    let Some(lot) = movement.lot_id else {
        return Ok(());
    };
    let (sku, remaining, reserved, quarantined, produced): (String, String, String, String, String) = conn.query_row("SELECT sku, quantity_remaining, quantity_reserved, quantity_quarantined, quantity_produced FROM lots WHERE id = ?1", [lot.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    if sku != movement.sku {
        return Err(invalid("Lot does not belong to the stock SKU"));
    }
    if movement.lot_effect == LotEffect::AlreadyRecorded {
        return Ok(());
    }
    let mut delta = if movement.lot_effect == LotEffect::TransitLoss {
        -movement.quantity
    } else {
        Decimal::ZERO
    };
    let mut allocation = Decimal::ZERO;
    for (warehouse, (change, allocated)) in warehouses {
        delta = add(delta, *change)?;
        allocation = add(allocation, *allocated)?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT quantity FROM lot_locations WHERE lot_id = ?1 AND location_id = ?2",
                params![lot.to_string(), warehouse],
                |r| r.get(0),
            )
            .optional()?;
        let current = existing.as_deref().map(decimal).transpose()?.unwrap_or_default();
        let quantity = add(current, *change)?;
        if quantity < Decimal::ZERO {
            return Err(invalid("Lot placement is inconsistent; reconcile before moving stock"));
        }
        conn.execute("INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(lot_id, location_id) DO UPDATE SET quantity = excluded.quantity, updated_at = excluded.updated_at", params![lot.to_string(), warehouse, quantity.to_string(), now])?;
    }
    if movement.lot_effect == LotEffect::MetadataRecorded {
        return Ok(());
    }
    let old_remaining = decimal(&remaining)?;
    let mut remaining = old_remaining;
    if matches!(movement.lot_effect, LotEffect::Track | LotEffect::TransitLoss) {
        if delta < Decimal::ZERO {
            remaining = add(remaining, delta)?;
        }
        if delta > Decimal::ZERO {
            // Lot creation may describe stock before its first physical receipt.
            // Assign those units before increasing the lot's produced quantity.
            let mut placed = Decimal::ZERO;
            let mut stmt = conn.prepare("SELECT quantity FROM lot_locations WHERE lot_id = ?1")?;
            for row in stmt.query_map([lot.to_string()], |r| r.get::<_, String>(0))? {
                placed = add(placed, decimal(&row?)?)?;
            }
            // Transit is still owned by the lot even though it occupies no shelf.
            let mut stmt = conn.prepare("SELECT i.quantity_shipped, i.quantity_received, i.quantity_variance FROM transfer_order_items i JOIN transfer_orders t ON t.id = i.transfer_order_id WHERE i.lot_id = ?1 AND t.status IN ('in_transit', 'partially_received')")?;
            for row in stmt.query_map([lot.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })? {
                let (shipped, received, missing) = row?;
                placed =
                    add(placed, decimal(&shipped)? - decimal(&received)? - decimal(&missing)?)?;
            }
            remaining = remaining.max(placed);
        }
    }
    let reserved = add(decimal(&reserved)?, allocation)?;
    if reserved < Decimal::ZERO || remaining < add(reserved, decimal(&quarantined)?)? {
        return Err(invalid("Lot movement would consume reserved or quarantined stock"));
    }
    let produced = add(decimal(&produced)?, (remaining - old_remaining).max(Decimal::ZERO))?;
    conn.execute("UPDATE lots SET quantity_remaining = ?1, quantity_reserved = ?2, quantity_produced = ?3, status = CASE WHEN ?6 THEN 'consumed' WHEN status = 'consumed' THEN 'active' ELSE status END, updated_at = ?4 WHERE id = ?5", params![remaining.to_string(), reserved.to_string(), produced.to_string(), now, lot.to_string(), remaining.is_zero()])?;
    let kind = if movement.from.is_some() && movement.to.is_some() {
        "transferred"
    } else if movement.to.is_some() {
        "received"
    } else {
        "consumed"
    };
    conn.execute("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, from_location_id, to_location_id, reason, performed_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![Uuid::new_v4().to_string(), lot.to_string(), kind, movement.quantity.to_string(), movement.reference_type.unwrap_or("stock_move"), movement.reference_id.unwrap_or(movement.id).to_string(), movement.from, movement.to, movement.reason, movement.performed_by, now])?;
    Ok(())
}

pub(crate) fn post(conn: &Connection, movement: &StockMove<'_>, now: &str) -> DbResult<()> {
    post_inner(conn, movement, now, true, true)
}
/// The owning inventory command has already posted the warehouse balance.
pub(crate) fn post_physical(
    conn: &Connection,
    movement: &StockMove<'_>,
    now: &str,
) -> DbResult<()> {
    post_inner(conn, movement, now, false, true)
}
fn post_inner(
    conn: &Connection,
    movement: &StockMove<'_>,
    now: &str,
    post_warehouse: bool,
    record_bin: bool,
) -> DbResult<()> {
    if conn.is_autocommit() {
        return Err(invalid("Stock posting requires the document's transaction"));
    }
    movement.validate().map_err(smuggle)?;
    let mut warehouses: BTreeMap<i32, (Decimal, Decimal)> = BTreeMap::new();
    let mut from_bin = None;
    let mut to_bin = None;
    if let Some(source) = movement.from {
        let warehouse = warehouse_of(conn, source)?;
        if movement.check_sellable {
            super::stock_guard::ensure_sellable(
                conn,
                movement.sku,
                movement.lot_id,
                None,
                Some(source),
            )
            .map_err(smuggle)?;
        }
        from_bin = Some(shelf_delta(
            conn,
            source,
            movement,
            -movement.quantity,
            -movement.release_reserved,
            now,
        )?);
        let changes = warehouses.entry(warehouse).or_default();
        changes.0 = add(changes.0, -movement.quantity)?;
        changes.1 = add(changes.1, -movement.release_reserved)?;
    }
    if let Some(destination) = movement.to {
        let warehouse = warehouse_of(conn, destination)?;
        let receivable: bool = conn.query_row(
            "SELECT is_receivable FROM locations WHERE id = ?1",
            [destination],
            |r| r.get(0),
        )?;
        if !receivable {
            return Err(invalid("Destination location does not accept stock"));
        }
        if movement.reserve_received > Decimal::ZERO && movement.check_sellable {
            super::stock_guard::ensure_sellable(
                conn,
                movement.sku,
                movement.lot_id,
                None,
                Some(destination),
            )
            .map_err(smuggle)?;
        }
        to_bin = Some(shelf_delta(
            conn,
            destination,
            movement,
            movement.quantity,
            movement.reserve_received,
            now,
        )?);
        let changes = warehouses.entry(warehouse).or_default();
        changes.0 = add(changes.0, movement.quantity)?;
        changes.1 = add(changes.1, movement.reserve_received)?;
    }
    for (warehouse, (quantity, reserved)) in &warehouses {
        if post_warehouse && (!quantity.is_zero() || !reserved.is_zero()) {
            warehouse_delta(conn, *warehouse, movement, *quantity, *reserved, now)?;
        }
    }
    if post_warehouse {
        for warehouse in warehouses.keys() {
            let mut physical = (Decimal::ZERO, Decimal::ZERO);
            let mut statement = conn.prepare("SELECT li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = ?1 AND li.sku = ?2")?;
            let rows = statement.query_map(params![warehouse, movement.sku], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (on_hand, reserved) = row?;
                physical.0 = add(physical.0, decimal(&on_hand)?)?;
                physical.1 = add(physical.1, decimal(&reserved)?)?;
            }
            let balance: (Decimal, Decimal) = conn.query_row("SELECT b.quantity_on_hand, b.quantity_allocated FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE b.location_id = ?1 AND i.sku = ?2", params![warehouse, movement.sku], |row| Ok((parse_decimal_row(&row.get::<_, String>(0)?, "stock", "on_hand")?, parse_decimal_row(&row.get::<_, String>(1)?, "stock", "allocated")?))).optional()?.unwrap_or_default();
            if physical != balance {
                return Err(invalid(
                    "Warehouse stock disagrees with physical stock; reconcile before posting",
                ));
            }
        }
    }
    lot_effect(conn, movement, &warehouses, now)?;
    conn.execute("INSERT INTO inventory_movements (id, movement_type, from_location_id, to_location_id, sku, lot_id, quantity, reference_type, reference_id, reason, performed_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)", params![movement.id.to_string(), movement.movement_type.to_string(), movement.from, movement.to, movement.sku, movement.lot_id.map(|id| id.to_string()), movement.ledger_quantity().to_string(), movement.reference_type, movement.reference_text(), movement.reason, movement.performed_by, now])?;
    if record_bin {
        conn.execute("INSERT INTO inventory_bin_movements (id, movement_type, from_bin_id, to_bin_id, sku, quantity, reason, reference_type, reference_id, performed_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![movement.id.to_string(), if movement.movement_type == stateset_core::MovementType::Return { "return_disposition" } else if movement.from.is_some() && movement.to.is_some() { "transfer" } else { "adjustment" }, from_bin, to_bin, movement.sku, movement.ledger_quantity().to_string(), movement.reason, movement.reference_type, movement.reference_text(), movement.performed_by, now])?;
    }

    super::stock_cost::post(conn, movement, now)?;
    Ok(())
}

/// Change physical allocation after the owning reservation has accounted for
/// the warehouse allocation in the same transaction.
pub(super) fn hold(
    conn: &Connection,
    location: i32,
    sku: &str,
    lot: Option<Uuid>,
    quantity: Decimal,
    reservation: Uuid,
    now: &str,
) -> DbResult<()> {
    if quantity.is_zero() {
        return Ok(());
    }
    if quantity > Decimal::ZERO {
        super::stock_guard::ensure_sellable(conn, sku, lot, None, Some(location))
            .map_err(smuggle)?;
    }
    let movement = StockMove {
        id: Uuid::new_v4(),
        sku,
        lot_id: lot,
        from: Some(location),
        to: None,
        quantity: quantity.abs(),
        release_reserved: Decimal::ZERO,
        reserve_received: Decimal::ZERO,
        movement_type: stateset_core::MovementType::Adjustment,
        lot_effect: LotEffect::Track,
        check_sellable: false,
        reference_type: Some("inventory_reservation"),
        reference_id: Some(reservation),
        reference_label: None,
        reason: Some("physical reservation"),
        performed_by: None,
    };
    shelf_delta(conn, location, &movement, Decimal::ZERO, quantity, now)?;
    if let Some(lot) = lot {
        let (remaining, reserved, quarantined): (String, String, String) = conn.query_row("SELECT quantity_remaining, quantity_reserved, quantity_quarantined FROM lots WHERE id = ?1 AND sku = ?2", params![lot.to_string(), sku], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let reserved = add(decimal(&reserved)?, quantity)?;
        if reserved < Decimal::ZERO || add(reserved, decimal(&quarantined)?)? > decimal(&remaining)?
        {
            return Err(invalid("Lot has insufficient available stock"));
        }
        conn.execute(
            "UPDATE lots SET quantity_reserved = ?1, updated_at = ?2 WHERE id = ?3",
            params![reserved.to_string(), now, lot.to_string()],
        )?;
        conn.execute("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, reason, created_at) VALUES (?1, ?2, ?3, ?4, 'inventory_reservation', ?5, 'physical reservation', ?6)", params![Uuid::new_v4().to_string(), lot.to_string(), if quantity > Decimal::ZERO { "reserved" } else { "released" }, quantity.abs().to_string(), reservation.to_string(), now])?;
    }
    Ok(())
}

/// Allocation whose lot metadata was already updated by the lot command.
#[allow(clippy::too_many_arguments)]
pub(super) fn allocate_recorded_lot(
    conn: &Connection,
    location: i32,
    sku: &str,
    lot: Uuid,
    quantity: Decimal,
    reason: &str,
    now: &str,
) -> DbResult<()> {
    if quantity.is_zero() {
        return Ok(());
    }
    let warehouse = warehouse_of(conn, location)?;
    let movement = StockMove {
        id: Uuid::new_v4(),
        sku,
        lot_id: Some(lot),
        from: Some(location),
        to: None,
        quantity: quantity.abs(),
        release_reserved: Decimal::ZERO,
        reserve_received: Decimal::ZERO,
        movement_type: stateset_core::MovementType::Adjustment,
        lot_effect: LotEffect::AlreadyRecorded,
        check_sellable: false,
        reference_type: Some("lot"),
        reference_id: Some(lot),
        reference_label: None,
        reason: Some(reason),
        performed_by: None,
    };
    shelf_delta(conn, location, &movement, Decimal::ZERO, quantity, now)?;
    warehouse_delta(conn, warehouse, &movement, Decimal::ZERO, quantity, now)?;
    Ok(())
}

/// A bin identifies a physical location, but can contain several lots. Post each
/// exact lot slice and record their total as one bin movement in the same transaction.
pub(crate) fn post_from_slices(
    tx: &Connection,
    movement: &StockMove<'_>,
    now: &str,
) -> DbResult<()> {
    movement.validate().map_err(smuggle)?;
    let Some(source) = movement.from else {
        return post(tx, movement, now);
    };
    if movement.lot_id.is_some() || !movement.release_reserved.is_zero() {
        return post(tx, movement, now);
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT lot_id, quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = ?1 AND sku = ?2 ORDER BY lot_id")?;
        let rows = stmt.query_map(rusqlite::params![source, movement.sku], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = movement.quantity;
    let mut first = true;
    for (key, on_hand, reserved) in rows {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        let mut slice = movement.clone();
        slice.id = if first { movement.id } else { Uuid::new_v4() };
        slice.quantity = take;
        slice.lot_id = if key.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&key).map_err(|_| invalid("Invalid stock lot"))?)
        };
        post_inner(tx, &slice, now, true, first)?;
        first = false;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(smuggle(CommerceError::InsufficientStock {
            sku: movement.sku.into(),
            requested: movement.quantity.to_string(),
            available: (movement.quantity - remaining).to_string(),
        }));
    }
    tx.execute(
        "UPDATE inventory_bin_movements SET quantity = ?1 WHERE id = ?2",
        rusqlite::params![(movement.ledger_quantity()).to_string(), (movement.id).to_string()],
    )?;
    Ok(())
}
