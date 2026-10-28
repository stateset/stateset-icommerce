//! Materialize legacy warehouse-only stock into explicit physical custody.
//! Existing projections are checked; conflicting totals require reconciliation.
use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::CommerceError;
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn ensure_warehouse(
    tx: &mut Transaction<'_, Postgres>,
    warehouse: i32,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let existing = sqlx::query_scalar::<_, i32>("SELECT id FROM warehouses WHERE id = $1")
        .bind(warehouse)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    if existing.is_none() {
        let identity =
            sqlx::query_scalar::<_, String>("SELECT name FROM inventory_locations WHERE id = $1")
                .bind(warehouse)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or_else(|| invalid("Unknown stock warehouse"))?;
        sqlx::query("LOCK TABLE warehouses IN SHARE ROW EXCLUSIVE MODE")
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        // A well-formed empty address: warehouse reads require every field.
        let address = serde_json::to_value(stateset_core::WarehouseAddress::default())
            .map_err(|e| invalid(&e.to_string()))?;
        sqlx::query("INSERT INTO warehouses (id, code, name, address_json, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5) ON CONFLICT(id) DO NOTHING").bind(warehouse).bind(format!("__inventory_{warehouse}")).bind(identity).bind(sqlx::types::Json(address)).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
        sqlx::query("SELECT setval(pg_get_serial_sequence('warehouses', 'id'), GREATEST((SELECT COALESCE(MAX(id), 1) FROM warehouses), (SELECT last_value FROM warehouses_id_seq)))").execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    let (code, name) =
        sqlx::query_as::<_, (String, String)>("SELECT code, name FROM warehouses WHERE id = $1")
            .bind(warehouse)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    sqlx::query("INSERT INTO inventory_locations (id, code, name) VALUES ($1, $2, $3) ON CONFLICT(id) DO NOTHING").bind(warehouse).bind(code).bind(name).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
pub(super) async fn materialize(
    tx: &mut Transaction<'_, Postgres>,
    warehouse: i32,
    sku: &str,
    now: DateTime<Utc>,
) -> DbResult<()> {
    ensure_warehouse(tx, warehouse, now).await?;
    let balance = sqlx::query_scalar::<_, Decimal>("SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1 AND b.location_id = $2").bind(sku).bind(warehouse).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
    let bins = sqlx::query_scalar::<_, i32>("SELECT b.id FROM warehouse_bins b JOIN inventory_bin_levels l ON l.bin_id = b.id LEFT JOIN stock_location_bins m ON m.bin_id = b.id WHERE b.warehouse_id = $1 AND l.sku = $2 AND m.location_id IS NULL ORDER BY b.id").bind(warehouse).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    for bin in bins {
        super::stock_move::location_for_bin(tx.as_mut(), bin, now).await?;
    }
    let lots = sqlx::query_as::<_, (Uuid, Decimal)>("SELECT ll.lot_id, ll.quantity FROM lot_locations ll JOIN lots l ON l.id = ll.lot_id WHERE ll.location_id = $1 AND l.sku = $2 ORDER BY ll.lot_id").bind(warehouse).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    for (lot, placed) in lots {
        let physical = sqlx::query_scalar::<_, Decimal>("SELECT li.quantity_on_hand FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = $1 AND li.sku = $2 AND li.lot_id = $3").bind(warehouse).bind(sku).bind(lot).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let physical: Decimal = physical.into_iter().sum();
        if physical > placed {
            return Err(invalid(
                "Physical lot stock exceeds its placement; reconcile before posting",
            ));
        }
        if placed > physical {
            baseline(tx, warehouse, sku, Some(lot), placed - physical, now).await?;
        }
    }
    let rows = sqlx::query_as::<_, (i32, Decimal)>("SELECT li.location_id, li.quantity_on_hand FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE l.warehouse_id = $1 AND li.sku = $2 ORDER BY li.location_id, li.lot_id").bind(warehouse).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let total: Decimal = rows.iter().map(|row| row.1).sum();
    if total > balance {
        return Err(invalid(
            "Physical stock exceeds the warehouse balance; reconcile before posting",
        ));
    }
    if balance > total {
        baseline(tx, warehouse, sku, None, balance - total, now).await?;
    }
    for (location, _) in rows {
        super::stock_move::bin_for_location(tx.as_mut(), location, now).await?;
    }
    Ok(())
}
pub(super) async fn baseline(
    tx: &mut Transaction<'_, Postgres>,
    warehouse: i32,
    sku: &str,
    lot: Option<Uuid>,
    quantity: Decimal,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let location =
        super::stock_move::role_location(tx.as_mut(), warehouse, "unassigned", now).await?;
    let bin = super::stock_move::bin_for_location(tx.as_mut(), location, now).await?;
    let lot_key = lot.unwrap_or_else(Uuid::nil);
    let current = sqlx::query_scalar::<_, Decimal>("SELECT quantity_on_hand FROM location_inventory WHERE location_id = $1 AND sku = $2 AND lot_id = $3").bind(location).bind(sku).bind(lot_key).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
    sqlx::query("INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at) VALUES ($1, $2, $3, $4, '0', $5) ON CONFLICT(location_id, sku, lot_id) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, updated_at = excluded.updated_at").bind(location).bind(sku).bind(lot_key).bind(current+quantity).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;

    let current = sqlx::query_scalar::<_, Decimal>(
        "SELECT quantity_on_hand FROM inventory_bin_levels WHERE bin_id = $1 AND sku = $2",
    )
    .bind(bin.id)
    .bind(sku)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .unwrap_or_default();
    sqlx::query("INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at) VALUES ($1, $2, $3, '0', $4) ON CONFLICT(bin_id, sku) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand, updated_at = excluded.updated_at").bind(bin.id).bind(sku).bind(current+quantity).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn adjust(
    tx: &mut Transaction<'_, Postgres>,
    sku: &str,
    warehouse: i32,
    delta: Decimal,
    kind: stateset_core::MovementType,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    reason: &str,
    materialize_existing: bool,
    now: DateTime<Utc>,
) -> DbResult<()> {
    if delta.is_zero() {
        return Ok(());
    }
    ensure_warehouse(tx, warehouse, now).await?;
    if materialize_existing {
        materialize(tx, warehouse, sku, now).await?;
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
        movement.to = Some(
            super::stock_move::role_location(tx.as_mut(), warehouse, "unassigned", now).await?,
        );
        super::stock_move::post_physical(tx, &movement, now).await?;
        return Ok(());
    }
    let rows = sqlx::query_as::<_, (i32, Uuid, Decimal, Decimal)>("SELECT li.location_id, li.lot_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE li.sku = $1 AND l.warehouse_id = $2 AND l.is_active = TRUE AND l.is_pickable = TRUE ORDER BY li.location_id, li.lot_id").bind(sku).bind(warehouse).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut remaining = -delta;
    for (location, key, on_hand, reserved) in rows {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        movement.from = Some(location);
        movement.lot_id = (!key.is_nil()).then_some(key);
        match super::stock_guard::ensure_sellable(
            tx.as_mut(),
            sku,
            movement.lot_id,
            None,
            Some(location),
        )
        .await
        {
            Ok(()) => {}
            Err(CommerceError::Conflict(_)) | Err(CommerceError::NotPermitted(_)) => continue,
            Err(error) => return Err(error),
        }
        movement.id = Uuid::new_v4();
        movement.quantity = take;
        super::stock_move::post_physical(tx, &movement, now).await?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient eligible physical stock"));
    }
    Ok(())
}
