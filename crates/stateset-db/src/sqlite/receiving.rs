//! SQLite implementation for receiving management
//!
//! Provides goods receipt, receiving, and put-away functionality.

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt,
    parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid_opt,
    parse_uuid_opt_row, parse_uuid_row, sum_decimal_query, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::{
    BatchResult, CommerceError, CompletePutAway, CreatePutAway, CreateReceipt, CreateReceiptItem,
    PutAway, PutAwayFilter, PutAwayStatus, Receipt, ReceiptFilter, ReceiptItem, ReceiptStatus,
    ReceiptType, ReceiveItems, ReceivingRepository, Result, UpdateReceipt, generate_receipt_number,
};

/// SQLite receiving repository
#[derive(Debug)]
pub struct SqliteReceivingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteReceivingRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_receipt(row: &rusqlite::Row<'_>) -> rusqlite::Result<Receipt> {
        Ok(Receipt {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "receipt", "id")?,
            receipt_number: row.get("receipt_number")?,
            receipt_type: parse_enum_row(
                &row.get::<_, String>("receipt_type")?,
                "receipt",
                "receipt_type",
            )?,
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
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "receipt",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "receipt",
                "updated_at",
            )?,
        })
    }

    fn row_to_receipt_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReceiptItem> {
        Ok(ReceiptItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "receipt_item", "id")?,
            receipt_id: parse_uuid_row(
                &row.get::<_, String>("receipt_id")?,
                "receipt_item",
                "receipt_id",
            )?,
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
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "receipt_item",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "receipt_item",
                "updated_at",
            )?,
        })
    }

    fn row_to_put_away(row: &rusqlite::Row<'_>) -> rusqlite::Result<PutAway> {
        Ok(PutAway {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "put_away", "id")?,
            receipt_id: parse_uuid_row(
                &row.get::<_, String>("receipt_id")?,
                "put_away",
                "receipt_id",
            )?,
            receipt_item_id: parse_uuid_row(
                &row.get::<_, String>("receipt_item_id")?,
                "put_away",
                "receipt_item_id",
            )?,
            sku: row.get("sku")?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "put_away",
                "quantity",
            )?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("lot_id")?,
                "put_away",
                "lot_id",
            )?,
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
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "put_away",
                "created_at",
            )?,
        })
    }

    /// Smuggle a domain error through the `rusqlite` closure boundary so
    /// [`map_db_error`] unwraps it again (and `with_retry` never mistakes it for
    /// a lock error and retries it).
    fn smuggle(e: CommerceError) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    }

    /// Error for a guarded receipt write that matched no row: `NotFound` when
    /// the receipt is gone, otherwise the `ValidationError` the callers (and
    /// the Postgres backend) already produce, built from the status that
    /// actually blocked the write.
    fn receipt_guard_error(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        message: impl Fn(&str) -> String,
    ) -> rusqlite::Error {
        let current: Option<String> = tx
            .query_row("SELECT status FROM receipts WHERE id = ?1", params![id], |row| row.get(0))
            .ok();
        current.map_or_else(
            || Self::smuggle(CommerceError::NotFound),
            |status| Self::smuggle(CommerceError::ValidationError(message(&status))),
        )
    }

    /// Error for a status-guarded put-away UPDATE that matched no row: either
    /// the row is gone (`NotFound`) or its status forbids the transition
    /// (`Conflict`, naming the status that blocked it).
    fn put_away_conflict(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        action: &str,
    ) -> rusqlite::Error {
        let current: Option<String> = tx
            .query_row("SELECT status FROM put_aways WHERE id = ?1", params![id], |row| row.get(0))
            .ok();
        current.map_or_else(
            || Self::smuggle(CommerceError::NotFound),
            |status| {
                Self::smuggle(CommerceError::Conflict(format!(
                    "cannot {action} put-away {id}: status is {status}"
                )))
            },
        )
    }

    /// Read a single receipt by id from within a transaction.
    fn read_receipt_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<Receipt, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM receipts WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_receipt(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Read a single put-away by id from within a transaction.
    fn read_put_away_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<PutAway, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM put_aways WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_put_away(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Recompute a receipt's header quantities from its lines, inside the same
    /// transaction that changed those lines.
    ///
    /// The quantity columns are TEXT (migration 017), so the sums are taken with
    /// `rust_decimal::Decimal` rather than SQL `SUM`, which would coerce them to
    /// IEEE-754 floats.
    fn update_receipt_totals_tx(
        tx: &rusqlite::Transaction<'_>,
        receipt_id: &str,
    ) -> std::result::Result<(), rusqlite::Error> {
        let mut exp_total = Decimal::ZERO;
        let mut rcv_total = Decimal::ZERO;
        {
            let mut stmt = tx.prepare(
                "SELECT expected_quantity, received_quantity FROM receipt_items WHERE receipt_id = ?1",
            )?;
            let mut rows = stmt.query(params![receipt_id])?;
            while let Some(row) = rows.next()? {
                let expected_str: String = row.get(0)?;
                let received_str: String = row.get(1)?;
                exp_total += parse_decimal_row(&expected_str, "receipt_item", "expected_quantity")?;
                rcv_total += parse_decimal_row(&received_str, "receipt_item", "received_quantity")?;
            }
        }

        tx.execute(
            "UPDATE receipts SET expected_quantity = ?1, received_quantity = ?2 WHERE id = ?3",
            params![exp_total.to_string(), rcv_total.to_string(), receipt_id],
        )?;

        Ok(())
    }
}

impl ReceivingRepository for SqliteReceivingRepository {
    /// Create a receipt and its lines.
    ///
    /// Header and lines are written in one transaction: on a bare connection a
    /// failure part-way through the line loop left a receipt whose
    /// `expected_quantity` header total counted lines that were never inserted.
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let receipt_number = input.receipt_number.unwrap_or_else(generate_receipt_number);

        // Calculate expected quantity from items
        let expected_total: Decimal = input.items.iter().map(|i| i.expected_quantity).sum();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO receipts (id, receipt_number, receipt_type, status, reference_type, reference_id,
                 supplier_id, warehouse_id, carrier, tracking_number, expected_date, expected_quantity,
                 notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![
                    id_str,
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
            )?;

            // Create receipt items
            for (idx, item) in input.items.iter().enumerate() {
                let item_id = Uuid::new_v4();
                tx.execute(
                    "INSERT INTO receipt_items (id, receipt_id, line_number, sku, description, po_line_id,
                     expected_quantity, unit_cost, lot_number, expiration_date, notes, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        item_id.to_string(),
                        id_str,
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
                )?;
            }

            Self::read_receipt_by_id_tx(tx, &id_str)
        })
    }

    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM receipts WHERE id = ?1").map_err(map_db_error)?;

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

        self.get_receipt(id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated receipt".into())
        })
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

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut receipts = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            receipts.push(Self::row_to_receipt(row).map_err(map_db_error)?);
        }

        Ok(receipts)
    }

    /// Delete a receipt that has not started receiving.
    ///
    /// The status check and the DELETE are one guarded statement, so a receipt
    /// that starts receiving concurrently can no longer be deleted between the
    /// check and the write.
    fn delete_receipt(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "DELETE FROM receipts WHERE id = ?1 AND status = 'expected'",
                params![id_str],
            )?;
            if changed == 0 {
                return Err(Self::receipt_guard_error(tx, &id_str, |_| {
                    "Can only delete receipts in 'expected' status".into()
                }));
            }
            Ok(())
        })
    }

    /// Start receiving against a receipt ([`ReceiptStatus::Expected`] only).
    fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE receipts SET status = ?1, received_date = ?2
                 WHERE id = ?3 AND status = 'expected'",
                params![ReceiptStatus::InProgress.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::receipt_guard_error(tx, &id_str, |_| {
                    "Can only start receiving for 'expected' receipts".into()
                }));
            }
            Self::read_receipt_by_id_tx(tx, &id_str)
        })
    }

    /// Record received quantities against a receipt's lines.
    ///
    /// The whole operation — receipt status guard, the `Expected -> InProgress`
    /// flip, every line update and the header totals — runs in ONE `IMMEDIATE`
    /// transaction. Previously each statement ran on a bare connection, so a
    /// failure part-way through left the receipt half-applied (line quantities
    /// updated but the header totals stale, or vice versa) and concurrent
    /// receipts could interleave and lose each other's quantities.
    ///
    /// Guards, in order:
    /// 1. the receipt must exist, and be `Expected` or `InProgress` — the
    ///    statuses in which goods can still arrive;
    /// 2. each line's quantity must be positive and its rejected quantity
    ///    non-negative (mirroring the purchase-order `receive` guard);
    /// 3. the line must belong to *this* receipt;
    /// 4. re-read under the write lock, the cumulative received quantity may not
    ///    exceed the line's expected quantity. Without this cap 100 units could
    ///    be received against a 10-unit line, corrupting inventory and the
    ///    downstream three-way match.
    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        let now = Utc::now().to_rfc3339();
        let receipt_id = input.receipt_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            // Verify receipt exists and is in correct status
            let status_str: String = tx
                .query_row(
                    "SELECT status FROM receipts WHERE id = ?1",
                    params![receipt_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Self::smuggle(CommerceError::NotFound),
                    other => other,
                })?;
            let status: ReceiptStatus = parse_enum_row(&status_str, "receipt", "status")?;

            if status != ReceiptStatus::InProgress && status != ReceiptStatus::Expected {
                return Err(Self::smuggle(CommerceError::ValidationError(
                    "Receipt must be 'expected' or 'in_progress' to receive items".into(),
                )));
            }

            // Update receipt to in_progress if expected
            if status == ReceiptStatus::Expected {
                tx.execute(
                    "UPDATE receipts SET status = ?1, received_date = ?2 WHERE id = ?3",
                    params![ReceiptStatus::InProgress.to_string(), now, receipt_id],
                )?;
            }

            // Process each item
            for line in &input.items {
                let reject_qty = line.quantity_rejected.unwrap_or(Decimal::ZERO);
                let serial_str = line.serial_numbers.as_ref().map(|v| v.join(","));
                let line_id = line.receipt_item_id.to_string();

                if line.quantity_received <= Decimal::ZERO {
                    return Err(Self::smuggle(CommerceError::ValidationError(
                        "Received quantity must be greater than zero".into(),
                    )));
                }
                if reject_qty < Decimal::ZERO {
                    return Err(Self::smuggle(CommerceError::ValidationError(
                        "Rejected quantity cannot be negative".into(),
                    )));
                }

                // received_quantity, rejected_quantity and expected_quantity are TEXT
                // columns (migration 017), so accumulating in SQL via
                // 'CAST(received_quantity AS REAL) + ?1' would coerce both operands to
                // IEEE-754 floats ('0.1' + '0.2' = 0.30000000000000004) — corrupting
                // the stored quantity and misclassifying the status at the
                // received/expected boundary. Read the current row, add with
                // `rust_decimal::Decimal`, and write exact precomputed strings back.
                //
                // The read is scoped by `receipt_id` too: a line id belonging to
                // another receipt must not be receivable through this one.
                let (cur_received, cur_rejected, expected, cur_status): (
                    String,
                    String,
                    String,
                    String,
                ) = tx
                    .query_row(
                        "SELECT received_quantity, rejected_quantity, expected_quantity, status
                         FROM receipt_items WHERE id = ?1 AND receipt_id = ?2",
                        params![line_id, receipt_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            Self::smuggle(CommerceError::NotFound)
                        }
                        other => other,
                    })?;

                let new_received =
                    parse_decimal_row(&cur_received, "receipt_item", "received_quantity")?
                        + line.quantity_received;
                let new_rejected =
                    parse_decimal_row(&cur_rejected, "receipt_item", "rejected_quantity")?
                        + reject_qty;
                let expected = parse_decimal_row(&expected, "receipt_item", "expected_quantity")?;

                if new_received > expected {
                    return Err(Self::smuggle(CommerceError::ValidationError(format!(
                        "Receiving {new_received} would exceed expected quantity {expected} for receipt item {line_id}"
                    ))));
                }

                let new_status = if new_received >= expected {
                    "received"
                } else if new_received > Decimal::ZERO {
                    "partially_received"
                } else {
                    cur_status.as_str()
                };

                // Update receipt item
                tx.execute(
                    "UPDATE receipt_items SET
                     received_quantity = ?1,
                     rejected_quantity = ?2,
                     lot_number = COALESCE(?3, lot_number),
                     serial_numbers = COALESCE(?4, serial_numbers),
                     expiration_date = COALESCE(?5, expiration_date),
                     notes = COALESCE(?6, notes),
                     status = ?8
                     WHERE id = ?7",
                    params![
                        new_received.to_string(),
                        new_rejected.to_string(),
                        line.lot_number,
                        serial_str,
                        line.expiration_date.map(|d| d.to_rfc3339()),
                        line.notes,
                        line_id,
                        new_status,
                    ],
                )?;
            }

            // Update receipt totals
            Self::update_receipt_totals_tx(tx, &receipt_id)?;

            Self::read_receipt_by_id_tx(tx, &receipt_id)
        })
    }

    /// Complete receiving ([`ReceiptStatus::InProgress`] only).
    ///
    /// The header flip and the line-status sweep are one transaction: on a bare
    /// connection a failure between them left a `Received` receipt whose lines
    /// still claimed to be pending (or vice versa).
    fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE receipts SET status = ?1, completed_date = ?2
                 WHERE id = ?3 AND status = 'in_progress'",
                params![ReceiptStatus::Received.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::receipt_guard_error(tx, &id_str, |_| {
                    "Can only complete 'in_progress' receipts".into()
                }));
            }

            // Mark all items as received
            tx.execute(
                "UPDATE receipt_items SET status = 'received' WHERE receipt_id = ?1 AND status != 'rejected'",
                params![id_str],
            )?;

            Self::read_receipt_by_id_tx(tx, &id_str)
        })
    }

    /// Cancel a receipt.
    ///
    /// Legal only while [`ReceiptStatus::can_cancel`] holds (`Expected` or
    /// `InProgress`); once goods are received the receipt is a record of what
    /// physically arrived. The check and the write are now one guarded
    /// statement, so a receipt cannot be cancelled by a caller that read its
    /// status just before another thread completed it.
    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE receipts SET status = ?1
                 WHERE id = ?2 AND status IN ('expected', 'in_progress')",
                params![ReceiptStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::receipt_guard_error(tx, &id_str, |status| {
                    format!("Cannot cancel a receipt in {status} status (goods already received)")
                }));
            }
            Self::read_receipt_by_id_tx(tx, &id_str)
        })
    }

    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM receipt_items WHERE receipt_id = ?1 ORDER BY line_number")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![receipt_id.to_string()]).map_err(map_db_error)?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            items.push(Self::row_to_receipt_item(row).map_err(map_db_error)?);
        }

        Ok(items)
    }

    /// Count receipts matching `filter`.
    ///
    /// Applies exactly the filters `list_receipts` applies (and that the
    /// Postgres backend counts on); a count that ignored `receipt_type`,
    /// `supplier_id`, `reference_id` or the date window reported a page total
    /// for a different result set than the page itself.
    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM receipts WHERE 1=1".to_string();
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

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

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
        let mut stmt =
            conn.prepare("SELECT * FROM put_aways WHERE id = ?1").map_err(map_db_error)?;

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
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut put_aways = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            put_aways.push(Self::row_to_put_away(row).map_err(map_db_error)?);
        }

        Ok(put_aways)
    }

    /// Assign (or re-assign) a put-away task.
    ///
    /// Legal from `Pending`/`Assigned`; the UPDATE also writes
    /// `status = 'assigned'`, so assigning a started task would rewind it and
    /// assigning a completed one would resurrect it — dropping its quantity out
    /// of `receipts.put_away_quantity` on the next recompute.
    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE put_aways SET assigned_to = ?1, status = ?2
                 WHERE id = ?3 AND status IN ('pending', 'assigned')",
                params![assigned_to, PutAwayStatus::Assigned.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::put_away_conflict(tx, &id_str, "assign"));
            }
            Self::read_put_away_by_id_tx(tx, &id_str)
        })
    }

    /// Start a put-away task (`Pending`/`Assigned` only).
    fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE put_aways SET status = ?1, started_at = ?2
                 WHERE id = ?3 AND status IN ('pending', 'assigned')",
                params![PutAwayStatus::InProgress.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::put_away_conflict(tx, &id_str, "start"));
            }
            Self::read_put_away_by_id_tx(tx, &id_str)
        })
    }

    /// Complete a put-away task and fold its quantity into the receipt.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress`. Completing a cancelled task
    /// used to succeed and add its quantity to `receipts.put_away_quantity` for
    /// stock that was never put away. The status flip and the receipt total are
    /// one transaction, so the receipt can never quote a total that excludes a
    /// put-away already marked completed.
    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        let now = Utc::now().to_rfc3339();
        let id_str = input.put_away_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let existing = Self::read_put_away_by_id_tx(tx, &id_str)?;
            let to_location = input.actual_location_id.unwrap_or(existing.to_location_id);

            let changed = tx.execute(
                "UPDATE put_aways SET status = ?1, to_location_id = ?2, completed_at = ?3, notes = COALESCE(?4, notes)
                 WHERE id = ?5 AND status IN ('pending', 'assigned', 'in_progress')",
                params![
                    PutAwayStatus::Completed.to_string(),
                    to_location,
                    now,
                    input.notes,
                    id_str,
                ],
            )?;
            if changed == 0 {
                return Err(Self::put_away_conflict(tx, &id_str, "complete"));
            }

            // Update receipt put_away_quantity
            let receipt_id_param = existing.receipt_id.to_string();
            let put_away_params: [&dyn rusqlite::ToSql; 1] = [&receipt_id_param];
            let put_away_total = sum_decimal_query(
                tx,
                "SELECT quantity FROM put_aways WHERE receipt_id = ?1 AND status = 'completed'",
                &put_away_params,
                "put_aways",
                "quantity",
            )
            .map_err(Self::smuggle)?;

            tx.execute(
                "UPDATE receipts SET put_away_quantity = ?1 WHERE id = ?2",
                params![put_away_total.to_string(), receipt_id_param],
            )?;

            Self::read_put_away_by_id_tx(tx, &id_str)
        })
    }

    /// Cancel a put-away task.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress`. A `Completed` task is
    /// refused: the stock has physically moved, and cancelling it would leave
    /// `receipts.put_away_quantity` counting a put-away that claims not to have
    /// happened (cancellation does not recompute that total).
    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE put_aways SET status = ?1
                 WHERE id = ?2 AND status IN ('pending', 'assigned', 'in_progress')",
                params![PutAwayStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::put_away_conflict(tx, &id_str, "cancel"));
            }
            Self::read_put_away_by_id_tx(tx, &id_str)
        })
    }

    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.list_put_aways(PutAwayFilter {
            receipt_id: Some(receipt_id),
            status: Some(PutAwayStatus::Pending),
            ..Default::default()
        })
    }

    /// Count put-aways matching `filter` (same filters as `list_put_aways`).
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

        if let Some(assigned_to) = filter.assigned_to {
            sql.push_str(" AND assigned_to = ?");
            params_vec.push(Box::new(assigned_to));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        let conn = self.conn()?;

        // Get PO items
        let mut stmt = conn
            .prepare("SELECT sku, name, quantity_ordered, unit_cost FROM purchase_order_items WHERE purchase_order_id = ?1")
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
            notes: Some(format!("Created from PO {po_id}")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateReceipt, CreateReceiptItem, CreateWarehouse, ReceiptItemStatus, ReceiptType,
        ReceiveItemLine, ReceiveItems, WarehouseRepository, WarehouseType,
    };

    /// A receiving repo backed by a DB that already has warehouse id 1 seeded
    /// (receipts carry a FOREIGN KEY onto `warehouses`).
    fn fresh_repo() -> SqliteReceivingRepository {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        db.warehouse()
            .create_warehouse(CreateWarehouse {
                code: "WH-RCV".into(),
                name: "Receiving Test".into(),
                warehouse_type: WarehouseType::Distribution,
                ..Default::default()
            })
            .expect("seed warehouse");
        db.receiving()
    }

    /// Create a receipt with a single line of the given expected quantity and
    /// return `(receipt_id, receipt_item_id)`.
    fn receipt_with_one_item(repo: &SqliteReceivingRepository, expected: Decimal) -> (Uuid, Uuid) {
        let receipt = repo
            .create_receipt(CreateReceipt {
                receipt_type: ReceiptType::PurchaseOrder,
                warehouse_id: 1,
                items: vec![CreateReceiptItem {
                    sku: "SKU-1".into(),
                    expected_quantity: expected,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create receipt");
        let items = repo.get_receipt_items(receipt.id).expect("items");
        let item_id = items.first().expect("one item").id;
        (receipt.id, item_id)
    }

    fn item_status(
        repo: &SqliteReceivingRepository,
        receipt_id: Uuid,
        item_id: Uuid,
    ) -> ReceiptItemStatus {
        repo.get_receipt_items(receipt_id)
            .expect("items")
            .into_iter()
            .find(|i| i.id == item_id)
            .expect("item present")
            .status
    }

    fn receive(repo: &SqliteReceivingRepository, receipt_id: Uuid, item_id: Uuid, qty: Decimal) {
        repo.receive_items(ReceiveItems {
            receipt_id,
            items: vec![ReceiveItemLine {
                receipt_item_id: item_id,
                quantity_received: qty,
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
            receiving_location_id: None,
            received_by: None,
        })
        .expect("receive items");
    }

    fn item_received(repo: &SqliteReceivingRepository, receipt_id: Uuid, item_id: Uuid) -> Decimal {
        repo.get_receipt_items(receipt_id)
            .expect("items")
            .into_iter()
            .find(|i| i.id == item_id)
            .expect("item present")
            .received_quantity
    }

    /// The status guards are expressed as SQL string literals; if an enum's
    /// `Display` ever drifts from those literals the guards would silently stop
    /// matching (allowing everything, or nothing). Pin the mapping.
    #[test]
    fn status_sql_literals_match_enum_display() {
        assert_eq!(ReceiptStatus::Expected.to_string(), "expected");
        assert_eq!(ReceiptStatus::InProgress.to_string(), "in_progress");
        assert_eq!(ReceiptStatus::Received.to_string(), "received");
        assert_eq!(ReceiptStatus::Cancelled.to_string(), "cancelled");

        assert_eq!(ReceiptItemStatus::Received.to_string(), "received");
        assert_eq!(ReceiptItemStatus::PartiallyReceived.to_string(), "partially_received");
        assert_eq!(ReceiptItemStatus::Rejected.to_string(), "rejected");

        assert_eq!(PutAwayStatus::Pending.to_string(), "pending");
        assert_eq!(PutAwayStatus::Assigned.to_string(), "assigned");
        assert_eq!(PutAwayStatus::InProgress.to_string(), "in_progress");
        assert_eq!(PutAwayStatus::Completed.to_string(), "completed");
        assert_eq!(PutAwayStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn create_receipt_from_po_copies_po_lines() {
        // Regression: the PO-line query selected a nonexistent `quantity` column
        // (schema has `quantity_ordered`), so this call always failed.
        use stateset_core::{
            CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PurchaseOrderRepository,
        };
        let db = SqliteDatabase::in_memory().expect("in-memory");
        db.warehouse()
            .create_warehouse(CreateWarehouse {
                code: "WH-PO".into(),
                name: "PO Receiving".into(),
                warehouse_type: WarehouseType::Distribution,
                ..Default::default()
            })
            .expect("seed warehouse");
        let supplier = db
            .purchase_orders()
            .create_supplier(CreateSupplier { name: "Acme".into(), ..Default::default() })
            .expect("supplier");
        let po = db
            .purchase_orders()
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![CreatePurchaseOrderItem {
                    sku: "SKU-PO".into(),
                    name: "Widget".into(),
                    quantity: dec!(7),
                    unit_cost: dec!(3.50),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create PO");

        let receipt =
            db.receiving().create_receipt_from_po(po.id.into(), 1).expect("receipt from PO");
        let items = db.receiving().get_receipt_items(receipt.id).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sku, "SKU-PO");
        assert_eq!(items[0].expected_quantity, dec!(7));
    }

    #[test]
    fn two_partial_receipts_keep_received_quantity_exact() {
        // Regression: received_quantity is a TEXT column and was accumulated via
        // 'CAST(received_quantity AS REAL) + ?', so 0.1 + 0.2 stored as
        // 0.30000000000000004. With Decimal arithmetic it must be exactly 0.3.
        let repo = fresh_repo();
        let (rid, iid) = receipt_with_one_item(&repo, dec!(1));

        receive(&repo, rid, iid, dec!(0.1));
        receive(&repo, rid, iid, dec!(0.2));

        assert_eq!(item_received(&repo, rid, iid), dec!(0.3));
    }

    #[test]
    fn receipt_item_status_tracks_received_vs_expected_exactly() {
        let repo = fresh_repo();
        let (rid, iid) = receipt_with_one_item(&repo, dec!(0.3));

        // Partial receipt -> partially_received.
        receive(&repo, rid, iid, dec!(0.1));
        assert_eq!(item_status(&repo, rid, iid), ReceiptItemStatus::PartiallyReceived);

        // 0.1 + 0.2 == 0.3 exactly meets expected -> received (a float residue
        // of 0.30000000000000004 would also pass >=, but an under-count like
        // 0.29999999999999998 would wrongly stay partially_received).
        receive(&repo, rid, iid, dec!(0.2));
        assert_eq!(item_status(&repo, rid, iid), ReceiptItemStatus::Received);
        assert_eq!(item_received(&repo, rid, iid), dec!(0.3));
    }
}
