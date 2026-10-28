//! Serial provenance is recorded in the receipt transaction and assigned at put-away.
use rusqlite::{OptionalExtension, Transaction};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, ReceiveItemLine};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn register(
    tx: &Transaction<'_>,
    line: &ReceiveItemLine,
    received_by: Option<&str>,
    now: &str,
) -> DbResult<Option<String>> {
    let (sku, previous_lot, previous_serials, previous_quantity) = tx.query_row("SELECT sku, lot_number, serial_numbers, received_quantity FROM receipt_items WHERE id = ?1", rusqlite::params![(line.receipt_item_id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?)))?;
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
        tx.query_row(
            "SELECT id FROM lots WHERE lot_number = ?1 AND sku = ?2",
            rusqlite::params![number, &sku],
            |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
        )
        .optional()?
        .ok_or_else(|| {
            invalid("Register the matching receipt lot before receiving serialized stock")
        })?
        .into()
    } else {
        None
    };
    for serial in serials {
        let id = Uuid::new_v4();
        tx.execute("INSERT INTO serial_numbers (id, serial, sku, status, lot_id, lot_number, received_at, created_at, updated_at) VALUES (?1, ?2, ?3, 'available', ?4, ?5, ?6, ?6, ?6)", rusqlite::params![(id).to_string(), serial, &sku, (lot_id).map(|id| id.to_string()), lot_number, now])?;
        tx.execute(
            "INSERT INTO receipt_serials (serial_id, receipt_item_id) VALUES (?1, ?2)",
            rusqlite::params![(id).to_string(), (line.receipt_item_id).to_string()],
        )?;
        tx.execute("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, performed_by, notes, created_at) VALUES (?1, ?2, 'received', 'receipt_item', ?3, 'available', 'available', ?4, 'Received; awaiting put-away', ?5)", rusqlite::params![(Uuid::new_v4()).to_string(), (id).to_string(), (line.receipt_item_id).to_string(), received_by, now])?;
    }
    let mut all = previous_serials.unwrap_or_default();
    if !all.is_empty() {
        all.push(',');
    }
    all.push_str(&serials.join(","));
    Ok(Some(all))
}
pub(super) fn place(
    tx: &Transaction<'_>,
    item: Uuid,
    put_away: Uuid,
    quantity: Decimal,
    location: i32,
    now: &str,
) -> DbResult<()> {
    let count = tx.query_row(
        "SELECT COUNT(*) FROM receipt_serials WHERE receipt_item_id = ?1",
        rusqlite::params![(item).to_string()],
        |r| r.get::<_, i32>(0),
    )?;
    if count == 0 {
        return Ok(());
    }
    if !quantity.fract().is_zero() {
        return Err(invalid("Serialized put-away quantity must be a whole number"));
    }
    let pending = {
        let mut stmt = tx.prepare("SELECT serial_id FROM receipt_serials WHERE receipt_item_id = ?1 AND put_away_id IS NULL ORDER BY serial_id")?;
        let rows = stmt.query_map(rusqlite::params![(item).to_string()], |r| {
            super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if Decimal::from(pending.len() as u64) < quantity {
        return Err(invalid("Not enough unplaced receipt serials"));
    }
    let mut placed = Decimal::ZERO;
    for serial in pending {
        if placed == quantity {
            break;
        }
        tx.execute(
            "UPDATE serial_numbers SET current_location_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![location, now, (serial).to_string()],
        )?;
        tx.execute(
            "UPDATE receipt_serials SET put_away_id = ?1 WHERE serial_id = ?2",
            rusqlite::params![(put_away).to_string(), (serial).to_string()],
        )?;
        tx.execute("INSERT INTO serial_history (id, serial_id, event_type, reference_type, reference_id, from_status, to_status, to_location_id, created_at) VALUES (?1, ?2, 'location_changed', 'put_away', ?3, 'available', 'available', ?4, ?5)", rusqlite::params![(Uuid::new_v4()).to_string(), (serial).to_string(), (put_away).to_string(), location, now])?;
        placed += Decimal::ONE;
    }
    Ok(())
}
