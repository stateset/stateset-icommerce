//! Materialize legacy warehouse-only stock into explicit physical custody.
//! Existing projections are checked; conflicting totals require reconciliation.
use rusqlite::{Connection, OptionalExtension};
use rust_decimal::Decimal;
use stateset_core::CommerceError;
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn ensure_warehouse(tx: &Connection, warehouse: i32, now: &str) -> DbResult<()> {
    let existing = tx
        .query_row("SELECT id FROM warehouses WHERE id = ?1", rusqlite::params![warehouse], |r| {
            r.get::<_, i32>(0)
        })
        .optional()?;
    if existing.is_none() {
        let identity = tx
            .query_row(
                "SELECT name FROM inventory_locations WHERE id = ?1",
                rusqlite::params![warehouse],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| invalid("Unknown stock warehouse"))?;
        // A well-formed empty address: warehouse reads require every field.
        let address = serde_json::to_string(&stateset_core::WarehouseAddress::default())
            .map_err(|e| invalid(&e.to_string()))?;
        tx.execute("INSERT INTO warehouses (id, code, name, address_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5) ON CONFLICT(id) DO NOTHING", rusqlite::params![warehouse, format!("__inventory_{warehouse}"), identity, address, now])?;
    }
    let (code, name) = tx.query_row(
        "SELECT code, name FROM warehouses WHERE id = ?1",
        rusqlite::params![warehouse],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    tx.execute("INSERT INTO inventory_locations (id, code, name) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO NOTHING", rusqlite::params![warehouse, code, name])?;
    Ok(())
}
pub(super) fn materialize(tx: &Connection, warehouse: i32, sku: &str, now: &str) -> DbResult<()> {
    ensure_warehouse(tx, warehouse, now)?;
    let balance = tx.query_row("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = ?1 AND b.location_id = ?2", rusqlite::params![sku, warehouse], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
    let bins = {
        let mut stmt = tx.prepare("SELECT b.id FROM warehouse_bins b JOIN inventory_bin_levels l ON l.bin_id = b.id LEFT JOIN stock_location_bins m ON m.bin_id = b.id WHERE b.warehouse_id = ?1 AND l.sku = ?2 AND m.location_id IS NULL ORDER BY b.id")?;
        let rows = stmt.query_map(rusqlite::params![warehouse, sku], |r| r.get::<_, i32>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for bin in bins {
        super::stock_move::location_for_bin(tx, bin, now)?;
    }
    let lots = {
        let mut stmt = tx.prepare("SELECT ll.lot_id, ll.quantity FROM lot_locations ll JOIN lots l ON l.id = ll.lot_id WHERE ll.location_id = ?1 AND l.sku = ?2 ORDER BY ll.lot_id")?;
        let rows = stmt.query_map(rusqlite::params![warehouse, sku], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (lot, placed) in lots {
        let physical = {
            let mut stmt = tx.prepare("SELECT li.quantity_on_hand FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = ?1 AND li.sku = ?2 AND li.lot_id = ?3")?;
            let rows = stmt
                .query_map(rusqlite::params![warehouse, sku, (lot).to_string()], |r| {
                    super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let physical: Decimal = physical.into_iter().sum();
        if physical > placed {
            return Err(invalid(
                "Physical lot stock exceeds its placement; reconcile before posting",
            ));
        }
        if placed > physical {
            baseline(tx, warehouse, sku, Some(lot), placed - physical, now)?;
        }
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT li.location_id, li.quantity_on_hand FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = ?1 AND li.sku = ?2 ORDER BY li.location_id, li.lot_id")?;
        let rows = stmt.query_map(rusqlite::params![warehouse, sku], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let total: Decimal = rows.iter().map(|row| row.1).sum();
    if total > balance {
        return Err(invalid(
            "Physical stock exceeds the warehouse balance; reconcile before posting",
        ));
    }
    if balance > total {
        baseline(tx, warehouse, sku, None, balance - total, now)?;
    }
    for (location, _) in rows {
        super::stock_move::bin_for_location(tx, location, now)?;
    }
    Ok(())
}
pub(super) fn baseline(
    tx: &Connection,
    warehouse: i32,
    sku: &str,
    lot: Option<Uuid>,
    quantity: Decimal,
    now: &str,
) -> DbResult<()> {
    let location = super::stock_move::role_location(tx, warehouse, "unassigned", now)?;
    let bin = super::stock_move::bin_for_location(tx, location, now)?;
    let lot_key = lot.map(|id| id.to_string()).unwrap_or_default();
    let current = tx.query_row("SELECT quantity_on_hand FROM location_inventory WHERE location_id = ?1 AND sku = ?2 AND lot_id = ?3", rusqlite::params![location, sku, &lot_key], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
    tx.execute("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) VALUES (?1, ?2, ?3, ?4, '0', ?5) ON CONFLICT(location_id, sku, lot_id) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, updated_at = excluded.updated_at", rusqlite::params![location, sku, &lot_key, (current+quantity).to_string(), now])?;

    let current = tx
        .query_row(
            "SELECT quantity_on_hand FROM inventory_bin_levels WHERE bin_id = ?1 AND sku = ?2",
            rusqlite::params![bin.id, sku],
            |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity"),
        )
        .optional()?
        .unwrap_or_default();
    tx.execute("INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at) VALUES (?1, ?2, ?3, '0', ?4) ON CONFLICT(bin_id, sku) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, updated_at = excluded.updated_at", rusqlite::params![bin.id, sku, (current+quantity).to_string(), now])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn adjust(
    tx: &Connection,
    sku: &str,
    warehouse: i32,
    delta: Decimal,
    kind: stateset_core::MovementType,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    reason: &str,
    materialize_existing: bool,
    now: &str,
) -> DbResult<()> {
    if delta.is_zero() {
        return Ok(());
    }
    ensure_warehouse(tx, warehouse, now)?;
    if materialize_existing {
        materialize(tx, warehouse, sku, now)?;
    }
    let mut movement = crate::stock_move::StockMove {
        id: Uuid::new_v4(),
        sku,
        lot_id: None,
        from: None,
        to: None,
        quantity: delta.abs(),
        release_reserved: Decimal::ZERO,
        reserve_received: Decimal::ZERO,
        movement_type: kind,
        lot_effect: crate::stock_move::LotEffect::Track,
        check_sellable: delta < Decimal::ZERO,
        reference_type,
        reference_id: reference_id.and_then(|value| Uuid::parse_str(value).ok()),
        reference_label: reference_id,
        reason: Some(reason),
        performed_by: None,
    };
    if delta > Decimal::ZERO {
        movement.to = Some(super::stock_move::role_location(tx, warehouse, "unassigned", now)?);
        super::stock_move::post_physical(tx, &movement, now)?;
        return Ok(());
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT li.location_id, li.lot_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE li.sku = ?1 AND l.warehouse_id = ?2 AND l.is_active = 1 AND l.is_pickable = 1 ORDER BY li.location_id, li.lot_id")?;
        let rows = stmt.query_map(rusqlite::params![sku, warehouse], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = -delta;
    for (location, key, on_hand, reserved) in rows {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        movement.from = Some(location);
        movement.lot_id = if key.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&key).map_err(|_| invalid("Invalid stock lot"))?)
        };
        match super::stock_guard::ensure_sellable(tx, sku, movement.lot_id, None, Some(location)) {
            Ok(()) => {}
            Err(CommerceError::Conflict(_)) | Err(CommerceError::NotPermitted(_)) => continue,
            Err(error) => return Err(super::bins::smuggle(error)),
        }
        movement.id = Uuid::new_v4();
        movement.quantity = take;
        super::stock_move::post_physical(tx, &movement, now)?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient eligible physical stock"));
    }
    Ok(())
}
