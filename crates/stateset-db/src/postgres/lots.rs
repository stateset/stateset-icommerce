//! PostgreSQL implementation of lot repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AddLotCertificate, AdjustLot, BatchResult, CertificateType, CommerceError, ConsumeLot,
    CreateLot, Lot, LotCertificate, LotFilter, LotLocation, LotRepository, LotStatus,
    LotTransaction, LotTransactionType, MergeLots, ReserveLot, Result, SplitLot, TraceNode,
    TraceNodeType, TraceabilityResult, TransferLot, UpdateLot, validate_batch_size,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgLotRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct LotRow {
    id: Uuid,
    lot_number: String,
    sku: String,
    status: String,
    quantity_produced: Decimal,
    quantity_remaining: Decimal,
    quantity_reserved: Decimal,
    quantity_quarantined: Decimal,
    production_date: DateTime<Utc>,
    expiration_date: Option<DateTime<Utc>>,
    best_before_date: Option<DateTime<Utc>>,
    supplier_lot: Option<String>,
    supplier_id: Option<Uuid>,
    work_order_id: Option<Uuid>,
    purchase_order_id: Option<Uuid>,
    cost_per_unit: Option<Decimal>,
    attributes: serde_json::Value,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotTransactionRow {
    id: Uuid,
    lot_id: Uuid,
    transaction_type: String,
    quantity: Decimal,
    reference_type: String,
    reference_id: Uuid,
    from_location_id: Option<i32>,
    to_location_id: Option<i32>,
    reason: Option<String>,
    performed_by: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotCertificateRow {
    id: Uuid,
    lot_id: Uuid,
    certificate_type: String,
    certificate_number: Option<String>,
    document_url: Option<String>,
    issued_by: Option<String>,
    issued_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotLocationRow {
    lot_id: Uuid,
    location_id: i32,
    quantity: Decimal,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotReservationRow {
    lot_id: Uuid,
    quantity: Decimal,
    reference_type: String,
    reference_id: Uuid,
}

impl PgLotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn generate_lot_number(sku: &str) -> String {
        format!(
            "LOT-{}-{}",
            sku.chars().take(6).collect::<String>().to_uppercase(),
            Utc::now().format("%Y%m%d%H%M%S")
        )
    }

    fn row_to_lot(row: LotRow) -> Result<Lot> {
        let status: LotStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid lot.status '{}': {}", row.status, e))
        })?;

        Ok(Lot {
            id: row.id,
            lot_number: row.lot_number,
            sku: row.sku,
            status,
            quantity_produced: row.quantity_produced,
            quantity_remaining: row.quantity_remaining,
            quantity_reserved: row.quantity_reserved,
            quantity_quarantined: row.quantity_quarantined,
            production_date: row.production_date,
            expiration_date: row.expiration_date,
            best_before_date: row.best_before_date,
            supplier_lot: row.supplier_lot,
            supplier_id: row.supplier_id,
            work_order_id: row.work_order_id,
            purchase_order_id: row.purchase_order_id,
            cost_per_unit: row.cost_per_unit,
            attributes: row.attributes,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_transaction(row: LotTransactionRow) -> Result<LotTransaction> {
        let transaction_type: LotTransactionType = row.transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid lot_transaction.transaction_type '{}': {}",
                row.transaction_type, e
            ))
        })?;

        Ok(LotTransaction {
            id: row.id,
            lot_id: row.lot_id,
            transaction_type,
            quantity: row.quantity,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            from_location_id: row.from_location_id,
            to_location_id: row.to_location_id,
            reason: row.reason,
            performed_by: row.performed_by,
            created_at: row.created_at,
        })
    }

    fn row_to_certificate(row: LotCertificateRow) -> Result<LotCertificate> {
        let certificate_type: CertificateType = row.certificate_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid lot_certificate.certificate_type '{}': {}",
                row.certificate_type, e
            ))
        })?;

        Ok(LotCertificate {
            id: row.id,
            lot_id: row.lot_id,
            certificate_type,
            certificate_number: row.certificate_number,
            document_url: row.document_url,
            issued_by: row.issued_by,
            issued_at: row.issued_at,
            expires_at: row.expires_at,
            notes: row.notes,
            created_at: row.created_at,
        })
    }

    fn row_to_location(row: LotLocationRow) -> LotLocation {
        LotLocation {
            lot_id: row.lot_id,
            location_id: row.location_id,
            quantity: row.quantity,
            updated_at: row.updated_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_transaction_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
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

        sqlx::query(
            r#"
            INSERT INTO lot_transactions (
                id, lot_id, transaction_type, quantity, reference_type, reference_id,
                from_location_id, to_location_id, reason, performed_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(id)
        .bind(lot_id)
        .bind(transaction_type.to_string())
        .bind(quantity)
        .bind(reference_type)
        .bind(reference_id)
        .bind(from_location_id)
        .bind(to_location_id)
        .bind(reason)
        .bind(performed_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
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
            reason: reason.map(|s| s.to_string()),
            performed_by: performed_by.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub async fn create_async(&self, input: CreateLot) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let id = Uuid::new_v4();
        let lot_number = input.lot_number.unwrap_or_else(|| Self::generate_lot_number(&input.sku));
        let now = Utc::now();
        let production_date = input.production_date.unwrap_or(now);
        let attributes = input.attributes.unwrap_or_else(|| serde_json::json!({}));

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                cost_per_unit, attributes, notes, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,0,0,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
            "#,
        )
        .bind(id)
        .bind(&lot_number)
        .bind(&input.sku)
        .bind(LotStatus::Active.to_string())
        .bind(input.quantity)
        .bind(input.quantity)
        .bind(production_date)
        .bind(input.expiration_date)
        .bind(input.best_before_date)
        .bind(&input.supplier_lot)
        .bind(input.supplier_id)
        .bind(input.work_order_id)
        .bind(input.purchase_order_id)
        .bind(input.cost_per_unit)
        .bind(&attributes)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let reference_id = input.work_order_id.or(input.purchase_order_id).unwrap_or(id);
        let reference_type = if input.work_order_id.is_some() {
            "work_order"
        } else if input.purchase_order_id.is_some() {
            "purchase_order"
        } else {
            "lot_creation"
        };

        self.record_transaction_tx(
            &mut tx,
            id,
            LotTransactionType::Received,
            input.quantity,
            reference_type,
            reference_id,
            None,
            input.initial_location_id,
            None,
            None,
        )
        .await?;

        if let Some(location_id) = input.initial_location_id {
            sqlx::query(
                r#"
                INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
                VALUES ($1,$2,$3,$4)
                ON CONFLICT (lot_id, location_id) DO UPDATE SET
                    quantity = excluded.quantity,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(id)
            .bind(location_id)
            .bind(input.quantity)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

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
            attributes,
            notes: input.notes,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<Lot>> {
        let row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_lot).transpose()
    }

    pub async fn get_by_number_async(&self, lot_number: &str) -> Result<Option<Lot>> {
        let row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE lot_number = $1")
            .bind(lot_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_lot).transpose()
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE lots SET
                status = COALESCE($1, status),
                expiration_date = COALESCE($2, expiration_date),
                best_before_date = COALESCE($3, best_before_date),
                cost_per_unit = COALESCE($4, cost_per_unit),
                attributes = COALESCE($5, attributes),
                notes = COALESCE($6, notes),
                updated_at = $7
            WHERE id = $8
            "#,
        )
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.expiration_date)
        .bind(input.best_before_date)
        .bind(input.cost_per_unit)
        .bind(input.attributes)
        .bind(input.notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM lots WHERE 1=1");

        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(lot_number) = &filter.lot_number {
            builder.push(" AND lot_number = ").push_bind(lot_number);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(supplier_id) = &filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if filter.has_quantity == Some(true) {
            builder.push(" AND quantity_remaining > 0");
        }

        builder.push(" ORDER BY created_at DESC");

        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        builder.push(" LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset);

        let rows =
            builder.build_query_as::<LotRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM lot_transactions WHERE lot_id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        if count.0 > 1 {
            return Err(CommerceError::ValidationError(
                "Cannot delete lot with transaction history".to_string(),
            ));
        }

        sqlx::query("DELETE FROM lots WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn adjust_async(&self, input: AdjustLot) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let lot = Self::row_to_lot(lot_row)?;

        let new_remaining = lot.quantity_remaining + input.quantity_change;
        if new_remaining < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Cannot reduce quantity below zero".to_string(),
            ));
        }

        sqlx::query("UPDATE lots SET quantity_remaining = $1, updated_at = $2 WHERE id = $3")
            .bind(new_remaining)
            .bind(Utc::now())
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let transaction = self
            .record_transaction_tx(
                &mut tx,
                input.lot_id,
                LotTransactionType::Adjusted,
                input.quantity_change,
                input.reference_type.as_deref().unwrap_or("manual_adjustment"),
                input.reference_id.unwrap_or(input.lot_id),
                None,
                input.location_id,
                Some(&input.reason),
                input.performed_by.as_deref(),
            )
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn consume_async(&self, input: ConsumeLot) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let lot = Self::row_to_lot(lot_row)?;

        if !lot.can_consume(input.quantity) {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: input.quantity.to_string(),
                available: lot.quantity_available().to_string(),
            });
        }

        let new_remaining = lot.quantity_remaining - input.quantity;
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };

        sqlx::query(
            "UPDATE lots SET quantity_remaining = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_remaining)
        .bind(new_status.to_string())
        .bind(Utc::now())
        .bind(input.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = self
            .record_transaction_tx(
                &mut tx,
                input.lot_id,
                LotTransactionType::Consumed,
                -input.quantity,
                &input.reference_type,
                input.reference_id,
                input.location_id,
                None,
                None,
                input.performed_by.as_deref(),
            )
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn reserve_async(&self, input: ReserveLot) -> Result<Uuid> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let lot = Self::row_to_lot(lot_row)?;

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

        sqlx::query(
            r#"
            INSERT INTO lot_reservations (id, lot_id, quantity, reference_type, reference_id,
                                          reserved_at, expires_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(reservation_id)
        .bind(input.lot_id)
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(input.reference_id)
        .bind(now)
        .bind(expires_at)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let new_reserved = lot.quantity_reserved + input.quantity;
        sqlx::query("UPDATE lots SET quantity_reserved = $1, updated_at = $2 WHERE id = $3")
            .bind(new_reserved)
            .bind(now)
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Reserved,
            input.quantity,
            &input.reference_type,
            input.reference_id,
            None,
            None,
            None,
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(reservation_id)
    }

    pub async fn release_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row = sqlx::query_as::<_, LotReservationRow>(
            "SELECT lot_id, quantity, reference_type, reference_id
             FROM lot_reservations
             WHERE id = $1 AND released_at IS NULL AND confirmed_at IS NULL",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let now = Utc::now();

        sqlx::query("UPDATE lot_reservations SET released_at = $1 WHERE id = $2")
            .bind(now)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE lots SET quantity_reserved = quantity_reserved - $1, updated_at = $2 WHERE id = $3",
        )
        .bind(row.quantity)
        .bind(now)
        .bind(row.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.record_transaction_tx(
            &mut tx,
            row.lot_id,
            LotTransactionType::Released,
            -row.quantity,
            &row.reference_type,
            row.reference_id,
            None,
            None,
            Some("Reservation released"),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn confirm_reservation_async(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row = sqlx::query_as::<_, LotReservationRow>(
            "SELECT lot_id, quantity, reference_type, reference_id
             FROM lot_reservations
             WHERE id = $1 AND released_at IS NULL AND confirmed_at IS NULL",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let now = Utc::now();

        sqlx::query("UPDATE lot_reservations SET confirmed_at = $1 WHERE id = $2")
            .bind(now)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE lots SET quantity_reserved = quantity_reserved - $1, quantity_remaining = quantity_remaining - $1, updated_at = $2 WHERE id = $3",
        )
        .bind(row.quantity)
        .bind(now)
        .bind(row.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = self
            .record_transaction_tx(
                &mut tx,
                row.lot_id,
                LotTransactionType::Consumed,
                -row.quantity,
                &row.reference_type,
                row.reference_id,
                None,
                None,
                Some("Reservation confirmed"),
                None,
            )
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn transfer_async(&self, input: TransferLot) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE lot_locations SET quantity = quantity - $1, updated_at = $2 WHERE lot_id = $3 AND location_id = $4",
        )
        .bind(input.quantity)
        .bind(now)
        .bind(input.lot_id)
        .bind(input.from_location_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (lot_id, location_id) DO UPDATE SET
                quantity = lot_locations.quantity + excluded.quantity,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(input.lot_id)
        .bind(input.to_location_id)
        .bind(input.quantity)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = self
            .record_transaction_tx(
                &mut tx,
                input.lot_id,
                LotTransactionType::Transferred,
                input.quantity,
                "transfer",
                input.lot_id,
                Some(input.from_location_id),
                Some(input.to_location_id),
                input.reason.as_deref(),
                input.performed_by.as_deref(),
            )
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn split_async(&self, input: SplitLot) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let original_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let original = Self::row_to_lot(original_row)?;

        if original.quantity_remaining < input.quantity {
            return Err(CommerceError::ValidationError(
                "Insufficient quantity to split".to_string(),
            ));
        }

        let new_lot_id = Uuid::new_v4();
        let new_lot_number =
            input.new_lot_number.unwrap_or_else(|| format!("{}-SPLIT", original.lot_number));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                cost_per_unit, attributes, notes, created_at, updated_at
            )
            SELECT $1, $2, sku, status, $3, $3, 0, 0, production_date, expiration_date,
                   best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                   cost_per_unit, attributes, $4, $5, $5
            FROM lots WHERE id = $6
            "#,
        )
        .bind(new_lot_id)
        .bind(&new_lot_number)
        .bind(input.quantity)
        .bind(input.reason.as_ref().map(|r| format!("Split from {}: {}", original.lot_number, r)))
        .bind(now)
        .bind(input.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let new_remaining = original.quantity_remaining - input.quantity;
        sqlx::query("UPDATE lots SET quantity_remaining = $1, updated_at = $2 WHERE id = $3")
            .bind(new_remaining)
            .bind(now)
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Split,
            -input.quantity,
            "lot_split",
            new_lot_id,
            None,
            None,
            input.reason.as_deref(),
            None,
        )
        .await?;

        self.record_transaction_tx(
            &mut tx,
            new_lot_id,
            LotTransactionType::Received,
            input.quantity,
            "lot_split",
            input.lot_id,
            None,
            None,
            Some(&format!("Split from lot {}", original.lot_number)),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(new_lot_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn merge_async(&self, input: MergeLots) -> Result<Lot> {
        if input.source_lot_ids.len() < 2 {
            return Err(CommerceError::ValidationError(
                "Need at least 2 lots to merge".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let mut total_quantity = Decimal::ZERO;
        let mut sku: Option<String> = None;
        let mut lots_to_consume: Vec<(Uuid, String, Decimal)> = Vec::new();

        for lot_id in &input.source_lot_ids {
            let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
                .bind(lot_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or_else(|| {
                    CommerceError::ValidationError(format!("Lot {} not found", lot_id))
                })?;
            let lot = Self::row_to_lot(lot_row)?;

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

        let sku =
            sku.ok_or_else(|| CommerceError::ValidationError("No lots to merge".to_string()))?;

        let new_lot_id = Uuid::new_v4();
        let new_lot_number = input
            .target_lot_number
            .unwrap_or_else(|| format!("MERGED-{}", Utc::now().format("%Y%m%d%H%M%S")));

        let template_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.source_lot_ids[0])
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let template = Self::row_to_lot(template_row)?;

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, cost_per_unit, attributes, notes, created_at, updated_at
            ) VALUES ($1,$2,$3,'active',$4,$4,0,0,$5,$6,$7,$8,'{}',$9,$10,$10)
            "#,
        )
        .bind(new_lot_id)
        .bind(&new_lot_number)
        .bind(&sku)
        .bind(total_quantity)
        .bind(template.production_date)
        .bind(template.expiration_date)
        .bind(template.best_before_date)
        .bind(template.cost_per_unit)
        .bind(input.reason.as_ref().map(|r| format!("Merged lots: {}", r)))
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for (lot_id, _lot_number, quantity) in lots_to_consume {
            sqlx::query(
                "UPDATE lots SET status = 'consumed', quantity_remaining = 0, updated_at = $1 WHERE id = $2",
            )
            .bind(now)
            .bind(lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            self.record_transaction_tx(
                &mut tx,
                lot_id,
                LotTransactionType::Merged,
                -quantity,
                "lot_merge",
                new_lot_id,
                None,
                None,
                Some(&format!("Merged into lot {}", new_lot_number)),
                None,
            )
            .await?;
        }

        self.record_transaction_tx(
            &mut tx,
            new_lot_id,
            LotTransactionType::Received,
            total_quantity,
            "lot_merge",
            input.source_lot_ids[0],
            None,
            None,
            Some("Created from merge"),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(new_lot_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn quarantine_async(&self, id: Uuid, reason: &str) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let lot = Self::row_to_lot(lot_row)?;

        let available = lot.quantity_available();

        sqlx::query(
            "UPDATE lots SET status = $1, quantity_quarantined = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(LotStatus::Quarantine.to_string())
        .bind(available)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.record_transaction_tx(
            &mut tx,
            id,
            LotTransactionType::Quarantined,
            available,
            "quarantine",
            id,
            None,
            None,
            Some(reason),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn release_quarantine_async(&self, id: Uuid) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let (quarantined,): (Decimal,) =
            sqlx::query_as("SELECT quantity_quarantined FROM lots WHERE id = $1")
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE lots SET status = 'active', quantity_quarantined = 0, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.record_transaction_tx(
            &mut tx,
            id,
            LotTransactionType::QuarantineReleased,
            quarantined,
            "quarantine_release",
            id,
            None,
            None,
            Some("Released from quarantine"),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_transactions_async(
        &self,
        lot_id: Uuid,
        limit: u32,
    ) -> Result<Vec<LotTransaction>> {
        let rows = sqlx::query_as::<_, LotTransactionRow>(
            "SELECT * FROM lot_transactions WHERE lot_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(lot_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut transactions = Vec::with_capacity(rows.len());
        for row in rows {
            transactions.push(Self::row_to_transaction(row)?);
        }
        Ok(transactions)
    }

    pub async fn get_quantity_at_location_async(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<Decimal>> {
        let row = sqlx::query_as::<_, (Decimal,)>(
            "SELECT quantity FROM lot_locations WHERE lot_id = $1 AND location_id = $2",
        )
        .bind(lot_id)
        .bind(location_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(|r| r.0))
    }

    pub async fn get_lot_locations_async(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        let rows = sqlx::query_as::<_, LotLocationRow>(
            "SELECT lot_id, location_id, quantity, updated_at FROM lot_locations WHERE lot_id = $1",
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_location).collect())
    }

    pub async fn add_certificate_async(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO lot_certificates (
                id, lot_id, certificate_type, certificate_number, document_url,
                issued_by, issued_at, expires_at, notes, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(id)
        .bind(input.lot_id)
        .bind(input.certificate_type.to_string())
        .bind(&input.certificate_number)
        .bind(&input.document_url)
        .bind(&input.issued_by)
        .bind(input.issued_at)
        .bind(input.expires_at)
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
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

    pub async fn get_certificates_async(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        let rows = sqlx::query_as::<_, LotCertificateRow>(
            "SELECT * FROM lot_certificates WHERE lot_id = $1 ORDER BY created_at DESC",
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut certificates = Vec::with_capacity(rows.len());
        for row in rows {
            certificates.push(Self::row_to_certificate(row)?);
        }
        Ok(certificates)
    }

    pub async fn delete_certificate_async(&self, certificate_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM lot_certificates WHERE id = $1")
            .bind(certificate_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_expiring_lots_async(&self, days: i32) -> Result<Vec<Lot>> {
        let threshold = Utc::now() + chrono::Duration::days(days as i64);

        let rows = sqlx::query_as::<_, LotRow>(
            r#"
            SELECT * FROM lots
            WHERE status = 'active' AND expiration_date IS NOT NULL
              AND expiration_date <= $1 AND expiration_date > NOW()
            ORDER BY expiration_date ASC
            "#,
        )
        .bind(threshold)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn get_expired_lots_async(&self) -> Result<Vec<Lot>> {
        let rows = sqlx::query_as::<_, LotRow>(
            r#"
            SELECT * FROM lots
            WHERE status = 'active' AND expiration_date IS NOT NULL
              AND expiration_date <= NOW()
            ORDER BY expiration_date ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn get_available_lots_for_sku_async(&self, sku: &str) -> Result<Vec<Lot>> {
        self.list_async(LotFilter {
            sku: Some(sku.to_string()),
            status: Some(LotStatus::Active),
            has_quantity: Some(true),
            ..Default::default()
        })
        .await
    }

    pub async fn trace_async(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        let lot = self.get_async(lot_id).await?.ok_or(CommerceError::NotFound)?;

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

        let rows = sqlx::query_as::<_, (String, String, Uuid, Decimal, DateTime<Utc>)>(
            r#"
            SELECT transaction_type, reference_type, reference_id, quantity, created_at
            FROM lot_transactions
            WHERE lot_id = $1 AND transaction_type IN ('consumed', 'shipped')
            ORDER BY created_at ASC
            "#,
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let downstream = rows
            .into_iter()
            .map(|(_tx_type, ref_type, ref_id, quantity, created_at)| {
                let node_type = match ref_type.as_str() {
                    "order" => TraceNodeType::Order,
                    "shipment" => TraceNodeType::Shipment,
                    "work_order" => TraceNodeType::WorkOrder,
                    _ => TraceNodeType::Adjustment,
                };

                TraceNode {
                    node_type,
                    node_id: ref_id,
                    reference_number: None,
                    lot_number: Some(lot.lot_number.clone()),
                    serial_number: None,
                    quantity,
                    timestamp: created_at,
                    entity_name: None,
                }
            })
            .collect();

        Ok(TraceabilityResult { lot, upstream, downstream })
    }

    pub async fn count_async(&self, filter: LotFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM lots WHERE 1=1");

        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_batch_async(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(lot) => result.record_success(lot),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM lots WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows =
            builder.build_query_as::<LotRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }
}

impl LotRepository for PgLotRepository {
    fn create(&self, input: CreateLot) -> Result<Lot> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        block_on(self.get_async(id))
    }

    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        block_on(self.get_by_number_async(lot_number))
    }

    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        block_on(self.update_async(id, input))
    }

    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_async(id))
    }

    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        block_on(self.adjust_async(input))
    }

    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        block_on(self.consume_async(input))
    }

    fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        block_on(self.reserve_async(input))
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        block_on(self.release_reservation_async(reservation_id))
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        block_on(self.confirm_reservation_async(reservation_id))
    }

    fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        block_on(self.transfer_async(input))
    }

    fn split(&self, input: SplitLot) -> Result<Lot> {
        block_on(self.split_async(input))
    }

    fn merge(&self, input: MergeLots) -> Result<Lot> {
        block_on(self.merge_async(input))
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        block_on(self.quarantine_async(id, reason))
    }

    fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        block_on(self.release_quarantine_async(id))
    }

    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        block_on(self.get_transactions_async(lot_id, limit))
    }

    fn get_quantity_at_location(&self, lot_id: Uuid, location_id: i32) -> Result<Option<Decimal>> {
        block_on(self.get_quantity_at_location_async(lot_id, location_id))
    }

    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        block_on(self.get_lot_locations_async(lot_id))
    }

    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        block_on(self.add_certificate_async(input))
    }

    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        block_on(self.get_certificates_async(lot_id))
    }

    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        block_on(self.delete_certificate_async(certificate_id))
    }

    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        block_on(self.get_expiring_lots_async(days))
    }

    fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        block_on(self.get_expired_lots_async())
    }

    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        block_on(self.get_available_lots_for_sku_async(sku))
    }

    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        block_on(self.trace_async(lot_id))
    }

    fn count(&self, filter: LotFilter) -> Result<u64> {
        block_on(self.count_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        block_on(self.create_batch_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        block_on(self.get_batch_async(ids))
    }
}
