//! PostgreSQL implementation of serial number repository

use super::{PgLotRepository, PgWarrantyRepository, block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    BatchResult, ChangeSerialStatus, CommerceError, CreateSerialNumber, CreateSerialNumbersBulk,
    MoveSerial, ReserveSerialNumber, Result, SerialEventType, SerialFilter, SerialHistory,
    SerialHistoryFilter, SerialLookupResult, SerialNumber, SerialRepository, SerialReservation,
    SerialStatus, SerialValidation, TransferSerialOwnership, UpdateSerialNumber,
    WarrantyLookupStatus, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgSerialRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SerialRow {
    id: Uuid,
    serial: String,
    sku: String,
    status: String,
    lot_id: Option<Uuid>,
    lot_number: Option<String>,
    current_location_id: Option<i32>,
    current_owner_id: Option<Uuid>,
    current_owner_type: Option<String>,
    warranty_id: Option<Uuid>,
    manufactured_at: Option<DateTime<Utc>>,
    received_at: Option<DateTime<Utc>>,
    sold_at: Option<DateTime<Utc>>,
    activated_at: Option<DateTime<Utc>>,
    last_service_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    attributes: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct SerialHistoryRow {
    id: Uuid,
    serial_id: Uuid,
    event_type: String,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    from_status: String,
    to_status: String,
    from_location_id: Option<i32>,
    to_location_id: Option<i32>,
    from_owner_id: Option<Uuid>,
    to_owner_id: Option<Uuid>,
    performed_by: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct SerialReservationRow {
    id: Uuid,
    serial_id: Uuid,
    reference_type: String,
    reference_id: Uuid,
    reserved_by: Option<String>,
    reserved_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    confirmed_at: Option<DateTime<Utc>>,
    released_at: Option<DateTime<Utc>>,
}

impl PgSerialRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_serial(row: SerialRow) -> Result<SerialNumber> {
        let status: SerialStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid serial.status '{}': {}", row.status, e))
        })?;

        Ok(SerialNumber {
            id: row.id,
            serial: row.serial,
            sku: row.sku,
            status,
            lot_id: row.lot_id,
            lot_number: row.lot_number,
            current_location_id: row.current_location_id,
            current_owner_id: row.current_owner_id,
            current_owner_type: row.current_owner_type,
            warranty_id: row.warranty_id,
            manufactured_at: row.manufactured_at,
            received_at: row.received_at,
            sold_at: row.sold_at,
            activated_at: row.activated_at,
            last_service_at: row.last_service_at,
            notes: row.notes,
            attributes: row.attributes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_history(row: SerialHistoryRow) -> Result<SerialHistory> {
        let event_type: SerialEventType = row.event_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid serial_history.event_type '{}': {}",
                row.event_type, e
            ))
        })?;
        let from_status: SerialStatus = row.from_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid serial_history.from_status '{}': {}",
                row.from_status, e
            ))
        })?;
        let to_status: SerialStatus = row.to_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid serial_history.to_status '{}': {}",
                row.to_status, e
            ))
        })?;

        Ok(SerialHistory {
            id: row.id,
            serial_id: row.serial_id,
            event_type,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            from_status,
            to_status,
            from_location_id: row.from_location_id,
            to_location_id: row.to_location_id,
            from_owner_id: row.from_owner_id,
            to_owner_id: row.to_owner_id,
            performed_by: row.performed_by,
            notes: row.notes,
            created_at: row.created_at,
        })
    }

    fn row_to_reservation(row: SerialReservationRow) -> SerialReservation {
        SerialReservation {
            id: row.id,
            serial_id: row.serial_id,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            reserved_by: row.reserved_by,
            reserved_at: row.reserved_at,
            expires_at: row.expires_at,
            confirmed_at: row.confirmed_at,
            released_at: row.released_at,
        }
    }

    /// Load a serial inside a transaction with `FOR UPDATE`, so every
    /// status decision below is made against a row no concurrent writer can
    /// change until we commit. Missing rows map to `NotFound`.
    async fn load_for_update(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<SerialNumber> {
        let row =
            sqlx::query_as::<_, SerialRow>("SELECT * FROM serial_numbers WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        row.map(Self::row_to_serial).transpose()?.ok_or(CommerceError::NotFound)
    }

    /// Load a reservation inside a transaction with `FOR UPDATE`.
    async fn load_reservation_for_update(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<SerialReservation> {
        let row = sqlx::query_as::<_, SerialReservationRow>(
            "SELECT * FROM serial_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_reservation).ok_or(CommerceError::NotFound)
    }

    /// Close every open reservation on a serial (released/consumed/swept all
    /// look the same in the store: `released_at` set, `active_key` cleared so
    /// the unique backstop index frees the slot).
    async fn close_open_reservations(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        serial_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE serial_reservations SET released_at = $1, active_key = NULL
             WHERE serial_id = $2 AND released_at IS NULL",
        )
        .bind(now)
        .bind(serial_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(result.rows_affected())
    }

    /// Move `serial` to `to` — the ONE place a status is written.
    ///
    /// Checks the state machine ([`SerialStatus::allowed_transitions`]), then
    /// writes `status` conditionally on the status the caller observed
    /// (`WHERE id = $3 AND status = $4`) so a concurrent writer that got there
    /// first is detected instead of overwritten (the caller holds the row
    /// lock from [`Self::load_for_update`], so in practice this is belt and
    /// braces). Leaving `Reserved` consumes any open reservation in the same
    /// transaction. Callers needing extra columns update them afterwards
    /// under the same lock.
    async fn write_transition(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        serial: &SerialNumber,
        to: SerialStatus,
        now: DateTime<Utc>,
    ) -> Result<()> {
        serial.ensure_can_transition_to(to)?;
        let result = sqlx::query(
            "UPDATE serial_numbers SET status = $1, updated_at = $2 WHERE id = $3 AND status = $4",
        )
        .bind(to.to_string())
        .bind(now)
        .bind(serial.id)
        .bind(serial.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() != 1 {
            return Err(CommerceError::Conflict(format!(
                "Serial {} ({}) changed concurrently while moving from {} to {}",
                serial.serial, serial.id, serial.status, to
            )));
        }
        if serial.status == SerialStatus::Reserved {
            Self::close_open_reservations(tx, serial.id, now).await?;
        }
        Ok(())
    }

    fn generate_serial(prefix: Option<&str>) -> String {
        let unique_part = Uuid::new_v4().to_string().replace('-', "").to_uppercase();
        let short = &unique_part[..12];
        match prefix {
            Some(p) => format!("{}-{}", p, short),
            None => format!("SN-{}", short),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_history_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        serial_id: Uuid,
        event_type: SerialEventType,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        from_status: SerialStatus,
        to_status: SerialStatus,
        from_location_id: Option<i32>,
        to_location_id: Option<i32>,
        from_owner_id: Option<Uuid>,
        to_owner_id: Option<Uuid>,
        performed_by: Option<&str>,
        notes: Option<&str>,
    ) -> Result<()> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO serial_history (
                id, serial_id, event_type, reference_type, reference_id,
                from_status, to_status, from_location_id, to_location_id,
                from_owner_id, to_owner_id, performed_by, notes, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            "#,
        )
        .bind(id)
        .bind(serial_id)
        .bind(event_type.to_string())
        .bind(reference_type)
        .bind(reference_id)
        .bind(from_status.to_string())
        .bind(to_status.to_string())
        .bind(from_location_id)
        .bind(to_location_id)
        .bind(from_owner_id)
        .bind(to_owner_id)
        .bind(performed_by)
        .bind(notes)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn create_async(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let serial = input.serial.unwrap_or_else(|| Self::generate_serial(None));
        let attributes = input.attributes.unwrap_or(serde_json::Value::Null);

        sqlx::query(
            r#"
            INSERT INTO serial_numbers (
                id, serial, sku, status, lot_id, lot_number, current_location_id,
                manufactured_at, notes, attributes, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
            "#,
        )
        .bind(id)
        .bind(&serial)
        .bind(&input.sku)
        .bind(SerialStatus::Available.to_string())
        .bind(input.lot_id)
        .bind(&input.lot_number)
        .bind(input.location_id)
        .bind(input.manufactured_at)
        .bind(&input.notes)
        .bind(attributes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn create_bulk_async(
        &self,
        input: CreateSerialNumbersBulk,
    ) -> Result<Vec<SerialNumber>> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut serials = Vec::with_capacity(input.quantity as usize);
        let now = Utc::now();

        for i in 0..input.quantity {
            let id = Uuid::new_v4();
            let serial_number = match &input.prefix {
                Some(prefix) => format!("{}-{:06}", prefix, i + 1),
                None => Self::generate_serial(None),
            };

            sqlx::query(
                r#"
                INSERT INTO serial_numbers (
                    id, serial, sku, status, lot_id, lot_number, current_location_id,
                    manufactured_at, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
                "#,
            )
            .bind(id)
            .bind(&serial_number)
            .bind(&input.sku)
            .bind(SerialStatus::Available.to_string())
            .bind(input.lot_id)
            .bind(&input.lot_number)
            .bind(input.location_id)
            .bind(input.manufactured_at)
            .bind(now)
            .execute(tx.as_mut())
            .await
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
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(serials)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        let row = sqlx::query_as::<_, SerialRow>("SELECT * FROM serial_numbers WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_serial).transpose()
    }

    pub async fn get_by_serial_async(&self, serial: &str) -> Result<Option<SerialNumber>> {
        let row = sqlx::query_as::<_, SerialRow>("SELECT * FROM serial_numbers WHERE serial = $1")
            .bind(serial)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_serial).transpose()
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        // A status change through `update` is a state-machine transition like
        // any other; a same-status "change" is a no-op.
        let status_change = match input.status {
            Some(to) if to != serial.status => {
                serial.ensure_can_transition_to(to)?;
                Some(to)
            }
            _ => None,
        };

        let result = sqlx::query(
            r#"
            UPDATE serial_numbers SET
                status = COALESCE($1, status),
                current_location_id = COALESCE($2, current_location_id),
                lot_id = COALESCE($3, lot_id),
                warranty_id = COALESCE($4, warranty_id),
                notes = COALESCE($5, notes),
                attributes = COALESCE($6, attributes),
                updated_at = $7
            WHERE id = $8 AND status = $9
            "#,
        )
        .bind(status_change.map(|s| s.to_string()))
        .bind(input.location_id)
        .bind(input.lot_id)
        .bind(input.warranty_id)
        .bind(input.notes)
        .bind(input.attributes)
        .bind(now)
        .bind(id)
        .bind(serial.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() != 1 {
            return Err(CommerceError::Conflict(format!(
                "Serial {} ({}) changed concurrently during update",
                serial.serial, serial.id
            )));
        }

        if let Some(to) = status_change {
            if serial.status == SerialStatus::Reserved {
                Self::close_open_reservations(&mut tx, serial.id, now).await?;
            }
            Self::record_history_tx(
                &mut tx,
                id,
                SerialEventType::StatusChanged,
                None,
                None,
                serial.status,
                to,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM serial_numbers WHERE 1=1");

        if let Some(serial) = &filter.serial {
            builder.push(" AND serial = ").push_bind(serial);
        }
        if let Some(prefix) = &filter.serial_prefix {
            builder.push(" AND serial LIKE ").push_bind(format!("{}%", prefix));
        }
        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(statuses) = &filter.statuses {
            if !statuses.is_empty() {
                builder.push(" AND status IN (");
                {
                    let mut separated = builder.separated(", ");
                    for status in statuses {
                        separated.push_bind(status.to_string());
                    }
                }
                builder.push(")");
            }
        }
        if let Some(lot_id) = &filter.lot_id {
            builder.push(" AND lot_id = ").push_bind(lot_id);
        }
        if let Some(lot_number) = &filter.lot_number {
            builder.push(" AND lot_number = ").push_bind(lot_number);
        }
        if let Some(loc_id) = filter.location_id {
            builder.push(" AND current_location_id = ").push_bind(loc_id);
        }
        if let Some(owner_id) = &filter.owner_id {
            builder.push(" AND current_owner_id = ").push_bind(owner_id);
        }
        if let Some(owner_type) = &filter.owner_type {
            builder.push(" AND current_owner_type = ").push_bind(owner_type);
        }
        if let Some(warranty_id) = &filter.warranty_id {
            builder.push(" AND warranty_id = ").push_bind(warranty_id);
        }
        if let Some(has_warranty) = filter.has_warranty {
            if has_warranty {
                builder.push(" AND warranty_id IS NOT NULL");
            } else {
                builder.push(" AND warranty_id IS NULL");
            }
        }
        if let Some(after) = &filter.manufactured_after {
            builder.push(" AND manufactured_at >= ").push_bind(after);
        }
        if let Some(before) = &filter.manufactured_before {
            builder.push(" AND manufactured_at <= ").push_bind(before);
        }
        if let Some(after) = &filter.sold_after {
            builder.push(" AND sold_at >= ").push_bind(after);
        }
        if let Some(before) = &filter.sold_before {
            builder.push(" AND sold_at <= ").push_bind(before);
        }

        builder.push(" ORDER BY created_at DESC");
        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;
        builder.push(" LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset);

        let rows = builder
            .build_query_as::<SerialRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // Only an existing, Available serial with no post-creation history may be
        // deleted (matching the SQLite backend). Without these guards Postgres
        // returned Ok(()) for a missing id and permanently deleted a sold serial.
        let serial = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        if serial.status != SerialStatus::Available {
            return Err(CommerceError::ValidationError(
                "Can only delete serials with 'available' status".to_string(),
            ));
        }

        let history_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM serial_history WHERE serial_id = $1 AND event_type != $2",
        )
        .bind(id)
        .bind(SerialEventType::Created.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        if history_count.0 > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete serial with transaction history".to_string(),
            ));
        }

        sqlx::query("DELETE FROM serial_history WHERE serial_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM serial_reservations WHERE serial_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM serial_numbers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn change_status_async(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, input.serial_id).await?;

        Self::write_transition(&mut tx, &serial, input.new_status, now).await?;

        sqlx::query(
            r#"
            UPDATE serial_numbers SET
                current_location_id = COALESCE($1, current_location_id),
                current_owner_id = COALESCE($2, current_owner_id),
                current_owner_type = COALESCE($3, current_owner_type)
            WHERE id = $4
            "#,
        )
        .bind(input.location_id)
        .bind(input.owner_id)
        .bind(&input.owner_type)
        .bind(input.serial_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_history_tx(
            &mut tx,
            input.serial_id,
            SerialEventType::StatusChanged,
            input.reference_type.as_deref(),
            input.reference_id,
            serial.status,
            input.new_status,
            serial.current_location_id,
            input.location_id,
            serial.current_owner_id,
            input.owner_id,
            input.performed_by.as_deref(),
            input.notes.as_deref(),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(input.serial_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reserve_async(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Row lock: two concurrent reservations of one unit serialize here, and
        // the loser sees `reserved` and is refused below.
        let mut serial = Self::load_for_update(&mut tx, input.serial_id).await?;

        // Lazy expiry: a serial still `Reserved` by an expired, unconfirmed
        // reservation is returned to stock in-line so the sweeper is not on the
        // critical path of the next order.
        if serial.status == SerialStatus::Reserved {
            let stale = sqlx::query_as::<_, SerialReservationRow>(
                r#"
                SELECT * FROM serial_reservations
                WHERE serial_id = $1 AND released_at IS NULL AND confirmed_at IS NULL
                  AND expires_at IS NOT NULL AND expires_at <= $2
                ORDER BY reserved_at DESC LIMIT 1
                "#,
            )
            .bind(input.serial_id)
            .bind(now)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .map(Self::row_to_reservation);
            if let Some(stale) = stale {
                Self::write_transition(&mut tx, &serial, SerialStatus::Available, now).await?;
                Self::record_history_tx(
                    &mut tx,
                    serial.id,
                    SerialEventType::Released,
                    Some(&stale.reference_type),
                    Some(stale.reference_id),
                    SerialStatus::Reserved,
                    SerialStatus::Available,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("reservation expired"),
                )
                .await?;
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
        Self::close_open_reservations(&mut tx, serial.id, now).await?;

        let id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds.map(|secs| now + chrono::Duration::seconds(secs));

        sqlx::query(
            r#"
            INSERT INTO serial_reservations (
                id, serial_id, reference_type, reference_id, reserved_by,
                reserved_at, expires_at, active_key
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(id)
        .bind(input.serial_id)
        .bind(&input.reference_type)
        .bind(input.reference_id)
        .bind(&input.reserved_by)
        .bind(now)
        .bind(expires_at)
        .bind(input.serial_id.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::write_transition(&mut tx, &serial, SerialStatus::Reserved, now).await?;

        Self::record_history_tx(
            &mut tx,
            input.serial_id,
            SerialEventType::Reserved,
            Some(&input.reference_type),
            Some(input.reference_id),
            SerialStatus::Available,
            SerialStatus::Reserved,
            None,
            None,
            None,
            None,
            input.reserved_by.as_deref(),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

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

    pub async fn release_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let reservation = Self::load_reservation_for_update(&mut tx, reservation_id).await?;
        if reservation.released_at.is_some() {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} is already closed (released, consumed or expired)"
            )));
        }

        // The reservation may only be released while it still holds the serial.
        // Once the unit shipped or sold the reservation was consumed; releasing
        // it now must not flip a shipped unit back to `available`.
        let serial = Self::load_for_update(&mut tx, reservation.serial_id).await?;
        if serial.status != SerialStatus::Reserved {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} cannot be released: serial {} ({}) is {}",
                serial.serial, serial.id, serial.status
            )));
        }

        let result = sqlx::query(
            "UPDATE serial_reservations SET released_at = $1, active_key = NULL
             WHERE id = $2 AND released_at IS NULL",
        )
        .bind(now)
        .bind(reservation_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() != 1 {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} was closed concurrently"
            )));
        }

        Self::write_transition(&mut tx, &serial, SerialStatus::Available, now).await?;

        Self::record_history_tx(
            &mut tx,
            reservation.serial_id,
            SerialEventType::Released,
            Some(&reservation.reference_type),
            Some(reservation.reference_id),
            SerialStatus::Reserved,
            SerialStatus::Available,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn confirm_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let reservation = Self::load_reservation_for_update(&mut tx, reservation_id).await?;
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

        let serial = Self::load_for_update(&mut tx, reservation.serial_id).await?;
        if serial.status != SerialStatus::Reserved {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} cannot be confirmed: serial {} ({}) is {}",
                serial.serial, serial.id, serial.status
            )));
        }

        let result = sqlx::query(
            "UPDATE serial_reservations SET confirmed_at = $1
             WHERE id = $2 AND released_at IS NULL AND confirmed_at IS NULL",
        )
        .bind(now)
        .bind(reservation_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() != 1 {
            return Err(CommerceError::Conflict(format!(
                "Reservation {reservation_id} changed concurrently"
            )));
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    pub async fn get_reservation_async(
        &self,
        reservation_id: Uuid,
    ) -> Result<Option<SerialReservation>> {
        let row = sqlx::query_as::<_, SerialReservationRow>(
            "SELECT * FROM serial_reservations WHERE id = $1",
        )
        .bind(reservation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(row.map(Self::row_to_reservation))
    }

    pub async fn release_expired_reservations_async(&self, now: DateTime<Utc>) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let expired: Vec<SerialReservation> = sqlx::query_as::<_, SerialReservationRow>(
            r#"
            SELECT * FROM serial_reservations
            WHERE released_at IS NULL AND confirmed_at IS NULL
              AND expires_at IS NOT NULL AND expires_at <= $1
            ORDER BY reserved_at ASC
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(now)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .into_iter()
        .map(Self::row_to_reservation)
        .collect();

        let mut returned_to_stock = 0u64;
        for reservation in expired {
            sqlx::query(
                "UPDATE serial_reservations SET released_at = $1, active_key = NULL
                 WHERE id = $2 AND released_at IS NULL",
            )
            .bind(now)
            .bind(reservation.id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let serial = Self::load_for_update(&mut tx, reservation.serial_id).await?;
            if serial.status != SerialStatus::Reserved {
                continue; // The unit already moved on; only the stale row needed closing.
            }
            Self::write_transition(&mut tx, &serial, SerialStatus::Available, now).await?;
            Self::record_history_tx(
                &mut tx,
                serial.id,
                SerialEventType::Released,
                Some(&reservation.reference_type),
                Some(reservation.reference_id),
                SerialStatus::Reserved,
                SerialStatus::Available,
                None,
                None,
                None,
                None,
                None,
                Some("reservation expired"),
            )
            .await?;
            returned_to_stock += 1;
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(returned_to_stock)
    }

    pub async fn move_serial_async(&self, input: MoveSerial) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial_row =
            sqlx::query_as::<_, SerialRow>("SELECT * FROM serial_numbers WHERE id = $1")
                .bind(input.serial_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let serial = Self::row_to_serial(serial_row)?;

        sqlx::query(
            "UPDATE serial_numbers SET current_location_id = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(input.to_location_id)
        .bind(now)
        .bind(input.serial_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_history_tx(
            &mut tx,
            input.serial_id,
            SerialEventType::LocationChanged,
            None,
            None,
            serial.status,
            serial.status,
            serial.current_location_id,
            Some(input.to_location_id),
            None,
            None,
            input.performed_by.as_deref(),
            input.notes.as_deref(),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(input.serial_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn transfer_ownership_async(
        &self,
        input: TransferSerialOwnership,
    ) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, input.serial_id).await?;

        Self::write_transition(&mut tx, &serial, SerialStatus::Transferred, now).await?;

        sqlx::query(
            "UPDATE serial_numbers SET current_owner_id = $1, current_owner_type = $2 WHERE id = $3",
        )
        .bind(input.new_owner_id)
        .bind(&input.new_owner_type)
        .bind(input.serial_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_history_tx(
            &mut tx,
            input.serial_id,
            SerialEventType::Transferred,
            input.reference_type.as_deref(),
            input.reference_id,
            serial.status,
            SerialStatus::Transferred,
            None,
            None,
            serial.current_owner_id,
            Some(input.new_owner_id),
            None,
            input.notes.as_deref(),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(input.serial_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_sold_async(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        // Selling consumes the open reservation (write_transition closes it
        // when leaving `Reserved`).
        Self::write_transition(&mut tx, &serial, SerialStatus::Sold, now).await?;

        sqlx::query(
            r#"
            UPDATE serial_numbers SET
                current_owner_id = $1,
                current_owner_type = 'customer',
                sold_at = $2
            WHERE id = $3
            "#,
        )
        .bind(customer_id)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::Sold,
            order_id.map(|_| "order"),
            order_id,
            serial.status,
            SerialStatus::Sold,
            None,
            None,
            None,
            Some(customer_id),
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_shipped_async(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        // Shipping consumes the open reservation (see write_transition).
        Self::write_transition(&mut tx, &serial, SerialStatus::Shipped, now).await?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::Shipped,
            Some("shipment"),
            Some(shipment_id),
            serial.status,
            SerialStatus::Shipped,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_returned_async(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        Self::write_transition(&mut tx, &serial, SerialStatus::Returned, now).await?;

        sqlx::query(
            "UPDATE serial_numbers SET current_owner_id = NULL, current_owner_type = NULL WHERE id = $1",
        )
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::Returned,
            Some("return"),
            Some(return_id),
            serial.status,
            SerialStatus::Returned,
            None,
            None,
            serial.current_owner_id,
            None,
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn activate_async(&self, id: Uuid) -> Result<SerialNumber> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE serial_numbers SET activated_at = $1, updated_at = $1 WHERE id = $2 AND activated_at IS NULL",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(serial) = self.get_async(id).await? {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            Self::record_history_tx(
                &mut tx,
                id,
                SerialEventType::Activated,
                None,
                None,
                serial.status,
                serial.status,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            tx.commit().await.map_err(map_db_error)?;
        }

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn quarantine_async(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        Self::write_transition(&mut tx, &serial, SerialStatus::Quarantined, now).await?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::Quarantined,
            None,
            None,
            serial.status,
            SerialStatus::Quarantined,
            None,
            None,
            None,
            None,
            None,
            Some(reason),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn release_quarantine_async(&self, id: Uuid) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;
        if serial.status != SerialStatus::Quarantined {
            return Err(CommerceError::Conflict(format!(
                "Serial {} ({}) is not quarantined (status: {}); cannot release to available",
                serial.serial, serial.id, serial.status
            )));
        }

        Self::write_transition(&mut tx, &serial, SerialStatus::Available, now).await?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::QuarantineReleased,
            None,
            None,
            SerialStatus::Quarantined,
            SerialStatus::Available,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Quarantine every `Available` / `Reserved` serial of `lot_id` on the
    /// caller's transaction, closing open reservations; returns how many moved.
    ///
    /// This is the serial half of a lot quarantine: `PgLotRepository` and the
    /// quality repository call it inside the transaction that flips the lot,
    /// so a quarantined lot can never leave a sellable serial behind.
    /// Shipped / sold / already-quarantined serials are untouched.
    pub(crate) async fn quarantine_for_lot_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Uuid,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let rows = sqlx::query_as::<_, SerialRow>(
            r#"
            SELECT * FROM serial_numbers
            WHERE lot_id = $1 AND status IN ($2, $3)
            ORDER BY created_at ASC
            FOR UPDATE
            "#,
        )
        .bind(lot_id)
        .bind(SerialStatus::Available.to_string())
        .bind(SerialStatus::Reserved.to_string())
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut count = 0u64;
        for row in rows {
            let serial = Self::row_to_serial(row)?;
            Self::write_transition(tx, &serial, SerialStatus::Quarantined, now).await?;
            Self::record_history_tx(
                tx,
                serial.id,
                SerialEventType::Quarantined,
                Some("lot"),
                Some(lot_id),
                serial.status,
                SerialStatus::Quarantined,
                None,
                None,
                None,
                None,
                None,
                Some(reason),
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    /// Return every `Quarantined` serial of `lot_id` to `Available` on the
    /// caller's transaction; the counterpart of [`Self::quarantine_for_lot_on`].
    pub(crate) async fn release_quarantine_for_lot_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let rows = sqlx::query_as::<_, SerialRow>(
            r#"
            SELECT * FROM serial_numbers
            WHERE lot_id = $1 AND status = $2
            ORDER BY created_at ASC
            FOR UPDATE
            "#,
        )
        .bind(lot_id)
        .bind(SerialStatus::Quarantined.to_string())
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut count = 0u64;
        for row in rows {
            let serial = Self::row_to_serial(row)?;
            Self::write_transition(tx, &serial, SerialStatus::Available, now).await?;
            Self::record_history_tx(
                tx,
                serial.id,
                SerialEventType::QuarantineReleased,
                Some("lot"),
                Some(lot_id),
                SerialStatus::Quarantined,
                SerialStatus::Available,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn quarantine_for_lot_async(&self, lot_id: Uuid, reason: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let count = Self::quarantine_for_lot_on(&mut tx, lot_id, reason, Utc::now()).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(count)
    }

    pub async fn release_quarantine_for_lot_async(&self, lot_id: Uuid) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let count = Self::release_quarantine_for_lot_on(&mut tx, lot_id, Utc::now()).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(count)
    }

    pub async fn scrap_async(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let serial = Self::load_for_update(&mut tx, id).await?;

        Self::write_transition(&mut tx, &serial, SerialStatus::Scrapped, now).await?;

        Self::record_history_tx(
            &mut tx,
            id,
            SerialEventType::Scrapped,
            None,
            None,
            serial.status,
            SerialStatus::Scrapped,
            None,
            None,
            None,
            None,
            None,
            Some(reason),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_history_async(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM serial_history WHERE serial_id = ");
        builder.push_bind(serial_id);

        if let Some(event_type) = &filter.event_type {
            builder.push(" AND event_type = ").push_bind(event_type.to_string());
        }
        if let Some(reference_type) = &filter.reference_type {
            builder.push(" AND reference_type = ").push_bind(reference_type);
        }
        if let Some(from_date) = &filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = &filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date);
        }

        builder.push(" ORDER BY created_at DESC");
        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;
        builder.push(" LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset);

        let rows = builder
            .build_query_as::<SerialHistoryRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut history = Vec::with_capacity(rows.len());
        for row in rows {
            history.push(Self::row_to_history(row)?);
        }
        Ok(history)
    }

    pub async fn lookup_async(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        let serial_number = match self.get_by_serial_async(serial).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let recent_history = self
            .get_history_async(
                serial_number.id,
                SerialHistoryFilter { limit: Some(10), ..Default::default() },
            )
            .await?;

        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(p.name, v.name)
             FROM product_variants v
             LEFT JOIN products p ON p.id = v.product_id
             WHERE v.sku = $1",
        )
        .bind(&serial_number.sku)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        let lot = {
            let lot_repo = PgLotRepository::new(self.pool.clone());
            match (serial_number.lot_id, serial_number.lot_number.as_deref()) {
                (Some(lot_id), lot_number) => match lot_repo.get_async(lot_id).await? {
                    Some(lot) => Some(lot),
                    None => match lot_number {
                        Some(number) => lot_repo.get_by_number_async(number).await?,
                        None => None,
                    },
                },
                (None, Some(lot_number)) => lot_repo.get_by_number_async(lot_number).await?,
                (None, None) => None,
            }
        };

        let warranty_status = if let Some(warranty_id) = serial_number.warranty_id {
            let warranty_repo = PgWarrantyRepository::new(self.pool.clone());
            match warranty_repo.get_async(stateset_core::WarrantyId::from_uuid(warranty_id)).await?
            {
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

    pub async fn validate_async(&self, serial: &str) -> Result<SerialValidation> {
        match self.get_by_serial_async(serial).await? {
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

    pub async fn get_available_for_sku_async(
        &self,
        sku: &str,
        limit: u32,
    ) -> Result<Vec<SerialNumber>> {
        let rows = sqlx::query_as::<_, SerialRow>(
            "SELECT * FROM serial_numbers WHERE sku = $1 AND status = 'available' ORDER BY created_at ASC LIMIT $2",
        )
        .bind(sku)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }

    pub async fn get_for_lot_async(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        let rows = sqlx::query_as::<_, SerialRow>("SELECT * FROM serial_numbers WHERE lot_id = $1")
            .bind(lot_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }

    pub async fn get_for_customer_async(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        let rows = sqlx::query_as::<_, SerialRow>(
            "SELECT * FROM serial_numbers WHERE current_owner_id = $1 AND current_owner_type = 'customer'",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }

    pub async fn count_async(&self, filter: SerialFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM serial_numbers WHERE 1=1");

        if let Some(serial) = &filter.serial {
            builder.push(" AND serial = ").push_bind(serial);
        }
        if let Some(prefix) = &filter.serial_prefix {
            builder.push(" AND serial LIKE ").push_bind(format!("{}%", prefix));
        }
        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(statuses) = &filter.statuses {
            if !statuses.is_empty() {
                builder.push(" AND status IN (");
                {
                    let mut separated = builder.separated(", ");
                    for status in statuses {
                        separated.push_bind(status.to_string());
                    }
                }
                builder.push(")");
            }
        }
        if let Some(lot_id) = &filter.lot_id {
            builder.push(" AND lot_id = ").push_bind(lot_id);
        }
        if let Some(lot_number) = &filter.lot_number {
            builder.push(" AND lot_number = ").push_bind(lot_number);
        }
        if let Some(loc_id) = filter.location_id {
            builder.push(" AND current_location_id = ").push_bind(loc_id);
        }
        if let Some(owner_id) = &filter.owner_id {
            builder.push(" AND current_owner_id = ").push_bind(owner_id);
        }
        if let Some(owner_type) = &filter.owner_type {
            builder.push(" AND current_owner_type = ").push_bind(owner_type);
        }
        if let Some(warranty_id) = &filter.warranty_id {
            builder.push(" AND warranty_id = ").push_bind(warranty_id);
        }
        if let Some(has_warranty) = filter.has_warranty {
            if has_warranty {
                builder.push(" AND warranty_id IS NOT NULL");
            } else {
                builder.push(" AND warranty_id IS NULL");
            }
        }
        if let Some(after) = &filter.manufactured_after {
            builder.push(" AND manufactured_at >= ").push_bind(after);
        }
        if let Some(before) = &filter.manufactured_before {
            builder.push(" AND manufactured_at <= ").push_bind(before);
        }
        if let Some(after) = &filter.sold_after {
            builder.push(" AND sold_at >= ").push_bind(after);
        }
        if let Some(before) = &filter.sold_before {
            builder.push(" AND sold_at <= ").push_bind(before);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateSerialNumber>,
    ) -> Result<BatchResult<SerialNumber>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(serial) => result.record_success(serial),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM serial_numbers WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<SerialRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }

    pub async fn get_batch_by_serial_async(
        &self,
        serials: Vec<String>,
    ) -> Result<Vec<SerialNumber>> {
        if serials.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM serial_numbers WHERE serial IN (");
        {
            let mut separated = builder.separated(", ");
            for serial in serials {
                separated.push_bind(serial);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<SerialRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut serials = Vec::with_capacity(rows.len());
        for row in rows {
            serials.push(Self::row_to_serial(row)?);
        }
        Ok(serials)
    }
}

impl SerialRepository for PgSerialRepository {
    fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        block_on(self.create_async(input))
    }

    fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>> {
        block_on(self.create_bulk_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        block_on(self.get_async(id))
    }

    fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>> {
        block_on(self.get_by_serial_async(serial))
    }

    fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        block_on(self.update_async(id, input))
    }

    fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_async(id))
    }

    fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        block_on(self.change_status_async(input))
    }

    fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        block_on(self.reserve_async(input))
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        block_on(self.release_reservation_async(reservation_id))
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        block_on(self.confirm_reservation_async(reservation_id))
    }

    fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber> {
        block_on(self.move_serial_async(input))
    }

    fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber> {
        block_on(self.transfer_ownership_async(input))
    }

    fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber> {
        block_on(self.mark_sold_async(id, customer_id, order_id))
    }

    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        block_on(self.mark_shipped_async(id, shipment_id))
    }

    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        block_on(self.mark_returned_async(id, return_id))
    }

    fn activate(&self, id: Uuid) -> Result<SerialNumber> {
        block_on(self.activate_async(id))
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        block_on(self.quarantine_async(id, reason))
    }

    fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber> {
        block_on(self.release_quarantine_async(id))
    }

    fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        block_on(self.scrap_async(id, reason))
    }

    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<SerialReservation>> {
        block_on(self.get_reservation_async(reservation_id))
    }

    fn release_expired_reservations(&self, now: DateTime<Utc>) -> Result<u64> {
        block_on(self.release_expired_reservations_async(now))
    }

    fn quarantine_for_lot(&self, lot_id: Uuid, reason: &str) -> Result<u64> {
        block_on(self.quarantine_for_lot_async(lot_id, reason))
    }

    fn release_quarantine_for_lot(&self, lot_id: Uuid) -> Result<u64> {
        block_on(self.release_quarantine_for_lot_async(lot_id))
    }

    fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>> {
        block_on(self.get_history_async(serial_id, filter))
    }

    fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        block_on(self.lookup_async(serial))
    }

    fn validate(&self, serial: &str) -> Result<SerialValidation> {
        block_on(self.validate_async(serial))
    }

    fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>> {
        block_on(self.get_available_for_sku_async(sku, limit))
    }

    fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        block_on(self.get_for_lot_async(lot_id))
    }

    fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        block_on(self.get_for_customer_async(customer_id))
    }

    fn count(&self, filter: SerialFilter) -> Result<u64> {
        block_on(self.count_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> Result<BatchResult<SerialNumber>> {
        block_on(self.create_batch_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        block_on(self.get_batch_async(ids))
    }

    fn get_batch_by_serial(&self, serials: Vec<String>) -> Result<Vec<SerialNumber>> {
        block_on(self.get_batch_by_serial_async(serials))
    }
}
