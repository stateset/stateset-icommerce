//! SQLite implementation for receiving management
//!
//! Provides goods receipt, receiving, and put-away functionality.

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt,
    parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid_opt,
    parse_uuid_opt_row, parse_uuid_row, sum_decimal_query,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use uuid::Uuid;

use stateset_core::{
    BatchResult, CompletePutAway, CommerceError, CreatePutAway, CreateReceipt, CreateReceiptItem,
    PutAway, PutAwayFilter, PutAwayStatus, Receipt, ReceiptFilter, ReceiptItem,
    ReceiptStatus, ReceiptType, ReceiveItems, ReceivingRepository, Result, UpdateReceipt,
    generate_receipt_number,
};

/// SQLite receiving repository
pub struct SqliteReceivingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteReceivingRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_receipt(row: &rusqlite::Row) -> rusqlite::Result<Receipt> {
        Ok(Receipt {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "receipt", "id")?,
            receipt_number: row.get("receipt_number")?,
            receipt_type: parse_enum_row(&row.get::<_, String>("receipt_type")?, "receipt", "receipt_type")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "receipt", "status")?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("reference_id")?,
                "receipt",
                "reference_id",
            )?,
            supplier_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("supplier_id")?,
                "receipt",
                "supplier_id",
            )?,
            warehouse_id: row.get("warehouse_id")?,
            carrier: row.get("carrier")?,
            tracking_number: row.get("tracking_number")?,
            expected_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expected_date")?,
                "receipt",
                "expected_date",
            )?,
            received_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("received_date")?,
                "receipt",
                "received_date",
            )?,
            completed_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("completed_date")?,
                "receipt",
                "completed_date",
            )?,
            expected_quantity: parse_decimal_row(
                &row.get::<_, String>("expected_quantity")?,
                "receipt",
                "expected_quantity",
            )?,
            received_quantity: parse_decimal_row(
                &row.get::<_, String>("received_quantity")?,
                "receipt",
                "received_quantity",
            )?,
            pending_inspection_quantity: parse_decimal_row(
                &row.get::<_, String>("pending_inspection_quantity")?,
                "receipt",
                "pending_inspection_quantity",
            )?,
            put_away_quantity: parse_decimal_row(
                &row.get::<_, String>("put_away_quantity")?,
                "receipt",
                "put_away_quantity",
            )?,
            notes: row.get("notes")?,
            created_by: row.get("created_by")?,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "receipt", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "receipt", "updated_at")?,
        })
    }

    fn row_to_receipt_item(row: &rusqlite::Row) -> rusqlite::Result<ReceiptItem> {
        Ok(ReceiptItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "receipt_item", "id")?,
            receipt_id: parse_uuid_row(&row.get::<_, String>("receipt_id")?, "receipt_item", "receipt_id")?,
            line_number: row.get("line_number")?,
            sku: row.get("sku")?,
            description: row.get("description")?,
            po_line_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("po_line_id")?,
                "receipt_item",
                "po_line_id",
            )?,
            expected_quantity: parse_decimal_row(
                &row.get::<_, String>("expected_quantity")?,
                "receipt_item",
                "expected_quantity",
            )?,
            received_quantity: parse_decimal_row(
                &row.get::<_, String>("received_quantity")?,
                "receipt_item",
                "received_quantity",
            )?,
            rejected_quantity: parse_decimal_row(
                &row.get::<_, String>("rejected_quantity")?,
                "receipt_item",
                "rejected_quantity",
            )?,
            unit_cost: parse_decimal_opt_row(
                row.get::<_, Option<String>>("unit_cost")?,
                "receipt_item",
                "unit_cost",
            )?,
            lot_number: row.get("lot_number")?,
            serial_numbers: row.get("serial_numbers")?,
            expiration_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expiration_date")?,
                "receipt_item",
                "expiration_date",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "receipt_item", "status")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "receipt_item", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "receipt_item", "updated_at")?,
        })
    }

    fn row_to_put_away(row: &rusqlite::Row) -> rusqlite::Result<PutAway> {
        Ok(PutAway {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "put_away", "id")?,
            receipt_id: parse_uuid_row(&row.get::<_, String>("receipt_id")?, "put_away", "receipt_id")?,
            receipt_item_id: parse_uuid_row(
                &row.get::<_, String>("receipt_item_id")?,
                "put_away",
                "receipt_item_id",
            )?,
            sku: row.get("sku")?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            quantity: parse_decimal_row(&row.get::<_, String>("quantity")?, "put_away", "quantity")?,
            lot_id: parse_uuid_opt_row(row.get::<_, Option<String>>("lot_id")?, "put_away", "lot_id")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "put_away", "status")?,
            assigned_to: row.get("assigned_to")?,
            started_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("started_at")?,
                "put_away",
                "started_at",
            )?,
            completed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("completed_at")?,
                "put_away",
                "completed_at",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "put_away", "created_at")?,
        })
    }

    fn update_receipt_totals(&self, receipt_id: Uuid) -> Result<()> {
        let conn = self.conn()?;

        // Calculate totals from items
        let receipt_id_param = receipt_id.to_string();
        let mut stmt = conn
            .prepare(
                "SELECT expected_quantity, received_quantity FROM receipt_items WHERE receipt_id = ?1",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt
            .query(params![&receipt_id_param])
            .map_err(map_db_error)?;
        let mut exp_total = Decimal::ZERO;
        let mut rcv_total = Decimal::ZERO;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let expected_str: String = row.get(0).map_err(map_db_error)?;
            let received_str: String = row.get(1).map_err(map_db_error)?;
            exp_total += parse_decimal_strict(&expected_str, "receipt_item", "expected_quantity")?;
            rcv_total += parse_decimal_strict(&received_str, "receipt_item", "received_quantity")?;
        }

        conn.execute(
            "UPDATE receipts SET expected_quantity = ?1, received_quantity = ?2 WHERE id = ?3",
            params![exp_total.to_string(), rcv_total.to_string(), receipt_id_param],
        )
        .map_err(map_db_error)?;

        Ok(())
    }
}

impl ReceivingRepository for SqliteReceivingRepository {
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let receipt_number = input.receipt_number.unwrap_or_else(generate_receipt_number);

        // Calculate expected quantity from items
        let expected_total: Decimal = input.items.iter().map(|i| i.expected_quantity).sum();

        {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO receipts (id, receipt_number, receipt_type, status, reference_type, reference_id,
                 supplier_id, warehouse_id, carrier, tracking_number, expected_date, expected_quantity,
                 notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![
                    id.to_string(),
                    receipt_number,
                    input.receipt_type.to_string(),
                    ReceiptStatus::Expected.to_string(),
                    input.reference_type,
                    input.reference_id.map(|id| id.to_string()),
                    input.supplier_id.map(|id| id.to_string()),
                    input.warehouse_id,
                    input.carrier,
                    input.tracking_number,
                    input.expected_date.map(|d| d.to_rfc3339()),
                    expected_total.to_string(),
                    input.notes,
                    input.created_by,
                    now,
                ],
            )
            .map_err(map_db_error)?;

            // Create receipt items
            for (idx, item) in input.items.iter().enumerate() {
                let item_id = Uuid::new_v4();
                conn.execute(
                    "INSERT INTO receipt_items (id, receipt_id, line_number, sku, description, po_line_id,
                     expected_quantity, unit_cost, lot_number, expiration_date, notes, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        item_id.to_string(),
                        id.to_string(),
                        (idx + 1) as i32,
                        item.sku,
                        item.description,
                        item.po_line_id.map(|id| id.to_string()),
                        item.expected_quantity.to_string(),
                        item.unit_cost.map(|d| d.to_string()),
                        item.lot_number,
                        item.expiration_date.map(|d| d.to_rfc3339()),
                        item.notes,
                        now,
                    ],
                )
                .map_err(map_db_error)?;
            }
        }

        self.get_receipt(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve created receipt".into()))
    }

    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM receipts WHERE id = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_receipt(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM receipts WHERE receipt_number = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![number]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_receipt(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        let conn = self.conn()?;
        let existing = self.get_receipt(id)?.ok_or(CommerceError::NotFound)?;

        let carrier = input.carrier.or(existing.carrier);
        let tracking_number = input.tracking_number.or(existing.tracking_number);
        let expected_date = input.expected_date.or(existing.expected_date);
        let notes = input.notes.or(existing.notes);

        conn.execute(
            "UPDATE receipts SET carrier = ?1, tracking_number = ?2, expected_date = ?3, notes = ?4 WHERE id = ?5",
            params![
                carrier,
                tracking_number,
                expected_date.map(|d| d.to_rfc3339()),
                notes,
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_receipt(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve updated receipt".into()))
    }

    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM receipts WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(receipt_type) = filter.receipt_type {
            sql.push_str(" AND receipt_type = ?");
            params_vec.push(Box::new(receipt_type.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(supplier_id) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }

        if let Some(reference_id) = filter.reference_id {
            sql.push_str(" AND reference_id = ?");
            params_vec.push(Box::new(reference_id.to_string()));
        }

        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND created_at >= ?");
            params_vec.push(Box::new(from_date.to_rfc3339()));
        }

        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND created_at <= ?");
            params_vec.push(Box::new(to_date.to_rfc3339()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut receipts = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            receipts.push(Self::row_to_receipt(row).map_err(map_db_error)?);
        }

        Ok(receipts)
    }

    fn delete_receipt(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let existing = self.get_receipt(id)?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Can only delete receipts in 'expected' status".into(),
            ));
        }

        conn.execute("DELETE FROM receipts WHERE id = ?1", params![id.to_string()])
            .map_err(map_db_error)?;

        Ok(())
    }

    fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        let conn = self.conn()?;
        let existing = self.get_receipt(id)?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Can only start receiving for 'expected' receipts".into(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE receipts SET status = ?1, received_date = ?2 WHERE id = ?3",
            params![ReceiptStatus::InProgress.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_receipt(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update receipt".into()))
    }

    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        // Verify receipt exists and is in correct status
        let existing = self.get_receipt(input.receipt_id)?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::InProgress && existing.status != ReceiptStatus::Expected {
            return Err(CommerceError::ValidationError(
                "Receipt must be 'expected' or 'in_progress' to receive items".into(),
            ));
        }

        // Update receipt to in_progress if expected
        if existing.status == ReceiptStatus::Expected {
            conn.execute(
                "UPDATE receipts SET status = ?1, received_date = ?2 WHERE id = ?3",
                params![
                    ReceiptStatus::InProgress.to_string(),
                    now,
                    input.receipt_id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        // Process each item
        for line in &input.items {
            let reject_qty = line.quantity_rejected.unwrap_or(Decimal::ZERO);
            let serial_str = line.serial_numbers.as_ref().map(|v| v.join(","));

            // Update receipt item
            conn.execute(
                "UPDATE receipt_items SET
                 received_quantity = CAST(received_quantity AS REAL) + ?1,
                 rejected_quantity = CAST(rejected_quantity AS REAL) + ?2,
                 lot_number = COALESCE(?3, lot_number),
                 serial_numbers = COALESCE(?4, serial_numbers),
                 expiration_date = COALESCE(?5, expiration_date),
                 notes = COALESCE(?6, notes),
                 status = CASE
                     WHEN CAST(received_quantity AS REAL) + ?1 >= CAST(expected_quantity AS REAL) THEN 'received'
                     WHEN CAST(received_quantity AS REAL) + ?1 > 0 THEN 'partially_received'
                     ELSE status
                 END
                 WHERE id = ?7",
                params![
                    line.quantity_received.to_string(),
                    reject_qty.to_string(),
                    line.lot_number,
                    serial_str,
                    line.expiration_date.map(|d| d.to_rfc3339()),
                    line.notes,
                    line.receipt_item_id.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        }

        // Update receipt totals
        self.update_receipt_totals(input.receipt_id)?;

        self.get_receipt(input.receipt_id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve receipt".into()))
    }

    fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        let conn = self.conn()?;
        let existing = self.get_receipt(id)?.ok_or(CommerceError::NotFound)?;

        if existing.status != ReceiptStatus::InProgress {
            return Err(CommerceError::ValidationError(
                "Can only complete 'in_progress' receipts".into(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE receipts SET status = ?1, completed_date = ?2 WHERE id = ?3",
            params![ReceiptStatus::Received.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        // Mark all items as received
        conn.execute(
            "UPDATE receipt_items SET status = 'received' WHERE receipt_id = ?1 AND status != 'rejected'",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_receipt(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update receipt".into()))
    }

    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        let conn = self.conn()?;
        let existing = self.get_receipt(id)?.ok_or(CommerceError::NotFound)?;

        if existing.status == ReceiptStatus::Completed {
            return Err(CommerceError::ValidationError(
                "Cannot cancel completed receipts".into(),
            ));
        }

        conn.execute(
            "UPDATE receipts SET status = ?1 WHERE id = ?2",
            params![ReceiptStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_receipt(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update receipt".into()))
    }

    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM receipt_items WHERE receipt_id = ?1 ORDER BY line_number")
            .map_err(map_db_error)?;

        let mut rows = stmt
            .query(params![receipt_id.to_string()])
            .map_err(map_db_error)?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            items.push(Self::row_to_receipt_item(row).map_err(map_db_error)?);
        }

        Ok(items)
    }

    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM receipts WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    // Put-away operations
    fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO put_aways (id, receipt_id, receipt_item_id, sku, from_location_id, to_location_id,
             quantity, lot_id, assigned_to, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id.to_string(),
                input.receipt_id.to_string(),
                input.receipt_item_id.to_string(),
                input.sku,
                input.from_location_id,
                input.to_location_id,
                input.quantity.to_string(),
                input.lot_id.map(|id| id.to_string()),
                input.assigned_to,
                input.notes,
                now,
            ],
        )
        .map_err(map_db_error)?;

        self.get_put_away(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create put-away".into()))
    }

    fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM put_aways WHERE id = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_put_away(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM put_aways WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(receipt_id) = filter.receipt_id {
            sql.push_str(" AND receipt_id = ?");
            params_vec.push(Box::new(receipt_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(assigned_to) = filter.assigned_to {
            sql.push_str(" AND assigned_to = ?");
            params_vec.push(Box::new(assigned_to));
        }

        sql.push_str(" ORDER BY created_at");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut put_aways = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            put_aways.push(Self::row_to_put_away(row).map_err(map_db_error)?);
        }

        Ok(put_aways)
    }

    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE put_aways SET assigned_to = ?1, status = ?2 WHERE id = ?3",
            params![assigned_to, PutAwayStatus::Assigned.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_put_away(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign put-away".into()))
    }

    fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE put_aways SET status = ?1, started_at = ?2 WHERE id = ?3",
            params![PutAwayStatus::InProgress.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_put_away(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start put-away".into()))
    }

    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        let existing = self.get_put_away(input.put_away_id)?.ok_or(CommerceError::NotFound)?;

        let to_location = input.actual_location_id.unwrap_or(existing.to_location_id);

        conn.execute(
            "UPDATE put_aways SET status = ?1, to_location_id = ?2, completed_at = ?3, notes = COALESCE(?4, notes) WHERE id = ?5",
            params![
                PutAwayStatus::Completed.to_string(),
                to_location,
                now,
                input.notes,
                input.put_away_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Update receipt put_away_quantity
        let receipt_id = existing.receipt_id;
        let receipt_id_param = receipt_id.to_string();
        let put_away_params: [&dyn rusqlite::ToSql; 1] = [&receipt_id_param];
        let put_away_total = sum_decimal_query(
            &conn,
            "SELECT quantity FROM put_aways WHERE receipt_id = ?1 AND status = 'completed'",
            &put_away_params,
            "put_aways",
            "quantity",
        )?;

        conn.execute(
            "UPDATE receipts SET put_away_quantity = ?1 WHERE id = ?2",
            params![put_away_total.to_string(), receipt_id_param],
        )
        .map_err(map_db_error)?;

        self.get_put_away(input.put_away_id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete put-away".into()))
    }

    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE put_aways SET status = ?1 WHERE id = ?2",
            params![PutAwayStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_put_away(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel put-away".into()))
    }

    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.list_put_aways(PutAwayFilter {
            receipt_id: Some(receipt_id),
            status: Some(PutAwayStatus::Pending),
            ..Default::default()
        })
    }

    fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM put_aways WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(receipt_id) = filter.receipt_id {
            sql.push_str(" AND receipt_id = ?");
            params_vec.push(Box::new(receipt_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        let conn = self.conn()?;

        // Get PO items
        let mut stmt = conn
            .prepare("SELECT sku, name, quantity, unit_cost FROM purchase_order_items WHERE purchase_order_id = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![po_id.to_string()]).map_err(map_db_error)?;

        let mut items: Vec<CreateReceiptItem> = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let sku: String = row.get(0).map_err(map_db_error)?;
            let description: Option<String> = row.get(1).map_err(map_db_error)?;
            let qty_str: String = row.get(2).map_err(map_db_error)?;
            let cost_str: Option<String> = row.get(3).map_err(map_db_error)?;

            let expected_quantity =
                parse_decimal_strict(&qty_str, "purchase_order_item", "quantity")?;
            let unit_cost = parse_decimal_opt(cost_str, "purchase_order_item", "unit_cost")?;

            items.push(CreateReceiptItem {
                sku,
                description,
                po_line_id: None,
                expected_quantity,
                unit_cost,
                lot_number: None,
                expiration_date: None,
                notes: None,
            });
        }

        // Get supplier ID from PO
        let supplier_id_raw: Option<String> = conn
            .query_row(
                "SELECT supplier_id FROM purchase_orders WHERE id = ?1",
                params![po_id.to_string()],
                |row| row.get(0),
            )
            .ok();
        let supplier_id = parse_uuid_opt(supplier_id_raw, "purchase_order", "supplier_id")?;

        self.create_receipt(CreateReceipt {
            receipt_number: None,
            receipt_type: ReceiptType::PurchaseOrder,
            reference_type: Some("purchase_order".into()),
            reference_id: Some(po_id),
            supplier_id,
            warehouse_id,
            carrier: None,
            tracking_number: None,
            expected_date: None,
            notes: Some(format!("Created from PO {}", po_id)),
            created_by: None,
            items,
        })
    }

    fn create_receipts_batch(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>> {
        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_receipt(input) {
                Ok(receipt) => result.record_success(receipt),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        let mut receipts = Vec::new();
        for id in ids {
            if let Some(receipt) = self.get_receipt(id)? {
                receipts.push(receipt);
            }
        }
        Ok(receipts)
    }
}
