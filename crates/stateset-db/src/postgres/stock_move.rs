//! Transaction-scoped physical stock posting across all quantity ledgers.
use super::{bins::BinRow, map_db_error};
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{PgConnection, Postgres, Transaction};
use stateset_core::{CommerceError, Result, WarehouseBin};
use std::collections::BTreeMap;
use uuid::Uuid;

fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn add(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_add(b).ok_or_else(|| invalid("Stock quantity overflow"))
}

pub(super) async fn warehouse_of(conn: &mut PgConnection, location: i32) -> Result<i32> {
    sqlx::query_scalar("SELECT l.warehouse_id FROM locations l JOIN warehouses w ON w.id = l.warehouse_id WHERE l.id = $1 AND l.is_active AND w.is_active FOR SHARE OF l, w")
        .bind(location).fetch_optional(conn).await.map_err(map_db_error)?.ok_or_else(|| invalid("Stock location and its warehouse must be active"))
}

pub(super) async fn bin_for_location(
    conn: &mut PgConnection,
    location: i32,
    now: DateTime<Utc>,
) -> Result<WarehouseBin> {
    // Protect deletion without serializing unrelated SKUs at the same shelf.
    sqlx::query("SELECT id FROM locations WHERE id = $1 FOR SHARE")
        .bind(location)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    if let Some(row) = sqlx::query_as::<_, BinRow>("SELECT b.* FROM warehouse_bins b JOIN stock_location_bins m ON m.bin_id = b.id WHERE m.location_id = $1")
        .bind(location).fetch_optional(&mut *conn).await.map_err(map_db_error)? { return row.try_into(); }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 781146))")
        .bind(location.to_string())
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    if let Some(row) = sqlx::query_as::<_, BinRow>("SELECT b.* FROM warehouse_bins b JOIN stock_location_bins m ON m.bin_id = b.id WHERE m.location_id = $1").bind(location).fetch_optional(&mut *conn).await.map_err(map_db_error)? { return row.try_into(); }
    let warehouse = warehouse_of(conn, location).await?;
    let kind: String = sqlx::query_scalar("SELECT location_type FROM locations WHERE id = $1")
        .bind(location)
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;
    let kind = match kind.as_str() {
        "pick" | "receiving" | "staging" | "quarantine" | "returns" => kind.as_str(),
        _ => "bulk",
    };
    let id: i32 = sqlx::query_scalar("INSERT INTO warehouse_bins (warehouse_id, code, bin_type, is_active, created_at, updated_at) VALUES ($1, $2, $3, TRUE, $4, $4) RETURNING id")
        .bind(warehouse).bind(format!("__stock_location_{location}")).bind(kind).bind(now).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    sqlx::query("INSERT INTO stock_location_bins (location_id, bin_id) VALUES ($1, $2)")
        .bind(location)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    sqlx::query("INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at) SELECT $1, sku, SUM(quantity_on_hand), SUM(quantity_reserved), $2 FROM location_inventory WHERE location_id = $3 GROUP BY sku")
        .bind(id).bind(now).bind(location).execute(&mut *conn).await.map_err(map_db_error)?;
    super::bins::load_bin_pg(conn, id).await
}

pub(super) async fn location_for_bin(
    conn: &mut PgConnection,
    bin: i32,
    now: DateTime<Utc>,
) -> Result<i32> {
    if let Some(location) =
        sqlx::query_scalar("SELECT location_id FROM stock_location_bins WHERE bin_id = $1")
            .bind(bin)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?
    {
        return Ok(location);
    }
    let bin = super::bins::load_bin_pg(conn, bin).await?;
    if !bin.is_active {
        return Err(invalid("Stock bin is inactive"));
    }
    if let Some(location) =
        sqlx::query_scalar("SELECT location_id FROM stock_location_bins WHERE bin_id = $1")
            .bind(bin.id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?
    {
        return Ok(location);
    }
    let kind = bin.bin_type.to_string();
    let location: i32 = sqlx::query_scalar("INSERT INTO locations (warehouse_id, code, location_type, is_pickable, is_receivable, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, TRUE, TRUE, $5, $5) RETURNING id")
        .bind(bin.warehouse_id).bind(format!("__stock_bin_{}", bin.id)).bind(&kind).bind(!matches!(kind.as_str(), "staging" | "quarantine")).bind(now).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    sqlx::query("INSERT INTO stock_location_bins (location_id, bin_id) VALUES ($1, $2)")
        .bind(location)
        .bind(bin.id)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    sqlx::query("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) SELECT $1, sku, $2, quantity_on_hand, quantity_allocated, $3 FROM inventory_bin_levels WHERE bin_id = $4")
        .bind(location).bind(Uuid::nil()).bind(now).bind(bin.id).execute(&mut *conn).await.map_err(map_db_error)?;
    Ok(location)
}

pub(super) async fn role_location(
    conn: &mut PgConnection,
    warehouse: i32,
    role: &str,
    now: DateTime<Utc>,
) -> Result<i32> {
    if !matches!(role, "unassigned" | "staging") {
        return Err(invalid("Unknown stock location role"));
    }
    if let Some(location) = sqlx::query_scalar(
        "SELECT location_id FROM stock_warehouse_locations WHERE warehouse_id = $1 AND role = $2",
    )
    .bind(warehouse)
    .bind(role)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?
    {
        return Ok(location);
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 781145))")
        .bind(format!("{warehouse}:{role}"))
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    if let Some(location) = sqlx::query_scalar(
        "SELECT location_id FROM stock_warehouse_locations WHERE warehouse_id = $1 AND role = $2",
    )
    .bind(warehouse)
    .bind(role)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?
    {
        return Ok(location);
    }
    let kind = if role == "staging" { "staging" } else { "bulk" };
    let location: i32 = sqlx::query_scalar("INSERT INTO locations (warehouse_id, code, location_type, is_pickable, is_receivable, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, TRUE, TRUE, $5, $5) RETURNING id")
        .bind(warehouse).bind(format!("__stock_{role}")).bind(kind).bind(role != "staging").bind(now).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    sqlx::query("INSERT INTO stock_warehouse_locations (warehouse_id, role, location_id) VALUES ($1, $2, $3)").bind(warehouse).bind(role).bind(location).execute(&mut *conn).await.map_err(map_db_error)?;
    bin_for_location(conn, location, now).await?;
    Ok(location)
}

async fn shelf_delta(
    conn: &mut PgConnection,
    location: i32,
    movement: &StockMove<'_>,
    delta: Decimal,
    allocation: Decimal,
    now: DateTime<Utc>,
) -> Result<i32> {
    let bin = bin_for_location(conn, location, now).await?;
    if !bin.is_active {
        return Err(invalid("Stock bin is inactive"));
    }
    let lot = movement.lot_id.unwrap_or_else(Uuid::nil);
    sqlx::query("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) VALUES ($1, $2, $3, 0, 0, $4) ON CONFLICT DO NOTHING")
        .bind(location).bind(movement.sku).bind(lot).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    let current: (Decimal, Decimal) = sqlx::query_as("SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = $1 AND sku = $2 AND lot_id = $3 FOR UPDATE")
        .bind(location).bind(movement.sku).bind(lot).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    let on_hand = add(current.0, delta)?;
    let reserved = add(current.1, allocation)?;
    if reserved < Decimal::ZERO || on_hand < reserved {
        return Err(CommerceError::InsufficientStock {
            sku: movement.sku.into(),
            requested: delta.abs().to_string(),
            available: (current.0 - current.1).to_string(),
        });
    }
    sqlx::query("UPDATE location_inventory SET quantity_on_hand = $1, quantity_reserved = $2, updated_at = $3 WHERE location_id = $4 AND sku = $5 AND lot_id = $6")
        .bind(on_hand).bind(reserved).bind(now).bind(location).bind(movement.sku).bind(lot).execute(&mut *conn).await.map_err(map_db_error)?;
    // The mapped location lock serializes the first bin-level insert as well.
    let updated =
        super::bins::apply_bin_delta_pg(conn, &bin, movement.sku, delta, allocation, now).await?;
    let physical: (Decimal, Decimal) = sqlx::query_as("SELECT COALESCE(SUM(quantity_on_hand), 0), COALESCE(SUM(quantity_reserved), 0) FROM location_inventory WHERE location_id = $1 AND sku = $2")
        .bind(location).bind(movement.sku).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    if physical != (updated.quantity_on_hand, updated.quantity_allocated) {
        return Err(invalid(
            "Bin stock disagrees with its physical location; reconcile before posting",
        ));
    }
    Ok(bin.id)
}

async fn lock_balance(
    conn: &mut PgConnection,
    warehouse: i32,
    sku: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    super::warehouse::ensure_inventory_item_pg(conn, sku).await?;
    sqlx::query("INSERT INTO inventory_locations (id, code, name) SELECT id, code, name FROM warehouses WHERE id = $1 ON CONFLICT DO NOTHING").bind(warehouse).execute(&mut *conn).await.map_err(map_db_error)?;
    sqlx::query("INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at) SELECT id, $2, 0, 0, 0, $3 FROM inventory_items WHERE sku = $1 ON CONFLICT DO NOTHING").bind(sku).bind(warehouse).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    sqlx::query("SELECT b.id FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1 AND b.location_id = $2 FOR UPDATE OF b").bind(sku).bind(warehouse).execute(conn).await.map_err(map_db_error)?;
    Ok(())
}

async fn warehouse_delta(
    conn: &mut PgConnection,
    warehouse: i32,
    movement: &StockMove<'_>,
    delta: Decimal,
    allocation: Decimal,
    now: DateTime<Utc>,
) -> Result<()> {
    super::warehouse::ensure_inventory_item_pg(conn, movement.sku).await?;
    let (code, name): (String, String) =
        sqlx::query_as("SELECT code, name FROM warehouses WHERE id = $1")
            .bind(warehouse)
            .fetch_one(&mut *conn)
            .await
            .map_err(map_db_error)?;
    sqlx::query("INSERT INTO inventory_locations (id, code, name) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING").bind(warehouse).bind(code).bind(name).execute(&mut *conn).await.map_err(map_db_error)?;
    let item: i64 = sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
        .bind(movement.sku)
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;
    sqlx::query("INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at) VALUES ($1, $2, 0, 0, 0, $3) ON CONFLICT DO NOTHING")
        .bind(item).bind(warehouse).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    let current: (Decimal, Decimal) = sqlx::query_as("SELECT quantity_on_hand, quantity_allocated FROM inventory_balances WHERE item_id = $1 AND location_id = $2 FOR UPDATE").bind(item).bind(warehouse).fetch_one(&mut *conn).await.map_err(map_db_error)?;
    let on_hand = add(current.0, delta)?;
    let reserved = add(current.1, allocation)?;
    if reserved < Decimal::ZERO || on_hand < reserved {
        return Err(invalid(
            "Warehouse stock is insufficient or disagrees with physical stock; reconcile before posting",
        ));
    }
    sqlx::query("UPDATE inventory_balances SET quantity_on_hand = $1, quantity_allocated = $2, quantity_available = $3, version = version + 1, updated_at = $4 WHERE item_id = $5 AND location_id = $6")
        .bind(on_hand).bind(reserved).bind(on_hand-reserved).bind(now).bind(item).bind(warehouse).execute(&mut *conn).await.map_err(map_db_error)?;
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
    sqlx::query("INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
        .bind(item).bind(warehouse).bind(kind).bind(if delta.is_zero() { allocation } else { delta }).bind(movement.reference_type).bind(movement.reference_text()).bind(movement.reason).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    Ok(())
}

async fn lot_effect(
    conn: &mut PgConnection,
    movement: &StockMove<'_>,
    warehouses: &BTreeMap<i32, (Decimal, Decimal)>,
    now: DateTime<Utc>,
) -> Result<()> {
    let Some(lot) = movement.lot_id else {
        return Ok(());
    };
    let (sku, old_remaining, old_reserved, quarantined, produced): (String, Decimal, Decimal, Decimal, Decimal) = sqlx::query_as("SELECT sku, quantity_remaining, quantity_reserved, quantity_quarantined, quantity_produced FROM lots WHERE id = $1 FOR UPDATE")
        .bind(lot).fetch_one(&mut *conn).await.map_err(map_db_error)?;
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
        let current: Decimal = sqlx::query_scalar(
            "SELECT quantity FROM lot_locations WHERE lot_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(lot)
        .bind(warehouse)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?
        .unwrap_or_default();
        let quantity = add(current, *change)?;
        if quantity < Decimal::ZERO {
            return Err(invalid("Lot placement is inconsistent; reconcile before moving stock"));
        }
        sqlx::query("INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at) VALUES ($1, $2, $3, $4) ON CONFLICT(lot_id, location_id) DO UPDATE SET quantity = EXCLUDED.quantity, updated_at = EXCLUDED.updated_at")
            .bind(lot).bind(warehouse).bind(quantity).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    }
    if movement.lot_effect == LotEffect::MetadataRecorded {
        return Ok(());
    }
    let mut remaining = old_remaining;
    if matches!(movement.lot_effect, LotEffect::Track | LotEffect::TransitLoss) {
        if delta < Decimal::ZERO {
            remaining = add(remaining, delta)?;
        }
        if delta > Decimal::ZERO {
            let placed: Decimal = sqlx::query_scalar(
                "SELECT COALESCE(SUM(quantity), 0) FROM lot_locations WHERE lot_id = $1",
            )
            .bind(lot)
            .fetch_one(&mut *conn)
            .await
            .map_err(map_db_error)?;
            let transit: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(i.quantity_shipped - i.quantity_received - i.quantity_variance), 0) FROM transfer_order_items i JOIN transfer_orders t ON t.id = i.transfer_order_id WHERE i.lot_id = $1 AND t.status IN ('in_transit', 'partially_received')").bind(lot).fetch_one(&mut *conn).await.map_err(map_db_error)?;
            remaining = remaining.max(add(placed, transit)?);
        }
    }
    let reserved = add(old_reserved, allocation)?;
    if reserved < Decimal::ZERO || remaining < add(reserved, quarantined)? {
        return Err(invalid("Lot movement would consume reserved or quarantined stock"));
    }
    let produced = add(produced, (remaining - old_remaining).max(Decimal::ZERO))?;
    sqlx::query("UPDATE lots SET quantity_remaining = $1, quantity_reserved = $2, quantity_produced = $3, status = CASE WHEN $4 THEN 'consumed' WHEN status = 'consumed' THEN 'active' ELSE status END, updated_at = $5 WHERE id = $6")
        .bind(remaining).bind(reserved).bind(produced).bind(remaining.is_zero()).bind(now).bind(lot).execute(&mut *conn).await.map_err(map_db_error)?;
    let kind = if movement.from.is_some() && movement.to.is_some() {
        "transferred"
    } else if movement.to.is_some() {
        "received"
    } else {
        "consumed"
    };
    sqlx::query("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, from_location_id, to_location_id, reason, performed_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)")
        .bind(Uuid::new_v4()).bind(lot).bind(kind).bind(movement.quantity).bind(movement.reference_type.unwrap_or("stock_move")).bind(movement.reference_id.unwrap_or(movement.id)).bind(movement.from).bind(movement.to).bind(movement.reason).bind(movement.performed_by).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    Ok(())
}

pub(crate) async fn post(
    tx: &mut Transaction<'_, Postgres>,
    movement: &StockMove<'_>,
    now: DateTime<Utc>,
) -> Result<()> {
    post_inner(tx, movement, now, true, true).await
}
/// The owning inventory command has already posted the warehouse balance.
pub(crate) async fn post_physical(
    tx: &mut Transaction<'_, Postgres>,
    movement: &StockMove<'_>,
    now: DateTime<Utc>,
) -> Result<()> {
    post_inner(tx, movement, now, false, true).await
}
async fn post_inner(
    tx: &mut Transaction<'_, Postgres>,
    movement: &StockMove<'_>,
    now: DateTime<Utc>,
    post_warehouse: bool,
    record_bin: bool,
) -> Result<()> {
    movement.validate()?;
    let conn = tx.as_mut();
    let mut warehouses: BTreeMap<i32, (Decimal, Decimal)> = BTreeMap::new();
    let mut from_bin = None;
    let mut to_bin = None;
    // Fixed location lock ordering for opposite-direction transfers.
    let mut locations: Vec<i32> = [movement.from, movement.to].into_iter().flatten().collect();
    locations.sort_unstable();
    let mut warehouse_ids = std::collections::BTreeSet::new();
    for location in &locations {
        warehouse_ids.insert(warehouse_of(conn, *location).await?);
    }
    for warehouse in warehouse_ids {
        lock_balance(conn, warehouse, movement.sku, now).await?;
    }
    if let Some(lot) = movement.lot_id {
        sqlx::query("SELECT id FROM lots WHERE id = $1 FOR UPDATE")
            .bind(lot)
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
    }
    for location in locations {
        bin_for_location(conn, location, now).await?;
    }
    if let Some(source) = movement.from {
        let warehouse = warehouse_of(conn, source).await?;
        if movement.check_sellable {
            super::stock_guard::ensure_sellable(
                conn,
                movement.sku,
                movement.lot_id,
                None,
                Some(source),
            )
            .await?;
        }
        from_bin = Some(
            shelf_delta(
                conn,
                source,
                movement,
                -movement.quantity,
                -movement.release_reserved,
                now,
            )
            .await?,
        );
        let changes = warehouses.entry(warehouse).or_default();
        changes.0 = add(changes.0, -movement.quantity)?;
        changes.1 = add(changes.1, -movement.release_reserved)?;
    }
    if let Some(destination) = movement.to {
        let warehouse = warehouse_of(conn, destination).await?;
        let receivable: bool =
            sqlx::query_scalar("SELECT is_receivable FROM locations WHERE id = $1")
                .bind(destination)
                .fetch_one(&mut *conn)
                .await
                .map_err(map_db_error)?;
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
            .await?;
        }
        to_bin = Some(
            shelf_delta(
                conn,
                destination,
                movement,
                movement.quantity,
                movement.reserve_received,
                now,
            )
            .await?,
        );
        let changes = warehouses.entry(warehouse).or_default();
        changes.0 = add(changes.0, movement.quantity)?;
        changes.1 = add(changes.1, movement.reserve_received)?;
    }
    for (warehouse, (quantity, reserved)) in &warehouses {
        if post_warehouse && (!quantity.is_zero() || !reserved.is_zero()) {
            warehouse_delta(conn, *warehouse, movement, *quantity, *reserved, now).await?;
        }
    }
    if post_warehouse {
        for warehouse in warehouses.keys() {
            let physical: (Decimal, Decimal) = sqlx::query_as("SELECT COALESCE(SUM(li.quantity_on_hand), 0), COALESCE(SUM(li.quantity_reserved), 0) FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = $1 AND li.sku = $2")
                .bind(warehouse).bind(movement.sku).fetch_one(&mut *conn).await.map_err(map_db_error)?;
            let balance: (Decimal, Decimal) = sqlx::query_as("SELECT b.quantity_on_hand, b.quantity_allocated FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE b.location_id = $1 AND i.sku = $2")
                .bind(warehouse).bind(movement.sku).fetch_optional(&mut *conn).await.map_err(map_db_error)?.unwrap_or_default();
            if physical != balance {
                return Err(invalid(
                    "Warehouse stock disagrees with physical stock; reconcile before posting",
                ));
            }
        }
    }
    lot_effect(conn, movement, &warehouses, now).await?;
    sqlx::query("INSERT INTO inventory_movements (id, movement_type, from_location_id, to_location_id, sku, lot_id, quantity, reference_type, reference_id, reason, performed_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)")
        .bind(movement.id).bind(movement.movement_type.to_string()).bind(movement.from).bind(movement.to).bind(movement.sku).bind(movement.lot_id).bind(movement.ledger_quantity()).bind(movement.reference_type).bind(movement.reference_id).bind(movement.reason).bind(movement.performed_by).bind(now).execute(&mut *conn).await.map_err(map_db_error)?;
    if record_bin {
        sqlx::query("INSERT INTO inventory_bin_movements (id, movement_type, from_bin_id, to_bin_id, sku, quantity, reason, reference_type, reference_id, performed_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)")
        .bind(movement.id).bind(if movement.movement_type == stateset_core::MovementType::Return { "return_disposition" } else if movement.from.is_some() && movement.to.is_some() { "transfer" } else { "adjustment" }).bind(from_bin).bind(to_bin).bind(movement.sku).bind(movement.ledger_quantity()).bind(movement.reason).bind(movement.reference_type).bind(movement.reference_text()).bind(movement.performed_by).bind(now).execute(conn).await.map_err(map_db_error)?;
    }

    super::stock_cost::post(tx, movement, now).await?;
    Ok(())
}

/// Physical counterpart of the owning reservation's warehouse allocation.
pub(super) async fn hold(
    conn: &mut PgConnection,
    location: i32,
    sku: &str,
    lot: Option<Uuid>,
    quantity: Decimal,
    reservation: Uuid,
    now: DateTime<Utc>,
) -> Result<()> {
    if quantity.is_zero() {
        return Ok(());
    }
    let warehouse = warehouse_of(conn, location).await?;
    lock_balance(conn, warehouse, sku, now).await?;
    if let Some(lot) = lot {
        sqlx::query("SELECT id FROM lots WHERE id = $1 FOR UPDATE")
            .bind(lot)
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
    }
    if quantity > Decimal::ZERO {
        super::stock_guard::ensure_sellable(conn, sku, lot, None, Some(location)).await?;
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
    shelf_delta(conn, location, &movement, Decimal::ZERO, quantity, now).await?;
    if let Some(lot) = lot {
        let (remaining, reserved, quarantined): (Decimal, Decimal, Decimal) = sqlx::query_as("SELECT quantity_remaining, quantity_reserved, quantity_quarantined FROM lots WHERE id = $1 AND sku = $2 FOR UPDATE").bind(lot).bind(sku).fetch_one(&mut *conn).await.map_err(map_db_error)?;
        let reserved = add(reserved, quantity)?;
        if reserved < Decimal::ZERO || add(reserved, quarantined)? > remaining {
            return Err(invalid("Lot has insufficient available stock"));
        }
        sqlx::query("UPDATE lots SET quantity_reserved = $1, updated_at = $2 WHERE id = $3")
            .bind(reserved)
            .bind(now)
            .bind(lot)
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
        sqlx::query("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, reason, created_at) VALUES ($1, $2, $3, $4, 'inventory_reservation', $5, 'physical reservation', $6)")
            .bind(Uuid::new_v4()).bind(lot).bind(if quantity > Decimal::ZERO { "reserved" } else { "released" }).bind(quantity.abs()).bind(reservation).bind(now).execute(conn).await.map_err(map_db_error)?;
    }
    Ok(())
}

/// Allocation whose lot metadata was already updated by the lot command.
#[allow(clippy::too_many_arguments)]
pub(super) async fn allocate_recorded_lot(
    tx: &mut Transaction<'_, Postgres>,
    location: i32,
    sku: &str,
    lot: Uuid,
    quantity: Decimal,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    if quantity.is_zero() {
        return Ok(());
    }
    let conn = tx.as_mut();
    let warehouse = warehouse_of(conn, location).await?;
    lock_balance(conn, warehouse, sku, now).await?;
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
    shelf_delta(conn, location, &movement, Decimal::ZERO, quantity, now).await?;
    warehouse_delta(conn, warehouse, &movement, Decimal::ZERO, quantity, now).await?;
    Ok(())
}

/// A bin identifies a physical location, but can contain several lots. Post each
/// exact lot slice and record their total as one bin movement in the same transaction.
pub(crate) async fn post_from_slices(
    tx: &mut Transaction<'_, Postgres>,
    movement: &StockMove<'_>,
    now: DateTime<Utc>,
) -> Result<()> {
    movement.validate()?;
    let Some(source) = movement.from else {
        return post(tx, movement, now).await;
    };
    if movement.lot_id.is_some() || !movement.release_reserved.is_zero() {
        return post(tx, movement, now).await;
    }
    let rows = sqlx::query_as::<_, (Uuid, Decimal, Decimal)>("SELECT lot_id, quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = $1 AND sku = $2 ORDER BY lot_id").bind(source).bind(movement.sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
        slice.lot_id = (!key.is_nil()).then_some(key);
        post_inner(tx, &slice, now, true, first).await?;
        first = false;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(CommerceError::InsufficientStock {
            sku: movement.sku.into(),
            requested: movement.quantity.to_string(),
            available: (movement.quantity - remaining).to_string(),
        });
    }
    sqlx::query("UPDATE inventory_bin_movements SET quantity = $1 WHERE id = $2")
        .bind(movement.ledger_quantity())
        .bind(movement.id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    Ok(())
}
