//! Lot stock postings and ownership of physical reservations/quarantine.
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, Lot, MovementType};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn ensure_unheld(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    location: i32,
    now: DateTime<Utc>,
) -> DbResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1, 731049))")
        .bind(&lot.sku)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let held = sqlx::query_scalar::<_, i32>("SELECT COUNT(*)::integer FROM quality_holds WHERE sku = $1 AND released_at IS NULL AND (expires_at IS NULL OR expires_at > $4) AND (lot_number IS NULL OR lot_number = $2) AND (location_id IS NULL OR location_id = $3)").bind(&lot.sku).bind(&lot.lot_number).bind(location).bind(now).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if held > 0 {
        return Err(invalid("Lot stock is blocked by an active quality hold"));
    }
    Ok(())
}
pub(super) async fn slices(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    warehouse: Option<i32>,
    now: DateTime<Utc>,
) -> DbResult<Vec<(i32, Decimal, Decimal)>> {
    let _ = now;
    sqlx::query_as::<_, (i32, Decimal, Decimal)>("SELECT li.location_id, li.quantity_on_hand, li.quantity_reserved FROM location_inventory li JOIN locations l ON l.id = li.location_id WHERE li.lot_id = $1 AND li.sku = $2 AND ($3::integer IS NULL OR l.warehouse_id = $3) ORDER BY l.warehouse_id, li.location_id").bind(lot.id).bind(&lot.sku).bind(warehouse).fetch_all(tx.as_mut()).await.map_err(map_db_error)
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
pub(super) async fn change(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    warehouse: Option<i32>,
    delta: Decimal,
    placement_recorded: bool,
    reason: &str,
    now: DateTime<Utc>,
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
                let placements = sqlx::query_scalar::<_, i32>(
                    "SELECT location_id FROM lot_locations WHERE lot_id = $1 ORDER BY location_id",
                )
                .bind(lot.id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                if placements.len() != 1 {
                    return Err(invalid(
                        "A positive lot adjustment requires an unambiguous warehouse location",
                    ));
                }
                placements[0]
            }
        };
        super::stock_placement::ensure_warehouse(tx, warehouse, now).await?;
        posting.to = Some(
            super::stock_move::role_location(tx.as_mut(), warehouse, "unassigned", now).await?,
        );
        super::stock_move::post(tx, &posting, now).await?;
        return Ok(());
    }
    let mut remaining = -delta;
    for (location, on_hand, reserved) in slices(tx, lot, warehouse, now).await? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now).await?;
        posting.id = Uuid::new_v4();
        posting.from = Some(location);
        posting.quantity = take;
        super::stock_move::post(tx, &posting, now).await?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid(
            "Insufficient unreserved physical lot stock; receive or reconcile the lot first",
        ));
    }
    Ok(())
}
pub(super) async fn record_hold(
    tx: &mut Transaction<'_, Postgres>,
    lot: Uuid,
    owner: Uuid,
    location: i32,
    quantity: Decimal,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let existing = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM lot_stock_holds WHERE owner_id = $1 AND lot_id = $2 AND location_id = $3 FOR UPDATE").bind(owner).bind(lot).bind(location).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
    let total =
        existing.checked_add(quantity).ok_or_else(|| invalid("Lot hold quantity overflow"))?;
    if total < Decimal::ZERO {
        return Err(invalid("Lot hold ownership is inconsistent"));
    }
    sqlx::query("INSERT INTO lot_stock_holds (owner_id, lot_id, location_id, quantity) VALUES ($1, $2, $3, $4) ON CONFLICT(owner_id, lot_id, location_id) DO UPDATE SET quantity = excluded.quantity").bind(owner).bind(lot).bind(location).bind(total).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
pub(super) async fn allocate(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    owner: Uuid,
    quantity: Decimal,
    check_quality: bool,
    reason: &str,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let mut remaining = quantity;
    let stock = slices(tx, lot, None, now).await?;
    if !check_quality && stock.is_empty() && unplaced(tx, lot.id).await? {
        // A lot registered ahead of receipt can be blocked before it arrives.
        return Ok(());
    }
    for (location, on_hand, reserved) in stock {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        if check_quality {
            ensure_unheld(tx, lot, location, now).await?;
        }
        super::stock_move::allocate_recorded_lot(tx, location, &lot.sku, lot.id, take, reason, now)
            .await?;
        record_hold(tx, lot.id, owner, location, take, now).await?;
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
pub(super) async fn finish_hold(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    owner: Uuid,
    quantity: Decimal,
    consume: bool,
    quarantine: bool,
    reason: &str,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let rows = sqlx::query_as::<_, (i32, Decimal)>("SELECT location_id, quantity FROM lot_stock_holds WHERE owner_id = $1 AND lot_id = $2 ORDER BY location_id FOR UPDATE").bind(owner).bind(lot.id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let owned: Decimal = rows.iter().map(|row| row.1).sum();
    if owner == lot.id && !consume && rows.is_empty() && unplaced(tx, lot.id).await? {
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
            ensure_unheld(tx, lot, location, now).await?;
            let mut posting = movement(lot, take, reason);
            posting.from = Some(location);
            posting.release_reserved = take;
            posting.movement_type = MovementType::Shipment;
            super::stock_move::post(tx, &posting, now).await?;
        } else if quarantine {
            record_hold(tx, lot.id, lot.id, location, take, now).await?;
        } else {
            super::stock_move::allocate_recorded_lot(
                tx, location, &lot.sku, lot.id, -take, reason, now,
            )
            .await?;
        }
        record_hold(tx, lot.id, owner, location, -take, now).await?;
    }
    Ok(())
}

async fn unplaced(tx: &mut Transaction<'_, Postgres>, lot: Uuid) -> DbResult<bool> {
    sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM lot_locations WHERE lot_id = $1)")
        .bind(lot)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)
}
pub(super) async fn transfer(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    from_warehouse: i32,
    to_warehouse: i32,
    quantity: Decimal,
    reason: &str,
    now: DateTime<Utc>,
) -> DbResult<()> {
    if from_warehouse == to_warehouse {
        return Err(invalid("Lot transfer requires distinct warehouses"));
    }
    super::stock_placement::ensure_warehouse(tx, to_warehouse, now).await?;
    let destination =
        super::stock_move::role_location(tx.as_mut(), to_warehouse, "unassigned", now).await?;
    let mut remaining = quantity;
    for (location, on_hand, reserved) in slices(tx, lot, Some(from_warehouse), now).await? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now).await?;
        let mut posting = movement(lot, take, reason);
        posting.from = Some(location);
        posting.to = Some(destination);
        posting.movement_type = MovementType::Transfer;
        super::stock_move::post(tx, &posting, now).await?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient quantity at source location for lot transfer"));
    }
    Ok(())
}
pub(super) async fn relabel(
    tx: &mut Transaction<'_, Postgres>,
    lot: &Lot,
    target: &Lot,
    quantity: Decimal,
    now: DateTime<Utc>,
) -> DbResult<()> {
    super::stock_cost::relabel(tx, lot.id, target.id, quantity, now).await?;
    let mut remaining = quantity;
    for (location, on_hand, reserved) in slices(tx, lot, None, now).await? {
        let take = remaining.min(on_hand - reserved);
        if take <= Decimal::ZERO {
            continue;
        }
        ensure_unheld(tx, lot, location, now).await?;
        let mut outgoing = movement(lot, take, "Lot genealogy: source");
        outgoing.reference_type = Some("lot_relabel");
        outgoing.from = Some(location);
        super::stock_move::post(tx, &outgoing, now).await?;
        let mut incoming = movement(target, take, "Lot genealogy: destination");
        incoming.reference_type = Some("lot_relabel");
        incoming.to = Some(location);
        super::stock_move::post(tx, &incoming, now).await?;
        remaining -= take;
    }
    if !remaining.is_zero() {
        return Err(invalid("Insufficient unreserved physical stock for lot split or merge"));
    }
    Ok(())
}
