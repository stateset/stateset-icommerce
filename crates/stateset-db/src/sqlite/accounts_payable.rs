//! SQLite implementation for Accounts Payable

use crate::sqlite::{
    map_db_error, parse_datetime, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid,
    parse_uuid_opt_row, parse_uuid_row, sum_decimal_query,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
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
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
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
            &conn,
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
            params![subtotal_dec.to_string(), tax_dec.to_string(), total.to_string(), paid.to_string(), due.to_string(), bill_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn recalculate_bill(&self, bill_id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        Self::recalculate_bill_with_conn(&conn, bill_id)
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

        {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO ap_bills (id, bill_number, supplier_id, purchase_order_id, status, bill_date, due_date,
                 payment_terms, currency, reference_number, memo, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![
                    id.to_string(),
                    bill_number,
                    supplier_id.to_string(),
                    purchase_order_id.map(|id| id.to_string()),
                    BillStatus::Draft.to_string(),
                    bill_date.unwrap_or(now).to_rfc3339(),
                    due_date.to_rfc3339(),
                    payment_terms,
                    currency.unwrap_or_else(|| "USD".to_string()),
                    reference_number,
                    memo,
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        for item in &items {
            self.add_bill_item(
                id,
                CreateBillItem {
                    description: item.description.clone(),
                    account_code: item.account_code.clone(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    tax_rate: item.tax_rate,
                    po_line_id: item.po_line_id,
                },
            )?;
        }

        self.recalculate_bill(id)?;
        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create bill".into()))
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
                input.due_date.unwrap_or(existing.due_date).to_rfc3339(),
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
        if filter.overdue_only == Some(true) {
            sql.push_str(" AND due_date < datetime('now') AND status NOT IN ('paid', 'cancelled')");
        }

        sql.push_str(" ORDER BY due_date");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut bills = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            bills.push(Self::row_to_bill(row).map_err(map_db_error)?);
        }
        Ok(bills)
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
        conn.execute(
            "UPDATE ap_bills SET status = ?1 WHERE id = ?2 AND status IN ('draft', 'pending')",
            params![BillStatus::Approved.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve bill".into()))
    }

    fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE ap_bills SET status = ?1 WHERE id = ?2",
            params![BillStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_bill(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel bill".into()))
    }

    fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE ap_bills SET status = ?1 WHERE id = ?2",
            params![BillStatus::Disputed.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

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
        let created = {
            let conn = self.conn()?;
            let line_number: i32 = conn.query_row(
                "SELECT COALESCE(MAX(line_number), 0) + 1 FROM ap_bill_items WHERE bill_id = ?1",
                params![bill_id.to_string()],
                |row| row.get(0),
            ).map_err(map_db_error)?;

            let amount = item.quantity * item.unit_price;
            let tax_amount =
                item.tax_rate.map(|r| amount * r / Decimal::from(100)).unwrap_or(Decimal::ZERO);

            conn.execute(
                "INSERT INTO ap_bill_items (id, bill_id, line_number, description, account_code, quantity, unit_price, amount, tax_rate, tax_amount, po_line_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    id.to_string(),
                    bill_id.to_string(),
                    line_number,
                    item.description,
                    item.account_code,
                    item.quantity.to_string(),
                    item.unit_price.to_string(),
                    amount.to_string(),
                    item.tax_rate.map(|r| r.to_string()),
                    tax_amount.to_string(),
                    item.po_line_id.map(|id| id.to_string()),
                    now,
                ],
            ).map_err(map_db_error)?;

            let mut stmt =
                conn.prepare("SELECT * FROM ap_bill_items WHERE id = ?1").map_err(map_db_error)?;
            let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

            if let Some(row) = rows.next().map_err(map_db_error)? {
                Self::row_to_bill_item(row).map_err(map_db_error)?
            } else {
                return Err(CommerceError::DatabaseError("Failed to create bill item".into()));
            }
        };

        self.recalculate_bill(bill_id)?;
        Ok(created)
    }

    fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        let bill_id: String = {
            let conn = self.conn()?;
            let bill_id: String = conn
                .query_row(
                    "SELECT bill_id FROM ap_bill_items WHERE id = ?1",
                    params![item_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            conn.execute("DELETE FROM ap_bill_items WHERE id = ?1", params![item_id.to_string()])
                .map_err(map_db_error)?;
            bill_id
        };

        self.recalculate_bill(parse_uuid(&bill_id, "bill_item", "bill_id")?)?;
        Ok(())
    }

    fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM ap_bills WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.list_bills(BillFilter { overdue_only: Some(true), ..Default::default() })
    }

    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM ap_bills WHERE due_date <= datetime('now', '+' || ?1 || ' days') AND status NOT IN ('paid', 'cancelled') ORDER BY due_date"
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

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
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

        for (bill_id, allocated_amount) in &allocation_by_bill {
            let (supplier_id, status, amount_due): (String, String, String) = tx
                .query_row(
                    "SELECT supplier_id, status, amount_due FROM ap_bills WHERE id = ?1",
                    params![bill_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(map_db_error)?;

            let parsed_supplier_id = parse_uuid(&supplier_id, "bill", "supplier_id")?;
            if parsed_supplier_id != input.supplier_id {
                return Err(CommerceError::ValidationError(
                    "Allocation bill supplier does not match payment supplier".into(),
                ));
            }

            let bill_status: BillStatus = status.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid bill status '{}' while creating payment: {}",
                    status, e
                ))
            })?;
            if !matches!(
                bill_status,
                BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue
            ) {
                return Err(CommerceError::ValidationError(
                    "Bill is not in a payable status".into(),
                ));
            }

            let amount_due = parse_decimal_strict(&amount_due, "bill", "amount_due")?;
            if *allocated_amount > amount_due {
                return Err(CommerceError::ValidationError(
                    "Allocation amount exceeds bill amount due".into(),
                ));
            }
        }

        tx.execute(
            "INSERT INTO ap_payments (id, payment_number, supplier_id, payment_date, payment_method, amount, currency, reference_number, bank_account, check_number, memo, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
            params![
                id.to_string(),
                payment_number,
                input.supplier_id.to_string(),
                input.payment_date.unwrap_or(now).to_rfc3339(),
                input.payment_method.to_string(),
                input.amount.to_string(),
                input.currency.unwrap_or_else(|| "USD".to_string()),
                input.reference_number,
                input.bank_account,
                input.check_number,
                input.memo,
                PaymentStatusAP::Pending.to_string(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        // Create allocations
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
            )
            .map_err(map_db_error)?;

            // Update bill amount_paid and status
            Self::recalculate_bill_with_conn(&tx, alloc.bill_id)?;

            let bill = tx
                .query_row(
                    "SELECT * FROM ap_bills WHERE id = ?1",
                    params![alloc.bill_id.to_string()],
                    Self::row_to_bill,
                )
                .map_err(map_db_error)?;
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
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

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

        if let Some(supplier_id) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY payment_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut payments = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            payments.push(Self::row_to_payment(row).map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let allocations: Vec<PaymentAllocation> = {
            let mut stmt = tx
                .prepare("SELECT id, payment_id, bill_id, amount, created_at FROM ap_payment_allocations WHERE payment_id = ?1")
                .map_err(map_db_error)?;
            let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;
            let mut values = Vec::new();
            while let Some(row) = rows.next().map_err(map_db_error)? {
                values.push(PaymentAllocation {
                    id: parse_uuid(
                        &row.get::<_, String>(0).map_err(map_db_error)?,
                        "payment_allocation",
                        "id",
                    )?,
                    payment_id: parse_uuid(
                        &row.get::<_, String>(1).map_err(map_db_error)?,
                        "payment_allocation",
                        "payment_id",
                    )?,
                    bill_id: parse_uuid(
                        &row.get::<_, String>(2).map_err(map_db_error)?,
                        "payment_allocation",
                        "bill_id",
                    )?,
                    amount: parse_decimal_strict(
                        &row.get::<_, String>(3).map_err(map_db_error)?,
                        "payment_allocation",
                        "amount",
                    )?,
                    created_at: parse_datetime(
                        &row.get::<_, String>(4).map_err(map_db_error)?,
                        "payment_allocation",
                        "created_at",
                    )?,
                });
            }
            values
        };

        tx.execute(
            "UPDATE ap_payments SET status = ?1 WHERE id = ?2",
            params![PaymentStatusAP::Voided.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        tx.execute(
            "DELETE FROM ap_payment_allocations WHERE payment_id = ?1",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        for alloc in allocations {
            Self::recalculate_bill_with_conn(&tx, alloc.bill_id)?;

            let bill = tx
                .query_row(
                    "SELECT * FROM ap_bills WHERE id = ?1",
                    params![alloc.bill_id.to_string()],
                    Self::row_to_bill,
                )
                .map_err(map_db_error)?;
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
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        self.get_payment(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to void payment".into()))
    }

    fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE ap_payments SET status = ?1 WHERE id = ?2",
            params![PaymentStatusAP::Cleared.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

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

    fn count_payments(&self, _filter: BillPaymentFilter) -> Result<u64> {
        let conn = self.conn()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM ap_payments", [], |row| row.get(0))
            .map_err(map_db_error)?;
        Ok(count as u64)
    }

    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let run_number = generate_payment_run_number();

        // Calculate total
        let mut total = Decimal::ZERO;
        for bill_id in &input.bill_ids {
            if let Some(bill) = self.get_bill(*bill_id)? {
                total += bill.amount_due;
            }
        }

        conn.execute(
            "INSERT INTO ap_payment_runs (id, run_number, status, payment_date, payment_method, total_amount, payment_count, notes, created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                id.to_string(),
                run_number,
                PaymentRunStatus::Draft.to_string(),
                input.payment_date.to_rfc3339(),
                input.payment_method.to_string(),
                total.to_string(),
                input.bill_ids.len() as i32,
                input.notes,
                input.created_by,
                now,
            ],
        ).map_err(map_db_error)?;

        // Add bills to run
        for bill_id in input.bill_ids {
            conn.execute(
                "INSERT INTO ap_payment_run_bills (run_id, bill_id) VALUES (?1, ?2)",
                params![id.to_string(), bill_id.to_string()],
            )
            .map_err(map_db_error)?;
        }

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

    fn list_payment_runs(&self, _filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM ap_payment_runs ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let mut rows = stmt.query([]).map_err(map_db_error)?;

        let mut runs = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            runs.push(Self::row_to_payment_run(row).map_err(map_db_error)?);
        }
        Ok(runs)
    }

    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE ap_payment_runs SET status = ?1, approved_by = ?2, approved_at = ?3 WHERE id = ?4",
            params![PaymentRunStatus::Approved.to_string(), approved_by, now, id.to_string()],
        ).map_err(map_db_error)?;

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve run".into()))
    }

    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE ap_payment_runs SET status = ?1, processed_at = ?2 WHERE id = ?3",
            params![PaymentRunStatus::Completed.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_payment_run(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to process run".into()))
    }

    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE ap_payment_runs SET status = ?1 WHERE id = ?2",
            params![PaymentRunStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

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
        let now = Utc::now();
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
        let now = Utc::now();
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
