//! SQLite implementation for Accounts Payable

use crate::sqlite::{
    map_db_error, parse_datetime, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid,
    parse_uuid_opt_row, parse_uuid_row, sum_decimal_query, with_immediate_transaction,
};
use chrono::{DateTime, NaiveTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use rust_decimal::{Decimal, RoundingStrategy};
use std::collections::HashMap;
use uuid::Uuid;

use stateset_core::{
    AccountsPayableRepository, ApAgingSummary, BatchResult, Bill, BillFilter, BillItem,
    BillPayment, BillPaymentFilter, BillStatus, CommerceError, CreateBill, CreateBillItem,
    CreateBillPayment, CreatePaymentRun, PaymentAllocation, PaymentRun, PaymentRunFilter,
    PaymentRunStatus, PaymentStatusAP, Result, SupplierApSummary, UpdateBill,
    generate_ap_payment_number, generate_bill_number, generate_payment_run_number,
};

#[derive(Debug)]
pub struct SqliteAccountsPayableRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAccountsPayableRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Insert one bill line (next line number, rounded amounts) on the given
    /// connection/transaction. Status guards are the caller's responsibility.
    fn insert_bill_item_with_conn(
        conn: &rusqlite::Connection,
        item_id: Uuid,
        bill_id: Uuid,
        item: &CreateBillItem,
        now: &str,
    ) -> rusqlite::Result<()> {
        let line_number: i32 = conn.query_row(
            "SELECT COALESCE(MAX(line_number), 0) + 1 FROM ap_bill_items WHERE bill_id = ?1",
            params![bill_id.to_string()],
            |row| row.get(0),
        )?;

        let amount = item.quantity * item.unit_price;
        let tax_amount = item.tax_rate.map_or(Decimal::ZERO, |r| amount * r / Decimal::from(100));

        conn.execute(
            "INSERT INTO ap_bill_items (id, bill_id, line_number, description, account_code, quantity, unit_price, amount, tax_rate, tax_amount, po_line_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                item_id.to_string(),
                bill_id.to_string(),
                line_number,
                item.description,
                item.account_code,
                round_ap(item.quantity, 4),
                round_ap(item.unit_price, 4),
                round_ap(amount, 4),
                item.tax_rate.map(|r| round_ap(r, 6)),
                round_ap(tax_amount, 4),
                item.po_line_id.map(|id| id.to_string()),
                now,
            ],
        )?;
        Ok(())
    }

    fn row_to_bill(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bill> {
        Ok(Bill {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "bill", "id")?,
            bill_number: row.get("bill_number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "bill",
                "supplier_id",
            )?,
            supplier_name: row.get("supplier_name")?,
            purchase_order_id: parse_uuid_opt_row(
                row.get("purchase_order_id")?,
                "bill",
                "purchase_order_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "bill", "status")?,
            bill_date: parse_datetime_row(
                &row.get::<_, String>("bill_date")?,
                "bill",
                "bill_date",
            )?,
            due_date: parse_datetime_row(&row.get::<_, String>("due_date")?, "bill", "due_date")?,
            payment_terms: row.get("payment_terms")?,
            subtotal: parse_decimal_row(&row.get::<_, String>("subtotal")?, "bill", "subtotal")?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "bill",
                "tax_amount",
            )?,
            shipping_amount: parse_decimal_row(
                &row.get::<_, String>("shipping_amount")?,
                "bill",
                "shipping_amount",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "bill",
                "discount_amount",
            )?,
            total_amount: parse_decimal_row(
                &row.get::<_, String>("total_amount")?,
                "bill",
                "total_amount",
            )?,
            amount_paid: parse_decimal_row(
                &row.get::<_, String>("amount_paid")?,
                "bill",
                "amount_paid",
            )?,
            amount_due: parse_decimal_row(
                &row.get::<_, String>("amount_due")?,
                "bill",
                "amount_due",
            )?,
            currency: row.get("currency")?,
            reference_number: row.get("reference_number")?,
            memo: row.get("memo")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "bill",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "bill",
                "updated_at",
            )?,
        })
    }

    fn row_to_bill_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillItem> {
        Ok(BillItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "bill_item", "id")?,
            bill_id: parse_uuid_row(&row.get::<_, String>("bill_id")?, "bill_item", "bill_id")?,
            line_number: row.get("line_number")?,
            description: row.get("description")?,
            account_code: row.get("account_code")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "bill_item",
                "quantity",
            )?,
            unit_price: parse_decimal_row(
                &row.get::<_, String>("unit_price")?,
                "bill_item",
                "unit_price",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "bill_item", "amount")?,
            tax_rate: parse_decimal_opt_row(row.get("tax_rate")?, "bill_item", "tax_rate")?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "bill_item",
                "tax_amount",
            )?,
            po_line_id: parse_uuid_opt_row(row.get("po_line_id")?, "bill_item", "po_line_id")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "bill_item",
                "created_at",
            )?,
        })
    }

    fn row_to_payment(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillPayment> {
        Ok(BillPayment {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "bill_payment", "id")?,
            payment_number: row.get("payment_number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "bill_payment",
                "supplier_id",
            )?,
            payment_date: parse_datetime_row(
                &row.get::<_, String>("payment_date")?,
                "bill_payment",
                "payment_date",
            )?,
            payment_method: parse_enum_row(
                &row.get::<_, String>("payment_method")?,
                "bill_payment",
                "payment_method",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "bill_payment", "amount")?,
            currency: row.get("currency")?,
            reference_number: row.get("reference_number")?,
            bank_account: row.get("bank_account")?,
            check_number: row.get("check_number")?,
            memo: row.get("memo")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "bill_payment", "status")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "bill_payment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "bill_payment",
                "updated_at",
            )?,
        })
    }

    fn row_to_payment_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<PaymentRun> {
        Ok(PaymentRun {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "payment_run", "id")?,
            run_number: row.get("run_number")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "payment_run", "status")?,
            payment_date: parse_datetime_row(
                &row.get::<_, String>("payment_date")?,
                "payment_run",
                "payment_date",
            )?,
            payment_method: parse_enum_row(
                &row.get::<_, String>("payment_method")?,
                "payment_run",
                "payment_method",
            )?,
            total_amount: parse_decimal_row(
                &row.get::<_, String>("total_amount")?,
                "payment_run",
                "total_amount",
            )?,
            payment_count: row.get("payment_count")?,
            notes: row.get("notes")?,
            created_by: row.get("created_by")?,
            approved_by: row.get("approved_by")?,
            approved_at: parse_datetime_opt_row(
                row.get("approved_at")?,
                "payment_run",
                "approved_at",
            )?,
            processed_at: parse_datetime_opt_row(
                row.get("processed_at")?,
                "payment_run",
                "processed_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment_run",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment_run",
                "updated_at",
            )?,
        })
    }

    fn recalculate_bill_with_conn(conn: &rusqlite::Connection, bill_id: Uuid) -> Result<()> {
        let bill_id_param = bill_id.to_string();
        let mut stmt = conn
            .prepare("SELECT amount, tax_amount FROM ap_bill_items WHERE bill_id = ?1")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![&bill_id_param]).map_err(map_db_error)?;
        let mut subtotal_dec = Decimal::ZERO;
        let mut tax_dec = Decimal::ZERO;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let amount_str: String = row.get(0).map_err(map_db_error)?;
            let tax_str: String = row.get(1).map_err(map_db_error)?;
            subtotal_dec += parse_decimal_strict(&amount_str, "ap_bill_items", "amount")?;
            tax_dec += parse_decimal_strict(&tax_str, "ap_bill_items", "tax_amount")?;
        }
        let total = subtotal_dec + tax_dec;

        let payment_params: [&dyn rusqlite::ToSql; 1] = [&bill_id_param];
        let paid = sum_decimal_query(
            conn,
            "SELECT a.amount
             FROM ap_payment_allocations a
             JOIN ap_payments p ON p.id = a.payment_id
             WHERE a.bill_id = ?1
               AND p.status NOT IN ('voided', 'failed')",
            &payment_params,
            "ap_payment_allocations",
            "amount",
        )?;
        let due = total - paid;

        conn.execute(
            "UPDATE ap_bills SET subtotal = ?1, tax_amount = ?2, total_amount = ?3, amount_paid = ?4, amount_due = ?5 WHERE id = ?6",
            params![
                round_ap(subtotal_dec, 4),
                round_ap(tax_dec, 4),
                round_ap(total, 4),
                round_ap(paid, 4),
                round_ap(due, 4),
                bill_id.to_string()
            ],
        ).map_err(map_db_error)?;

        Ok(())
    }

    /// Returns all `ap_bills` matching `filter` (`supplier_id`, `status`,
    /// `purchase_order_id`, `overdue_only`, the `from_date`/`to_date` range, and
    /// the Rust-side min/max amount thresholds), ordered by `due_date`, WITHOUT
    /// pagination. Shared by `list_bills`
    /// (which then paginates) and `count_bills` (which counts the result) so a
    /// filtered count always matches the filtered list, both agreeing with Postgres.
    fn matched_bills(&self, filter: &BillFilter) -> Result<Vec<Bill>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM ap_bills WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(supplier_id) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }
        if let Some(purchase_order_id) = filter.purchase_order_id {
            sql.push_str(" AND purchase_order_id = ?");
            params_vec.push(Box::new(purchase_order_id.to_string()));
        }
        if filter.overdue_only == Some(true) {
            // `due_date` is stored at midnight UTC ('...T00:00:00+00:00') and
            // `datetime('now')` renders '... HH:MM:SS'; when the date parts are
            // equal the lexical compare sees 'T' > ' ', so a bill due today is
            // NOT overdue — exactly matching Postgres's strict
            // `due_date < CURRENT_DATE`. (Only `<=` comparisons diverge; see
            // `get_bills_due_soon`.)
            sql.push_str(" AND due_date < datetime('now') AND status NOT IN ('paid', 'cancelled')");
        }
        // `bill_date` is stored truncated to midnight UTC (like Postgres's `DATE`),
        // and the bound value is likewise midnight UTC, so the RFC3339 string
        // comparison is a well-defined date comparison — matching Postgres, which
        // filters `bill_date >= from_date` / `bill_date <= to_date`.
        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND bill_date >= ?");
            params_vec.push(Box::new(ap_date_rfc3339(from_date)));
        }
        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND bill_date <= ?");
            params_vec.push(Box::new(ap_date_rfc3339(to_date)));
        }

        sql.push_str(" ORDER BY due_date");

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        // The min/max_amount thresholds compare the TEXT `total_amount` column,
        // which SQLite cannot compare numerically without a lossy `CAST` — filter
        // them exactly in Rust (already parsed to `Decimal`).
        let mut matched = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let bill = Self::row_to_bill(row).map_err(map_db_error)?;
            if let Some(min) = filter.min_amount {
                if bill.total_amount < min {
                    continue;
                }
            }
            if let Some(max) = filter.max_amount {
                if bill.total_amount > max {
                    continue;
                }
            }
            matched.push(bill);
        }
        Ok(matched)
    }
}

/// Round an accounts-payable money value to the scale of the Postgres
/// `NUMERIC(12, s)` columns before storing it as TEXT, using `MidpointAwayFromZero`
/// (Postgres numeric rounding) so the two backends store and read back identical
/// values. Money columns use scale 4; `tax_rate` uses scale 6.
fn round_ap(value: Decimal, scale: u32) -> String {
    value.round_dp_with_strategy(scale, RoundingStrategy::MidpointAwayFromZero).to_string()
}

/// Format an accounts-payable date column (`bill_date`, `due_date`, `payment_date`)
/// for storage. Postgres holds these as `DATE`, which drops the time-of-day and
/// reads back at midnight UTC; this truncates to midnight UTC so SQLite reads back
/// the same value, while keeping the RFC3339 format so every existing reader (and
/// the auto-posting date parsers) continues to work unchanged.
fn ap_date_rfc3339(dt: DateTime<Utc>) -> String {
    dt.date_naive().and_time(NaiveTime::MIN).and_utc().to_rfc3339()
}

/// Appends the shared `ap_payments` filter predicates (`supplier_id`, `status`,
/// `payment_method`, `from_date`/`to_date`) used by both `list_payments` and
/// `count_payments`, so a filtered count always matches the filtered list and
/// both agree with Postgres.
fn push_payment_filters(
    sql: &mut String,
    params_vec: &mut Vec<Box<dyn rusqlite::ToSql>>,
    filter: &BillPaymentFilter,
) {
    if let Some(supplier_id) = filter.supplier_id {
        sql.push_str(" AND supplier_id = ?");
        params_vec.push(Box::new(supplier_id.to_string()));
    }
    if let Some(status) = filter.status {
        sql.push_str(" AND status = ?");
        params_vec.push(Box::new(status.to_string()));
    }
    if let Some(method) = filter.payment_method {
        sql.push_str(" AND payment_method = ?");
        params_vec.push(Box::new(method.to_string()));
    }
    // `payment_date` is stored/bound at midnight UTC (like Postgres's `DATE`), so
    // the RFC3339 comparison is a date comparison — matching Postgres.
    if let Some(from_date) = filter.from_date {
        sql.push_str(" AND payment_date >= ?");
        params_vec.push(Box::new(ap_date_rfc3339(from_date)));
    }
    if let Some(to_date) = filter.to_date {
        sql.push_str(" AND payment_date <= ?");
        params_vec.push(Box::new(ap_date_rfc3339(to_date)));
    }
}

impl AccountsPayableRepository for SqliteAccountsPayableRepository {
    fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let CreateBill {
            bill_number,
            supplier_id,
            purchase_order_id,
            bill_date,
            due_date,
            payment_terms,
            currency,
            reference_number,
            memo,
            items,
            ..
        } = input;
        let bill_number = bill_number.unwrap_or_else(generate_bill_number);

        // Header, every line, and the recalculated totals commit together: a
        // failing line insert must not leave a partial draft bill behind
        // (previously each item was added in its own transaction, so a 5-line
        // bill could persist with 3 lines and an understated total).
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO ap_bills (id, bill_number, supplier_id, purchase_order_id, status, bill_date, due_date,
                 payment_terms, currency, reference_number, memo, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![
                    id.to_string(),
                    bill_number,
                    supplier_id.to_string(),
                    purchase_order_id.map(|id| id.to_string()),
                    BillStatus::Draft.to_string(),
                    ap_date_rfc3339(bill_date.unwrap_or(now)),
                    ap_date_rfc3339(due_date),
                    payment_terms,
                    currency.unwrap_or_default(),
                    reference_number,
                    memo,
                    now.to_rfc3339(),
                ],
            )?;

            let now_str = now.to_rfc3339();
            for item in &items {
                Self::insert_bill_item_with_conn(tx, Uuid::new_v4(), id, item, &now_str)?;
            }

            Self::recalculate_bill_with_conn(tx, id)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            tx.query_row(
                "SELECT * FROM ap_bills WHERE id = ?1",
                params![id.to_string()],
                Self::row_to_bill,
            )
        })
    }

    fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM ap_bills WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_bill(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM ap_bills WHERE bill_number = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![number]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_bill(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        let conn = self.conn()?;
        let existing = self.get_bill(id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE ap_bills SET due_date = ?1, payment_terms = ?2, reference_number = ?3, memo = ?4 WHERE id = ?5",
            params![
                ap_date_rfc3339(input.due_date.unwrap_or(existing.due_date)),
                input.payment_terms.or(existing.payment_terms),
                input.reference_number.or(existing.reference_number),
                input.memo.or(existing.memo),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update bill".into()))
    }

    fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        // Shared matching logic (WHERE + Rust min/max) with `count_bills`, then
        // paginate — matching the Postgres order (WHERE then LIMIT/OFFSET).
        let mut matched = self.matched_bills(&filter)?;

        let offset = filter.offset.unwrap_or(0) as usize;
        let mut page = if offset >= matched.len() { Vec::new() } else { matched.split_off(offset) };
        if let Some(limit) = filter.limit {
            page.truncate(limit as usize);
        }
        Ok(page)
    }

    fn delete_bill(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let bill = self.get_bill(id)?.ok_or(CommerceError::NotFound)?;

        if bill.status != BillStatus::Draft {
            return Err(CommerceError::ValidationError("Can only delete draft bills".into()));
        }

        conn.execute("DELETE FROM ap_bills WHERE id = ?1", params![id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        let conn = self.conn()?;
        // Status-guarded UPDATE: only a draft/pending bill can be approved, and
        // 0 matched rows is an error (previously a silent success that returned
        // the bill unchanged).
        let rows = conn
            .execute(
                "UPDATE ap_bills SET status = ?1 WHERE id = ?2 AND status IN ('draft', 'pending')",
                params![BillStatus::Approved.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Bill not found or not in an approvable status".into(),
            ));
        }

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve bill".into()))
    }

    fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        let conn = self.conn()?;
        // A bill with payments applied (paid/partially paid) or already
        // cancelled cannot be cancelled; void the payments first.
        let rows = conn
            .execute(
                "UPDATE ap_bills SET status = ?1 WHERE id = ?2
                 AND status IN ('draft', 'pending', 'approved', 'overdue', 'disputed')",
                params![BillStatus::Cancelled.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Bill not found or not in a cancellable status".into(),
            ));
        }

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel bill".into()))
    }

    fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        let conn = self.conn()?;
        // Only an open (unpaid or partially paid) bill can be disputed — never a
        // cancelled or fully paid one.
        let rows = conn
            .execute(
                "UPDATE ap_bills SET status = ?1 WHERE id = ?2
                 AND status IN ('draft', 'pending', 'approved', 'partially_paid', 'overdue')",
                params![BillStatus::Disputed.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Bill not found or not in a disputable status".into(),
            ));
        }

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to dispute bill".into()))
    }

    fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM ap_bill_items WHERE bill_id = ?1 ORDER BY line_number")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![bill_id.to_string()]).map_err(map_db_error)?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            items.push(Self::row_to_bill_item(row).map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        // Item edits change the bill total, so only draft/pending bills may be
        // edited (payments cannot exist before approval, so `amount_due` can
        // never go negative under this guard). The status check shares the
        // mutation's transaction so a concurrent approval cannot slip between
        // the check and the insert. Previously unguarded: adding an item to a
        // paid bill created owed money invisible to outstanding/aging.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

            let status: String = tx.query_row(
                "SELECT status FROM ap_bills WHERE id = ?1",
                params![bill_id.to_string()],
                |row| row.get(0),
            )?;
            let bill_status: BillStatus = status.parse().map_err(|e| {
                to_rusqlite(CommerceError::DatabaseError(format!(
                    "Invalid bill status '{status}' while adding bill item: {e}"
                )))
            })?;
            if !matches!(bill_status, BillStatus::Draft | BillStatus::Pending) {
                return Err(to_rusqlite(CommerceError::Conflict(format!(
                    "Cannot add items to a bill in status '{status}'; only draft or pending bills may be edited"
                ))));
            }

            Self::insert_bill_item_with_conn(tx, id, bill_id, &item, &now)?;

            Self::recalculate_bill_with_conn(tx, bill_id).map_err(to_rusqlite)?;

            tx.query_row(
                "SELECT * FROM ap_bill_items WHERE id = ?1",
                params![id.to_string()],
                Self::row_to_bill_item,
            )
        })
    }

    fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        // Same guard as `add_bill_item`: only draft/pending bills may have
        // items removed. Previously unguarded: removing an item from a fully
        // paid bill drove `amount_due` negative while status stayed 'paid'.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

            let (bill_id, status): (String, String) = tx.query_row(
                "SELECT b.id, b.status FROM ap_bill_items i JOIN ap_bills b ON b.id = i.bill_id WHERE i.id = ?1",
                params![item_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let bill_status: BillStatus = status.parse().map_err(|e| {
                to_rusqlite(CommerceError::DatabaseError(format!(
                    "Invalid bill status '{status}' while removing bill item: {e}"
                )))
            })?;
            if !matches!(bill_status, BillStatus::Draft | BillStatus::Pending) {
                return Err(to_rusqlite(CommerceError::Conflict(format!(
                    "Cannot remove items from a bill in status '{status}'; only draft or pending bills may be edited"
                ))));
            }

            tx.execute("DELETE FROM ap_bill_items WHERE id = ?1", params![item_id.to_string()])?;

            let bill_uuid = parse_uuid(&bill_id, "bill_item", "bill_id").map_err(to_rusqlite)?;
            Self::recalculate_bill_with_conn(tx, bill_uuid).map_err(to_rusqlite)?;
            Ok(())
        })
    }

    fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        // Share `list_bills`' matching logic (all filters except pagination) so a
        // filtered count always equals the length of the filtered list.
        Ok(self.matched_bills(&filter)?.len() as u64)
    }

    fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.list_bills(BillFilter { overdue_only: Some(true), ..Default::default() })
    }

    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        let conn = self.conn()?;
        // Compare calendar dates, not strings: `due_date` is stored RFC3339
        // ('...T00:00:00+00:00') while `datetime('now', ...)` renders
        // '... HH:MM:SS', and lexically 'T' > ' ', so a plain `<=` always
        // excluded a bill due exactly on the window's last day — diverging from
        // Postgres's `due_date <= CURRENT_DATE + $1`. `date()` normalizes both
        // sides to 'YYYY-MM-DD'.
        let mut stmt = conn.prepare(
            "SELECT * FROM ap_bills WHERE date(due_date) <= date('now', '+' || ?1 || ' days') AND status NOT IN ('paid', 'cancelled') ORDER BY due_date"
        ).map_err(map_db_error)?;

        let mut rows = stmt.query(params![days]).map_err(map_db_error)?;
        let mut bills = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            bills.push(Self::row_to_bill(row).map_err(map_db_error)?);
        }
        Ok(bills)
    }

    fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Payment amount must be greater than zero".into(),
            ));
        }
        if input.allocations.is_empty() {
            return Err(CommerceError::ValidationError(
                "At least one payment allocation is required".into(),
            ));
        }

        let now = Utc::now();
        let id = Uuid::new_v4();
        let payment_number = generate_ap_payment_number();

        let mut allocation_total = Decimal::ZERO;
        let mut allocation_by_bill: HashMap<Uuid, Decimal> = HashMap::new();
        for alloc in &input.allocations {
            if alloc.amount <= Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Allocation amount must be greater than zero".into(),
                ));
            }
            allocation_total += alloc.amount;
            *allocation_by_bill.entry(alloc.bill_id).or_insert(Decimal::ZERO) += alloc.amount;
        }

        if allocation_total != input.amount {
            return Err(CommerceError::ValidationError(
                "Payment amount must equal allocation total".into(),
            ));
        }

        // Immediate (write-locked) transaction so concurrent payments against the
        // same bill serialize: each re-reads amount_due under the lock, so a
        // second payment cannot over-pay a supplier past the bill balance. Was a
        // deferred `super::begin_immediate(&mut conn)` with no BUSY retry.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

            for (bill_id, allocated_amount) in &allocation_by_bill {
                let (supplier_id, status, amount_due): (String, String, String) = tx.query_row(
                    "SELECT supplier_id, status, amount_due FROM ap_bills WHERE id = ?1",
                    params![bill_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;

                let parsed_supplier_id =
                    parse_uuid(&supplier_id, "bill", "supplier_id").map_err(to_rusqlite)?;
                if parsed_supplier_id != input.supplier_id {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Allocation bill supplier does not match payment supplier".into(),
                    )));
                }

                let bill_status: BillStatus = status.parse().map_err(|e| {
                    to_rusqlite(CommerceError::DatabaseError(format!(
                        "Invalid bill status '{status}' while creating payment: {e}"
                    )))
                })?;
                if !matches!(
                    bill_status,
                    BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue
                ) {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Bill is not in a payable status".into(),
                    )));
                }

                let amount_due =
                    parse_decimal_strict(&amount_due, "bill", "amount_due").map_err(to_rusqlite)?;
                if *allocated_amount > amount_due {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Allocation amount exceeds bill amount due".into(),
                    )));
                }
            }

            tx.execute(
                "INSERT INTO ap_payments (id, payment_number, supplier_id, payment_date, payment_method, amount, currency, reference_number, bank_account, check_number, memo, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                params![
                    id.to_string(),
                    payment_number,
                    input.supplier_id.to_string(),
                    ap_date_rfc3339(input.payment_date.unwrap_or(now)),
                    input.payment_method.to_string(),
                    input.amount.to_string(),
                    input.currency.unwrap_or_default(),
                    &input.reference_number,
                    &input.bank_account,
                    &input.check_number,
                    &input.memo,
                    PaymentStatusAP::Pending.to_string(),
                    now.to_rfc3339(),
                ],
            )?;

            for alloc in &input.allocations {
                let alloc_id = Uuid::new_v4();
                tx.execute(
                    "INSERT INTO ap_payment_allocations (id, payment_id, bill_id, amount, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        alloc_id.to_string(),
                        id.to_string(),
                        alloc.bill_id.to_string(),
                        alloc.amount.to_string(),
                        now.to_rfc3339()
                    ],
                )?;

                Self::recalculate_bill_with_conn(tx, alloc.bill_id).map_err(to_rusqlite)?;

                let bill = tx.query_row(
                    "SELECT * FROM ap_bills WHERE id = ?1",
                    params![alloc.bill_id.to_string()],
                    Self::row_to_bill,
                )?;
                let new_status = if bill.amount_due <= Decimal::ZERO {
                    BillStatus::Paid
                } else if bill.amount_paid > Decimal::ZERO {
                    BillStatus::PartiallyPaid
                } else {
                    bill.status
                };

                tx.execute(
                    "UPDATE ap_bills SET status = ?1 WHERE id = ?2",
                    params![new_status.to_string(), alloc.bill_id.to_string()],
                )?;
            }

            Ok(())
        })?;

        self.get_payment(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create payment".into()))
    }

    fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM ap_payments WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_payment(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM ap_payments WHERE payment_number = ?1")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![number]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_payment(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM ap_payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        push_payment_filters(&mut sql, &mut params_vec, &filter);

        sql.push_str(" ORDER BY payment_date DESC");

        // SQLite rejects OFFSET without LIMIT, so use `LIMIT -1` (unbounded) when
        // only an offset is set.
        match (filter.limit, filter.offset) {
            (Some(limit), Some(offset)) => sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}")),
            (Some(limit), None) => sql.push_str(&format!(" LIMIT {limit}")),
            (None, Some(offset)) => sql.push_str(&format!(" LIMIT -1 OFFSET {offset}")),
            (None, None) => {}
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut payments = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            payments.push(Self::row_to_payment(row).map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        // Immediate (write-locked) transaction with a status guard so a
        // duplicate/concurrent void cannot reverse the same payment twice, and so
        // it serializes (with BUSY retry) against create_payment on the same bill.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

            // Guard: only a not-yet-voided payment can be voided.
            let rows = tx.execute(
                "UPDATE ap_payments SET status = ?1 WHERE id = ?2 AND status != ?1",
                params![PaymentStatusAP::Voided.to_string(), id.to_string()],
            )?;
            if rows == 0 {
                return Err(to_rusqlite(CommerceError::Conflict(
                    "Payment not found or already voided".into(),
                )));
            }

            let allocations: Vec<PaymentAllocation> = {
                let mut stmt = tx.prepare("SELECT id, payment_id, bill_id, amount, created_at FROM ap_payment_allocations WHERE payment_id = ?1")?;
                let mut rows = stmt.query(params![id.to_string()])?;
                let mut values = Vec::new();
                while let Some(row) = rows.next()? {
                    values.push(PaymentAllocation {
                        id: parse_uuid(&row.get::<_, String>(0)?, "payment_allocation", "id")
                            .map_err(to_rusqlite)?,
                        payment_id: parse_uuid(
                            &row.get::<_, String>(1)?,
                            "payment_allocation",
                            "payment_id",
                        )
                        .map_err(to_rusqlite)?,
                        bill_id: parse_uuid(
                            &row.get::<_, String>(2)?,
                            "payment_allocation",
                            "bill_id",
                        )
                        .map_err(to_rusqlite)?,
                        amount: parse_decimal_strict(
                            &row.get::<_, String>(3)?,
                            "payment_allocation",
                            "amount",
                        )
                        .map_err(to_rusqlite)?,
                        created_at: parse_datetime(
                            &row.get::<_, String>(4)?,
                            "payment_allocation",
                            "created_at",
                        )
                        .map_err(to_rusqlite)?,
                    });
                }
                values
            };

            tx.execute(
                "DELETE FROM ap_payment_allocations WHERE payment_id = ?1",
                params![id.to_string()],
            )?;

            for alloc in allocations {
                Self::recalculate_bill_with_conn(tx, alloc.bill_id).map_err(to_rusqlite)?;

                let bill = tx.query_row(
                    "SELECT * FROM ap_bills WHERE id = ?1",
                    params![alloc.bill_id.to_string()],
                    Self::row_to_bill,
                )?;
                let new_status = if bill.amount_due <= Decimal::ZERO {
                    BillStatus::Paid
                } else if bill.amount_paid > Decimal::ZERO {
                    BillStatus::PartiallyPaid
                } else if bill.status == BillStatus::Overdue {
                    BillStatus::Overdue
                } else {
                    BillStatus::Approved
                };

                tx.execute(
                    "UPDATE ap_bills SET status = ?1 WHERE id = ?2",
                    params![new_status.to_string(), bill.id.to_string()],
                )?;
            }

            Ok(())
        })?;

        self.get_payment(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to void payment".into()))
    }

    fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        let conn = self.conn()?;
        // Status-guarded UPDATE (same pattern as `void_payment`): only a payment
        // still in flight (pending/processed) can clear the bank — a voided,
        // failed, or already-cleared payment cannot.
        let rows = conn
            .execute(
                "UPDATE ap_payments SET status = ?1 WHERE id = ?2 AND status IN ('pending', 'processed')",
                params![PaymentStatusAP::Cleared.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Payment not found or not in a clearable status".into(),
            ));
        }

        self.get_payment(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to clear payment".into()))
    }

    fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>> {
        use crate::sqlite::parse_datetime;
        use crate::sqlite::parse_helpers::parse_decimal;

        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM ap_payment_allocations WHERE payment_id = ?1")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![payment_id.to_string()]).map_err(map_db_error)?;

        let mut allocations = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let id_str: String = row.get("id").map_err(map_db_error)?;
            let payment_id_str: String = row.get("payment_id").map_err(map_db_error)?;
            let bill_id_str: String = row.get("bill_id").map_err(map_db_error)?;
            let amount_str: String = row.get("amount").map_err(map_db_error)?;
            let created_at_str: String = row.get("created_at").map_err(map_db_error)?;

            allocations.push(PaymentAllocation {
                id: parse_uuid(&id_str, "payment_allocation", "id")?,
                payment_id: parse_uuid(&payment_id_str, "payment_allocation", "payment_id")?,
                bill_id: parse_uuid(&bill_id_str, "payment_allocation", "bill_id")?,
                amount: parse_decimal(&amount_str, "payment_allocation", "amount")?,
                created_at: parse_datetime(&created_at_str, "payment_allocation", "created_at")?,
            });
        }
        Ok(allocations)
    }

    fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT p.* FROM ap_payments p
             JOIN ap_payment_allocations a ON p.id = a.payment_id
             WHERE a.bill_id = ?1",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![bill_id.to_string()]).map_err(map_db_error)?;
        let mut payments = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            payments.push(Self::row_to_payment(row).map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM ap_payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        push_payment_filters(&mut sql, &mut params_vec, &filter);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        if input.bill_ids.is_empty() {
            return Err(CommerceError::ValidationError(
                "Payment run requires at least one bill".into(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for bill_id in &input.bill_ids {
            if !seen.insert(*bill_id) {
                return Err(CommerceError::ValidationError("Duplicate bill in payment run".into()));
            }
        }

        let now = Utc::now();
        let id = Uuid::new_v4();
        let run_number = generate_payment_run_number();

        // Immediate (write-locked) transaction so bill validation, the run
        // header, and the run-bill rows commit atomically, and concurrent run
        // creation serializes: a bill cannot land in two active runs. Each bill
        // must exist, be in a payable status with a positive balance, and not
        // already sit in another draft/pending/approved/processing run.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

            let mut total = Decimal::ZERO;
            for bill_id in &input.bill_ids {
                let bill = tx
                    .query_row(
                        "SELECT * FROM ap_bills WHERE id = ?1",
                        params![bill_id.to_string()],
                        Self::row_to_bill,
                    )
                    .optional()?
                    .ok_or_else(|| to_rusqlite(CommerceError::NotFound))?;

                if !matches!(
                    bill.status,
                    BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue
                ) {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Bill is not in a payable status".into(),
                    )));
                }
                if bill.amount_due <= Decimal::ZERO {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Bill has no amount due".into(),
                    )));
                }

                let active_runs: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM ap_payment_run_bills rb
                     JOIN ap_payment_runs r ON r.id = rb.run_id
                     WHERE rb.bill_id = ?1
                       AND r.status IN ('draft', 'pending', 'approved', 'processing')",
                    params![bill_id.to_string()],
                    |row| row.get(0),
                )?;
                if active_runs > 0 {
                    return Err(to_rusqlite(CommerceError::ValidationError(
                        "Bill is already included in an active payment run".into(),
                    )));
                }

                total += bill.amount_due;
            }

            tx.execute(
                "INSERT INTO ap_payment_runs (id, run_number, status, payment_date, payment_method, total_amount, payment_count, notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    id.to_string(),
                    run_number,
                    PaymentRunStatus::Draft.to_string(),
                    ap_date_rfc3339(input.payment_date),
                    input.payment_method.to_string(),
                    total.to_string(),
                    input.bill_ids.len() as i32,
                    &input.notes,
                    &input.created_by,
                    now.to_rfc3339(),
                ],
            )?;

            for bill_id in &input.bill_ids {
                tx.execute(
                    "INSERT INTO ap_payment_run_bills (run_id, bill_id) VALUES (?1, ?2)",
                    params![id.to_string(), bill_id.to_string()],
                )?;
            }

            Ok(())
        })?;

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create payment run".into()))
    }

    fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM ap_payment_runs WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_payment_run(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM ap_payment_runs WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }
        // `payment_date` is stored/bound at midnight UTC (like Postgres's `DATE`), so
        // the RFC3339 comparison is a date comparison — matching Postgres.
        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND payment_date >= ?");
            params_vec.push(Box::new(ap_date_rfc3339(from_date)));
        }
        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND payment_date <= ?");
            params_vec.push(Box::new(ap_date_rfc3339(to_date)));
        }

        sql.push_str(" ORDER BY created_at DESC");

        // SQLite rejects OFFSET without LIMIT, so use `LIMIT -1` (unbounded) when
        // only an offset is set.
        match (filter.limit, filter.offset) {
            (Some(limit), Some(offset)) => sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}")),
            (Some(limit), None) => sql.push_str(&format!(" LIMIT {limit}")),
            (None, Some(offset)) => sql.push_str(&format!(" LIMIT -1 OFFSET {offset}")),
            (None, None) => {}
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut runs = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            runs.push(Self::row_to_payment_run(row).map_err(map_db_error)?);
        }
        Ok(runs)
    }

    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        // Status-guarded UPDATE (same pattern as `clear_payment`): only a
        // draft/pending run can be approved — a cancelled, completed, or
        // in-flight run cannot.
        let rows = conn.execute(
            "UPDATE ap_payment_runs SET status = ?1, approved_by = ?2, approved_at = ?3 WHERE id = ?4 AND status IN ('draft', 'pending')",
            params![PaymentRunStatus::Approved.to_string(), approved_by, now, id.to_string()],
        ).map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Payment run not found or not in an approvable status".into(),
            ));
        }

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve run".into()))
    }

    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        // One immediate (write-locked) transaction for the whole disbursement:
        // the status claim (approved → completed), every payment + allocation
        // row, the bill updates, and the run totals commit atomically. A
        // failure anywhere rolls everything back, leaving the run approved
        // (retry-safe) and no money moved.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite =
                |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));
            let now = Utc::now();

            // Claim: only an approved run may process.
            let rows = tx.execute(
                "UPDATE ap_payment_runs SET status = ?1, processed_at = ?2 WHERE id = ?3 AND status = ?4",
                params![
                    PaymentRunStatus::Completed.to_string(),
                    now.to_rfc3339(),
                    id.to_string(),
                    PaymentRunStatus::Approved.to_string(),
                ],
            )?;
            if rows == 0 {
                return Err(to_rusqlite(CommerceError::Conflict(
                    "Payment run not found or not in a processable status".into(),
                )));
            }

            let (run_method, run_date): (String, String) = tx.query_row(
                "SELECT payment_method, payment_date FROM ap_payment_runs WHERE id = ?1",
                params![id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let bills: Vec<Bill> = {
                let mut stmt = tx.prepare(
                    "SELECT b.* FROM ap_bills b JOIN ap_payment_run_bills rb ON b.id = rb.bill_id WHERE rb.run_id = ?1",
                )?;
                let mut rows = stmt.query(params![id.to_string()])?;
                let mut values = Vec::new();
                while let Some(row) = rows.next()? {
                    values.push(Self::row_to_bill(row)?);
                }
                values
            };

            let mut disbursed_total = Decimal::ZERO;
            let mut disbursed_count: i32 = 0;
            let mut skipped: u32 = 0;

            for bill in bills {
                // Balance/status were re-read under the write lock (the SELECT
                // above runs inside this immediate transaction). A bill paid
                // off or made unpayable (paid/cancelled/disputed) since run
                // creation is skipped, not double-paid — recorded below.
                if bill.amount_due <= Decimal::ZERO
                    || !matches!(
                        bill.status,
                        BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue
                    )
                {
                    skipped += 1;
                    continue;
                }

                let amount = bill.amount_due;
                let payment_id = Uuid::new_v4();
                let payment_number = generate_ap_payment_number();
                tx.execute(
                    "INSERT INTO ap_payments (id, payment_number, supplier_id, payment_date, payment_method, amount, currency, reference_number, bank_account, check_number, memo, status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL, NULL, ?8, ?9, ?9)",
                    params![
                        payment_id.to_string(),
                        payment_number,
                        bill.supplier_id.to_string(),
                        &run_date,
                        &run_method,
                        amount.to_string(),
                        bill.currency,
                        PaymentStatusAP::Pending.to_string(),
                        now.to_rfc3339(),
                    ],
                )?;

                let alloc_id = Uuid::new_v4();
                tx.execute(
                    "INSERT INTO ap_payment_allocations (id, payment_id, bill_id, amount, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        alloc_id.to_string(),
                        payment_id.to_string(),
                        bill.id.to_string(),
                        amount.to_string(),
                        now.to_rfc3339(),
                    ],
                )?;

                Self::recalculate_bill_with_conn(tx, bill.id).map_err(to_rusqlite)?;

                let updated = tx.query_row(
                    "SELECT * FROM ap_bills WHERE id = ?1",
                    params![bill.id.to_string()],
                    Self::row_to_bill,
                )?;
                let new_status = if updated.amount_due <= Decimal::ZERO {
                    BillStatus::Paid
                } else if updated.amount_paid > Decimal::ZERO {
                    BillStatus::PartiallyPaid
                } else {
                    updated.status
                };
                tx.execute(
                    "UPDATE ap_bills SET status = ?1 WHERE id = ?2",
                    params![new_status.to_string(), bill.id.to_string()],
                )?;

                disbursed_total += amount;
                disbursed_count += 1;
            }

            // Reflect what was actually disbursed on the run row, and record
            // any bills skipped between run creation and processing.
            tx.execute(
                "UPDATE ap_payment_runs SET total_amount = ?1, payment_count = ?2 WHERE id = ?3",
                params![disbursed_total.to_string(), disbursed_count, id.to_string()],
            )?;
            if skipped > 0 {
                let note = format!(
                    "{skipped} bill(s) skipped at processing: fully paid or no longer payable"
                );
                tx.execute(
                    "UPDATE ap_payment_runs SET notes = CASE WHEN notes IS NULL OR notes = '' THEN ?1 ELSE notes || '; ' || ?1 END WHERE id = ?2",
                    params![note, id.to_string()],
                )?;
            }

            Ok(())
        })?;

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to process run".into()))
    }

    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        let conn = self.conn()?;

        // Status-guarded UPDATE (same pattern as `clear_payment`): only a run
        // that has not started disbursing (draft/pending/approved) can be
        // cancelled — a processing, completed, or already-cancelled run cannot.
        let rows = conn
            .execute(
                "UPDATE ap_payment_runs SET status = ?1 WHERE id = ?2 AND status IN ('draft', 'pending', 'approved')",
                params![PaymentRunStatus::Cancelled.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(CommerceError::Conflict(
                "Payment run not found or not in a cancellable status".into(),
            ));
        }

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel run".into()))
    }

    fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT b.* FROM ap_bills b JOIN ap_payment_run_bills rb ON b.id = rb.bill_id WHERE rb.run_id = ?1"
        ).map_err(map_db_error)?;

        let mut rows = stmt.query(params![run_id.to_string()]).map_err(map_db_error)?;
        let mut bills = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            bills.push(Self::row_to_bill(row).map_err(map_db_error)?);
        }
        Ok(bills)
    }

    fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        let conn = self.conn()?;
        // Bucket on calendar dates: `due_date` is stored at midnight UTC and
        // Postgres buckets against `CURRENT_DATE`, so truncate `now` (and the
        // cutoffs derived from it) to midnight UTC too — otherwise a bill due
        // today lands in 1-30 instead of current, and every bucket boundary
        // shifts by the time of day.
        let now = Utc::now().date_naive().and_time(NaiveTime::MIN).and_utc();
        let cutoff_30 = now - chrono::Duration::days(30);
        let cutoff_60 = now - chrono::Duration::days(60);
        let cutoff_90 = now - chrono::Duration::days(90);

        let mut current = Decimal::ZERO;
        let mut days_1_30 = Decimal::ZERO;
        let mut days_31_60 = Decimal::ZERO;
        let mut days_61_90 = Decimal::ZERO;
        let mut days_over_90 = Decimal::ZERO;

        let mut stmt = conn.prepare(
            "SELECT due_date, amount_due FROM ap_bills WHERE status NOT IN ('paid', 'cancelled')",
        ).map_err(map_db_error)?;
        let mut rows = stmt.query([]).map_err(map_db_error)?;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let due_date_str: String = row.get(0).map_err(map_db_error)?;
            let due_date = parse_datetime(&due_date_str, "ap_bill", "due_date")?;
            let amount_str: String = row.get(1).map_err(map_db_error)?;
            let amount = parse_decimal_strict(&amount_str, "ap_bills", "amount_due")?;

            if due_date >= now {
                current += amount;
            } else if due_date >= cutoff_30 {
                days_1_30 += amount;
            } else if due_date >= cutoff_60 {
                days_31_60 += amount;
            } else if due_date >= cutoff_90 {
                days_61_90 += amount;
            } else {
                days_over_90 += amount;
            }
        }

        let total = current + days_1_30 + days_31_60 + days_61_90 + days_over_90;

        Ok(ApAgingSummary { current, days_1_30, days_31_60, days_61_90, days_over_90, total })
    }

    fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>> {
        let conn = self.conn()?;
        // Overdue means past the due DATE (Postgres: `due_date < CURRENT_DATE`),
        // so compare against midnight UTC — with the raw `Utc::now()` a bill due
        // today counted as overdue for most of the day.
        let now = Utc::now().date_naive().and_time(NaiveTime::MIN).and_utc();
        let supplier_id_param = supplier_id.to_string();

        let supplier_exists: Option<String> = conn
            .query_row(
                "SELECT id FROM suppliers WHERE id = ?1",
                params![&supplier_id_param],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;

        if supplier_exists.is_none() {
            return Ok(None);
        }

        let mut stmt = conn.prepare(
            "SELECT due_date, amount_due FROM ap_bills WHERE supplier_id = ?1 AND status NOT IN ('paid', 'cancelled')",
        ).map_err(map_db_error)?;
        let mut rows = stmt.query(params![&supplier_id_param]).map_err(map_db_error)?;
        let mut outstanding = Decimal::ZERO;
        let mut overdue = Decimal::ZERO;
        let mut count: i32 = 0;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            count += 1;
            let due_date_str: String = row.get(0).map_err(map_db_error)?;
            let due_date = parse_datetime(&due_date_str, "ap_bill", "due_date")?;
            let amount_str: String = row.get(1).map_err(map_db_error)?;
            let amount = parse_decimal_strict(&amount_str, "ap_bills", "amount_due")?;
            outstanding += amount;
            if due_date < now {
                overdue += amount;
            }
        }

        Ok(Some(SupplierApSummary {
            supplier_id,
            supplier_name: None,
            total_outstanding: outstanding,
            total_overdue: overdue,
            bill_count: count,
        }))
    }

    fn get_total_outstanding(&self) -> Result<Decimal> {
        let conn = self.conn()?;
        let total = sum_decimal_query(
            &conn,
            "SELECT amount_due FROM ap_bills WHERE status NOT IN ('paid', 'cancelled')",
            &[],
            "ap_bills",
            "amount_due",
        )?;

        Ok(total)
    }

    fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        let mut result = BatchResult::new();
        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_bill(input) {
                Ok(bill) => result.record_success(bill),
                Err(e) => result.record_failure(index, None, &e),
            }
        }
        Ok(result)
    }

    fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        let mut bills = Vec::new();
        for id in ids {
            if let Some(bill) = self.get_bill(id)? {
                bills.push(bill);
            }
        }
        Ok(bills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AccountsPayableRepository, BillFilter, BillPaymentFilter, BillStatus, CreateBill,
        CreateBillItem, CreateBillPayment, PaymentAllocationInput, PaymentMethodAP,
    };

    fn fresh_repo() -> SqliteAccountsPayableRepository {
        SqliteDatabase::in_memory().expect("in-memory").accounts_payable()
    }

    fn make_bill(
        repo: &SqliteAccountsPayableRepository,
        supplier: Uuid,
        qty: Decimal,
        price: Decimal,
    ) -> Bill {
        repo.create_bill(CreateBill {
            bill_number: None,
            supplier_id: supplier,
            purchase_order_id: None,
            bill_date: None,
            due_date: Utc::now() + Duration::days(30),
            payment_terms: Some("NET30".into()),
            currency: None,
            reference_number: None,
            memo: Some("test bill".into()),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: Some("5000".into()),
                quantity: qty,
                unit_price: price,
                tax_rate: None,
                po_line_id: None,
            }],
        })
        .expect("create bill")
    }

    #[test]
    fn list_bills_honors_po_amount_and_offset_filters() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let po = Uuid::new_v4();

        make_bill(&repo, supplier, dec!(1), dec!(20));
        let big = make_bill(&repo, supplier, dec!(1), dec!(200));
        let with_po = repo
            .create_bill(CreateBill {
                bill_number: None,
                supplier_id: supplier,
                purchase_order_id: Some(po),
                bill_date: None,
                due_date: Utc::now() + Duration::days(30),
                payment_terms: None,
                currency: None,
                reference_number: None,
                memo: None,
                items: vec![CreateBillItem {
                    description: "X".into(),
                    account_code: None,
                    quantity: dec!(1),
                    unit_price: dec!(50),
                    tax_rate: None,
                    po_line_id: None,
                }],
            })
            .expect("po bill");

        // min_amount: only bills with total_amount >= 100.
        let over = repo
            .list_bills(BillFilter { min_amount: Some(dec!(100)), ..Default::default() })
            .expect("min_amount");
        assert_eq!(over.len(), 1, "min_amount must filter: {over:?}");
        assert_eq!(over[0].id, big.id);

        // max_amount: bills with total_amount <= 100 (excludes the $200 bill).
        let under = repo
            .list_bills(BillFilter { max_amount: Some(dec!(100)), ..Default::default() })
            .expect("max_amount");
        assert!(!under.iter().any(|b| b.id == big.id), "max_amount must exclude the $200 bill");
        assert!(under.iter().any(|b| b.id == with_po.id));

        // purchase_order_id: only the PO-tagged bill.
        let by_po = repo
            .list_bills(BillFilter { purchase_order_id: Some(po), ..Default::default() })
            .expect("po");
        assert_eq!(by_po.len(), 1, "purchase_order_id must filter: {by_po:?}");
        assert_eq!(by_po[0].id, with_po.id);

        // offset skips rows.
        assert_eq!(repo.list_bills(BillFilter::default()).expect("all").len(), 3);
        let offset1 =
            repo.list_bills(BillFilter { offset: Some(1), ..Default::default() }).expect("offset");
        assert_eq!(offset1.len(), 2, "offset must skip rows");
    }

    #[test]
    fn create_bill_starts_in_draft_with_items() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let bill = make_bill(&repo, supplier, dec!(10), dec!(5));
        assert_eq!(bill.supplier_id, supplier);
        assert_eq!(bill.status, BillStatus::Draft);
        assert!(bill.bill_number.starts_with("BILL-"));

        let items = repo.get_bill_items(bill.id).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, dec!(10));
    }

    #[test]
    fn get_bill_and_get_bill_by_number_round_trip() {
        let repo = fresh_repo();
        let bill = make_bill(&repo, Uuid::new_v4(), dec!(1), dec!(1));
        let by_id = repo.get_bill(bill.id).expect("ok").expect("found");
        assert_eq!(by_id.id, bill.id);
        let by_num = repo.get_bill_by_number(&bill.bill_number).expect("ok").expect("found");
        assert_eq!(by_num.id, bill.id);
        assert!(repo.get_bill_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn approve_bill_transitions_status() {
        let repo = fresh_repo();
        let bill = make_bill(&repo, Uuid::new_v4(), dec!(1), dec!(50));
        let approved = repo.approve_bill(bill.id).expect("approve");
        assert_eq!(approved.status, BillStatus::Approved);
    }

    #[test]
    fn list_bills_filters_by_supplier() {
        let repo = fresh_repo();
        let s_a = Uuid::new_v4();
        let s_b = Uuid::new_v4();
        make_bill(&repo, s_a, dec!(1), dec!(1));
        make_bill(&repo, s_a, dec!(2), dec!(2));
        make_bill(&repo, s_b, dec!(3), dec!(3));

        let for_a = repo
            .list_bills(BillFilter { supplier_id: Some(s_a), ..Default::default() })
            .expect("list");
        assert_eq!(for_a.len(), 2);
        assert!(for_a.iter().all(|b| b.supplier_id == s_a));
    }

    #[test]
    fn list_bills_filters_by_status() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let draft = make_bill(&repo, supplier, dec!(1), dec!(1));
        let to_approve = make_bill(&repo, supplier, dec!(1), dec!(1));
        repo.approve_bill(to_approve.id).expect("approve");

        let drafts = repo
            .list_bills(BillFilter { status: Some(BillStatus::Draft), ..Default::default() })
            .expect("drafts");
        let approved = repo
            .list_bills(BillFilter { status: Some(BillStatus::Approved), ..Default::default() })
            .expect("approved");
        assert!(drafts.iter().any(|b| b.id == draft.id));
        assert!(approved.iter().any(|b| b.id == to_approve.id));
    }

    #[test]
    fn empty_db_returns_zero_aging_and_outstanding() {
        let repo = fresh_repo();
        let aging = repo.get_aging_summary().expect("aging");
        assert_eq!(aging.total, dec!(0));
        let outstanding = repo.get_total_outstanding().expect("outstanding");
        assert_eq!(outstanding, dec!(0));
    }

    #[test]
    fn empty_db_returns_no_overdue_or_due_soon() {
        let repo = fresh_repo();
        assert!(repo.get_overdue_bills().expect("overdue").is_empty());
        assert!(repo.get_bills_due_soon(7).expect("due soon").is_empty());
    }

    #[test]
    fn get_supplier_summary_for_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_supplier_summary(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn create_payment_with_allocation_links_to_bill() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let bill = make_bill(&repo, supplier, dec!(2), dec!(50));
        repo.approve_bill(bill.id).expect("approve");

        let payment = repo
            .create_payment(CreateBillPayment {
                supplier_id: supplier,
                payment_date: None,
                payment_method: PaymentMethodAP::Ach,
                amount: dec!(100),
                currency: None,
                reference_number: Some("ACH-1".into()),
                bank_account: None,
                check_number: None,
                memo: None,
                allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(100) }],
            })
            .expect("create payment");
        assert_eq!(payment.supplier_id, supplier);
        assert_eq!(payment.amount, dec!(100));

        let allocations = repo.get_payment_allocations(payment.id).expect("allocations");
        assert_eq!(allocations.len(), 1);
        assert_eq!(allocations[0].bill_id, bill.id);

        let payments_for_bill = repo.get_payments_for_bill(bill.id).expect("payments");
        assert!(payments_for_bill.iter().any(|p| p.id == payment.id));
    }

    #[test]
    fn void_payment_is_guarded_against_double_void() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let bill = make_bill(&repo, supplier, dec!(10), dec!(10)); // amount_due = 100
        repo.approve_bill(bill.id).expect("approve");

        let payment = repo
            .create_payment(CreateBillPayment {
                supplier_id: supplier,
                payment_date: None,
                payment_method: PaymentMethodAP::Ach,
                amount: dec!(100),
                currency: None,
                reference_number: None,
                bank_account: None,
                check_number: None,
                memo: None,
                allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(100) }],
            })
            .expect("pay");

        repo.void_payment(payment.id).expect("first void succeeds");
        // Bill balance is restored by the void.
        let after_void = repo.get_bill(bill.id).expect("get").expect("bill");
        assert_eq!(after_void.amount_paid, dec!(0), "void reverses the payment");
        assert_eq!(after_void.amount_due, dec!(100));

        // A second void must be rejected, not silently reverse again.
        let second = repo.void_payment(payment.id);
        assert!(second.is_err(), "double-void must be rejected");
        let after_second = repo.get_bill(bill.id).expect("get").expect("bill");
        assert_eq!(after_second.amount_due, dec!(100), "balance unchanged by the rejected void");
    }

    #[test]
    fn create_payment_is_atomic_under_concurrency() {
        use std::sync::{Arc, Barrier};

        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        // Bill with amount_due = 10 * 10 = 100.
        let bill = make_bill(&repo, supplier, dec!(10), dec!(10));
        repo.approve_bill(bill.id).expect("approve");

        const THREADS: usize = 10;
        let barrier = Arc::new(Barrier::new(THREADS));

        // Each thread pays the full 100 against the same bill; only one may win.
        let successes = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let repo = &repo;
                    let bill_id = bill.id;
                    s.spawn(move || {
                        barrier.wait();
                        repo.create_payment(CreateBillPayment {
                            supplier_id: supplier,
                            payment_date: None,
                            payment_method: PaymentMethodAP::Ach,
                            amount: dec!(100),
                            currency: None,
                            reference_number: None,
                            bank_account: None,
                            check_number: None,
                            memo: None,
                            allocations: vec![PaymentAllocationInput {
                                bill_id,
                                amount: dec!(100),
                            }],
                        })
                        .is_ok()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).filter(|&ok| ok).count()
        });

        // Safety invariant: the 100 bill can be paid AT MOST once (never
        // double-paid). Under extreme lock contention the sole winner can fail
        // transiently (retryable), so zero successes is acceptable; two or more
        // is the double-pay bug this guards against.
        assert!(successes <= 1, "the bill was paid more than once across {THREADS} threads");
        let after = repo.get_bill(bill.id).expect("get").expect("bill");
        assert_eq!(
            after.amount_paid,
            dec!(100) * Decimal::from(successes as u64),
            "amount_paid must reflect exactly the successful payment"
        );
        assert_eq!(after.amount_due, dec!(100) - after.amount_paid);
    }

    #[test]
    fn list_payments_filters_by_supplier() {
        let repo = fresh_repo();
        let supplier_a = Uuid::new_v4();
        let supplier_b = Uuid::new_v4();
        let bill_a = make_bill(&repo, supplier_a, dec!(1), dec!(10));
        let bill_b = make_bill(&repo, supplier_b, dec!(1), dec!(10));
        repo.approve_bill(bill_a.id).expect("approve a");
        repo.approve_bill(bill_b.id).expect("approve b");

        repo.create_payment(CreateBillPayment {
            supplier_id: supplier_a,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(10),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill_a.id, amount: dec!(10) }],
        })
        .expect("pay a");
        repo.create_payment(CreateBillPayment {
            supplier_id: supplier_b,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(10),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill_b.id, amount: dec!(10) }],
        })
        .expect("pay b");

        let payments_a = repo
            .list_payments(BillPaymentFilter {
                supplier_id: Some(supplier_a),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(payments_a.len(), 1);
        assert_eq!(payments_a[0].supplier_id, supplier_a);
    }

    #[test]
    fn create_bills_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let result = repo
            .create_bills_batch(vec![
                CreateBill {
                    supplier_id: supplier,
                    due_date: Utc::now() + Duration::days(30),
                    items: vec![CreateBillItem {
                        description: "A".into(),
                        account_code: None,
                        quantity: dec!(1),
                        unit_price: dec!(1),
                        tax_rate: None,
                        po_line_id: None,
                    }],
                    ..Default::default()
                },
                CreateBill {
                    supplier_id: supplier,
                    due_date: Utc::now() + Duration::days(30),
                    items: vec![CreateBillItem {
                        description: "B".into(),
                        account_code: None,
                        quantity: dec!(2),
                        unit_price: dec!(2),
                        tax_rate: None,
                        po_line_id: None,
                    }],
                    ..Default::default()
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_bill_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_bill(Uuid::new_v4()).expect("ok").is_none());
    }
}
