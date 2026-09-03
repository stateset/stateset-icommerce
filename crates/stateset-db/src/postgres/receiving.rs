//! PostgreSQL implementation for receiving management

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    BatchResult, CommerceError, CompletePutAway, CreatePutAway, CreateReceipt, CreateReceiptItem,
    PutAway, PutAwayFilter, PutAwayStatus, Receipt, ReceiptFilter, ReceiptItem, ReceiptItemStatus,
    ReceiptStatus, ReceiptType, ReceiveItems, ReceivingRepository, Result, UpdateReceipt,
    generate_receipt_number, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgReceivingRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ReceiptRow {
    id: Uuid,
    receipt_number: String,
    receipt_type: String,
    status: String,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    supplier_id: Option<Uuid>,
    warehouse_id: i32,
    carrier: Option<String>,
    tracking_number: Option<String>,
    expected_date: Option<DateTime<Utc>>,
    received_date: Option<DateTime<Utc>>,
    completed_date: Option<DateTime<Utc>>,
    expected_quantity: Decimal,
    received_quantity: Decimal,
    pending_inspection_quantity: Decimal,
    put_away_quantity: Decimal,
    notes: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ReceiptItemRow {
    id: Uuid,
    receipt_id: Uuid,
    line_number: i32,
    sku: String,
    description: Option<String>,
    po_line_id: Option<Uuid>,
    expected_quantity: Decimal,
    received_quantity: Decimal,
    rejected_quantity: Decimal,
    unit_cost: Option<Decimal>,
    lot_number: Option<String>,
    serial_numbers: Option<String>,
    expiration_date: Option<DateTime<Utc>>,
    status: String,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PutAwayRow {
    id: Uuid,
    receipt_id: Uuid,
    receipt_item_id: Uuid,
    sku: String,
    from_location_id: Option<i32>,
    to_location_id: i32,
    quantity: Decimal,
    lot_id: Option<Uuid>,
    status: String,
    assigned_to: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgReceivingRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_receipt(row: ReceiptRow) -> Result<Receipt> {
        let ReceiptRow {
            id,
            receipt_number,
            receipt_type,
            status,
            reference_type,
            reference_id,
            supplier_id,
            warehouse_id,
            carrier,
            tracking_number,
            expected_date,
            received_date,
            completed_date,
            expected_quantity,
            received_quantity,
            pending_inspection_quantity,
            put_away_quantity,
            notes,
            created_by,
            created_at,
            updated_at,
        } = row;

        let receipt_type: ReceiptType = receipt_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid receipt.receipt_type '{}': {}",
                receipt_type, e
            ))
        })?;
        let status: ReceiptStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid receipt.status '{}': {}", status, e))
        })?;

        Ok(Receipt {
            id,
            receipt_number,
            receipt_type,
            status,
            reference_type,
            reference_id,
            supplier_id,
            warehouse_id,
            carrier,
            tracking_number,
            expected_date,
            received_date,
            completed_date,
            expected_quantity,
            received_quantity,
            pending_inspection_quantity,
            put_away_quantity,
            notes,
            created_by,
            created_at,
            updated_at,
        })
    }

    fn row_to_receipt_item(row: ReceiptItemRow) -> Result<ReceiptItem> {
        let ReceiptItemRow {
            id,
            receipt_id,
            line_number,
            sku,
            description,
            po_line_id,
            expected_quantity,
            received_quantity,
            rejected_quantity,
            unit_cost,
            lot_number,
            serial_numbers,
            expiration_date,
            status,
            notes,
            created_at,
            updated_at,
        } = row;

        let status: ReceiptItemStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid receipt_item.status '{}': {}", status, e))
        })?;

        Ok(ReceiptItem {
            id,
            receipt_id,
            line_number,
            sku,
            description,
            po_line_id,
            expected_quantity,
            received_quantity,
            rejected_quantity,
            unit_cost,
            lot_number,
            serial_numbers,
            expiration_date,
            status,
            notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_put_away(row: PutAwayRow) -> Result<PutAway> {
        let PutAwayRow {
            id,
            receipt_id,
            receipt_item_id,
            sku,
            from_location_id,
            to_location_id,
            quantity,
            lot_id,
            status,
            assigned_to,
            started_at,
            completed_at,
            notes,
            created_at,
        } = row;

        let status: PutAwayStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid put_away.status '{}': {}", status, e))
        })?;

        Ok(PutAway {
            id,
            receipt_id,
            receipt_item_id,
            sku,
            from_location_id,
            to_location_id,
            quantity,
            lot_id,
            status,
            assigned_to,
            started_at,
            completed_at,
            notes,
            created_at,
        })
    }

    /// Recompute a receipt's header quantities from its lines, on the same
    /// connection (and so inside the same transaction) that changed them.
    async fn update_receipt_totals(conn: &mut sqlx::PgConnection, receipt_id: Uuid) -> Result<()> {
        let (expected_total, received_total): (Decimal, Decimal) = sqlx::query_as(
            "SELECT COALESCE(SUM(expected_quantity), 0), COALESCE(SUM(received_quantity), 0) FROM receipt_items WHERE receipt_id = $1",
        )
        .bind(receipt_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE receipts SET expected_quantity = $1, received_quantity = $2 WHERE id = $3",
        )
        .bind(expected_total)
        .bind(received_total)
        .bind(receipt_id)
        .execute(conn)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Error for a guarded receipt write that matched no row: `NotFound` when
    /// the receipt is gone, otherwise the `ValidationError` callers already
    /// expect, built from the status that actually blocked the write.
    async fn receipt_guard_error(
        conn: &mut sqlx::PgConnection,
        id: Uuid,
        message: impl Fn(&str) -> String,
    ) -> CommerceError {
        let current: Option<String> =
            sqlx::query_scalar("SELECT status FROM receipts WHERE id = $1")
                .bind(id)
                .fetch_optional(conn)
                .await
                .ok()
                .flatten();
        current.map_or(CommerceError::NotFound, |status| {
            CommerceError::ValidationError(message(&status))
        })
    }

    /// Error for a status-guarded put-away UPDATE that matched no row: either
    /// the row is gone (`NotFound`) or its status forbids the transition
    /// (`Conflict`, naming the status that blocked it).
    async fn put_away_conflict(
        conn: &mut sqlx::PgConnection,
        id: Uuid,
        action: &str,
    ) -> CommerceError {
        let current: Option<String> =
            sqlx::query_scalar("SELECT status FROM put_aways WHERE id = $1")
                .bind(id)
                .fetch_optional(conn)
                .await
                .ok()
                .flatten();
        current.map_or(CommerceError::NotFound, |status| {
            CommerceError::Conflict(format!("cannot {action} put-away {id}: status is {status}"))
        })
    }

    /// Create a receipt and its lines.
    ///
    /// Header and lines are written in one transaction: run separately on the
    /// pool, a failure part-way through the line loop left a receipt whose
    /// `expected_quantity` header total counted lines that were never inserted.
    pub async fn create_receipt_async(&self, input: CreateReceipt) -> Result<Receipt> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let receipt_number = input.receipt_number.unwrap_or_else(generate_receipt_number);

        let expected_total: Decimal = input.items.iter().map(|i| i.expected_quantity).sum();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO receipts (
                id, receipt_number, receipt_type, status, reference_type, reference_id,
                supplier_id, warehouse_id, carrier, tracking_number, expected_date, expected_quantity,
                notes, created_by, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$15)
            "#,
        )
        .bind(id)
        .bind(&receipt_number)
        .bind(input.receipt_type.to_string())
        .bind(ReceiptStatus::Expected.to_string())
        .bind(&input.reference_type)
        .bind(input.reference_id)
        .bind(input.supplier_id)
        .bind(input.warehouse_id)
        .bind(&input.carrier)
        .bind(&input.tracking_number)
        .bind(input.expected_date)
        .bind(expected_total)
        .bind(&input.notes)
        .bind(&input.created_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for (idx, item) in input.items.iter().enumerate() {
            let item_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO receipt_items (
                    id, receipt_id, line_number, sku, description, po_line_id,
                    expected_quantity, unit_cost, lot_number, expiration_date, notes, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$12)
                "#,
            )
            .bind(item_id)
            .bind(id)
            .bind((idx + 1) as i32)
            .bind(&item.sku)
            .bind(&item.description)
            .bind(item.po_line_id)
            .bind(item.expected_quantity)
            .bind(item.unit_cost)
            .bind(&item.lot_number)
            .bind(item.expiration_date)
            .bind(&item.notes)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create receipt".into()))?;

        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_receipt(row)
    }

    pub async fn get_receipt_async(&self, id: Uuid) -> Result<Option<Receipt>> {
        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_receipt).transpose()
    }

    pub async fn get_receipt_by_number_async(&self, number: &str) -> Result<Option<Receipt>> {
        let row =
            sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE receipt_number = $1")
                .bind(number)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        row.map(Self::row_to_receipt).transpose()
    }

    pub async fn update_receipt_async(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        sqlx::query(
            r#"
            UPDATE receipts SET
                carrier = COALESCE($1, carrier),
                tracking_number = COALESCE($2, tracking_number),
                expected_date = COALESCE($3, expected_date),
                notes = COALESCE($4, notes)
            WHERE id = $5
            "#,
        )
        .bind(input.carrier)
        .bind(input.tracking_number)
        .bind(input.expected_date)
        .bind(input.notes)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_receipt_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update receipt".into()))
    }

    pub async fn list_receipts_async(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM receipts WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(receipt_type) = filter.receipt_type {
            builder.push(" AND receipt_type = ").push_bind(receipt_type.to_string());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(reference_id) = filter.reference_id {
            builder.push(" AND reference_id = ").push_bind(reference_id);
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ReceiptRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_receipt).collect::<Result<Vec<_>>>()
    }

    /// Delete a receipt that has not started receiving.
    ///
    /// The status check and the DELETE are one guarded statement, so a receipt
    /// that starts receiving concurrently can no longer be deleted between the
    /// check and the write.
    pub async fn delete_receipt_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query("DELETE FROM receipts WHERE id = $1 AND status = 'expected'")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .rows_affected();
        if changed == 0 {
            return Err(Self::receipt_guard_error(tx.as_mut(), id, |_| {
                "Can only delete receipts in 'expected' status".into()
            })
            .await);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Start receiving against a receipt ([`ReceiptStatus::Expected`] only).
    pub async fn start_receiving_async(&self, id: Uuid) -> Result<Receipt> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE receipts SET status = $1, received_date = $2
             WHERE id = $3 AND status = 'expected'",
        )
        .bind(ReceiptStatus::InProgress.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::receipt_guard_error(tx.as_mut(), id, |_| {
                "Can only start receiving for 'expected' receipts".into()
            })
            .await);
        }

        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_receipt(row)
    }

    /// Record received quantities against a receipt's lines.
    ///
    /// The whole operation — receipt status guard, the `Expected -> InProgress`
    /// flip, every line update and the header totals — runs in ONE transaction,
    /// with `SELECT ... FOR UPDATE` on the receipt and on each line being
    /// received. Previously each statement ran on the pool, so a failure
    /// part-way through left the receipt half-applied (line quantities updated
    /// but the header totals stale, or vice versa) and concurrent receipts could
    /// interleave.
    ///
    /// Guards, in order (identical to the SQLite backend):
    /// 1. the receipt must exist, and be `Expected` or `InProgress` — the
    ///    statuses in which goods can still arrive;
    /// 2. each line's quantity must be positive and its rejected quantity
    ///    non-negative (mirroring the purchase-order `receive` guard);
    /// 3. the line must belong to *this* receipt;
    /// 4. re-read under the row lock, the cumulative received quantity may not
    ///    exceed the line's expected quantity. Without this cap 100 units could
    ///    be received against a 10-unit line, corrupting inventory and the
    ///    downstream three-way match.
    pub async fn receive_items_async(&self, input: ReceiveItems) -> Result<Receipt> {
        let now = Utc::now();
        let receipt_id = input.receipt_id;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status_str: String =
            sqlx::query_scalar("SELECT status FROM receipts WHERE id = $1 FOR UPDATE")
                .bind(receipt_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        let status: ReceiptStatus = status_str.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid receipt.status '{status_str}': {e}"))
        })?;

        if status != ReceiptStatus::InProgress && status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Receipt must be 'expected' or 'in_progress' to receive items".into(),
            ));
        }

        if status == ReceiptStatus::Expected {
            sqlx::query("UPDATE receipts SET status = $1, received_date = $2 WHERE id = $3")
                .bind(ReceiptStatus::InProgress.to_string())
                .bind(now)
                .bind(receipt_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }

        for line in &input.items {
            let reject_qty = line.quantity_rejected.unwrap_or(Decimal::ZERO);
            let serial_str = line.serial_numbers.as_ref().map(|v| v.join(","));

            if line.quantity_received <= Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Received quantity must be greater than zero".into(),
                ));
            }
            if reject_qty < Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Rejected quantity cannot be negative".into(),
                ));
            }

            // Locked read of the line, scoped by `receipt_id` so a line
            // belonging to another receipt cannot be received through this one.
            let (cur_received, cur_rejected, expected, cur_status): (
                Decimal,
                Decimal,
                Decimal,
                String,
            ) = sqlx::query_as(
                "SELECT received_quantity, rejected_quantity, expected_quantity, status
                 FROM receipt_items WHERE id = $1 AND receipt_id = $2 FOR UPDATE",
            )
            .bind(line.receipt_item_id)
            .bind(receipt_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            let new_received = cur_received + line.quantity_received;
            let new_rejected = cur_rejected + reject_qty;

            // `expected_quantity = 0` marks a blind receipt: any positive
            // quantity is accepted and the line is `received` in full.
            let blind = expected == Decimal::ZERO;
            if !blind && new_received > expected {
                return Err(CommerceError::ValidationError(format!(
                    "Receiving {new_received} would exceed expected quantity {expected} for receipt item {}",
                    line.receipt_item_id
                )));
            }

            let new_status = if new_received >= expected {
                "received"
            } else if new_received > Decimal::ZERO {
                "partially_received"
            } else {
                cur_status.as_str()
            };

            sqlx::query(
                r#"
                UPDATE receipt_items SET
                    received_quantity = $1,
                    rejected_quantity = $2,
                    lot_number = COALESCE($3, lot_number),
                    serial_numbers = COALESCE($4, serial_numbers),
                    expiration_date = COALESCE($5, expiration_date),
                    notes = COALESCE($6, notes),
                    status = $8
                WHERE id = $7
                "#,
            )
            .bind(new_received)
            .bind(new_rejected)
            .bind(&line.lot_number)
            .bind(serial_str)
            .bind(line.expiration_date)
            .bind(&line.notes)
            .bind(line.receipt_item_id)
            .bind(new_status)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        Self::update_receipt_totals(tx.as_mut(), receipt_id).await?;

        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(receipt_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve receipt".into()))?;

        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_receipt(row)
    }

    /// Complete receiving ([`ReceiptStatus::InProgress`] only).
    ///
    /// The status guard is now part of the header UPDATE and runs in the same
    /// transaction as the line sweep, so a receipt cannot be completed twice by
    /// two callers that both read `in_progress` before either wrote.
    pub async fn complete_receiving_async(&self, id: Uuid) -> Result<Receipt> {
        let now = Utc::now();

        // Complete the receipt header and mark its non-rejected line items
        // `received` in one transaction (matching the SQLite backend, which marks
        // items received on completion — the Postgres path previously updated only
        // the header, leaving items in their prior status).
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE receipts SET status = $1, completed_date = $2
             WHERE id = $3 AND status = 'in_progress'",
        )
        .bind(ReceiptStatus::Received.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::receipt_guard_error(tx.as_mut(), id, |_| {
                "Can only complete 'in_progress' receipts".into()
            })
            .await);
        }

        sqlx::query(
            "UPDATE receipt_items SET status = 'received' WHERE receipt_id = $1 AND status != 'rejected'",
        )
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_receipt(row)
    }

    /// Cancel a receipt.
    ///
    /// Legal only while [`ReceiptStatus::can_cancel`] holds (`Expected` or
    /// `InProgress`); once goods are received the receipt is a record of what
    /// physically arrived. The check and the write are now one guarded
    /// statement.
    pub async fn cancel_receipt_async(&self, id: Uuid) -> Result<Receipt> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE receipts SET status = $1
             WHERE id = $2 AND status IN ('expected', 'in_progress')",
        )
        .bind(ReceiptStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::receipt_guard_error(tx.as_mut(), id, |status| {
                format!("Cannot cancel a receipt in {status} status (goods already received)")
            })
            .await);
        }

        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_receipt(row)
    }

    pub async fn get_receipt_items_async(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        let rows = sqlx::query_as::<_, ReceiptItemRow>(
            "SELECT * FROM receipt_items WHERE receipt_id = $1 ORDER BY line_number",
        )
        .bind(receipt_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_receipt_item).collect::<Result<Vec<_>>>()
    }

    pub async fn count_receipts_async(&self, filter: ReceiptFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM receipts WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(receipt_type) = filter.receipt_type {
            builder.push(" AND receipt_type = ").push_bind(receipt_type.to_string());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(reference_id) = filter.reference_id {
            builder.push(" AND reference_id = ").push_bind(reference_id);
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    /// Create a put-away task against a received line.
    ///
    /// Guards (inside one transaction, with `SELECT ... FOR UPDATE` on the
    /// receipt line so two concurrent put-aways serialize on the cap):
    /// 1. `quantity` must be positive;
    /// 2. `receipt_item_id` must be a line of `receipt_id`;
    /// 3. the requested quantity plus every non-cancelled put-away already
    ///    planned for the line may not exceed the line's `received_quantity`.
    pub async fn create_put_away_async(&self, input: CreatePutAway) -> Result<PutAway> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Put-away quantity must be greater than zero".into(),
            ));
        }
        let now = Utc::now();
        let id = Uuid::new_v4();
        let item_id = input.receipt_item_id;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let (owner, received): (Uuid, Decimal) = sqlx::query_as(
            "SELECT receipt_id, received_quantity FROM receipt_items WHERE id = $1 FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        if owner != input.receipt_id {
            return Err(CommerceError::ValidationError(format!(
                "Receipt item {item_id} does not belong to receipt {}",
                input.receipt_id
            )));
        }

        let (planned,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(quantity), 0) FROM put_aways
             WHERE receipt_item_id = $1 AND status != 'cancelled'",
        )
        .bind(item_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if planned + input.quantity > received {
            return Err(CommerceError::ValidationError(format!(
                "Cannot put away {} of receipt item {item_id}: {received} received, {planned} already planned, {} available",
                input.quantity,
                received - planned
            )));
        }

        sqlx::query(
            r#"
            INSERT INTO put_aways (
                id, receipt_id, receipt_item_id, sku, from_location_id, to_location_id,
                quantity, lot_id, assigned_to, notes, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(id)
        .bind(input.receipt_id)
        .bind(item_id)
        .bind(&input.sku)
        .bind(input.from_location_id)
        .bind(input.to_location_id)
        .bind(input.quantity)
        .bind(input.lot_id)
        .bind(&input.assigned_to)
        .bind(&input.notes)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::commit_and_read_put_away(tx, id).await
    }

    pub async fn get_put_away_async(&self, id: Uuid) -> Result<Option<PutAway>> {
        let row = sqlx::query_as::<_, PutAwayRow>("SELECT * FROM put_aways WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_put_away).transpose()
    }

    pub async fn list_put_aways_async(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM put_aways WHERE 1=1");

        if let Some(receipt_id) = filter.receipt_id {
            builder.push(" AND receipt_id = ").push_bind(receipt_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        builder.push(" ORDER BY created_at");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PutAwayRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_put_away).collect::<Result<Vec<_>>>()
    }

    /// Assign (or re-assign) a put-away task.
    ///
    /// Legal from `Pending`/`Assigned`; the UPDATE also writes
    /// `status = 'assigned'`, so assigning a started task would rewind it and
    /// assigning a completed one would resurrect it — dropping its quantity out
    /// of `receipts.put_away_quantity` on the next recompute.
    pub async fn assign_put_away_async(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE put_aways SET assigned_to = $1, status = $2
             WHERE id = $3 AND status IN ('pending', 'assigned')",
        )
        .bind(assigned_to)
        .bind(PutAwayStatus::Assigned.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::put_away_conflict(tx.as_mut(), id, "assign").await);
        }

        Self::commit_and_read_put_away(tx, id).await
    }

    /// Start a put-away task (`Pending`/`Assigned` only).
    pub async fn start_put_away_async(&self, id: Uuid) -> Result<PutAway> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE put_aways SET status = $1, started_at = $2
             WHERE id = $3 AND status IN ('pending', 'assigned')",
        )
        .bind(PutAwayStatus::InProgress.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::put_away_conflict(tx.as_mut(), id, "start").await);
        }

        Self::commit_and_read_put_away(tx, id).await
    }

    /// Complete a put-away task and fold its quantity into the receipt.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress`. Completing a cancelled task
    /// used to succeed and add its quantity to `receipts.put_away_quantity` for
    /// stock that was never put away. The status flip and the receipt total are
    /// one transaction, so the receipt can never quote a total that excludes a
    /// put-away already marked completed.
    pub async fn complete_put_away_async(&self, input: CompletePutAway) -> Result<PutAway> {
        let now = Utc::now();
        let id = input.put_away_id;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let existing =
            sqlx::query_as::<_, PutAwayRow>("SELECT * FROM put_aways WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        let receipt_id = existing.receipt_id;
        let to_location = input.actual_location_id.unwrap_or(existing.to_location_id);

        let changed = sqlx::query(
            "UPDATE put_aways SET status = $1, to_location_id = $2, completed_at = $3, notes = COALESCE($4, notes)
             WHERE id = $5 AND status IN ('pending', 'assigned', 'in_progress')",
        )
        .bind(PutAwayStatus::Completed.to_string())
        .bind(to_location)
        .bind(now)
        .bind(input.notes)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::put_away_conflict(tx.as_mut(), id, "complete").await);
        }

        let (put_away_total,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(quantity), 0) FROM put_aways WHERE receipt_id = $1 AND status = 'completed'",
        )
        .bind(receipt_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE receipts SET put_away_quantity = $1 WHERE id = $2")
            .bind(put_away_total)
            .bind(receipt_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::commit_and_read_put_away(tx, id).await
    }

    /// Cancel a put-away task.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress`. A `Completed` task is
    /// refused: the stock has physically moved, and cancelling it would leave
    /// `receipts.put_away_quantity` counting a put-away that claims not to have
    /// happened (cancellation does not recompute that total).
    pub async fn cancel_put_away_async(&self, id: Uuid) -> Result<PutAway> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE put_aways SET status = $1
             WHERE id = $2 AND status IN ('pending', 'assigned', 'in_progress')",
        )
        .bind(PutAwayStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(Self::put_away_conflict(tx.as_mut(), id, "cancel").await);
        }

        Self::commit_and_read_put_away(tx, id).await
    }

    /// Read a put-away back inside its transaction, then commit.
    async fn commit_and_read_put_away(
        mut tx: sqlx::Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<PutAway> {
        let row = sqlx::query_as::<_, PutAwayRow>("SELECT * FROM put_aways WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_put_away(row)
    }

    pub async fn get_pending_put_aways_async(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.list_put_aways_async(PutAwayFilter {
            receipt_id: Some(receipt_id),
            status: Some(PutAwayStatus::Pending),
            ..Default::default()
        })
        .await
    }

    /// Count put-aways matching `filter` (same filters as
    /// `list_put_aways_async`).
    pub async fn count_put_aways_async(&self, filter: PutAwayFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM put_aways WHERE 1=1");

        if let Some(receipt_id) = filter.receipt_id {
            builder.push(" AND receipt_id = ").push_bind(receipt_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_receipt_from_po_async(
        &self,
        po_id: Uuid,
        warehouse_id: i32,
    ) -> Result<Receipt> {
        let rows = sqlx::query_as::<_, (String, Option<String>, Decimal, Decimal)>(
            "SELECT sku, name, quantity_ordered, unit_cost FROM purchase_order_items WHERE purchase_order_id = $1",
        )
        .bind(po_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut items = Vec::new();
        for (sku, description, quantity, unit_cost) in rows {
            items.push(CreateReceiptItem {
                sku,
                description,
                po_line_id: None,
                expected_quantity: quantity,
                unit_cost: Some(unit_cost),
                lot_number: None,
                expiration_date: None,
                notes: None,
            });
        }

        let supplier_id: Option<Uuid> =
            sqlx::query_as("SELECT supplier_id FROM purchase_orders WHERE id = $1")
                .bind(po_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?
                .map(|row: (Uuid,)| row.0);

        self.create_receipt_async(CreateReceipt {
            receipt_number: None,
            receipt_type: ReceiptType::PurchaseOrder,
            reference_type: Some("purchase_order".to_string()),
            reference_id: Some(po_id),
            supplier_id,
            warehouse_id,
            carrier: None,
            tracking_number: None,
            expected_date: None,
            items,
            notes: None,
            created_by: None,
        })
        .await
    }

    pub async fn create_receipts_batch_async(
        &self,
        inputs: Vec<CreateReceipt>,
    ) -> Result<BatchResult<Receipt>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_receipt_async(input).await {
                Ok(receipt) => result.record_success(receipt),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_receipts_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM receipts WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<ReceiptRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_receipt).collect::<Result<Vec<_>>>()
    }
}

impl ReceivingRepository for PgReceivingRepository {
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        block_on(self.create_receipt_async(input))
    }

    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        block_on(self.get_receipt_async(id))
    }

    fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        block_on(self.get_receipt_by_number_async(number))
    }

    fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        block_on(self.update_receipt_async(id, input))
    }

    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        block_on(self.list_receipts_async(filter))
    }

    fn delete_receipt(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_receipt_async(id))
    }

    fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        block_on(self.start_receiving_async(id))
    }

    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        block_on(self.receive_items_async(input))
    }

    fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        block_on(self.complete_receiving_async(id))
    }

    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        block_on(self.cancel_receipt_async(id))
    }

    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        block_on(self.get_receipt_items_async(receipt_id))
    }

    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        block_on(self.count_receipts_async(filter))
    }

    fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        block_on(self.create_put_away_async(input))
    }

    fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        block_on(self.get_put_away_async(id))
    }

    fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        block_on(self.list_put_aways_async(filter))
    }

    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        block_on(self.assign_put_away_async(id, assigned_to))
    }

    fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        block_on(self.start_put_away_async(id))
    }

    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        block_on(self.complete_put_away_async(input))
    }

    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        block_on(self.cancel_put_away_async(id))
    }

    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        block_on(self.get_pending_put_aways_async(receipt_id))
    }

    fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        block_on(self.count_put_aways_async(filter))
    }

    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        block_on(self.create_receipt_from_po_async(po_id, warehouse_id))
    }

    fn create_receipts_batch(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>> {
        block_on(self.create_receipts_batch_async(inputs))
    }

    fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        block_on(self.get_receipts_batch_async(ids))
    }
}
