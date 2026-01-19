//! PostgreSQL implementation for receiving management

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, Postgres, QueryBuilder};
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, CompletePutAway, CommerceError, CreatePutAway, CreateReceipt, CreateReceiptItem,
    PutAway, PutAwayFilter, PutAwayStatus, Receipt, ReceiptFilter, ReceiptItem, ReceiptItemStatus,
    ReceiptStatus, ReceiptType, ReceiveItems, ReceivingRepository, Result, UpdateReceipt,
    generate_receipt_number, validate_batch_size,
};
use uuid::Uuid;

#[derive(Clone)]
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
    pub fn new(pool: PgPool) -> Self {
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
            CommerceError::DatabaseError(format!(
                "Invalid receipt.status '{}': {}",
                status, e
            ))
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
            CommerceError::DatabaseError(format!(
                "Invalid receipt_item.status '{}': {}",
                status, e
            ))
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
            CommerceError::DatabaseError(format!(
                "Invalid put_away.status '{}': {}",
                status, e
            ))
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

    async fn update_receipt_totals(&self, receipt_id: Uuid) -> Result<()> {
        let (expected_total, received_total): (Decimal, Decimal) = sqlx::query_as(
            "SELECT COALESCE(SUM(expected_quantity), 0), COALESCE(SUM(received_quantity), 0) FROM receipt_items WHERE receipt_id = $1",
        )
        .bind(receipt_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE receipts SET expected_quantity = $1, received_quantity = $2 WHERE id = $3",
        )
        .bind(expected_total)
        .bind(received_total)
        .bind(receipt_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn create_receipt_async(&self, input: CreateReceipt) -> Result<Receipt> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let receipt_number = input.receipt_number.unwrap_or_else(generate_receipt_number);

        let expected_total: Decimal = input.items.iter().map(|i| i.expected_quantity).sum();

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
        .execute(&self.pool)
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
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        }

        self.get_receipt_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to create receipt".into())
        })
    }

    pub async fn get_receipt_async(&self, id: Uuid) -> Result<Option<Receipt>> {
        let row = sqlx::query_as::<_, ReceiptRow>("SELECT * FROM receipts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_receipt).transpose()?)
    }

    pub async fn get_receipt_by_number_async(&self, number: &str) -> Result<Option<Receipt>> {
        let row = sqlx::query_as::<_, ReceiptRow>(
            "SELECT * FROM receipts WHERE receipt_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_receipt).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ReceiptRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_receipt).collect::<Result<Vec<_>>>()?)
    }

    pub async fn delete_receipt_async(&self, id: Uuid) -> Result<()> {
        let existing = self.get_receipt_async(id).await?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Can only delete receipts in 'expected' status".into(),
            ));
        }

        sqlx::query("DELETE FROM receipts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn start_receiving_async(&self, id: Uuid) -> Result<Receipt> {
        let existing = self.get_receipt_async(id).await?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Can only start receiving for 'expected' receipts".into(),
            ));
        }

        let now = Utc::now();
        sqlx::query("UPDATE receipts SET status = $1, received_date = $2 WHERE id = $3")
            .bind(ReceiptStatus::InProgress.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_receipt_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update receipt".into()))
    }

    pub async fn receive_items_async(&self, input: ReceiveItems) -> Result<Receipt> {
        let now = Utc::now();

        let existing = self.get_receipt_async(input.receipt_id).await?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::InProgress && existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Receipt must be 'expected' or 'in_progress' to receive items".into(),
            ));
        }

        if existing.status == ReceiptStatus::Expected {
            sqlx::query("UPDATE receipts SET status = $1, received_date = $2 WHERE id = $3")
                .bind(ReceiptStatus::InProgress.to_string())
                .bind(now)
                .bind(input.receipt_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        for line in &input.items {
            let reject_qty = line.quantity_rejected.unwrap_or(Decimal::ZERO);
            let serial_str = line.serial_numbers.as_ref().map(|v| v.join(","));

            sqlx::query(
                r#"
                UPDATE receipt_items SET
                    received_quantity = received_quantity + $1,
                    rejected_quantity = rejected_quantity + $2,
                    lot_number = COALESCE($3, lot_number),
                    serial_numbers = COALESCE($4, serial_numbers),
                    expiration_date = COALESCE($5, expiration_date),
                    notes = COALESCE($6, notes),
                    status = CASE
                        WHEN received_quantity + $1 >= expected_quantity THEN 'received'
                        WHEN received_quantity + $1 > 0 THEN 'partially_received'
                        ELSE status
                    END
                WHERE id = $7
                "#,
            )
            .bind(line.quantity_received)
            .bind(reject_qty)
            .bind(&line.lot_number)
            .bind(serial_str)
            .bind(line.expiration_date)
            .bind(&line.notes)
            .bind(line.receipt_item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        }

        self.update_receipt_totals(input.receipt_id).await?;

        self.get_receipt_async(input.receipt_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve receipt".into()))
    }

    pub async fn complete_receiving_async(&self, id: Uuid) -> Result<Receipt> {
        let existing = self.get_receipt_async(id).await?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::InProgress {
            return Err(CommerceError::ValidationError(
                "Can only complete 'in_progress' receipts".into(),
            ));
        }

        let now = Utc::now();
        sqlx::query("UPDATE receipts SET status = $1, completed_date = $2 WHERE id = $3")
            .bind(ReceiptStatus::Received.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_receipt_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete receipt".into()))
    }

    pub async fn cancel_receipt_async(&self, id: Uuid) -> Result<Receipt> {
        let existing = self.get_receipt_async(id).await?.ok_or(CommerceError::NotFound)?;

        if existing.status == ReceiptStatus::Received {
            return Err(CommerceError::ValidationError(
                "Cannot cancel a received receipt".into(),
            ));
        }

        sqlx::query("UPDATE receipts SET status = $1 WHERE id = $2")
            .bind(ReceiptStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_receipt_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel receipt".into()))
    }

    pub async fn get_receipt_items_async(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        let rows = sqlx::query_as::<_, ReceiptItemRow>(
            "SELECT * FROM receipt_items WHERE receipt_id = $1 ORDER BY line_number",
        )
        .bind(receipt_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_receipt_item).collect::<Result<Vec<_>>>()?)
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

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_put_away_async(&self, input: CreatePutAway) -> Result<PutAway> {
        let now = Utc::now();
        let id = Uuid::new_v4();

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
        .bind(input.receipt_item_id)
        .bind(&input.sku)
        .bind(input.from_location_id)
        .bind(input.to_location_id)
        .bind(input.quantity)
        .bind(input.lot_id)
        .bind(&input.assigned_to)
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_put_away_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create put-away".into()))
    }

    pub async fn get_put_away_async(&self, id: Uuid) -> Result<Option<PutAway>> {
        let row = sqlx::query_as::<_, PutAwayRow>("SELECT * FROM put_aways WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_put_away).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PutAwayRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_put_away).collect::<Result<Vec<_>>>()?)
    }

    pub async fn assign_put_away_async(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        sqlx::query("UPDATE put_aways SET assigned_to = $1, status = $2 WHERE id = $3")
            .bind(assigned_to)
            .bind(PutAwayStatus::Assigned.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_put_away_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign put-away".into()))
    }

    pub async fn start_put_away_async(&self, id: Uuid) -> Result<PutAway> {
        let now = Utc::now();

        sqlx::query("UPDATE put_aways SET status = $1, started_at = $2 WHERE id = $3")
            .bind(PutAwayStatus::InProgress.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_put_away_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start put-away".into()))
    }

    pub async fn complete_put_away_async(&self, input: CompletePutAway) -> Result<PutAway> {
        let now = Utc::now();
        let existing = self.get_put_away_async(input.put_away_id).await?.ok_or(CommerceError::NotFound)?;
        let to_location = input.actual_location_id.unwrap_or(existing.to_location_id);

        sqlx::query(
            "UPDATE put_aways SET status = $1, to_location_id = $2, completed_at = $3, notes = COALESCE($4, notes) WHERE id = $5",
        )
        .bind(PutAwayStatus::Completed.to_string())
        .bind(to_location)
        .bind(now)
        .bind(input.notes)
        .bind(input.put_away_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (put_away_total,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(quantity), 0) FROM put_aways WHERE receipt_id = $1 AND status = 'completed'",
        )
        .bind(existing.receipt_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE receipts SET put_away_quantity = $1 WHERE id = $2")
            .bind(put_away_total)
            .bind(existing.receipt_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_put_away_async(input.put_away_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete put-away".into()))
    }

    pub async fn cancel_put_away_async(&self, id: Uuid) -> Result<PutAway> {
        sqlx::query("UPDATE put_aways SET status = $1 WHERE id = $2")
            .bind(PutAwayStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_put_away_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel put-away".into()))
    }

    pub async fn get_pending_put_aways_async(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self
            .list_put_aways_async(PutAwayFilter {
                receipt_id: Some(receipt_id),
                status: Some(PutAwayStatus::Pending),
                ..Default::default()
            })
            .await
    }

    pub async fn count_put_aways_async(&self, filter: PutAwayFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM put_aways WHERE 1=1");

        if let Some(receipt_id) = filter.receipt_id {
            builder.push(" AND receipt_id = ").push_bind(receipt_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_receipt_from_po_async(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
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

        let supplier_id: Option<Uuid> = sqlx::query_as(
            "SELECT supplier_id FROM purchase_orders WHERE id = $1",
        )
        .bind(po_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .map(|row: (Uuid,)| row.0);

        self
            .create_receipt_async(CreateReceipt {
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

    pub async fn create_receipts_batch_async(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>> {
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

        Ok(rows.into_iter().map(Self::row_to_receipt).collect::<Result<Vec<_>>>()?)
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
