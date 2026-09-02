//! SQLite implementation of Serial Number Repository

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, Row, params};
use stateset_core::CommerceError;
use stateset_core::{
    BatchResult, ChangeSerialStatus, CreateSerialNumber, CreateSerialNumbersBulk, LotRepository,
    MoveSerial, ReserveSerialNumber, SerialEventType, SerialFilter, SerialHistory,
    SerialHistoryFilter, SerialLookupResult, SerialNumber, SerialRepository, SerialReservation,
    SerialStatus, SerialValidation, TransferSerialOwnership, UpdateSerialNumber, WarrantyId,
    WarrantyLookupStatus, WarrantyRepository,
};
use uuid::Uuid;

use super::{
    SqliteLotRepository, SqliteWarrantyRepository, build_in_clause, map_db_error, params_refs,
    parse_datetime_opt_row, parse_datetime_row, parse_enum_row, parse_json_opt_row,
    parse_uuid_opt_row, parse_uuid_row, string_params, uuid_params,
};

/// SQLite implementation of `SerialRepository`
#[derive(Debug)]
pub struct SqliteSerialRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSerialRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Build the shared `WHERE` conditions (and their bound params) for serial
    /// queries, so `list` and `count` filter identically — a divergence between
    /// them was exactly how `count` came to ignore most filters.
    fn serial_filter_conditions(
        filter: &SerialFilter,
    ) -> (Vec<String>, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(serial) = &filter.serial {
            conditions.push("serial = ?".to_string());
            params.push(Box::new(serial.clone()));
        }
        if let Some(prefix) = &filter.serial_prefix {
            conditions.push("serial LIKE ?".to_string());
            params.push(Box::new(format!("{prefix}%")));
        }
        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?".to_string());
            params.push(Box::new(sku.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(statuses) = &filter.statuses {
            let placeholders = build_in_clause(statuses.len());
            conditions.push(format!("status IN ({placeholders})"));
            for s in statuses {
                params.push(Box::new(s.to_string()));
            }
        }
        if let Some(lot_id) = &filter.lot_id {
            conditions.push("lot_id = ?".to_string());
            params.push(Box::new(lot_id.to_string()));
        }
        if let Some(lot_number) = &filter.lot_number {
            conditions.push("lot_number = ?".to_string());
            params.push(Box::new(lot_number.clone()));
        }
        if let Some(loc_id) = filter.location_id {
            conditions.push("current_location_id = ?".to_string());
            params.push(Box::new(loc_id));
        }
        if let Some(owner_id) = &filter.owner_id {
            conditions.push("current_owner_id = ?".to_string());
            params.push(Box::new(owner_id.to_string()));
        }
        if let Some(owner_type) = &filter.owner_type {
            conditions.push("current_owner_type = ?".to_string());
            params.push(Box::new(owner_type.clone()));
        }
        if let Some(warranty_id) = &filter.warranty_id {
            conditions.push("warranty_id = ?".to_string());
            params.push(Box::new(warranty_id.to_string()));
        }
        if let Some(has_warranty) = filter.has_warranty {
            if has_warranty {
                conditions.push("warranty_id IS NOT NULL".to_string());
            } else {
                conditions.push("warranty_id IS NULL".to_string());
            }
        }
        if let Some(after) = &filter.manufactured_after {
            conditions.push("manufactured_at >= ?".to_string());
            params.push(Box::new(after.to_rfc3339()));
        }
        if let Some(before) = &filter.manufactured_before {
            conditions.push("manufactured_at <= ?".to_string());
            params.push(Box::new(before.to_rfc3339()));
        }
        if let Some(after) = &filter.sold_after {
            conditions.push("sold_at >= ?".to_string());
            params.push(Box::new(after.to_rfc3339()));
        }
        if let Some(before) = &filter.sold_before {
            conditions.push("sold_at <= ?".to_string());
            params.push(Box::new(before.to_rfc3339()));
        }

        (conditions, params)
    }

    fn map_serial_row(row: &Row<'_>) -> Result<SerialNumber, rusqlite::Error> {
        let status_str: String = row.get("status")?;
        let attributes_str: Option<String> = row.get("attributes")?;

        Ok(SerialNumber {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial", "id")?,
            serial: row.get("serial")?,
            sku: row.get("sku")?,
            status: parse_enum_row(&status_str, "serial", "status")?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("lot_id")?,
                "serial",
                "lot_id",
            )?,
            lot_number: row.get("lot_number")?,
            current_location_id: row.get("current_location_id")?,
            current_owner_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("current_owner_id")?,
                "serial",
                "current_owner_id",
            )?,
            current_owner_type: row.get("current_owner_type")?,
            warranty_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("warranty_id")?,
                "serial",
                "warranty_id",
            )?,
            manufactured_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("manufactured_at")?,
                "serial",
                "manufactured_at",
            )?,
            received_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("received_at")?,
                "serial",
                "received_at",
            )?,
            sold_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("sold_at")?,
                "serial",
                "sold_at",
            )?,
            activated_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("activated_at")?,
                "serial",
                "activated_at",
            )?,
            last_service_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("last_service_at")?,
                "serial",
                "last_service_at",
            )?,
            notes: row.get("notes")?,
            attributes: parse_json_opt_row(attributes_str, "serial", "attributes")?
                .unwrap_or(serde_json::Value::Null),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "serial",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "serial",
                "updated_at",
            )?,
        })
    }

    fn map_history_row(row: &Row<'_>) -> Result<SerialHistory, rusqlite::Error> {
        let event_type_str: String = row.get("event_type")?;
        let from_status_str: String = row.get("from_status")?;
        let to_status_str: String = row.get("to_status")?;

        Ok(SerialHistory {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial_history", "id")?,
            serial_id: parse_uuid_row(
                &row.get::<_, String>("serial_id")?,
                "serial_history",
                "serial_id",
            )?,
            event_type: parse_enum_row(&event_type_str, "serial_history", "event_type")?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("reference_id")?,
                "serial_history",
                "reference_id",
            )?,
            from_status: parse_enum_row(&from_status_str, "serial_history", "from_status")?,
            to_status: parse_enum_row(&to_status_str, "serial_history", "to_status")?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            from_owner_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("from_owner_id")?,
                "serial_history",
                "from_owner_id",
            )?,
            to_owner_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("to_owner_id")?,
                "serial_history",
                "to_owner_id",
            )?,
            performed_by: row.get("performed_by")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "serial_history",
                "created_at",
            )?,
        })
    }

    fn map_reservation_row(row: &Row<'_>) -> Result<SerialReservation, rusqlite::Error> {
        Ok(SerialReservation {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial_reservation", "id")?,
            serial_id: parse_uuid_row(
                &row.get::<_, String>("serial_id")?,
                "serial_reservation",
                "serial_id",
            )?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_row(
                &row.get::<_, String>("reference_id")?,
                "serial_reservation",
                "reference_id",
            )?,
            reserved_by: row.get("reserved_by")?,
            reserved_at: parse_datetime_row(
                &row.get::<_, String>("reserved_at")?,
                "serial_reservation",
                "reserved_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expires_at")?,
                "serial_reservation",
                "expires_at",
            )?,
            confirmed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("confirmed_at")?,
                "serial_reservation",
                "confirmed_at",
            )?,
            released_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("released_at")?,
                "serial_reservation",
                "released_at",
            )?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_history(
        conn: &rusqlite::Connection,
        serial: &SerialNumber,
        event_type: SerialEventType,
        from_status: SerialStatus,
        to_status: SerialStatus,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        from_location_id: Option<i32>,
        to_location_id: Option<i32>,
        from_owner_id: Option<Uuid>,
        to_owner_id: Option<Uuid>,
        performed_by: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, from_location_id, to_location_id,
                from_owner_id, to_owner_id, performed_by, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                serial.id.to_string(),
                event_type.to_string(),
                reference_type,
                reference_id.map(|id| id.to_string()),
                from_status.to_string(),
                to_status.to_string(),
                from_location_id,
                to_location_id,
                from_owner_id.map(|id| id.to_string()),
                to_owner_id.map(|id| id.to_string()),
                performed_by,
                notes,
                now,
            ],
        )?;

        Ok(())
    }

    /// Load a serial inside a transaction, mapping a missing row to `NotFound`.
    fn load_in_tx(tx: &rusqlite::Transaction<'_>, id: Uuid) -> stateset_core::Result<SerialNumber> {
        tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        )
        .optional()
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)
    }

    /// Load a reservation inside a transaction, mapping a missing row to `NotFound`.
    fn load_reservation_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: Uuid,
    ) -> stateset_core::Result<SerialReservation> {
        tx.query_row(
            "SELECT * FROM serial_reservations WHERE id = ?",
            params![id.to_string()],
            Self::map_reservation_row,
        )
        .optional()
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)
    }

    /// Close every open reservation on a serial (released/consumed/swept all
    /// look the same in the store: `released_at` set, `active_key` cleared so
    /// the unique backstop index frees the slot).
    fn close_open_reservations(
        tx: &rusqlite::Transaction<'_>,
        serial_id: Uuid,
        now: &str,
    ) -> stateset_core::Result<usize> {
        tx.execute(
            "UPDATE serial_reservations SET released_at = ?, active_key = NULL
             WHERE serial_id = ? AND released_at IS NULL",
            params![now, serial_id.to_string()],
        )
        .map_err(map_db_error)
    }

    /// Move `serial` to `to` — the ONE place a status is written.
    ///
    /// Checks the state machine ([`SerialStatus::allowed_transitions`]), then
    /// writes `status` conditionally on the status the caller observed
    /// (`WHERE id = ? AND status = ?`) so a concurrent writer that got there
    /// first is detected instead of overwritten. Leaving `Reserved` consumes
    /// any open reservation in the same transaction. `extra_set` is appended
    /// to the `SET` list (leading comma included) with `extra_params` bound
    /// in order after `status`/`updated_at`.
    fn write_transition(
        tx: &rusqlite::Transaction<'_>,
        serial: &SerialNumber,
        to: SerialStatus,
        now: &str,
        extra_set: &str,
        extra_params: &[&dyn rusqlite::ToSql],
    ) -> stateset_core::Result<()> {
        serial.ensure_can_transition_to(to)?;
        let sql = format!(
            "UPDATE serial_numbers SET status = ?, updated_at = ?{extra_set} WHERE id = ? AND status = ?"
        );
        let to_str = to.to_string();
        let id_str = serial.id.to_string();
        let from_str = serial.status.to_string();
        let mut bound: Vec<&dyn rusqlite::ToSql> = vec![&to_str, &now];
        bound.extend_from_slice(extra_params);
        bound.push(&id_str);
        bound.push(&from_str);
        let rows = tx.execute(&sql, bound.as_slice()).map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::Conflict(format!(
                "Serial {} ({}) changed concurrently while moving from {} to {}",
                serial.serial, serial.id, serial.status, to
            )));
        }
        if serial.status == SerialStatus::Reserved {
            Self::close_open_reservations(tx, serial.id, now)?;
        }
        Ok(())
    }

    /// Quarantine every `Available` / `Reserved` serial of `lot_id` on the
    /// caller's transaction, closing open reservations; returns how many moved.
    ///
    /// This is the serial half of a lot quarantine: `SqliteLotRepository`
    /// and the quality repository call it inside the transaction that flips
    /// the lot, so a quarantined lot can never leave a sellable serial behind.
    /// Shipped / sold / already-quarantined serials are untouched.
    pub(crate) fn quarantine_for_lot_on(
        tx: &rusqlite::Transaction<'_>,
        lot_id: Uuid,
        reason: &str,
        now: &str,
    ) -> stateset_core::Result<u64> {
        let candidates: Vec<SerialNumber> = {
            let mut stmt = tx
                .prepare(
                    "SELECT * FROM serial_numbers WHERE lot_id = ? AND status IN (?, ?)
                     ORDER BY created_at ASC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map(
                    params![
                        lot_id.to_string(),
                        SerialStatus::Available.to_string(),
                        SerialStatus::Reserved.to_string()
                    ],
                    Self::map_serial_row,
                )
                .map_err(map_db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_db_error)?
        };

        let mut count = 0u64;
        for serial in candidates {
            Self::write_transition(tx, &serial, SerialStatus::Quarantined, now, "", &[])?;
            Self::record_history(
                tx,
                &serial,
                SerialEventType::Quarantined,
                serial.status,
                SerialStatus::Quarantined,
                Some("lot"),
                Some(lot_id),
                None,
                None,
                None,
                None,
                None,
                Some(reason),
            )
            .map_err(map_db_error)?;
            count += 1;
        }
        Ok(count)
    }

    /// Return every `Quarantined` serial of `lot_id` to `Available` on the
    /// caller's transaction; the counterpart of [`Self::quarantine_for_lot_on`].
    pub(crate) fn release_quarantine_for_lot_on(
        tx: &rusqlite::Transaction<'_>,
        lot_id: Uuid,
        now: &str,
    ) -> stateset_core::Result<u64> {
        let candidates: Vec<SerialNumber> = {
            let mut stmt = tx
                .prepare(
                    "SELECT * FROM serial_numbers WHERE lot_id = ? AND status = ?
                     ORDER BY created_at ASC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map(
                    params![lot_id.to_string(), SerialStatus::Quarantined.to_string()],
                    Self::map_serial_row,
                )
                .map_err(map_db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_db_error)?
        };

        let mut count = 0u64;
        for serial in candidates {
            Self::write_transition(tx, &serial, SerialStatus::Available, now, "", &[])?;
            Self::record_history(
                tx,
                &serial,
                SerialEventType::QuarantineReleased,
                SerialStatus::Quarantined,
                SerialStatus::Available,
                Some("lot"),
                Some(lot_id),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .map_err(map_db_error)?;
            count += 1;
        }
        Ok(count)
    }

    fn generate_serial(&self, prefix: Option<&str>) -> String {
        let unique_part = Uuid::new_v4().to_string().replace('-', "").to_uppercase();
        let short = &unique_part[..12];
        match prefix {
            Some(p) => format!("{p}-{short}"),
            None => format!("SN-{short}"),
        }
    }
}

impl SerialRepository for SqliteSerialRepository {
    fn create(&self, input: CreateSerialNumber) -> stateset_core::Result<SerialNumber> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let serial = input.serial.unwrap_or_else(|| self.generate_serial(None));
        let attributes = input.attributes.unwrap_or(serde_json::Value::Null);

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO serial_numbers (
                    id, serial, sku, status, lot_id, lot_number, current_location_id,
                    manufactured_at, notes, attributes, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    serial,
                    input.sku,
                    SerialStatus::Available.to_string(),
                    input.lot_id.map(|id| id.to_string()),
                    input.lot_number,
                    input.location_id,
                    input.manufactured_at.map(|dt| dt.to_rfc3339()),
                    input.notes,
                    serde_json::to_string(&attributes).ok(),
                    now,
                    now,
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn create_bulk(
        &self,
        input: CreateSerialNumbersBulk,
    ) -> stateset_core::Result<Vec<SerialNumber>> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(input.quantity as usize);
        let now = Utc::now().to_rfc3339();

        for i in 0..input.quantity {
            let id = Uuid::new_v4();
            let serial_number = match &input.prefix {
                Some(prefix) => format!("{}-{:06}", prefix, i + 1),
                None => self.generate_serial(None),
            };

            tx.execute(
                "INSERT INTO serial_numbers (
                    id, serial, sku, status, lot_id, lot_number, current_location_id,
                    manufactured_at, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    serial_number,
                    input.sku,
                    SerialStatus::Available.to_string(),
                    input.lot_id.map(|id| id.to_string()),
                    input.lot_number,
                    input.location_id,
                    input.manufactured_at.map(|dt| dt.to_rfc3339()),
                    now,
                    now,
                ],
            )
            .map_err(map_db_error)?;

            // Record creation history
            let history_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO serial_history (
                    id, serial_id, event_type, from_status, to_status, created_at
                ) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    history_id.to_string(),
                    id.to_string(),
                    SerialEventType::Created.to_string(),
                    SerialStatus::Available.to_string(),
                    SerialStatus::Available.to_string(),
                    now,
                ],
            )
            .map_err(map_db_error)?;

            serials.push(SerialNumber {
                id,
                serial: serial_number,
                sku: input.sku.clone(),
                status: SerialStatus::Available,
                lot_id: input.lot_id,
                lot_number: input.lot_number.clone(),
                current_location_id: input.location_id,
                current_owner_id: None,
                current_owner_type: None,
                warranty_id: None,
                manufactured_at: input.manufactured_at,
                received_at: None,
                sold_at: None,
                activated_at: None,
                last_service_at: None,
                notes: None,
                attributes: serde_json::Value::Null,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(serials)
    }

    fn get(&self, id: Uuid) -> stateset_core::Result<Option<SerialNumber>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        );

        match result {
            Ok(serial) => Ok(Some(serial)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_serial(&self, serial: &str) -> stateset_core::Result<Option<SerialNumber>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT * FROM serial_numbers WHERE serial = ?",
            params![serial],
            Self::map_serial_row,
        );

        match result {
            Ok(serial) => Ok(Some(serial)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateSerialNumber) -> stateset_core::Result<SerialNumber> {
        let now = Utc::now().to_rfc3339();

        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
            let serial = Self::load_in_tx(&tx, id)?;

            let mut updates = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.clone())];

            // A status change through `update` is a state-machine transition
            // like any other; a same-status "change" is a no-op.
            let status_change = match input.status {
                Some(to) if to != serial.status => {
                    serial.ensure_can_transition_to(to)?;
                    updates.push("status = ?".to_string());
                    params.push(Box::new(to.to_string()));
                    Some(to)
                }
                _ => None,
            };
            if let Some(loc) = input.location_id {
                updates.push("current_location_id = ?".to_string());
                params.push(Box::new(loc));
            }
            if let Some(lot_id) = &input.lot_id {
                updates.push("lot_id = ?".to_string());
                params.push(Box::new(lot_id.to_string()));
            }
            if let Some(warranty_id) = &input.warranty_id {
                updates.push("warranty_id = ?".to_string());
                params.push(Box::new(warranty_id.to_string()));
            }
            if let Some(notes) = &input.notes {
                updates.push("notes = ?".to_string());
                params.push(Box::new(notes.clone()));
            }
            if let Some(attrs) = &input.attributes {
                updates.push("attributes = ?".to_string());
                params.push(Box::new(serde_json::to_string(attrs).unwrap_or_default()));
            }

            params.push(Box::new(id.to_string()));
            params.push(Box::new(serial.status.to_string()));

            let sql = format!(
                "UPDATE serial_numbers SET {} WHERE id = ? AND status = ?",
                updates.join(", ")
            );
            let rows = tx
                .execute(
                    &sql,
                    rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
                )
                .map_err(map_db_error)?;
            if rows != 1 {
                return Err(CommerceError::Conflict(format!(
                    "Serial {} ({}) changed concurrently during update",
                    serial.serial, serial.id
                )));
            }

            if let Some(to) = status_change {
                if serial.status == SerialStatus::Reserved {
                    Self::close_open_reservations(&tx, serial.id, &now)?;
                }
                Self::record_history(
                    &tx,
                    &serial,
                    SerialEventType::StatusChanged,
                    serial.status,
                    to,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .map_err(map_db_error)?;
            }

            tx.commit().map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: SerialFilter) -> stateset_core::Result<Vec<SerialNumber>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let (conditions, mut params) = Self::serial_filter_conditions(&filter);

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM serial_numbers {where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
                Self::map_serial_row,
            )
            .map_err(map_db_error)?;

        let mut serials = Vec::new();
        for row in rows {
            serials.push(row.map_err(map_db_error)?);
        }

        Ok(serials)
    }

    fn delete(&self, id: Uuid) -> stateset_core::Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Check if serial can be deleted (only if Available and never used)
        let serial = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if serial.status != SerialStatus::Available {
            return Err(CommerceError::ValidationError(
                "Can only delete serials with 'available' status".to_string(),
            ));
        }

        // Check if there's any history beyond creation
        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM serial_history WHERE serial_id = ? AND event_type != ?",
                params![id.to_string(), SerialEventType::Created.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if history_count > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete serial with transaction history".to_string(),
            ));
        }

        // Delete history first
        conn.execute("DELETE FROM serial_history WHERE serial_id = ?", params![id.to_string()])
            .map_err(map_db_error)?;

        // Delete reservations
        conn.execute(
            "DELETE FROM serial_reservations WHERE serial_id = ?",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        // Delete serial
        conn.execute("DELETE FROM serial_numbers WHERE id = ?", params![id.to_string()])
            .map_err(map_db_error)?;

        Ok(())
    }

    fn change_status(&self, input: ChangeSerialStatus) -> stateset_core::Result<SerialNumber> {
        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
            let now = Utc::now().to_rfc3339();

            let serial = Self::load_in_tx(&tx, input.serial_id)?;
            let owner_id = input.owner_id.map(|id| id.to_string());

            Self::write_transition(
                &tx,
                &serial,
                input.new_status,
                &now,
                ", current_location_id = COALESCE(?, current_location_id),
                   current_owner_id = COALESCE(?, current_owner_id),
                   current_owner_type = COALESCE(?, current_owner_type)",
                &[&input.location_id, &owner_id, &input.owner_type],
            )?;

            Self::record_history(
                &tx,
                &serial,
                SerialEventType::StatusChanged,
                serial.status,
                input.new_status,
                input.reference_type.as_deref(),
                input.reference_id,
                serial.current_location_id,
                input.location_id,
                serial.current_owner_id,
                input.owner_id,
                input.performed_by.as_deref(),
                input.notes.as_deref(),
            )
            .map_err(map_db_error)?;

            tx.commit().map_err(map_db_error)?;
        }
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn reserve(&self, input: ReserveSerialNumber) -> stateset_core::Result<SerialReservation> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let mut serial = Self::load_in_tx(&tx, input.serial_id)?;

        // Lazy expiry: a serial still `Reserved` by an expired, unconfirmed
        // reservation is returned to stock in-line so the sweeper is not on the
        // critical path of the next order.
        if serial.status == SerialStatus::Reserved {
            let stale = tx
                .query_row(
                    "SELECT * FROM serial_reservations
                     WHERE serial_id = ? AND released_at IS NULL AND confirmed_at IS NULL
                       AND expires_at IS NOT NULL AND expires_at <= ?
                     ORDER BY reserved_at DESC LIMIT 1",
                    params![input.serial_id.to_string(), now_str],
                    Self::map_reservation_row,
                )
                .optional()
                .map_err(map_db_error)?;
            if let Some(stale) = stale {
                Self::write_transition(&tx, &serial, SerialStatus::Available, &now_str, "", &[])?;
                Self::record_history(
                    &tx,
                    &serial,
                    SerialEventType::Released,
                    SerialStatus::Reserved,
                    SerialStatus::Available,
                    Some(&stale.reference_type),
                    Some(stale.reference_id),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("reservation expired"),
                )
                .map_err(map_db_error)?;
                serial.status = SerialStatus::Available;
            } else {
                return Err(CommerceError::Conflict(format!(
                    "Serial {} ({}) already has an open reservation",
                    serial.serial, serial.id
                )));
            }
        }
        serial.ensure_can_transition_to(SerialStatus::Reserved)?;

        // Legacy rows (pre-lifecycle reservations that were never consumed)
        // must not hold the backstop key against this new reservation.
        Self::close_open_reservations(&tx, serial.id, &now_str)?;

        let id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds.map(|secs| now + chrono::Duration::seconds(secs));

        tx.execute(
            "INSERT INTO serial_reservations (
                id, serial_id, reference_type, reference_id, reserved_by,
                reserved_at, expires_at, active_key
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                input.serial_id.to_string(),
                input.reference_type,
                input.reference_id.to_string(),
                input.reserved_by,
                now_str,
                expires_at.map(|dt| dt.to_rfc3339()),
                input.serial_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        Self::write_transition(&tx, &serial, SerialStatus::Reserved, &now_str, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Reserved,
            SerialStatus::Available,
            SerialStatus::Reserved,
            Some(&input.reference_type),
            Some(input.reference_id),
            None,
            None,
            None,
            None,
            input.reserved_by.as_deref(),
            None,
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(SerialReservation {
            id,
            serial_id: input.serial_id,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            reserved_by: input.reserved_by,
            reserved_at: now,
            expires_at,
            confirmed_at: None,
            released_at: None,
        })
    }

    fn release_reservation(&self, reservation_id: Uuid) -> stateset_core::Result<()> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let reservation = Self::load_reservation_in_tx(&tx, reservation_id)?;
        if reservation.released_at.is_some() {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} is already closed (released, consumed or expired)"
            )));
        }

        // The reservation may only be released while it still holds the serial.
        // Once the unit shipped or sold the reservation was consumed; releasing
        // it now must not flip a shipped unit back to `available`.
        let serial = Self::load_in_tx(&tx, reservation.serial_id)?;
        if serial.status != SerialStatus::Reserved {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} cannot be released: serial {} ({}) is {}",
                serial.serial, serial.id, serial.status
            )));
        }

        let rows = tx
            .execute(
                "UPDATE serial_reservations SET released_at = ?, active_key = NULL
                 WHERE id = ? AND released_at IS NULL",
                params![now, reservation_id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} was closed concurrently"
            )));
        }

        Self::write_transition(&tx, &serial, SerialStatus::Available, &now, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Released,
            SerialStatus::Reserved,
            SerialStatus::Available,
            Some(&reservation.reference_type),
            Some(reservation.reference_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> stateset_core::Result<()> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let reservation = Self::load_reservation_in_tx(&tx, reservation_id)?;
        if reservation.released_at.is_some() {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} is already closed (released, consumed or expired)"
            )));
        }
        if reservation.confirmed_at.is_some() {
            return Ok(()); // Already confirmed — idempotent.
        }
        if reservation.expires_at.is_some_and(|expires| now > expires) {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} expired at {:?} and cannot be confirmed",
                reservation.expires_at
            )));
        }

        let serial = Self::load_in_tx(&tx, reservation.serial_id)?;
        if serial.status != SerialStatus::Reserved {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} cannot be confirmed: serial {} ({}) is {}",
                serial.serial, serial.id, serial.status
            )));
        }

        let rows = tx
            .execute(
                "UPDATE serial_reservations SET confirmed_at = ?
                 WHERE id = ? AND released_at IS NULL AND confirmed_at IS NULL",
                params![now.to_rfc3339(), reservation_id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} changed concurrently"
            )));
        }

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_reservation(
        &self,
        reservation_id: Uuid,
    ) -> stateset_core::Result<Option<SerialReservation>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.query_row(
            "SELECT * FROM serial_reservations WHERE id = ?",
            params![reservation_id.to_string()],
            Self::map_reservation_row,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn release_expired_reservations(
        &self,
        now: chrono::DateTime<Utc>,
    ) -> stateset_core::Result<u64> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now_str = now.to_rfc3339();

        let expired: Vec<SerialReservation> = {
            let mut stmt = tx
                .prepare(
                    "SELECT * FROM serial_reservations
                     WHERE released_at IS NULL AND confirmed_at IS NULL
                       AND expires_at IS NOT NULL AND expires_at <= ?
                     ORDER BY reserved_at ASC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map(params![now_str], Self::map_reservation_row)
                .map_err(map_db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_db_error)?
        };

        let mut returned_to_stock = 0u64;
        for reservation in expired {
            tx.execute(
                "UPDATE serial_reservations SET released_at = ?, active_key = NULL
                 WHERE id = ? AND released_at IS NULL",
                params![now_str, reservation.id.to_string()],
            )
            .map_err(map_db_error)?;

            let serial = Self::load_in_tx(&tx, reservation.serial_id)?;
            if serial.status != SerialStatus::Reserved {
                continue; // The unit already moved on; only the stale row needed closing.
            }
            Self::write_transition(&tx, &serial, SerialStatus::Available, &now_str, "", &[])?;
            Self::record_history(
                &tx,
                &serial,
                SerialEventType::Released,
                SerialStatus::Reserved,
                SerialStatus::Available,
                Some(&reservation.reference_type),
                Some(reservation.reference_id),
                None,
                None,
                None,
                None,
                None,
                Some("reservation expired"),
            )
            .map_err(map_db_error)?;
            returned_to_stock += 1;
        }

        tx.commit().map_err(map_db_error)?;
        Ok(returned_to_stock)
    }

    fn move_serial(&self, input: MoveSerial) -> stateset_core::Result<SerialNumber> {
        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
            let now = Utc::now().to_rfc3339();

            // Get current serial
            let serial: SerialNumber = tx
                .query_row(
                    "SELECT * FROM serial_numbers WHERE id = ?",
                    params![input.serial_id.to_string()],
                    Self::map_serial_row,
                )
                .map_err(map_db_error)?;

            let from_location = serial.current_location_id;

            // Update location
            tx.execute(
                "UPDATE serial_numbers SET current_location_id = ?, updated_at = ? WHERE id = ?",
                params![input.to_location_id, now, input.serial_id.to_string()],
            )
            .map_err(map_db_error)?;

            // Record history
            let history_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO serial_history (
                    id, serial_id, event_type, from_status, to_status,
                    from_location_id, to_location_id, performed_by, notes, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    history_id.to_string(),
                    input.serial_id.to_string(),
                    SerialEventType::LocationChanged.to_string(),
                    serial.status.to_string(),
                    serial.status.to_string(),
                    from_location,
                    input.to_location_id,
                    input.performed_by,
                    input.notes,
                    now,
                ],
            )
            .map_err(map_db_error)?;

            tx.commit().map_err(map_db_error)?;
        }
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn transfer_ownership(
        &self,
        input: TransferSerialOwnership,
    ) -> stateset_core::Result<SerialNumber> {
        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
            let now = Utc::now().to_rfc3339();

            let serial = Self::load_in_tx(&tx, input.serial_id)?;
            let new_owner = input.new_owner_id.to_string();

            Self::write_transition(
                &tx,
                &serial,
                SerialStatus::Transferred,
                &now,
                ", current_owner_id = ?, current_owner_type = ?",
                &[&new_owner, &input.new_owner_type],
            )?;

            Self::record_history(
                &tx,
                &serial,
                SerialEventType::Transferred,
                serial.status,
                SerialStatus::Transferred,
                input.reference_type.as_deref(),
                input.reference_id,
                None,
                None,
                serial.current_owner_id,
                Some(input.new_owner_id),
                None,
                input.notes.as_deref(),
            )
            .map_err(map_db_error)?;

            tx.commit().map_err(map_db_error)?;
        }
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> stateset_core::Result<SerialNumber> {
        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
            let now = Utc::now().to_rfc3339();

            let serial = Self::load_in_tx(&tx, id)?;
            let customer = customer_id.to_string();

            // Selling consumes the open reservation (write_transition closes it
            // when leaving `Reserved`).
            Self::write_transition(
                &tx,
                &serial,
                SerialStatus::Sold,
                &now,
                ", current_owner_id = ?, current_owner_type = 'customer', sold_at = ?",
                &[&customer, &now],
            )?;

            Self::record_history(
                &tx,
                &serial,
                SerialEventType::Sold,
                serial.status,
                SerialStatus::Sold,
                order_id.map(|_| "order"),
                order_id,
                None,
                None,
                None,
                Some(customer_id),
                None,
                None,
            )
            .map_err(map_db_error)?;

            tx.commit().map_err(map_db_error)?;
        }
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let serial = Self::load_in_tx(&tx, id)?;

        // Shipping consumes the open reservation (see write_transition).
        Self::write_transition(&tx, &serial, SerialStatus::Shipped, &now, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Shipped,
            serial.status,
            SerialStatus::Shipped,
            Some("shipment"),
            Some(shipment_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let serial = Self::load_in_tx(&tx, id)?;

        Self::write_transition(
            &tx,
            &serial,
            SerialStatus::Returned,
            &now,
            ", current_owner_id = NULL, current_owner_type = NULL",
            &[],
        )?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Returned,
            serial.status,
            SerialStatus::Returned,
            Some("return"),
            Some(return_id),
            None,
            None,
            serial.current_owner_id,
            None,
            None,
            None,
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn activate(&self, id: Uuid) -> stateset_core::Result<SerialNumber> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE serial_numbers SET activated_at = ?, updated_at = ? WHERE id = ? AND activated_at IS NULL",
            params![now, now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        if let Some(serial) = self.get(id)? {
            let history_id = Uuid::new_v4();
            conn.execute(
                "INSERT INTO serial_history (
                    id, serial_id, event_type, from_status, to_status, created_at
                ) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    history_id.to_string(),
                    id.to_string(),
                    SerialEventType::Activated.to_string(),
                    serial.status.to_string(),
                    serial.status.to_string(),
                    now,
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let serial = Self::load_in_tx(&tx, id)?;

        Self::write_transition(&tx, &serial, SerialStatus::Quarantined, &now, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Quarantined,
            serial.status,
            SerialStatus::Quarantined,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(reason),
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn release_quarantine(&self, id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let serial = Self::load_in_tx(&tx, id)?;
        if serial.status != SerialStatus::Quarantined {
            return Err(CommerceError::Conflict(format!(
                "Serial {} ({}) is not quarantined (status: {}); cannot release to available",
                serial.serial, serial.id, serial.status
            )));
        }

        Self::write_transition(&tx, &serial, SerialStatus::Available, &now, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::QuarantineReleased,
            SerialStatus::Quarantined,
            SerialStatus::Available,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn quarantine_for_lot(&self, lot_id: Uuid, reason: &str) -> stateset_core::Result<u64> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let count = Self::quarantine_for_lot_on(&tx, lot_id, reason, &Utc::now().to_rfc3339())?;
        tx.commit().map_err(map_db_error)?;
        Ok(count)
    }

    fn release_quarantine_for_lot(&self, lot_id: Uuid) -> stateset_core::Result<u64> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let count = Self::release_quarantine_for_lot_on(&tx, lot_id, &Utc::now().to_rfc3339())?;
        tx.commit().map_err(map_db_error)?;
        Ok(count)
    }

    fn scrap(&self, id: Uuid, reason: &str) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        let serial = Self::load_in_tx(&tx, id)?;

        Self::write_transition(&tx, &serial, SerialStatus::Scrapped, &now, "", &[])?;

        Self::record_history(
            &tx,
            &serial,
            SerialEventType::Scrapped,
            serial.status,
            SerialStatus::Scrapped,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(reason),
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> stateset_core::Result<Vec<SerialHistory>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut conditions = vec!["serial_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(serial_id.to_string())];

        if let Some(event_type) = &filter.event_type {
            conditions.push("event_type = ?".to_string());
            params.push(Box::new(event_type.to_string()));
        }
        if let Some(ref_type) = &filter.reference_type {
            conditions.push("reference_type = ?".to_string());
            params.push(Box::new(ref_type.clone()));
        }
        if let Some(from_date) = &filter.from_date {
            conditions.push("created_at >= ?".to_string());
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            conditions.push("created_at <= ?".to_string());
            params.push(Box::new(to_date.to_rfc3339()));
        }

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM serial_history WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
                Self::map_history_row,
            )
            .map_err(map_db_error)?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row.map_err(map_db_error)?);
        }

        Ok(history)
    }

    fn lookup(&self, serial: &str) -> stateset_core::Result<Option<SerialLookupResult>> {
        let serial_number = match self.get_by_serial(serial)? {
            Some(s) => s,
            None => return Ok(None),
        };

        // Get recent history
        let recent_history = self.get_history(
            serial_number.id,
            SerialHistoryFilter { limit: Some(10), ..Default::default() },
        )?;

        let product_name = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let result: rusqlite::Result<String> = conn.query_row(
                "SELECT COALESCE(p.name, v.name)
                 FROM product_variants v
                 LEFT JOIN products p ON p.id = v.product_id
                 WHERE v.sku = ?",
                [serial_number.sku.as_str()],
                |row| row.get(0),
            );
            match result {
                Ok(name) => Some(name),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(map_db_error(e)),
            }
        };

        let lot = {
            let lot_repo = SqliteLotRepository::new(self.pool.clone());
            match (serial_number.lot_id, serial_number.lot_number.as_deref()) {
                (Some(lot_id), lot_number) => match lot_repo.get(lot_id)? {
                    Some(lot) => Some(lot),
                    None => match lot_number {
                        Some(number) => lot_repo.get_by_number(number)?,
                        None => None,
                    },
                },
                (None, Some(lot_number)) => lot_repo.get_by_number(lot_number)?,
                (None, None) => None,
            }
        };

        let warranty_status = if let Some(warranty_id) = serial_number.warranty_id {
            let warranty_repo = SqliteWarrantyRepository::new(self.pool.clone());
            match warranty_repo.get(WarrantyId::from(warranty_id))? {
                Some(warranty) => Some(WarrantyLookupStatus {
                    warranty_id,
                    is_active: warranty.is_valid(),
                    expires_at: warranty.end_date,
                    coverage_type: Some(warranty.warranty_type.to_string()),
                }),
                None => None,
            }
        } else {
            None
        };

        Ok(Some(SerialLookupResult {
            serial: serial_number,
            product_name,
            lot,
            warranty_status,
            recent_history,
        }))
    }

    fn validate(&self, serial: &str) -> stateset_core::Result<SerialValidation> {
        match self.get_by_serial(serial)? {
            Some(s) => Ok(SerialValidation {
                is_valid: true,
                serial_id: Some(s.id),
                status: Some(s.status),
                sku: Some(s.sku),
                error_message: None,
            }),
            None => Ok(SerialValidation {
                is_valid: false,
                serial_id: None,
                status: None,
                sku: None,
                error_message: Some("Serial number not found".to_string()),
            }),
        }
    }

    fn get_available_for_sku(
        &self,
        sku: &str,
        limit: u32,
    ) -> stateset_core::Result<Vec<SerialNumber>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Allocate the OLDEST available serial first (FIFO), matching Postgres.
        // This deliberately orders ASC, unlike `list`'s newest-first (DESC) view.
        let mut stmt = conn
            .prepare(
                "SELECT * FROM serial_numbers WHERE sku = ? AND status = ? \
                 ORDER BY created_at ASC LIMIT ?",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map(
                params![sku, SerialStatus::Available.to_string(), i64::from(limit)],
                Self::map_serial_row,
            )
            .map_err(map_db_error)?;

        let mut serials = Vec::new();
        for row in rows {
            serials.push(row.map_err(map_db_error)?);
        }
        Ok(serials)
    }

    fn get_for_lot(&self, lot_id: Uuid) -> stateset_core::Result<Vec<SerialNumber>> {
        self.list(SerialFilter { lot_id: Some(lot_id), ..Default::default() })
    }

    fn get_for_customer(&self, customer_id: Uuid) -> stateset_core::Result<Vec<SerialNumber>> {
        self.list(SerialFilter {
            owner_id: Some(customer_id),
            owner_type: Some("customer".to_string()),
            ..Default::default()
        })
    }

    fn count(&self, filter: SerialFilter) -> stateset_core::Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Reuse the same conditions as `list` so the two never diverge.
        let (conditions, params) = Self::serial_filter_conditions(&filter);

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) FROM serial_numbers {where_clause}");

        let count: i64 = conn
            .query_row(
                &sql,
                rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_batch(
        &self,
        inputs: Vec<CreateSerialNumber>,
    ) -> stateset_core::Result<BatchResult<SerialNumber>> {
        let mut result = BatchResult::with_capacity(inputs.len());

        for (idx, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(serial) => result.record_success(serial),
                Err(e) => result.record_failure(idx, None, &e),
            }
        }

        Ok(result)
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> stateset_core::Result<Vec<SerialNumber>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&ids);
        let params_ref = params_refs(&params);

        let sql = format!("SELECT * FROM serial_numbers WHERE id IN ({placeholders})");

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_ref), Self::map_serial_row)
            .map_err(map_db_error)?;

        let mut serials = Vec::new();
        for row in rows {
            serials.push(row.map_err(map_db_error)?);
        }

        Ok(serials)
    }

    fn get_batch_by_serial(
        &self,
        serials: Vec<String>,
    ) -> stateset_core::Result<Vec<SerialNumber>> {
        if serials.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let placeholders = build_in_clause(serials.len());
        let params = string_params(&serials);
        let params_ref = params_refs(&params);

        let sql = format!("SELECT * FROM serial_numbers WHERE serial IN ({placeholders})");

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_ref), Self::map_serial_row)
            .map_err(map_db_error)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(map_db_error)?);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use stateset_core::{
        ChangeSerialStatus, CreateSerialNumber, CreateSerialNumbersBulk, ReserveSerialNumber,
        SerialFilter, SerialRepository, SerialStatus, TransferSerialOwnership, UpdateSerialNumber,
    };

    fn fresh_repo() -> SqliteSerialRepository {
        SqliteDatabase::in_memory().expect("in-memory").serials()
    }

    /// A serial repo plus a real lot row (`serials.lot_id` is a foreign key).
    fn repo_with_lot(sku: &str) -> (SqliteSerialRepository, Uuid) {
        use stateset_core::{CreateLot, LotRepository};
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let lot = db
            .lots()
            .create(CreateLot {
                sku: sku.into(),
                lot_number: Some(format!("LOT-{sku}")),
                quantity: rust_decimal::Decimal::from(10),
                ..Default::default()
            })
            .expect("create lot");
        (db.serials(), lot.id)
    }

    fn make_serial(repo: &SqliteSerialRepository, sku: &str, serial: &str) -> SerialNumber {
        repo.create(CreateSerialNumber {
            serial: Some(serial.into()),
            sku: sku.into(),
            lot_id: None,
            lot_number: Some("LOT-1".into()),
            location_id: Some(1),
            manufactured_at: None,
            notes: None,
            attributes: None,
        })
        .expect("create")
    }

    #[test]
    fn create_with_explicit_serial_starts_available() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-A", "S-001");
        assert_eq!(s.serial, "S-001");
        assert_eq!(s.sku, "SKU-A");
        assert_eq!(s.status, SerialStatus::Available);
    }

    #[test]
    fn delete_rejects_missing_and_non_available_serials() {
        let repo = fresh_repo();

        // A non-existent serial is NotFound, not a silent success.
        let err = repo.delete(Uuid::new_v4()).expect_err("missing serial must error");
        assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

        // A non-Available serial cannot be deleted, and must survive the attempt.
        let s = make_serial(&repo, "SKU-DEL", "S-DEL-1");
        repo.update(
            s.id,
            UpdateSerialNumber { status: Some(SerialStatus::Sold), ..Default::default() },
        )
        .expect("update to sold");
        let err = repo.delete(s.id).expect_err("a sold serial must not be deletable");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
        assert!(
            repo.get(s.id).expect("get").is_some(),
            "a rejected delete must not remove the serial"
        );
    }

    #[test]
    fn count_matches_list_for_all_filters() {
        let repo = fresh_repo();
        make_serial(&repo, "SKU", "S-1");
        make_serial(&repo, "SKU", "S-2");

        // Every predicate `list` honors, `count` must honor too — `count(f)` must
        // equal `list(f).len()` for the same filter (they previously diverged
        // because `count` only applied sku/status/lot_id).
        let filters = [
            SerialFilter { serial: Some("S-1".into()), ..Default::default() },
            SerialFilter { serial_prefix: Some("S-1".into()), ..Default::default() },
            SerialFilter { statuses: Some(vec![SerialStatus::Sold]), ..Default::default() },
            SerialFilter { sku: Some("SKU".into()), ..Default::default() },
        ];
        for filter in filters {
            let listed = repo.list(filter.clone()).expect("list").len() as u64;
            let counted = repo.count(filter).expect("count");
            assert_eq!(counted, listed, "count must match list for the same filter");
        }

        // Sanity: the distinguishing filters select the right subset.
        assert_eq!(
            repo.count(SerialFilter { serial: Some("S-1".into()), ..Default::default() }).unwrap(),
            1
        );
        assert_eq!(
            repo.count(SerialFilter {
                statuses: Some(vec![SerialStatus::Sold]),
                ..Default::default()
            })
            .unwrap(),
            0
        );
        assert_eq!(
            repo.count(SerialFilter { sku: Some("SKU".into()), ..Default::default() }).unwrap(),
            2
        );
    }

    #[test]
    fn get_available_for_sku_allocates_oldest_first_fifo() {
        let repo = fresh_repo();
        let oldest = make_serial(&repo, "SKU-F", "S-OLD");
        let newest = make_serial(&repo, "SKU-F", "S-NEW");

        // FIFO: the oldest available serial is allocated first (matching Postgres,
        // which orders by created_at ASC). SQLite used to inherit list's DESC order
        // and hand out the newest unit.
        let one = repo.get_available_for_sku("SKU-F", 1).expect("available");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].id, oldest.id, "must allocate the oldest serial first (FIFO)");

        // The full set is ordered oldest → newest.
        let all = repo.get_available_for_sku("SKU-F", 10).expect("all");
        assert_eq!(
            all.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![oldest.id, newest.id],
            "available serials must be FIFO-ordered"
        );
    }

    #[test]
    fn create_with_no_serial_generates_unique_one() {
        let repo = fresh_repo();
        let s1 = repo
            .create(CreateSerialNumber {
                serial: None,
                sku: "SKU-X".into(),
                lot_id: None,
                lot_number: None,
                location_id: None,
                manufactured_at: None,
                notes: None,
                attributes: None,
            })
            .expect("c1");
        let s2 = repo
            .create(CreateSerialNumber {
                serial: None,
                sku: "SKU-X".into(),
                lot_id: None,
                lot_number: None,
                location_id: None,
                manufactured_at: None,
                notes: None,
                attributes: None,
            })
            .expect("c2");
        assert_ne!(s1.serial, s2.serial);
    }

    #[test]
    fn get_and_get_by_serial_round_trip() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-RT", "S-RT-1");
        let by_id = repo.get(s.id).expect("ok").expect("found");
        assert_eq!(by_id.id, s.id);
        let by_serial = repo.get_by_serial("S-RT-1").expect("ok").expect("found");
        assert_eq!(by_serial.id, s.id);
        assert!(repo.get_by_serial("missing").expect("ok").is_none());
    }

    #[test]
    fn create_bulk_creates_n_serials_with_prefix() {
        let repo = fresh_repo();
        let serials = repo
            .create_bulk(CreateSerialNumbersBulk {
                sku: "SKU-BULK".into(),
                quantity: 5,
                prefix: Some("BLK".into()),
                lot_id: None,
                lot_number: None,
                location_id: Some(1),
                manufactured_at: None,
            })
            .expect("bulk");
        assert_eq!(serials.len(), 5);
        for (i, s) in serials.iter().enumerate() {
            assert_eq!(s.serial, format!("BLK-{:06}", i + 1));
            assert_eq!(s.status, SerialStatus::Available);
        }
    }

    #[test]
    fn list_filters_by_sku() {
        let repo = fresh_repo();
        make_serial(&repo, "SKU-L1", "S-L1-1");
        make_serial(&repo, "SKU-L1", "S-L1-2");
        make_serial(&repo, "SKU-L2", "S-L2-1");

        let l1 = repo
            .list(SerialFilter { sku: Some("SKU-L1".into()), ..Default::default() })
            .expect("list");
        assert_eq!(l1.len(), 2);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let s_available = make_serial(&repo, "SKU-S", "S-AV");
        let s_to_reserve = make_serial(&repo, "SKU-S", "S-TR");
        repo.change_status(ChangeSerialStatus {
            serial_id: s_to_reserve.id,
            new_status: SerialStatus::Reserved,
            ..Default::default()
        })
        .expect("change status");

        let available = repo
            .list(SerialFilter { status: Some(SerialStatus::Available), ..Default::default() })
            .expect("list available");
        let reserved = repo
            .list(SerialFilter { status: Some(SerialStatus::Reserved), ..Default::default() })
            .expect("list reserved");
        let av_ids: Vec<_> = available.iter().map(|s| s.id).collect();
        let res_ids: Vec<_> = reserved.iter().map(|s| s.id).collect();
        assert!(av_ids.contains(&s_available.id));
        assert!(res_ids.contains(&s_to_reserve.id));
    }

    #[test]
    fn change_status_transitions() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-T", "S-T-1");
        let updated = repo
            .change_status(ChangeSerialStatus {
                serial_id: s.id,
                new_status: SerialStatus::Sold,
                performed_by: Some("alice".into()),
                ..Default::default()
            })
            .expect("change");
        assert_eq!(updated.status, SerialStatus::Sold);
    }

    #[test]
    fn reserve_serial_creates_reservation_and_changes_status() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-R", "S-R-1");
        let order_id = Uuid::new_v4();
        let res = repo
            .reserve(ReserveSerialNumber {
                serial_id: s.id,
                reference_type: "order".into(),
                reference_id: order_id,
                reserved_by: Some("alice".into()),
                expires_in_seconds: Some(60),
            })
            .expect("reserve");
        assert_eq!(res.serial_id, s.id);
        let after = repo.get(s.id).expect("ok").expect("found");
        assert_eq!(after.status, SerialStatus::Reserved);

        repo.release_reservation(res.id).expect("release");
        let after_release = repo.get(s.id).expect("ok").expect("found");
        assert_eq!(after_release.status, SerialStatus::Available);
    }

    /// Force a serial into an arbitrary status, bypassing the state machine,
    /// so tests can start from any point in the state space.
    fn force_status(repo: &SqliteSerialRepository, id: Uuid, status: SerialStatus) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "UPDATE serial_numbers SET status = ? WHERE id = ?",
            params![status.to_string(), id.to_string()],
        )
        .expect("force status");
    }

    fn reserve_input(serial_id: Uuid) -> ReserveSerialNumber {
        ReserveSerialNumber {
            serial_id,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            reserved_by: None,
            expires_in_seconds: None,
        }
    }

    // S1: one open reservation per serial ----------------------------------

    #[test]
    fn reserve_refuses_second_reservation_on_same_serial() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S1", "S-S1-1");
        let first = repo.reserve(reserve_input(s.id)).expect("first reserve");
        let err = repo.reserve(reserve_input(s.id)).expect_err("second reserve must fail");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        // The first reservation still holds the serial.
        let held = repo.get_reservation(first.id).expect("ok").expect("found");
        assert!(held.is_open());
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Reserved);
    }

    #[test]
    fn reserve_backstop_index_rejects_duplicate_open_reservation() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S1", "S-S1-2");
        repo.reserve(reserve_input(s.id)).expect("reserve");
        // A writer bypassing the repository (raw SQL) still cannot open a second
        // reservation on the serial: the unique index on `active_key` refuses it.
        let conn = repo.pool.get().expect("conn");
        let err = conn
            .execute(
                "INSERT INTO serial_reservations (id, serial_id, reference_type, reference_id, reserved_at, active_key)
                 VALUES (?, ?, 'order', ?, ?, ?)",
                params![
                    Uuid::new_v4().to_string(),
                    s.id.to_string(),
                    Uuid::new_v4().to_string(),
                    Utc::now().to_rfc3339(),
                    s.id.to_string()
                ],
            )
            .expect_err("duplicate open reservation must violate the unique index");
        assert!(err.to_string().contains("UNIQUE"), "got {err}");
    }

    // S2: the state machine is enforced by every status write ----------------

    #[test]
    fn mark_shipped_refuses_scrapped_serial() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S2", "S-S2-1");
        repo.scrap(s.id, "crushed").expect("scrap");
        let err = repo.mark_shipped(s.id, Uuid::new_v4()).expect_err("scrapped cannot ship");
        match err {
            CommerceError::Conflict(msg) => {
                assert!(msg.contains("scrapped") && msg.contains("shipped"), "{msg}");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Scrapped);
    }

    #[test]
    fn change_status_accepts_exactly_the_transition_table() {
        let repo = fresh_repo();
        for from in SerialStatus::ALL {
            for to in SerialStatus::ALL {
                let s = make_serial(&repo, "SKU-SM", &format!("S-SM-{from}-{to}"));
                force_status(&repo, s.id, from);
                let result = repo.change_status(ChangeSerialStatus {
                    serial_id: s.id,
                    new_status: to,
                    ..Default::default()
                });
                assert_eq!(
                    result.is_ok(),
                    from.can_transition_to(to),
                    "{from} -> {to}: got {result:?}"
                );
                let expected = if from.can_transition_to(to) { to } else { from };
                assert_eq!(repo.get(s.id).unwrap().unwrap().status, expected, "{from} -> {to}");
                if !from.can_transition_to(to) {
                    assert!(matches!(result, Err(CommerceError::Conflict(_))), "{from} -> {to}");
                }
            }
        }
    }

    #[test]
    fn every_status_write_path_enforces_the_table() {
        // The named mutations must agree with `change_status` on every source
        // status: a mutation is accepted iff the table lists its target.
        type Op = (&'static str, SerialStatus, fn(&SqliteSerialRepository, Uuid) -> bool);
        let ops: [Op; 7] = [
            ("mark_shipped", SerialStatus::Shipped, |r, id| {
                r.mark_shipped(id, Uuid::new_v4()).is_ok()
            }),
            ("mark_sold", SerialStatus::Sold, |r, id| {
                r.mark_sold(id, Uuid::new_v4(), None).is_ok()
            }),
            ("mark_returned", SerialStatus::Returned, |r, id| {
                r.mark_returned(id, Uuid::new_v4()).is_ok()
            }),
            ("quarantine", SerialStatus::Quarantined, |r, id| r.quarantine(id, "qc").is_ok()),
            ("release_quarantine", SerialStatus::Available, |r, id| {
                r.release_quarantine(id).is_ok()
            }),
            ("scrap", SerialStatus::Scrapped, |r, id| r.scrap(id, "bin").is_ok()),
            ("transfer_ownership", SerialStatus::Transferred, |r, id| {
                r.transfer_ownership(TransferSerialOwnership {
                    serial_id: id,
                    new_owner_id: Uuid::new_v4(),
                    new_owner_type: "partner".into(),
                    ..Default::default()
                })
                .is_ok()
            }),
        ];
        let repo = fresh_repo();
        for (name, target, op) in ops {
            for from in SerialStatus::ALL {
                let s = make_serial(&repo, "SKU-OP", &format!("S-OP-{name}-{from}"));
                force_status(&repo, s.id, from);
                let accepted = op(&repo, s.id);
                let expected = if name == "release_quarantine" {
                    // release_quarantine is the Quarantined -> Available edge only.
                    from == SerialStatus::Quarantined
                } else {
                    from.can_transition_to(target)
                };
                assert_eq!(accepted, expected, "{name} from {from}");
            }
        }
    }

    #[test]
    fn update_with_status_goes_through_the_state_machine() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-U", "S-U-1");
        repo.scrap(s.id, "gone").expect("scrap");
        let err = repo
            .update(
                s.id,
                UpdateSerialNumber { status: Some(SerialStatus::Available), ..Default::default() },
            )
            .expect_err("update cannot resurrect a scrapped serial");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        // Non-status updates on a terminal serial are still fine.
        let updated = repo
            .update(
                s.id,
                UpdateSerialNumber { notes: Some("audited".into()), ..Default::default() },
            )
            .expect("notes update");
        assert_eq!(updated.notes.as_deref(), Some("audited"));
        assert_eq!(updated.status, SerialStatus::Scrapped);
    }

    // S3: reservation lifecycle ---------------------------------------------

    #[test]
    fn mark_sold_closes_open_reservation_and_release_is_refused() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S3", "S-S3-1");
        let res = repo.reserve(reserve_input(s.id)).expect("reserve");
        repo.confirm_reservation(res.id).expect("confirm");
        let confirmed = repo.get_reservation(res.id).unwrap().unwrap();
        assert!(confirmed.is_confirmed() && confirmed.is_open());
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Reserved);

        repo.mark_sold(s.id, Uuid::new_v4(), None).expect("sell");
        let closed = repo.get_reservation(res.id).unwrap().unwrap();
        assert!(closed.released_at.is_some(), "sale must consume the reservation");

        let err =
            repo.release_reservation(res.id).expect_err("cannot release a consumed reservation");
        assert!(
            matches!(err, CommerceError::Conflict(_) | CommerceError::ValidationError(_)),
            "got {err:?}"
        );
        assert_eq!(
            repo.get(s.id).unwrap().unwrap().status,
            SerialStatus::Sold,
            "release after sale must not flip the serial back to available"
        );
    }

    #[test]
    fn mark_shipped_closes_open_reservation() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S3", "S-S3-2");
        let res = repo.reserve(reserve_input(s.id)).expect("reserve");
        repo.mark_shipped(s.id, Uuid::new_v4()).expect("ship");
        assert!(repo.get_reservation(res.id).unwrap().unwrap().released_at.is_some());
        assert!(repo.release_reservation(res.id).is_err());
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Shipped);
    }

    #[test]
    fn release_reservation_is_idempotent_guarded_and_returns_serial_to_stock() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S3", "S-S3-3");
        let res = repo.reserve(reserve_input(s.id)).expect("reserve");
        repo.release_reservation(res.id).expect("release");
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Available);
        assert!(repo.release_reservation(res.id).is_err(), "second release must be refused");
        // A fresh reservation is possible again.
        repo.reserve(reserve_input(s.id)).expect("re-reserve");
    }

    #[test]
    fn confirm_reservation_is_transactional_and_refuses_closed_or_expired() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-S3", "S-S3-4");
        let res = repo.reserve(reserve_input(s.id)).expect("reserve");
        repo.confirm_reservation(res.id).expect("confirm");
        repo.confirm_reservation(res.id).expect("confirm again is idempotent");
        repo.release_reservation(res.id).expect("release confirmed (order cancelled)");
        assert!(repo.confirm_reservation(res.id).is_err(), "cannot confirm a released reservation");

        // Expired, unconfirmed reservations cannot be confirmed.
        let s2 = make_serial(&repo, "SKU-S3", "S-S3-5");
        let expired = repo
            .reserve(ReserveSerialNumber { expires_in_seconds: Some(-1), ..reserve_input(s2.id) })
            .expect("reserve with past expiry");
        let err = repo.confirm_reservation(expired.id).expect_err("expired cannot be confirmed");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    /// Confirmation commits the reservation row but the unit has not moved,
    /// so an order cancelled after confirmation can still hand the serial
    /// back; only shipping / selling consumes the reservation for good.
    #[test]
    fn release_reservation_after_confirm_returns_serial_to_stock() {
        let repo = fresh_repo();
        let serial = make_serial(&repo, "SKU-RC", "SN-RC-1");
        let res = repo.reserve(reserve_input(serial.id)).expect("reserve");
        repo.confirm_reservation(res.id).expect("confirm");
        assert_eq!(repo.get(serial.id).unwrap().unwrap().status, SerialStatus::Reserved);
        repo.release_reservation(res.id).expect("release after confirm is allowed");
        assert_eq!(repo.get(serial.id).unwrap().unwrap().status, SerialStatus::Available);
        let closed = repo.get_reservation(res.id).unwrap().unwrap();
        assert!(closed.released_at.is_some() && closed.confirmed_at.is_some());
        // Once shipped, the reservation is consumed and release is refused.
        let shipped = make_serial(&repo, "SKU-RC", "SN-RC-2");
        let res2 = repo.reserve(reserve_input(shipped.id)).expect("reserve");
        repo.confirm_reservation(res2.id).expect("confirm");
        repo.mark_shipped(shipped.id, Uuid::new_v4()).expect("ship");
        assert!(matches!(repo.release_reservation(res2.id), Err(CommerceError::Conflict(_))));
    }

    #[test]
    fn release_expired_reservations_returns_serials_to_available() {
        let repo = fresh_repo();
        let expired_serial = make_serial(&repo, "SKU-EXP", "S-EXP-1");
        let live_serial = make_serial(&repo, "SKU-EXP", "S-EXP-2");
        let confirmed_serial = make_serial(&repo, "SKU-EXP", "S-EXP-3");
        let expired = repo
            .reserve(ReserveSerialNumber {
                expires_in_seconds: Some(-1),
                ..reserve_input(expired_serial.id)
            })
            .expect("reserve expired");
        let live = repo
            .reserve(ReserveSerialNumber {
                expires_in_seconds: Some(3600),
                ..reserve_input(live_serial.id)
            })
            .expect("reserve live");
        let confirmed = repo
            .reserve(ReserveSerialNumber {
                expires_in_seconds: Some(-1),
                ..reserve_input(confirmed_serial.id)
            })
            .expect("reserve to confirm");
        // Confirm before the expiry could bite (confirmation suppresses expiry).
        force_status(&repo, confirmed_serial.id, SerialStatus::Reserved);
        {
            let conn = repo.pool.get().unwrap();
            conn.execute(
                "UPDATE serial_reservations SET confirmed_at = ? WHERE id = ?",
                params![Utc::now().to_rfc3339(), confirmed.id.to_string()],
            )
            .unwrap();
        }

        let swept = repo.release_expired_reservations(Utc::now()).expect("sweep");
        assert_eq!(swept, 1);
        assert_eq!(repo.get(expired_serial.id).unwrap().unwrap().status, SerialStatus::Available);
        assert!(repo.get_reservation(expired.id).unwrap().unwrap().released_at.is_some());
        assert_eq!(repo.get(live_serial.id).unwrap().unwrap().status, SerialStatus::Reserved);
        assert!(repo.get_reservation(live.id).unwrap().unwrap().is_open());
        assert_eq!(
            repo.get(confirmed_serial.id).unwrap().unwrap().status,
            SerialStatus::Reserved,
            "confirmed reservations never expire"
        );
        assert_eq!(repo.release_expired_reservations(Utc::now()).expect("sweep again"), 0);
    }

    #[test]
    fn reserve_lazily_expires_a_stale_reservation() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-LAZY", "S-LAZY-1");
        let stale = repo
            .reserve(ReserveSerialNumber { expires_in_seconds: Some(-1), ..reserve_input(s.id) })
            .expect("stale reserve");
        let fresh = repo.reserve(reserve_input(s.id)).expect("stale reservation is swept in-line");
        assert_ne!(stale.id, fresh.id);
        assert!(repo.get_reservation(stale.id).unwrap().unwrap().released_at.is_some());
        assert!(repo.get_reservation(fresh.id).unwrap().unwrap().is_open());
        assert_eq!(repo.get(s.id).unwrap().unwrap().status, SerialStatus::Reserved);
    }

    // S4: lot-level quarantine helpers ---------------------------------------

    #[test]
    fn quarantine_for_lot_only_touches_available_and_reserved() {
        let (repo, lot_id) = repo_with_lot("SKU-LOT");
        let with_lot = |serial: &str, status: SerialStatus| {
            let s = repo
                .create(CreateSerialNumber {
                    serial: Some(serial.into()),
                    sku: "SKU-LOT".into(),
                    lot_id: Some(lot_id),
                    ..Default::default()
                })
                .expect("create");
            force_status(&repo, s.id, status);
            s.id
        };
        let available = with_lot("S-L-AV", SerialStatus::Available);
        let reserved = with_lot("S-L-RS", SerialStatus::Reserved);
        let sold = with_lot("S-L-SO", SerialStatus::Sold);
        let scrapped = with_lot("S-L-SC", SerialStatus::Scrapped);
        let other_lot = make_serial(&repo, "SKU-LOT", "S-L-OTHER");

        let n = repo.quarantine_for_lot(lot_id, "supplier recall").expect("quarantine lot");
        assert_eq!(n, 2);
        for (id, expected) in [
            (available, SerialStatus::Quarantined),
            (reserved, SerialStatus::Quarantined),
            (sold, SerialStatus::Sold),
            (scrapped, SerialStatus::Scrapped),
            (other_lot.id, SerialStatus::Available),
        ] {
            assert_eq!(repo.get(id).unwrap().unwrap().status, expected);
        }

        let released = repo.release_quarantine_for_lot(lot_id).expect("release lot");
        assert_eq!(released, 2);
        assert_eq!(repo.get(available).unwrap().unwrap().status, SerialStatus::Available);
        assert_eq!(repo.get(reserved).unwrap().unwrap().status, SerialStatus::Available);
        assert_eq!(repo.release_quarantine_for_lot(lot_id).expect("nothing left"), 0);
    }

    #[test]
    fn quarantine_for_lot_closes_open_reservations() {
        let (repo, lot_id) = repo_with_lot("SKU-LQ");
        let s = repo
            .create(CreateSerialNumber {
                serial: Some("S-LQ-1".into()),
                sku: "SKU-LQ".into(),
                lot_id: Some(lot_id),
                ..Default::default()
            })
            .expect("create");
        let res = repo.reserve(reserve_input(s.id)).expect("reserve");
        assert_eq!(repo.quarantine_for_lot(lot_id, "hold").expect("quarantine"), 1);
        assert!(repo.get_reservation(res.id).unwrap().unwrap().released_at.is_some());
        assert!(repo.release_reservation(res.id).is_err());
    }

    #[test]
    fn quarantine_marks_quarantined_then_release() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-Q", "S-Q-1");
        let q = repo.quarantine(s.id, "qc fail").expect("quarantine");
        assert_eq!(q.status, SerialStatus::Quarantined);
        let r = repo.release_quarantine(s.id).expect("release");
        assert_eq!(r.status, SerialStatus::Available);
    }

    #[test]
    fn get_for_customer_filters_by_owner() {
        let repo = fresh_repo();
        let s = make_serial(&repo, "SKU-O", "S-O-1");
        let cust = Uuid::new_v4();
        repo.change_status(ChangeSerialStatus {
            serial_id: s.id,
            new_status: SerialStatus::Sold,
            owner_id: Some(cust),
            owner_type: Some("customer".into()),
            ..Default::default()
        })
        .expect("change");

        let owned = repo.get_for_customer(cust).expect("ok");
        let ids: Vec<_> = owned.iter().map(|s| s.id).collect();
        assert!(ids.contains(&s.id));

        let none = repo.get_for_customer(Uuid::new_v4()).expect("ok");
        assert!(none.is_empty());
    }

    #[test]
    fn get_available_for_sku_returns_only_available() {
        let repo = fresh_repo();
        make_serial(&repo, "SKU-AV", "S-AV-1");
        let s2 = make_serial(&repo, "SKU-AV", "S-AV-2");
        repo.change_status(ChangeSerialStatus {
            serial_id: s2.id,
            new_status: SerialStatus::Sold,
            ..Default::default()
        })
        .expect("change");

        let available = repo.get_available_for_sku("SKU-AV", 10).expect("ok");
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].status, SerialStatus::Available);
    }

    #[test]
    fn get_for_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn get_batch_returns_only_existing() {
        let repo = fresh_repo();
        let s1 = make_serial(&repo, "SKU-B", "S-B-1");
        let s2 = make_serial(&repo, "SKU-B", "S-B-2");
        let stranger = Uuid::new_v4();

        let batch = repo.get_batch(vec![s1.id, s2.id, stranger]).expect("batch");
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let result = repo
            .create_batch(vec![
                CreateSerialNumber {
                    serial: Some("S-CB-1".into()),
                    sku: "SKU-CB".into(),
                    lot_id: None,
                    lot_number: None,
                    location_id: None,
                    manufactured_at: None,
                    notes: None,
                    attributes: None,
                },
                CreateSerialNumber {
                    serial: Some("S-CB-2".into()),
                    sku: "SKU-CB".into(),
                    lot_id: None,
                    lot_number: None,
                    location_id: None,
                    manufactured_at: None,
                    notes: None,
                    attributes: None,
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }
}
