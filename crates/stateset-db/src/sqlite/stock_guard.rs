//! Eligibility checks run on the same transaction as stock allocation or movement.
use super::{map_db_error, parse_datetime_opt_row, parse_uuid_opt_row};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use stateset_core::{CommerceError, Result};
use uuid::Uuid;

/// Missing detail is conservative: an unqualified allocation cannot bypass a
/// hold whose lot, serial or location it has not identified.
pub(super) fn ensure_sellable(
    conn: &Connection,
    sku: &str,
    mut lot_id: Option<Uuid>,
    serial: Option<&str>,
    location: Option<i32>,
) -> Result<()> {
    if let Some(serial) = serial {
        let (serial_sku, status, serial_lot, serial_location): (String, String, Option<Uuid>, Option<i32>) = conn.query_row(
            "SELECT sku, status, lot_id, current_location_id FROM serial_numbers WHERE serial = ?1", [serial],
            |r| Ok((r.get(0)?, r.get(1)?, parse_uuid_opt_row(r.get(2)?, "serial", "lot_id")?, r.get(3)?)))
            .optional().map_err(map_db_error)?.ok_or(CommerceError::NotFound)?;
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
        let pending: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM receipt_serials r JOIN serial_numbers s ON s.id = r.serial_id WHERE s.serial = ?1 AND r.put_away_id IS NULL)", [serial], |r| r.get(0)).map_err(map_db_error)?;
        if pending {
            return Err(CommerceError::Conflict(
                "Receipt serial is awaiting physical put-away".into(),
            ));
        }
        lot_id = serial_lot.or(lot_id);
    }
    let lot_number = if let Some(id) = lot_id {
        let (lot_sku, number, status, expiry): (
            String,
            String,
            String,
            Option<chrono::DateTime<Utc>>,
        ) = conn
            .query_row(
                "SELECT sku, lot_number, status, expiration_date FROM lots WHERE id = ?1",
                [id.to_string()],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        parse_datetime_opt_row(r.get(3)?, "lot", "expiration_date")?,
                    ))
                },
            )
            .optional()
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
    let held: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM quality_holds WHERE sku = ?1 AND released_at IS NULL
         AND (expires_at IS NULL OR julianday(expires_at) > julianday(?5))
         AND (lot_number IS NULL OR ?2 IS NULL OR lot_number = ?2)
         AND (serial_number IS NULL OR ?3 IS NULL OR serial_number = ?3)
         AND (location_id IS NULL OR ?4 IS NULL OR location_id = ?4))",
            params![sku, lot_number, serial, location, Utc::now().to_rfc3339()],
            |r| r.get(0),
        )
        .map_err(map_db_error)?;
    if held {
        return Err(CommerceError::Conflict("Stock is blocked by an active quality hold".into()));
    }
    Ok(())
}

pub(super) fn ensure_pick_location(conn: &Connection, warehouse: i32, location: i32) -> Result<()> {
    let valid: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM locations l JOIN warehouses w ON w.id = l.warehouse_id WHERE l.id = ?1 AND w.id = ?2 AND l.is_active = 1 AND l.is_pickable = 1 AND w.is_active = 1)", params![location, warehouse], |r| r.get(0)).map_err(map_db_error)?;
    if !valid {
        return Err(CommerceError::Conflict(
            "Pick source must be active, pickable, and in the specified warehouse".into(),
        ));
    }
    Ok(())
}
