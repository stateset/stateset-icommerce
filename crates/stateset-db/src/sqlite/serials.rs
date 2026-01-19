//! SQLite implementation of Serial Number Repository

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use stateset_core::{
    BatchResult,
    ChangeSerialStatus, CreateSerialNumber, CreateSerialNumbersBulk, MoveSerial,
    ReserveSerialNumber, SerialEventType, SerialFilter, SerialHistory, SerialHistoryFilter,
    SerialLookupResult, SerialNumber, SerialReservation, SerialRepository, SerialStatus,
    SerialValidation, TransferSerialOwnership, UpdateSerialNumber, WarrantyLookupStatus,
};
use stateset_core::CommerceError;
use uuid::Uuid;

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_enum_row, parse_json_opt_row, parse_uuid_opt_row, parse_uuid_row, string_params,
    uuid_params,
};

/// SQLite implementation of SerialRepository
pub struct SqliteSerialRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSerialRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn map_serial_row(row: &Row) -> Result<SerialNumber, rusqlite::Error> {
        let status_str: String = row.get("status")?;
        let attributes_str: Option<String> = row.get("attributes")?;

        Ok(SerialNumber {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial", "id")?,
            serial: row.get("serial")?,
            sku: row.get("sku")?,
            status: parse_enum_row(&status_str, "serial", "status")?,
            lot_id: parse_uuid_opt_row(row.get::<_, Option<String>>("lot_id")?, "serial", "lot_id")?,
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
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "serial", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "serial", "updated_at")?,
        })
    }

    fn map_history_row(row: &Row) -> Result<SerialHistory, rusqlite::Error> {
        let event_type_str: String = row.get("event_type")?;
        let from_status_str: String = row.get("from_status")?;
        let to_status_str: String = row.get("to_status")?;

        Ok(SerialHistory {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial_history", "id")?,
            serial_id: parse_uuid_row(&row.get::<_, String>("serial_id")?, "serial_history", "serial_id")?,
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

    fn map_reservation_row(row: &Row) -> Result<SerialReservation, rusqlite::Error> {
        Ok(SerialReservation {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "serial_reservation", "id")?,
            serial_id: parse_uuid_row(&row.get::<_, String>("serial_id")?, "serial_reservation", "serial_id")?,
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

    #[allow(dead_code)]
    fn record_history(
        &self,
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

    fn generate_serial(&self, prefix: Option<&str>) -> String {
        let unique_part = Uuid::new_v4().to_string().replace("-", "").to_uppercase();
        let short = &unique_part[..12];
        match prefix {
            Some(p) => format!("{}-{}", p, short),
            None => format!("SN-{}", short),
        }
    }
}

impl SerialRepository for SqliteSerialRepository {
    fn create(&self, input: CreateSerialNumber) -> stateset_core::Result<SerialNumber> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let serial = input.serial.unwrap_or_else(|| self.generate_serial(None));
        let attributes = input.attributes.unwrap_or(serde_json::Value::Null);

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
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn create_bulk(&self, input: CreateSerialNumbersBulk) -> stateset_core::Result<Vec<SerialNumber>> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;

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
            ).map_err(map_db_error)?;

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
            ).map_err(map_db_error)?;

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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let mut updates = vec!["updated_at = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

        if let Some(status) = &input.status {
            updates.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
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

        let sql = format!(
            "UPDATE serial_numbers SET {} WHERE id = ?",
            updates.join(", ")
        );

        conn.execute(&sql, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())))
            .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: SerialFilter) -> stateset_core::Result<Vec<SerialNumber>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(serial) = &filter.serial {
            conditions.push("serial = ?".to_string());
            params.push(Box::new(serial.clone()));
        }
        if let Some(prefix) = &filter.serial_prefix {
            conditions.push("serial LIKE ?".to_string());
            params.push(Box::new(format!("{}%", prefix)));
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
            conditions.push(format!("status IN ({})", placeholders));
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

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM serial_numbers {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            where_clause
        );

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), Self::map_serial_row)
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
                "Can only delete serials with 'available' status".to_string()
            ));
        }

        // Check if there's any history beyond creation
        let history_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM serial_history WHERE serial_id = ? AND event_type != ?",
            params![id.to_string(), SerialEventType::Created.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        if history_count > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete serial with transaction history".to_string()
            ));
        }

        // Delete history first
        conn.execute(
            "DELETE FROM serial_history WHERE serial_id = ?",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        // Delete reservations
        conn.execute(
            "DELETE FROM serial_reservations WHERE serial_id = ?",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        // Delete serial
        conn.execute(
            "DELETE FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn change_status(&self, input: ChangeSerialStatus) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![input.serial_id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        let old_status = serial.status;

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET
                status = ?,
                current_location_id = COALESCE(?, current_location_id),
                current_owner_id = COALESCE(?, current_owner_id),
                current_owner_type = COALESCE(?, current_owner_type),
                updated_at = ?
            WHERE id = ?",
            params![
                input.new_status.to_string(),
                input.location_id,
                input.owner_id.map(|id| id.to_string()),
                input.owner_type,
                now,
                input.serial_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, from_location_id, to_location_id,
                from_owner_id, to_owner_id, performed_by, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                input.serial_id.to_string(),
                SerialEventType::StatusChanged.to_string(),
                input.reference_type,
                input.reference_id.map(|id| id.to_string()),
                old_status.to_string(),
                input.new_status.to_string(),
                serial.current_location_id,
                input.location_id,
                serial.current_owner_id.map(|id| id.to_string()),
                input.owner_id.map(|id| id.to_string()),
                input.performed_by,
                input.notes,
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn reserve(&self, input: ReserveSerialNumber) -> stateset_core::Result<SerialReservation> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Get and validate serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![input.serial_id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        if serial.status != SerialStatus::Available {
            return Err(CommerceError::ValidationError(
                format!("Serial is not available for reservation, current status: {}", serial.status)
            ));
        }

        // Check for existing active reservations
        let existing: i64 = tx.query_row(
            "SELECT COUNT(*) FROM serial_reservations
             WHERE serial_id = ? AND released_at IS NULL AND confirmed_at IS NULL
             AND (expires_at IS NULL OR expires_at > ?)",
            params![input.serial_id.to_string(), now_str],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        if existing > 0 {
            return Err(CommerceError::ValidationError(
                "Serial already has an active reservation".to_string()
            ));
        }

        let id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds
            .map(|secs| now + chrono::Duration::seconds(secs));

        // Create reservation
        tx.execute(
            "INSERT INTO serial_reservations (
                id, serial_id, reference_type, reference_id, reserved_by,
                reserved_at, expires_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                input.serial_id.to_string(),
                input.reference_type,
                input.reference_id.to_string(),
                input.reserved_by,
                now_str,
                expires_at.map(|dt| dt.to_rfc3339()),
            ],
        ).map_err(map_db_error)?;

        // Update serial status
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![
                SerialStatus::Reserved.to_string(),
                now_str,
                input.serial_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, performed_by, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                input.serial_id.to_string(),
                SerialEventType::Reserved.to_string(),
                input.reference_type,
                input.reference_id.to_string(),
                SerialStatus::Available.to_string(),
                SerialStatus::Reserved.to_string(),
                input.reserved_by,
                now_str,
            ],
        ).map_err(map_db_error)?;

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
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get reservation
        let reservation: SerialReservation = tx.query_row(
            "SELECT * FROM serial_reservations WHERE id = ?",
            params![reservation_id.to_string()],
            Self::map_reservation_row,
        ).map_err(map_db_error)?;

        if reservation.released_at.is_some() || reservation.confirmed_at.is_some() {
            return Err(CommerceError::ValidationError(
                "Reservation is already released or confirmed".to_string()
            ));
        }

        // Release reservation
        tx.execute(
            "UPDATE serial_reservations SET released_at = ? WHERE id = ?",
            params![now, reservation_id.to_string()],
        ).map_err(map_db_error)?;

        // Update serial status back to available
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![
                SerialStatus::Available.to_string(),
                now,
                reservation.serial_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                reservation.serial_id.to_string(),
                SerialEventType::Released.to_string(),
                reservation.reference_type,
                reservation.reference_id.to_string(),
                SerialStatus::Reserved.to_string(),
                SerialStatus::Available.to_string(),
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> stateset_core::Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        // Get reservation
        let reservation: SerialReservation = conn.query_row(
            "SELECT * FROM serial_reservations WHERE id = ?",
            params![reservation_id.to_string()],
            Self::map_reservation_row,
        ).map_err(map_db_error)?;

        if reservation.released_at.is_some() {
            return Err(CommerceError::ValidationError(
                "Reservation has been released".to_string()
            ));
        }

        if reservation.confirmed_at.is_some() {
            return Ok(()); // Already confirmed
        }

        conn.execute(
            "UPDATE serial_reservations SET confirmed_at = ? WHERE id = ?",
            params![now, reservation_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn move_serial(&self, input: MoveSerial) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![input.serial_id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        let from_location = serial.current_location_id;

        // Update location
        tx.execute(
            "UPDATE serial_numbers SET current_location_id = ?, updated_at = ? WHERE id = ?",
            params![input.to_location_id, now, input.serial_id.to_string()],
        ).map_err(map_db_error)?;

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
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn transfer_ownership(&self, input: TransferSerialOwnership) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![input.serial_id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        // Update ownership
        tx.execute(
            "UPDATE serial_numbers SET
                current_owner_id = ?,
                current_owner_type = ?,
                status = ?,
                updated_at = ?
            WHERE id = ?",
            params![
                input.new_owner_id.to_string(),
                input.new_owner_type,
                SerialStatus::Transferred.to_string(),
                now,
                input.serial_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, from_owner_id, to_owner_id, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                input.serial_id.to_string(),
                SerialEventType::Transferred.to_string(),
                input.reference_type,
                input.reference_id.map(|id| id.to_string()),
                serial.status.to_string(),
                SerialStatus::Transferred.to_string(),
                serial.current_owner_id.map(|id| id.to_string()),
                input.new_owner_id.to_string(),
                input.notes,
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(input.serial_id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_sold(&self, id: Uuid, customer_id: Uuid, order_id: Option<Uuid>) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET
                status = ?,
                current_owner_id = ?,
                current_owner_type = 'customer',
                sold_at = ?,
                updated_at = ?
            WHERE id = ?",
            params![
                SerialStatus::Sold.to_string(),
                customer_id.to_string(),
                now_str,
                now_str,
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, to_owner_id, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::Sold.to_string(),
                order_id.map(|_| "order"),
                order_id.map(|id| id.to_string()),
                serial.status.to_string(),
                SerialStatus::Sold.to_string(),
                customer_id.to_string(),
                now_str,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![SerialStatus::Shipped.to_string(), now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::Shipped.to_string(),
                "shipment",
                shipment_id.to_string(),
                serial.status.to_string(),
                SerialStatus::Shipped.to_string(),
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET
                status = ?,
                current_owner_id = NULL,
                current_owner_type = NULL,
                updated_at = ?
            WHERE id = ?",
            params![SerialStatus::Returned.to_string(), now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, from_owner_id, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::Returned.to_string(),
                "return",
                return_id.to_string(),
                serial.status.to_string(),
                SerialStatus::Returned.to_string(),
                serial.current_owner_id.map(|id| id.to_string()),
                now,
            ],
        ).map_err(map_db_error)?;

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
            ).map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![SerialStatus::Quarantined.to_string(), now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, from_status, to_status, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::Quarantined.to_string(),
                serial.status.to_string(),
                SerialStatus::Quarantined.to_string(),
                reason,
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn release_quarantine(&self, id: Uuid) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        if serial.status != SerialStatus::Quarantined {
            return Err(CommerceError::ValidationError(
                "Serial is not quarantined".to_string()
            ));
        }

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![SerialStatus::Available.to_string(), now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, from_status, to_status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::QuarantineReleased.to_string(),
                SerialStatus::Quarantined.to_string(),
                SerialStatus::Available.to_string(),
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn scrap(&self, id: Uuid, reason: &str) -> stateset_core::Result<SerialNumber> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now().to_rfc3339();

        // Get current serial
        let serial: SerialNumber = tx.query_row(
            "SELECT * FROM serial_numbers WHERE id = ?",
            params![id.to_string()],
            Self::map_serial_row,
        ).map_err(map_db_error)?;

        if !serial.can_scrap() {
            return Err(CommerceError::ValidationError(
                "Serial cannot be scrapped in current state".to_string()
            ));
        }

        // Update serial
        tx.execute(
            "UPDATE serial_numbers SET status = ?, updated_at = ? WHERE id = ?",
            params![SerialStatus::Scrapped.to_string(), now, id.to_string()],
        ).map_err(map_db_error)?;

        // Record history
        let history_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO serial_history (
                id, serial_id, event_type, from_status, to_status, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                history_id.to_string(),
                id.to_string(),
                SerialEventType::Scrapped.to_string(),
                serial.status.to_string(),
                SerialStatus::Scrapped.to_string(),
                reason,
                now,
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_history(&self, serial_id: Uuid, filter: SerialHistoryFilter) -> stateset_core::Result<Vec<SerialHistory>> {
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

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), Self::map_history_row)
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
            SerialHistoryFilter {
                limit: Some(10),
                ..Default::default()
            },
        )?;

        // Get lot if present (would need lot repository access)
        let lot = None; // TODO: Integrate with LotRepository

        // Get warranty status if present
        let warranty_status = serial_number.warranty_id.map(|warranty_id| {
            WarrantyLookupStatus {
                warranty_id,
                is_active: true, // TODO: Check actual warranty status
                expires_at: None,
                coverage_type: None,
            }
        });

        Ok(Some(SerialLookupResult {
            serial: serial_number,
            product_name: None, // TODO: Get from product repository
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

    fn get_available_for_sku(&self, sku: &str, limit: u32) -> stateset_core::Result<Vec<SerialNumber>> {
        self.list(SerialFilter {
            sku: Some(sku.to_string()),
            status: Some(SerialStatus::Available),
            limit: Some(limit),
            ..Default::default()
        })
    }

    fn get_for_lot(&self, lot_id: Uuid) -> stateset_core::Result<Vec<SerialNumber>> {
        self.list(SerialFilter {
            lot_id: Some(lot_id),
            ..Default::default()
        })
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

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?".to_string());
            params.push(Box::new(sku.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(lot_id) = &filter.lot_id {
            conditions.push("lot_id = ?".to_string());
            params.push(Box::new(lot_id.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) FROM serial_numbers {}", where_clause);

        let count: i64 = conn.query_row(
            &sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        ).map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> stateset_core::Result<BatchResult<SerialNumber>> {
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

        let sql = format!(
            "SELECT * FROM serial_numbers WHERE id IN ({})",
            placeholders
        );

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

    fn get_batch_by_serial(&self, serials: Vec<String>) -> stateset_core::Result<Vec<SerialNumber>> {
        if serials.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let placeholders = build_in_clause(serials.len());
        let params = string_params(&serials);
        let params_ref = params_refs(&params);

        let sql = format!(
            "SELECT * FROM serial_numbers WHERE serial IN ({})",
            placeholders
        );

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
