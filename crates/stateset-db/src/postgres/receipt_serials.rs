//! Serial provenance is recorded in the receipt transaction and assigned at put-away.
use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, ReceiveItemLine};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn register(
    tx: &mut Transaction<'_, Postgres>,
    line: &ReceiveItemLine,
    received_by: Option<&str>,
    now: DateTime<Utc>,
) -> DbResult<Option<String>> {
    let (sku, previous_lot, previous_serials, previous_quantity) = sqlx::query_as::<_, (String, Option<String>, Option<String>, Decimal)>("SELECT sku, lot_number, serial_numbers, received_quantity FROM receipt_items WHERE id = $1").bind(line.receipt_item_id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if previous_quantity > Decimal::ZERO
        && line.lot_number.as_ref().is_some_and(|lot| Some(lot) != previous_lot.as_ref())
    {
        return Err(invalid("A partially received line cannot change lot identity"));
    }
    let Some(serials) = line.serial_numbers.as_ref() else {
        if previous_serials.is_some() {
            return Err(invalid("Every additional serialized receipt requires serial numbers"));
        }
        return Ok(None);
    };
    if previous_quantity > Decimal::ZERO && previous_serials.is_none() {
        return Err(invalid(
            "Cannot add serial tracking after nonserialized quantities were received",
        ));
    }
    if serials.is_empty() || Decimal::from(serials.len() as u64) != line.quantity_received {
        return Err(invalid("Received serial count must equal the received quantity"));
    }
    let mut unique = std::collections::HashSet::new();
    for serial in serials {
        if serial.trim().is_empty()
            || serial != serial.trim()
            || serial.contains(',')
            || !unique.insert(serial)
        {
            return Err(invalid(
                "Receipt serial numbers must be nonempty, unique, and contain no commas or surrounding whitespace",
            ));
        }
    }
    let lot_number = line.lot_number.as_ref().or(previous_lot.as_ref());
    let lot_id = if let Some(number) = lot_number {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM lots WHERE lot_number = $1 AND sku = $2")
            .bind(number)
            .bind(&sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| {
                invalid("Register the matching receipt lot before receiving serialized stock")
            })?
            .into()
    } else {
        None
    };
    for serial in serials {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO serial_numbers (id, serial, sku, status, lot_id, lot_number, received_at, created_at, updated_at) VALUES ($1, $2, $3, 'available', $4, $5, $6, $6, $6)").bind(id).bind(serial).bind(&sku).bind(lot_id).bind(lot_number).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
        sqlx::query("INSERT INTO receipt_serials (serial_id, receipt_item_id) VALUES ($1, $2)")
            .bind(id)
            .bind(line.receipt_item_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, performed_by, notes, created_at) VALUES ($1, $2, 'received', 'receipt_item', $3, 'available', 'available', $4, 'Received; awaiting put-away', $5)").bind(Uuid::new_v4()).bind(id).bind(line.receipt_item_id).bind(received_by).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    let mut all = previous_serials.unwrap_or_default();
    if !all.is_empty() {
        all.push(',');
    }
    all.push_str(&serials.join(","));
    Ok(Some(all))
}
pub(super) async fn place(
    tx: &mut Transaction<'_, Postgres>,
    item: Uuid,
    put_away: Uuid,
    quantity: Decimal,
    location: i32,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let count = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*)::INTEGER FROM receipt_serials WHERE receipt_item_id = $1",
    )
    .bind(item)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if count == 0 {
        return Ok(());
    }
    if !quantity.fract().is_zero() {
        return Err(invalid("Serialized put-away quantity must be a whole number"));
    }
    let pending = sqlx::query_scalar::<_, Uuid>("SELECT serial_id FROM receipt_serials WHERE receipt_item_id = $1 AND put_away_id IS NULL ORDER BY serial_id FOR UPDATE").bind(item).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if Decimal::from(pending.len() as u64) < quantity {
        return Err(invalid("Not enough unplaced receipt serials"));
    }
    let mut placed = Decimal::ZERO;
    for serial in pending {
        if placed == quantity {
            break;
        }
        sqlx::query(
            "UPDATE serial_numbers SET current_location_id = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(location)
        .bind(now)
        .bind(serial)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query("UPDATE receipt_serials SET put_away_id = $1 WHERE serial_id = $2")
            .bind(put_away)
            .bind(serial)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, to_location_id, created_at) VALUES ($1, $2, 'location_changed', 'put_away', $3, 'available', 'available', $4, $5)").bind(Uuid::new_v4()).bind(serial).bind(put_away).bind(location).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
        placed += Decimal::ONE;
    }
    Ok(())
}
