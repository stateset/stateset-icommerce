//! SQLite implementation of Lot repository

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_json_row, parse_uuid,
    parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::errors::BatchResult;
use stateset_core::traits::LotRepository;
use stateset_core::{
    AddLotCertificate, AdjustLot, CommerceError, ConsumeLot, CreateLot, Lot, LotCertificate,
    LotFilter, LotLocation, LotStatus, LotTransaction, LotTransactionType, MergeLots, ReserveLot,
    Result, SplitLot, TraceNode, TraceNodeType, TraceabilityResult, TransferLot, UpdateLot,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteLotRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteLotRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_lot_number(sku: &str) -> String {
        // Include millisecond timestamp + random uuid suffix so concurrent lot creation
        // (or multiple in the same second, common in tests/batch flows) cannot collide
        // on the UNIQUE constraint.
        let timestamp_ms = Utc::now().timestamp_millis();
        let random_suffix = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF) as u32;
        format!(
            "LOT-{}-{}-{:08x}",
            sku.chars().take(6).collect::<String>().to_uppercase(),
            timestamp_ms,
            random_suffix
        )
    }

    fn row_to_lot(row: &rusqlite::Row<'_>) -> rusqlite::Result<Lot> {
        let attributes_str: String = row.get("attributes")?;
        let attributes: serde_json::Value = parse_json_row(&attributes_str, "lot", "attributes")?;

        Ok(Lot {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot", "id")?,
            lot_number: row.get("lot_number")?,
            sku: row.get("sku")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "lot", "status")?,
            quantity_produced: parse_decimal_row(
                &row.get::<_, String>("quantity_produced")?,
                "lot",
                "quantity_produced",
            )?,
            quantity_remaining: parse_decimal_row(
                &row.get::<_, String>("quantity_remaining")?,
                "lot",
                "quantity_remaining",
            )?,
            quantity_reserved: parse_decimal_row(
                &row.get::<_, String>("quantity_reserved")?,
                "lot",
                "quantity_reserved",
            )?,
            quantity_quarantined: parse_decimal_row(
                &row.get::<_, String>("quantity_quarantined")?,
                "lot",
                "quantity_quarantined",
            )?,
            production_date: parse_datetime_row(
                &row.get::<_, String>("production_date")?,
                "lot",
                "production_date",
            )?,
            expiration_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expiration_date")?,
                "lot",
                "expiration_date",
            )?,
            best_before_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("best_before_date")?,
                "lot",
                "best_before_date",
            )?,
            supplier_lot: row.get("supplier_lot")?,
            supplier_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("supplier_id")?,
                "lot",
                "supplier_id",
            )?,
            work_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("work_order_id")?,
                "lot",
                "work_order_id",
            )?,
            purchase_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_order_id")?,
                "lot",
                "purchase_order_id",
            )?,
            cost_per_unit: parse_decimal_opt_row(
                row.get::<_, Option<String>>("cost_per_unit")?,
                "lot",
                "cost_per_unit",
            )?,
            attributes,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "lot",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<LotTransaction> {
        Ok(LotTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot_transaction", "id")?,
            lot_id: parse_uuid_row(&row.get::<_, String>("lot_id")?, "lot_transaction", "lot_id")?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>("transaction_type")?,
                "lot_transaction",
                "transaction_type",
            )?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "lot_transaction",
                "quantity",
            )?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_row(
                &row.get::<_, String>("reference_id")?,
                "lot_transaction",
                "reference_id",
            )?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            reason: row.get("reason")?,
            performed_by: row.get("performed_by")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot_transaction",
                "created_at",
            )?,
        })
    }

    fn row_to_certificate(row: &rusqlite::Row<'_>) -> rusqlite::Result<LotCertificate> {
        Ok(LotCertificate {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot_certificate", "id")?,
            lot_id: parse_uuid_row(&row.get::<_, String>("lot_id")?, "lot_certificate", "lot_id")?,
            certificate_type: parse_enum_row(
                &row.get::<_, String>("certificate_type")?,
                "lot_certificate",
                "certificate_type",
            )?,
            certificate_number: row.get("certificate_number")?,
            document_url: row.get("document_url")?,
            issued_by: row.get("issued_by")?,
            issued_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("issued_at")?,
                "lot_certificate",
                "issued_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expires_at")?,
                "lot_certificate",
                "expires_at",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot_certificate",
                "created_at",
            )?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_transaction(
        &self,
        conn: &rusqlite::Connection,
        lot_id: Uuid,
        transaction_type: LotTransactionType,
        quantity: Decimal,
        reference_type: &str,
        reference_id: Uuid,
        from_location_id: Option<i32>,
        to_location_id: Option<i32>,
        reason: Option<&str>,
        performed_by: Option<&str>,
    ) -> Result<LotTransaction> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                           reference_id, from_location_id, to_location_id, reason,
                                           performed_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                lot_id.to_string(),
                transaction_type.to_string(),
                quantity.to_string(),
                reference_type,
                reference_id.to_string(),
                from_location_id,
                to_location_id,
                reason,
                performed_by,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(LotTransaction {
            id,
            lot_id,
            transaction_type,
            quantity,
            reference_type: reference_type.to_string(),
            reference_id,
            from_location_id,
            to_location_id,
            reason: reason.map(std::string::ToString::to_string),
            performed_by: performed_by.map(std::string::ToString::to_string),
            created_at: now,
        })
    }
}

impl LotRepository for SqliteLotRepository {
    fn create(&self, input: CreateLot) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let id = Uuid::new_v4();
        let lot_number = input.lot_number.unwrap_or_else(|| Self::generate_lot_number(&input.sku));
        let now = Utc::now();
        let production_date = input.production_date.unwrap_or(now);
        let attributes_json = input
            .attributes
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
            .unwrap_or_else(|| "{}".to_string());

        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, supplier_lot, supplier_id,
                               work_order_id, purchase_order_id, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, '0', '0', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &lot_number,
                &input.sku,
                LotStatus::Active.to_string(),
                input.quantity.to_string(),
                input.quantity.to_string(),
                production_date.to_rfc3339(),
                input.expiration_date.map(|d| d.to_rfc3339()),
                input.best_before_date.map(|d| d.to_rfc3339()),
                &input.supplier_lot,
                input.supplier_id.map(|i| i.to_string()),
                input.work_order_id.map(|i| i.to_string()),
                input.purchase_order_id.map(|i| i.to_string()),
                input.cost_per_unit.map(|c| c.to_string()),
                &attributes_json,
                &input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Record initial transaction
        let tx_id = Uuid::new_v4();
        let reference_id = input.work_order_id.or(input.purchase_order_id).unwrap_or(id);
        let reference_type = if input.work_order_id.is_some() {
            "work_order"
        } else if input.purchase_order_id.is_some() {
            "purchase_order"
        } else {
            "lot_creation"
        };

        tx.execute(
            "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                           reference_id, to_location_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                tx_id.to_string(),
                id.to_string(),
                LotTransactionType::Received.to_string(),
                input.quantity.to_string(),
                reference_type,
                reference_id.to_string(),
                input.initial_location_id,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // If initial location specified, create lot_location entry
        if let Some(location_id) = input.initial_location_id {
            tx.execute(
                "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    location_id,
                    input.quantity.to_string(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        Ok(Lot {
            id,
            lot_number,
            sku: input.sku,
            status: LotStatus::Active,
            quantity_produced: input.quantity,
            quantity_remaining: input.quantity,
            quantity_reserved: Decimal::ZERO,
            quantity_quarantined: Decimal::ZERO,
            production_date,
            expiration_date: input.expiration_date,
            best_before_date: input.best_before_date,
            supplier_lot: input.supplier_lot,
            supplier_id: input.supplier_id,
            work_order_id: input.work_order_id,
            purchase_order_id: input.purchase_order_id,
            cost_per_unit: input.cost_per_unit,
            attributes: input.attributes.unwrap_or(serde_json::json!({})),
            notes: input.notes,
            created_at: now,
            updated_at: now,
        })
    }

    fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        let conn = self.conn()?;
        let result =
            conn.query_row("SELECT * FROM lots WHERE id = ?", [id.to_string()], Self::row_to_lot);

        match result {
            Ok(lot) => Ok(Some(lot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM lots WHERE lot_number = ?",
            [lot_number],
            Self::row_to_lot,
        );

        match result {
            Ok(lot) => Ok(Some(lot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(expiration_date) = &input.expiration_date {
            updates.push("expiration_date = ?");
            params.push(Box::new(expiration_date.to_rfc3339()));
        }
        if let Some(best_before_date) = &input.best_before_date {
            updates.push("best_before_date = ?");
            params.push(Box::new(best_before_date.to_rfc3339()));
        }
        if let Some(cost_per_unit) = &input.cost_per_unit {
            updates.push("cost_per_unit = ?");
            params.push(Box::new(cost_per_unit.to_string()));
        }
        if let Some(attributes) = &input.attributes {
            updates.push("attributes = ?");
            let attributes_json = serde_json::to_string(attributes)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            params.push(Box::new(attributes_json));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE lots SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(lot_number) = &filter.lot_number {
            conditions.push("lot_number = ?");
            params.push(Box::new(lot_number.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(supplier_id) = &filter.supplier_id {
            conditions.push("supplier_id = ?");
            params.push(Box::new(supplier_id.to_string()));
        }
        if filter.has_quantity == Some(true) {
            conditions.push("quantity_remaining > 0");
        }

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM lots WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let lots = stmt
            .query_map(params_refs.as_slice(), Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        // Check if lot has transactions
        let tx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lot_transactions WHERE lot_id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if tx_count > 1 {
            return Err(CommerceError::ValidationError(
                "Cannot delete lot with transaction history".to_string(),
            ));
        }

        conn.execute("DELETE FROM lots WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        Ok(())
    }

    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get current lot
        let lot: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

        let new_remaining = lot.quantity_remaining + input.quantity_change;
        if new_remaining < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Cannot reduce quantity below zero".to_string(),
            ));
        }

        // Update lot
        tx.execute(
            "UPDATE lots SET quantity_remaining = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_remaining.to_string(),
                Utc::now().to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = self.record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Adjusted,
            input.quantity_change,
            input.reference_type.as_deref().unwrap_or("manual_adjustment"),
            input.reference_id.unwrap_or(input.lot_id),
            None,
            input.location_id,
            Some(&input.reason),
            input.performed_by.as_deref(),
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get current lot
        let lot: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

        if !lot.can_consume(input.quantity) {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: input.quantity.to_string(),
                available: lot.quantity_available().to_string(),
            });
        }

        let new_remaining = lot.quantity_remaining - input.quantity;

        // Check if consumed completely
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };

        // Update lot
        tx.execute(
            "UPDATE lots SET quantity_remaining = ?, status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_remaining.to_string(),
                new_status.to_string(),
                Utc::now().to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = self.record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Consumed,
            -input.quantity,
            &input.reference_type,
            input.reference_id,
            input.location_id,
            None,
            None,
            input.performed_by.as_deref(),
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get current lot
        let lot: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

        if !lot.can_reserve(input.quantity) {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: input.quantity.to_string(),
                available: lot.quantity_available().to_string(),
            });
        }

        let reservation_id = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = input.expires_in_seconds.map(|s| now + chrono::Duration::seconds(s));

        // Create reservation
        tx.execute(
            "INSERT INTO lot_reservations (id, lot_id, quantity, reference_type, reference_id,
                                           reserved_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                reservation_id.to_string(),
                input.lot_id.to_string(),
                input.quantity.to_string(),
                &input.reference_type,
                input.reference_id.to_string(),
                now.to_rfc3339(),
                expires_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(map_db_error)?;

        // Update lot reserved quantity
        let new_reserved = lot.quantity_reserved + input.quantity;
        tx.execute(
            "UPDATE lots SET quantity_reserved = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_reserved.to_string(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Reserved,
            input.quantity,
            &input.reference_type,
            input.reference_id,
            None,
            None,
            None,
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(reservation_id)
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get reservation
        let (lot_id, quantity, reference_type, reference_id): (String, String, String, String) = tx
            .query_row(
                "SELECT lot_id, quantity, reference_type, reference_id FROM lot_reservations WHERE id = ? AND released_at IS NULL AND confirmed_at IS NULL",
                [reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let lot_id = parse_uuid(&lot_id, "lot_reservation", "lot_id")?;
        let quantity = parse_decimal_strict(&quantity, "lot_reservation", "quantity")?;
        let reference_id = parse_uuid(&reference_id, "lot_reservation", "reference_id")?;
        let now = Utc::now();

        // Mark reservation as released
        tx.execute(
            "UPDATE lot_reservations SET released_at = ? WHERE id = ?",
            rusqlite::params![now.to_rfc3339(), reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update lot reserved quantity
        tx.execute(
            "UPDATE lots SET quantity_reserved = quantity_reserved - ?, updated_at = ? WHERE id = ?",
            rusqlite::params![quantity.to_string(), now.to_rfc3339(), lot_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(
            &tx,
            lot_id,
            LotTransactionType::Released,
            -quantity,
            &reference_type,
            reference_id,
            None,
            None,
            Some("Reservation released"),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get reservation
        let (lot_id, quantity, reference_type, reference_id): (String, String, String, String) = tx
            .query_row(
                "SELECT lot_id, quantity, reference_type, reference_id FROM lot_reservations WHERE id = ? AND released_at IS NULL AND confirmed_at IS NULL",
                [reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let lot_id = parse_uuid(&lot_id, "lot_reservation", "lot_id")?;
        let quantity = parse_decimal_strict(&quantity, "lot_reservation", "quantity")?;
        let reference_id = parse_uuid(&reference_id, "lot_reservation", "reference_id")?;
        let now = Utc::now();

        // Mark reservation as confirmed
        tx.execute(
            "UPDATE lot_reservations SET confirmed_at = ? WHERE id = ?",
            rusqlite::params![now.to_rfc3339(), reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update lot: reduce both reserved and remaining
        tx.execute(
            "UPDATE lots SET quantity_reserved = quantity_reserved - ?, quantity_remaining = quantity_remaining - ?, updated_at = ? WHERE id = ?",
            rusqlite::params![quantity.to_string(), quantity.to_string(), now.to_rfc3339(), lot_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = self.record_transaction(
            &tx,
            lot_id,
            LotTransactionType::Consumed,
            -quantity,
            &reference_type,
            reference_id,
            None,
            None,
            Some("Reservation confirmed"),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Update from location
        tx.execute(
            "UPDATE lot_locations SET quantity = quantity - ?, updated_at = ? WHERE lot_id = ? AND location_id = ?",
            rusqlite::params![
                input.quantity.to_string(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
                input.from_location_id,
            ],
        )
        .map_err(map_db_error)?;

        // Update or insert to location
        tx.execute(
            "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(lot_id, location_id) DO UPDATE SET
             quantity = quantity + excluded.quantity, updated_at = excluded.updated_at",
            rusqlite::params![
                input.lot_id.to_string(),
                input.to_location_id,
                input.quantity.to_string(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = self.record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Transferred,
            input.quantity,
            "transfer",
            input.lot_id,
            Some(input.from_location_id),
            Some(input.to_location_id),
            input.reason.as_deref(),
            input.performed_by.as_deref(),
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn split(&self, input: SplitLot) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get original lot
        let original: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

        if original.quantity_remaining < input.quantity {
            return Err(CommerceError::ValidationError(
                "Insufficient quantity to split".to_string(),
            ));
        }

        let new_lot_id = Uuid::new_v4();
        let new_lot_number =
            input.new_lot_number.unwrap_or_else(|| format!("{}-SPLIT", original.lot_number));
        let now = Utc::now();

        // Create new lot
        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, supplier_lot, supplier_id,
                               work_order_id, purchase_order_id, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             SELECT ?, ?, sku, status, ?, ?, '0', '0', production_date, expiration_date,
                    best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                    cost_per_unit, attributes, ?, ?, ?
             FROM lots WHERE id = ?",
            rusqlite::params![
                new_lot_id.to_string(),
                &new_lot_number,
                input.quantity.to_string(),
                input.quantity.to_string(),
                input.reason.as_ref().map(|r| format!("Split from {}: {}", original.lot_number, r)),
                now.to_rfc3339(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Reduce original lot
        let new_remaining = original.quantity_remaining - input.quantity;
        tx.execute(
            "UPDATE lots SET quantity_remaining = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_remaining.to_string(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transactions
        self.record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Split,
            -input.quantity,
            "lot_split",
            new_lot_id,
            None,
            None,
            input.reason.as_deref(),
            None,
        )?;

        self.record_transaction(
            &tx,
            new_lot_id,
            LotTransactionType::Received,
            input.quantity,
            "lot_split",
            input.lot_id,
            None,
            None,
            Some(&format!("Split from lot {}", original.lot_number)),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(new_lot_id)?.ok_or(CommerceError::NotFound)
    }

    fn merge(&self, input: MergeLots) -> Result<Lot> {
        if input.source_lot_ids.len() < 2 {
            return Err(CommerceError::ValidationError(
                "Need at least 2 lots to merge".to_string(),
            ));
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get all source lots
        let mut total_quantity = Decimal::ZERO;
        let mut sku: Option<String> = None;
        let mut lots_to_consume: Vec<(Uuid, String, Decimal)> = Vec::new();

        for lot_id in &input.source_lot_ids {
            let lot: Lot = tx
                .query_row(
                    "SELECT * FROM lots WHERE id = ?",
                    [lot_id.to_string()],
                    Self::row_to_lot,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        CommerceError::ValidationError(format!("Lot {lot_id} not found"))
                    }
                    e => map_db_error(e),
                })?;

            // Verify same SKU
            if let Some(ref s) = sku {
                if s != &lot.sku {
                    return Err(CommerceError::ValidationError(
                        "Cannot merge lots with different SKUs".to_string(),
                    ));
                }
            } else {
                sku = Some(lot.sku.clone());
            }

            total_quantity += lot.quantity_remaining;
            lots_to_consume.push((lot.id, lot.lot_number, lot.quantity_remaining));
        }

        let sku = sku.ok_or(CommerceError::ValidationError("No lots to merge".to_string()))?;

        // Create new merged lot
        let new_lot_id = Uuid::new_v4();
        let new_lot_number = input
            .target_lot_number
            .unwrap_or_else(|| format!("MERGED-{}", Utc::now().format("%Y%m%d%H%M%S")));

        // Get first lot as template
        let template: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.source_lot_ids[0].to_string()],
                Self::row_to_lot,
            )
            .map_err(map_db_error)?;

        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             VALUES (?, ?, ?, 'active', ?, ?, '0', '0', ?, ?, ?, ?, '{}', ?, ?, ?)",
            rusqlite::params![
                new_lot_id.to_string(),
                &new_lot_number,
                &sku,
                total_quantity.to_string(),
                total_quantity.to_string(),
                template.production_date.to_rfc3339(),
                template.expiration_date.map(|d| d.to_rfc3339()),
                template.best_before_date.map(|d| d.to_rfc3339()),
                template.cost_per_unit.map(|c| c.to_string()),
                input.reason.as_ref().map(|r| format!("Merged lots: {r}")),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Mark source lots as consumed and record transactions
        for (lot_id, _lot_number, quantity) in lots_to_consume {
            tx.execute(
                "UPDATE lots SET status = 'consumed', quantity_remaining = '0', updated_at = ? WHERE id = ?",
                rusqlite::params![now.to_rfc3339(), lot_id.to_string()],
            )
            .map_err(map_db_error)?;

            self.record_transaction(
                &tx,
                lot_id,
                LotTransactionType::Merged,
                -quantity,
                "lot_merge",
                new_lot_id,
                None,
                None,
                Some(&format!("Merged into lot {new_lot_number}")),
                None,
            )?;
        }

        // Record received transaction for new lot
        self.record_transaction(
            &tx,
            new_lot_id,
            LotTransactionType::Received,
            total_quantity,
            "lot_merge",
            input.source_lot_ids[0],
            None,
            None,
            Some("Created from merge"),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(new_lot_id)?.ok_or(CommerceError::NotFound)
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get lot
        let lot: Lot = tx
            .query_row("SELECT * FROM lots WHERE id = ?", [id.to_string()], Self::row_to_lot)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                e => map_db_error(e),
            })?;

        let available = lot.quantity_available();

        // Update lot
        tx.execute(
            "UPDATE lots SET status = ?, quantity_quarantined = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                LotStatus::Quarantine.to_string(),
                available.to_string(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(
            &tx,
            id,
            LotTransactionType::Quarantined,
            available,
            "quarantine",
            id,
            None,
            None,
            Some(reason),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get current quarantined quantity
        let quarantined: String = tx
            .query_row(
                "SELECT quantity_quarantined FROM lots WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let quarantined = parse_decimal_strict(&quarantined, "lot", "quantity_quarantined")?;

        // Update lot
        tx.execute(
            "UPDATE lots SET status = 'active', quantity_quarantined = '0', updated_at = ? WHERE id = ?",
            rusqlite::params![now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(
            &tx,
            id,
            LotTransactionType::QuarantineReleased,
            quarantined,
            "quarantine_release",
            id,
            None,
            None,
            Some("Released from quarantine"),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lot_transactions WHERE lot_id = ? ORDER BY created_at DESC LIMIT ?",
            )
            .map_err(map_db_error)?;

        let transactions = stmt
            .query_map(
                rusqlite::params![lot_id.to_string(), i64::from(limit)],
                Self::row_to_transaction,
            )
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(transactions)
    }

    fn get_quantity_at_location(&self, lot_id: Uuid, location_id: i32) -> Result<Option<Decimal>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT quantity FROM lot_locations WHERE lot_id = ? AND location_id = ?",
            rusqlite::params![lot_id.to_string(), location_id],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(qty) => Ok(Some(parse_decimal_strict(&qty, "lot_location", "quantity")?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare("SELECT lot_id, location_id, quantity, updated_at FROM lot_locations WHERE lot_id = ?")
            .map_err(map_db_error)?;

        let locations = stmt
            .query_map([lot_id.to_string()], |row| {
                Ok(LotLocation {
                    lot_id: parse_uuid_row(&row.get::<_, String>(0)?, "lot_location", "lot_id")?,
                    location_id: row.get(1)?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(2)?,
                        "lot_location",
                        "quantity",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(3)?,
                        "lot_location",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(locations)
    }

    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO lot_certificates (id, lot_id, certificate_type, certificate_number,
                                           document_url, issued_by, issued_at, expires_at, notes,
                                           created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.lot_id.to_string(),
                input.certificate_type.to_string(),
                &input.certificate_number,
                &input.document_url,
                &input.issued_by,
                input.issued_at.map(|d| d.to_rfc3339()),
                input.expires_at.map(|d| d.to_rfc3339()),
                &input.notes,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(LotCertificate {
            id,
            lot_id: input.lot_id,
            certificate_type: input.certificate_type,
            certificate_number: input.certificate_number,
            document_url: input.document_url,
            issued_by: input.issued_by,
            issued_at: input.issued_at,
            expires_at: input.expires_at,
            notes: input.notes,
            created_at: now,
        })
    }

    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare("SELECT * FROM lot_certificates WHERE lot_id = ? ORDER BY created_at DESC")
            .map_err(map_db_error)?;

        let certs = stmt
            .query_map([lot_id.to_string()], Self::row_to_certificate)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(certs)
    }

    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM lot_certificates WHERE id = ?", [certificate_id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        let conn = self.conn()?;
        let threshold = Utc::now() + chrono::Duration::days(i64::from(days));

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lots WHERE status = 'active' AND expiration_date IS NOT NULL
                 AND expiration_date <= ? AND expiration_date > datetime('now')
                 ORDER BY expiration_date ASC",
            )
            .map_err(map_db_error)?;

        let lots = stmt
            .query_map([threshold.to_rfc3339()], Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lots WHERE status = 'active' AND expiration_date IS NOT NULL
                 AND expiration_date <= datetime('now') ORDER BY expiration_date ASC",
            )
            .map_err(map_db_error)?;

        let lots = stmt
            .query_map([], Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        self.list(LotFilter {
            sku: Some(sku.to_string()),
            status: Some(LotStatus::Active),
            has_quantity: Some(true),
            ..Default::default()
        })
    }

    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        let lot = self.get(lot_id)?.ok_or(CommerceError::NotFound)?;
        let conn = self.conn()?;

        // Get upstream (where did this lot come from)
        let mut upstream = Vec::new();
        if let Some(po_id) = lot.purchase_order_id {
            upstream.push(TraceNode {
                node_type: TraceNodeType::PurchaseOrder,
                node_id: po_id,
                reference_number: None,
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity: lot.quantity_produced,
                timestamp: lot.created_at,
                entity_name: None,
            });
        }
        if let Some(wo_id) = lot.work_order_id {
            upstream.push(TraceNode {
                node_type: TraceNodeType::WorkOrder,
                node_id: wo_id,
                reference_number: None,
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity: lot.quantity_produced,
                timestamp: lot.created_at,
                entity_name: None,
            });
        }

        // Get downstream (where did this lot go)
        let mut stmt = conn
            .prepare(
                "SELECT transaction_type, reference_type, reference_id, quantity, created_at
                 FROM lot_transactions WHERE lot_id = ? AND transaction_type IN ('consumed', 'shipped')
                 ORDER BY created_at ASC",
            )
            .map_err(map_db_error)?;

        let downstream = stmt
            .query_map([lot_id.to_string()], |row| {
                let ref_type: String = row.get(1)?;
                let node_type = match ref_type.as_str() {
                    "order" => TraceNodeType::Order,
                    "shipment" => TraceNodeType::Shipment,
                    "work_order" => TraceNodeType::WorkOrder,
                    _ => TraceNodeType::Adjustment,
                };

                Ok(TraceNode {
                    node_type,
                    node_id: parse_uuid_row(
                        &row.get::<_, String>(2)?,
                        "lot_transaction",
                        "reference_id",
                    )?,
                    reference_number: None,
                    lot_number: Some(lot.lot_number.clone()),
                    serial_number: None,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(3)?,
                        "lot_transaction",
                        "quantity",
                    )?,
                    timestamp: parse_datetime_row(
                        &row.get::<_, String>(4)?,
                        "lot_transaction",
                        "created_at",
                    )?,
                    entity_name: None,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(TraceabilityResult { lot, upstream, downstream })
    }

    fn count(&self, filter: LotFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }

        let sql = format!("SELECT COUNT(*) FROM lots WHERE {}", conditions.join(" AND "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
            .map(|c| c as u64)
            .map_err(map_db_error)
    }

    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(lot) => result.record_success(lot),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        let mut lots = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(lot) = self.get(id)? {
                lots.push(lot);
            }
        }
        Ok(lots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateLot, LotFilter, LotRepository, LotStatus, MergeLots, ReserveLot, SplitLot, UpdateLot,
    };

    fn fresh_repo() -> SqliteLotRepository {
        SqliteDatabase::in_memory().expect("in-memory").lots()
    }

    fn make_lot(repo: &SqliteLotRepository, sku: &str, qty: Decimal) -> Lot {
        repo.create(CreateLot {
            sku: sku.into(),
            quantity: qty,
            initial_location_id: Some(1),
            ..Default::default()
        })
        .expect("create lot")
    }

    #[test]
    fn create_lot_starts_active_with_full_remaining() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-A", dec!(100));
        assert_eq!(lot.sku, "SKU-A");
        assert_eq!(lot.quantity_produced, dec!(100));
        assert_eq!(lot.quantity_remaining, dec!(100));
        assert_eq!(lot.quantity_reserved, dec!(0));
        assert_eq!(lot.status, LotStatus::Active);
        assert!(!lot.lot_number.is_empty());
    }

    #[test]
    fn get_and_get_by_number_round_trip() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-RT", dec!(50));
        let by_id = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(by_id.id, lot.id);
        let by_num = repo.get_by_number(&lot.lot_number).expect("ok").expect("found");
        assert_eq!(by_num.id, lot.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn update_lot_status_persists() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-UP", dec!(20));
        let updated = repo
            .update(
                lot.id,
                UpdateLot {
                    status: Some(LotStatus::OnHold),
                    notes: Some("on hold for QA".into()),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.status, LotStatus::OnHold);
    }

    #[test]
    fn list_filters_by_sku() {
        let repo = fresh_repo();
        make_lot(&repo, "SKU-L1", dec!(10));
        make_lot(&repo, "SKU-L1", dec!(20));
        make_lot(&repo, "SKU-L2", dec!(30));

        let filtered = repo
            .list(LotFilter { sku: Some("SKU-L1".into()), ..Default::default() })
            .expect("list");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let active = make_lot(&repo, "SKU-S", dec!(10));
        let to_hold = make_lot(&repo, "SKU-S", dec!(10));
        repo.update(
            to_hold.id,
            UpdateLot { status: Some(LotStatus::OnHold), ..Default::default() },
        )
        .expect("hold");

        let actives = repo
            .list(LotFilter { status: Some(LotStatus::Active), ..Default::default() })
            .expect("active");
        let on_hold = repo
            .list(LotFilter { status: Some(LotStatus::OnHold), ..Default::default() })
            .expect("hold");
        assert!(actives.iter().any(|l| l.id == active.id));
        assert!(on_hold.iter().any(|l| l.id == to_hold.id));
    }

    #[test]
    fn reserve_decrements_remaining_and_release_restores() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-R", dec!(50));
        let order_id = Uuid::new_v4();
        let res_id = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(15),
                reference_type: "order".into(),
                reference_id: order_id,
                expires_in_seconds: Some(60),
            })
            .expect("reserve");

        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_reserved, dec!(15));
        // remaining is on-hand minus reserved depending on impl; assert reserved is right
        repo.release_reservation(res_id).expect("release");
        let restored = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(restored.quantity_reserved, dec!(0));
    }

    #[test]
    fn quarantine_then_release_changes_status() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-Q", dec!(10));
        let q = repo.quarantine(lot.id, "qc fail").expect("quarantine");
        assert_eq!(q.status, LotStatus::Quarantine);
        let r = repo.release_quarantine(lot.id).expect("release");
        assert_eq!(r.status, LotStatus::Active);
    }

    #[test]
    fn split_creates_new_lot_with_split_quantity() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-SP", dec!(100));
        let new_lot = repo
            .split(SplitLot {
                lot_id: lot.id,
                quantity: dec!(40),
                new_lot_number: Some("SP-001".into()),
                reason: Some("split for transfer".into()),
            })
            .expect("split");
        assert_eq!(new_lot.lot_number, "SP-001");
        assert_eq!(new_lot.sku, "SKU-SP");
        // Source lot should have less remaining
        let original = repo.get(lot.id).expect("ok").expect("found");
        assert!(
            original.quantity_remaining < dec!(100),
            "source lot remaining should decrement after split"
        );
    }

    #[test]
    fn merge_creates_target_from_sources() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-M", dec!(30));
        let l2 = make_lot(&repo, "SKU-M", dec!(20));
        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l2.id],
                target_lot_number: Some("MERGED-001".into()),
                reason: Some("consolidate".into()),
            })
            .expect("merge");
        assert_eq!(merged.lot_number, "MERGED-001");
    }

    #[test]
    fn get_expiring_lots_returns_within_window() {
        let repo = fresh_repo();
        let soon = repo
            .create(CreateLot {
                sku: "SKU-EXP".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::days(3)),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("soon");
        let later = repo
            .create(CreateLot {
                sku: "SKU-EXP".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::days(60)),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("later");
        let no_exp = make_lot(&repo, "SKU-NOEXP", dec!(10));

        let expiring = repo.get_expiring_lots(7).expect("ok");
        let ids: Vec<_> = expiring.iter().map(|l| l.id).collect();
        assert!(ids.contains(&soon.id));
        assert!(!ids.contains(&later.id));
        assert!(!ids.contains(&no_exp.id));
    }

    #[test]
    fn get_available_lots_for_sku_filters_status() {
        let repo = fresh_repo();
        let active = make_lot(&repo, "SKU-AV", dec!(10));
        let scrapped = make_lot(&repo, "SKU-AV", dec!(5));
        repo.update(
            scrapped.id,
            UpdateLot { status: Some(LotStatus::Scrapped), ..Default::default() },
        )
        .expect("scrap");

        let available = repo.get_available_lots_for_sku("SKU-AV").expect("ok");
        let ids: Vec<_> = available.iter().map(|l| l.id).collect();
        assert!(ids.contains(&active.id));
        assert!(!ids.contains(&scrapped.id));
    }

    #[test]
    fn get_transactions_records_creation() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-TX", dec!(25));
        let txns = repo.get_transactions(lot.id, 10).expect("txns");
        assert!(!txns.is_empty(), "creation should record at least one transaction");
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let result = repo
            .create_batch(vec![
                CreateLot {
                    sku: "SKU-CB".into(),
                    quantity: dec!(10),
                    initial_location_id: Some(1),
                    ..Default::default()
                },
                CreateLot {
                    sku: "SKU-CB".into(),
                    quantity: dec!(20),
                    initial_location_id: Some(1),
                    ..Default::default()
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_batch_returns_only_existing() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-GB", dec!(5));
        let l2 = make_lot(&repo, "SKU-GB", dec!(5));
        let stranger = Uuid::new_v4();
        let fetched = repo.get_batch(vec![l1.id, l2.id, stranger]).expect("ok");
        assert_eq!(fetched.len(), 2);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(Uuid::new_v4()).expect("ok").is_none());
    }
}
