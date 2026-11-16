//! Eligibility checks run on the same transaction as stock allocation or movement.
use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use stateset_core::{CommerceError, Result};
use uuid::Uuid;

/// Serialize hold creation with eligibility checks, including an absent hold.
pub(super) async fn lock_hold_scope(conn: &mut PgConnection, sku: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 731049))")
        .bind(sku)
        .execute(conn)
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn ensure_sellable(
    conn: &mut PgConnection,
    sku: &str,
    mut lot_id: Option<Uuid>,
    serial: Option<&str>,
    location: Option<i32>,
) -> Result<()> {
    if let Some(serial) = serial {
        let (serial_sku, status, serial_lot, serial_location): (String, String, Option<Uuid>, Option<i32>) = sqlx::query_as(
            "SELECT sku, status, lot_id, current_location_id FROM serial_numbers WHERE serial = $1 FOR SHARE")
            .bind(serial).fetch_optional(&mut *conn).await.map_err(map_db_error)?.ok_or(CommerceError::NotFound)?;
        if serial_sku != sku
            || !matches!(status.as_str(), "available" | "reserved")
            || (lot_id != serial_lot)
            || (location.is_some() && location != serial_location)
        {
            return Err(CommerceError::Conflict(
                "Serial is unavailable or does not match the requested SKU, lot, and location"
                    .into(),
            ));
        }
        let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM receipt_serials r JOIN serial_numbers s ON s.id = r.serial_id WHERE s.serial = $1 AND r.put_away_id IS NULL)").bind(serial).fetch_one(&mut *conn).await.map_err(map_db_error)?;
        if pending {
            return Err(CommerceError::Conflict(
                "Receipt serial is awaiting physical put-away".into(),
            ));
        }
        lot_id = serial_lot.or(lot_id);
    }
    let lot_number = if let Some(id) = lot_id {
        let (lot_sku, number, status, expiry): (String, String, String, Option<DateTime<Utc>>) =
            sqlx::query_as(
                "SELECT sku, lot_number, status, expiration_date FROM lots WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        if lot_sku != sku || status != "active" || expiry.is_some_and(|expiry| expiry <= Utc::now())
        {
            return Err(CommerceError::Conflict(
                "Lot is blocked, expired, or belongs to another SKU".into(),
            ));
        }
        Some(number)
    } else {
        None
    };
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1, 731049))")
        .bind(sku)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    let held: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM quality_holds WHERE sku = $1 AND released_at IS NULL
         AND (expires_at IS NULL OR expires_at > $5)
         AND (lot_number IS NULL OR $2::text IS NULL OR lot_number = $2)
         AND (serial_number IS NULL OR $3::text IS NULL OR serial_number = $3)
         AND (location_id IS NULL OR $4::integer IS NULL OR location_id = $4))",
    )
    .bind(sku)
    .bind(lot_number)
    .bind(serial)
    .bind(location)
    .bind(Utc::now())
    .fetch_one(conn)
    .await
    .map_err(map_db_error)?;
    if held {
        return Err(CommerceError::Conflict("Stock is blocked by an active quality hold".into()));
    }
    Ok(())
}

pub(super) async fn ensure_pick_location(
    conn: &mut PgConnection,
    warehouse: i32,
    location: i32,
) -> Result<()> {
    let valid: Option<i32> = sqlx::query_scalar("SELECT l.id FROM locations l JOIN warehouses w ON w.id = l.warehouse_id WHERE l.id = $1 AND w.id = $2 AND l.is_active AND l.is_pickable AND w.is_active FOR SHARE OF l, w")
        .bind(location).bind(warehouse).fetch_optional(conn).await.map_err(map_db_error)?;
    if valid.is_none() {
        return Err(CommerceError::Conflict(
            "Pick source must be active, pickable, and in the specified warehouse".into(),
        ));
    }
    Ok(())
}
