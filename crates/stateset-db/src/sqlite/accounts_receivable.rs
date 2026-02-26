//! SQLite implementation of Accounts Receivable repository

use crate::sqlite::parse_helpers::{
    parse_datetime as parse_datetime_safe, parse_decimal as parse_decimal_safe,
};
use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_uuid, parse_uuid_opt_row, parse_uuid_row,
    sum_decimal_query,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params, params_from_iter, types::Value};
use rust_decimal::Decimal;
use stateset_core::{
    AccountsReceivableRepository, ApplyCreditMemo, ApplyPaymentToInvoices, ArAgingFilter,
    ArAgingSummary, ArPaymentApplication, CollectionActivity, CollectionActivityFilter,
    CollectionActivityType, CollectionStatus, CreateCollectionActivity, CreateCreditMemo,
    CreateWriteOff, CreditMemo, CreditMemoFilter, CreditMemoStatus, CustomerArAging,
    CustomerArSummary, CustomerId, CustomerStatement, DunningLetterType, GenerateStatementRequest,
    Invoice, InvoiceId, OrderId, Result, StatementLineItem, StatementTransactionType, WriteOff,
    WriteOffFilter, generate_credit_memo_number, generate_write_off_number,
};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteAccountsReceivableRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAccountsReceivableRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn map_collection_activity_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CollectionActivity> {
        let dunning_letter_type = match row.get::<_, Option<String>>(5)? {
            Some(value) => {
                Some(parse_enum_row(&value, "collection_activity", "dunning_letter_type")?)
            }
            None => None,
        };

        Ok(CollectionActivity {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "collection_activity", "id")?,
            invoice_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "collection_activity",
                "invoice_id",
            )?,
            customer_id: parse_uuid_row(
                &row.get::<_, String>(2)?,
                "collection_activity",
                "customer_id",
            )?,
            activity_type: parse_enum_row(
                &row.get::<_, String>(3)?,
                "collection_activity",
                "activity_type",
            )?,
            activity_date: parse_datetime_row(
                &row.get::<_, String>(4)?,
                "collection_activity",
                "activity_date",
            )?,
            dunning_letter_type,
            notes: row.get(6)?,
            contact_method: row.get(7)?,
            contact_result: row.get(8)?,
            promise_to_pay_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>(9)?,
                "collection_activity",
                "promise_to_pay_date",
            )?,
            promise_to_pay_amount: parse_decimal_opt_row(
                row.get::<_, Option<String>>(10)?,
                "collection_activity",
                "promise_to_pay_amount",
            )?,
            performed_by: row.get(11)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "collection_activity",
                "created_at",
            )?,
        })
    }

    fn map_write_off_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WriteOff> {
        Ok(WriteOff {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "write_off", "id")?,
            write_off_number: row.get(1)?,
            invoice_id: parse_uuid_row(&row.get::<_, String>(2)?, "write_off", "invoice_id")?,
            customer_id: parse_uuid_row(&row.get::<_, String>(3)?, "write_off", "customer_id")?,
            amount: parse_decimal_row(&row.get::<_, String>(4)?, "write_off", "amount")?,
            reason: parse_enum_row(&row.get::<_, String>(5)?, "write_off", "reason")?,
            notes: row.get(6)?,
            write_off_date: parse_datetime_row(
                &row.get::<_, String>(7)?,
                "write_off",
                "write_off_date",
            )?,
            approved_by: row.get(8)?,
            approved_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(9)?,
                "write_off",
                "approved_at",
            )?,
            reversed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(10)?,
                "write_off",
                "reversed_at",
            )?,
            gl_journal_entry_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(11)?,
                "write_off",
                "gl_journal_entry_id",
            )?,
            created_at: parse_datetime_row(&row.get::<_, String>(12)?, "write_off", "created_at")?,
        })
    }

    fn map_credit_memo_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CreditMemo> {
        Ok(CreditMemo {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "credit_memo", "id")?,
            credit_memo_number: row.get(1)?,
            customer_id: parse_uuid_row(&row.get::<_, String>(2)?, "credit_memo", "customer_id")?,
            original_invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(3)?,
                "credit_memo",
                "original_invoice_id",
            )?,
            reason: parse_enum_row(&row.get::<_, String>(4)?, "credit_memo", "reason")?,
            amount: parse_decimal_row(&row.get::<_, String>(5)?, "credit_memo", "amount")?,
            applied_amount: parse_decimal_row(
                &row.get::<_, String>(6)?,
                "credit_memo",
                "applied_amount",
            )?,
            unapplied_amount: parse_decimal_row(
                &row.get::<_, String>(7)?,
                "credit_memo",
                "unapplied_amount",
            )?,
            status: parse_enum_row(&row.get::<_, String>(8)?, "credit_memo", "status")?,
            notes: row.get(9)?,
            issue_date: parse_datetime_row(
                &row.get::<_, String>(10)?,
                "credit_memo",
                "issue_date",
            )?,
            gl_journal_entry_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(11)?,
                "credit_memo",
                "gl_journal_entry_id",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "credit_memo",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(13)?,
                "credit_memo",
                "updated_at",
            )?,
        })
    }

    fn map_payment_application_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<ArPaymentApplication> {
        Ok(ArPaymentApplication {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "payment_application", "id")?,
            payment_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "payment_application",
                "payment_id",
            )?,
            invoice_id: parse_uuid_row(
                &row.get::<_, String>(2)?,
                "payment_application",
                "invoice_id",
            )?,
            applied_amount: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "payment_application",
                "applied_amount",
            )?,
            applied_date: parse_datetime_row(
                &row.get::<_, String>(4)?,
                "payment_application",
                "applied_date",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(5)?,
                "payment_application",
                "created_at",
            )?,
        })
    }

    fn get_invoice_customer_id(&self, invoice_id: InvoiceId) -> Result<Uuid> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let customer_id: String = conn
            .query_row(
                "SELECT customer_id FROM invoices WHERE id = ?1",
                params![invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        parse_uuid_row(&customer_id, "invoice", "customer_id").map_err(map_db_error)
    }

    fn recalculate_invoice_with_conn(
        conn: &rusqlite::Connection,
        invoice_id: InvoiceId,
    ) -> Result<()> {
        // Sum all payment applications
        let paid: String = conn.query_row(
            "SELECT COALESCE(SUM(applied_amount), '0') FROM ar_payment_applications WHERE invoice_id = ?1",
            params![invoice_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        // Sum all credit memo applications
        let credits: String = conn.query_row(
            "SELECT COALESCE(SUM(applied_amount), '0') FROM ar_credit_memo_applications WHERE invoice_id = ?1",
            params![invoice_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        let paid_dec = parse_decimal_safe(&paid, "invoice", "paid_amount")?;
        let credits_dec = parse_decimal_safe(&credits, "invoice", "credits_amount")?;
        let total_applied = paid_dec + credits_dec;

        // Get invoice total
        let total: String = conn
            .query_row(
                "SELECT total FROM invoices WHERE id = ?1",
                params![invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let total_dec = parse_decimal_safe(&total, "invoice", "total")?;
        let balance_due = total_dec - total_applied;

        // Determine status
        let status = if balance_due <= Decimal::ZERO {
            "paid"
        } else if total_applied > Decimal::ZERO {
            "partially_paid"
        } else {
            "sent" // Keep original status if nothing applied
        };

        conn.execute(
            "UPDATE invoices SET amount_paid = ?1, balance_due = ?2, status = ?3 WHERE id = ?4",
            params![
                total_applied.to_string(),
                balance_due.to_string(),
                status,
                invoice_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn recalculate_invoice(&self, invoice_id: InvoiceId) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        Self::recalculate_invoice_with_conn(&conn, invoice_id)
    }
}

impl AccountsReceivableRepository for SqliteAccountsReceivableRepository {
    fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
            "SELECT due_date, balance_due FROM invoices WHERE status NOT IN ('paid', 'voided', 'written_off')",
        ).map_err(map_db_error)?;
        let mut rows = stmt.query([]).map_err(map_db_error)?;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let due_date_str: String = row.get(0).map_err(map_db_error)?;
            let due_date = parse_datetime_safe(&due_date_str, "invoice", "due_date")?;
            let balance_str: String = row.get(1).map_err(map_db_error)?;
            let balance = parse_decimal_safe(&balance_str, "invoice", "balance_due")?;
            if balance <= Decimal::ZERO {
                continue;
            }

            if due_date >= now {
                current += balance;
            } else if due_date >= cutoff_30 {
                days_1_30 += balance;
            } else if due_date >= cutoff_60 {
                days_31_60 += balance;
            } else if due_date >= cutoff_90 {
                days_61_90 += balance;
            } else {
                days_over_90 += balance;
            }
        }

        Ok(ArAgingSummary {
            current,
            days_1_30,
            days_31_60,
            days_61_90,
            days_over_90,
            total: current + days_1_30 + days_31_60 + days_61_90 + days_over_90,
            as_of_date: now,
        })
    }

    fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let customer_row: Option<(String, String, String)> = conn
            .query_row(
                "SELECT first_name, last_name, email FROM customers WHERE id = ?1",
                params![customer_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        let (first_name, last_name, email) = match customer_row {
            Some(row) => row,
            None => return Ok(None),
        };

        let now = Utc::now();
        let cutoff_30 = now - chrono::Duration::days(30);
        let cutoff_60 = now - chrono::Duration::days(60);
        let cutoff_90 = now - chrono::Duration::days(90);

        let mut current = Decimal::ZERO;
        let mut days_1_30 = Decimal::ZERO;
        let mut days_31_60 = Decimal::ZERO;
        let mut days_61_90 = Decimal::ZERO;
        let mut days_over_90 = Decimal::ZERO;
        let mut invoice_count: i32 = 0;
        let mut oldest_invoice_date: Option<chrono::DateTime<Utc>> = None;

        let mut stmt = conn
            .prepare(
                "SELECT due_date, balance_due, created_at
             FROM invoices
             WHERE customer_id = ?1
               AND status NOT IN ('paid', 'voided', 'written_off')",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![customer_id.to_string()]).map_err(map_db_error)?;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let balance_str: String = row.get(1).map_err(map_db_error)?;
            let balance = parse_decimal_safe(&balance_str, "invoice", "balance_due")?;
            if balance <= Decimal::ZERO {
                continue;
            }

            invoice_count += 1;
            let due_date_str: String = row.get(0).map_err(map_db_error)?;
            let due_date = parse_datetime_safe(&due_date_str, "invoice", "due_date")?;
            let created_at_str: String = row.get(2).map_err(map_db_error)?;
            let created_at = parse_datetime_safe(&created_at_str, "invoice", "created_at")?;

            oldest_invoice_date = match oldest_invoice_date {
                Some(existing) if existing <= created_at => Some(existing),
                _ => Some(created_at),
            };

            if due_date >= now {
                current += balance;
            } else if due_date >= cutoff_30 {
                days_1_30 += balance;
            } else if due_date >= cutoff_60 {
                days_31_60 += balance;
            } else if due_date >= cutoff_90 {
                days_61_90 += balance;
            } else {
                days_over_90 += balance;
            }
        }

        if invoice_count == 0 {
            return Err(stateset_core::CommerceError::NotFound);
        }

        Ok(Some(CustomerArAging {
            customer_id,
            customer_name: Some(format!("{} {}", first_name, last_name)),
            customer_email: Some(email),
            current,
            days_1_30,
            days_31_60,
            days_61_90,
            days_over_90,
            total_outstanding: current + days_1_30 + days_31_60 + days_61_90 + days_over_90,
            invoice_count,
            oldest_invoice_date,
            last_payment_date: None,
        }))
    }

    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT
                i.customer_id,
                c.first_name,
                c.last_name,
                c.email,
                i.due_date,
                i.balance_due,
                i.created_at
             FROM invoices i
             LEFT JOIN customers c ON i.customer_id = c.id
             WHERE i.status NOT IN ('paid', 'voided', 'written_off')",
        );

        if filter.customer_id.is_some() {
            sql.push_str(" AND i.customer_id = ?1");
        }

        let now = Utc::now();
        let cutoff_30 = now - chrono::Duration::days(30);
        let cutoff_60 = now - chrono::Duration::days(60);
        let cutoff_90 = now - chrono::Duration::days(90);

        #[derive(Default)]
        struct AgingAccum {
            customer_id: Uuid,
            customer_name: String,
            customer_email: String,
            current: Decimal,
            days_1_30: Decimal,
            days_31_60: Decimal,
            days_61_90: Decimal,
            days_over_90: Decimal,
            invoice_count: i32,
            oldest_invoice_date: Option<chrono::DateTime<Utc>>,
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let mut rows = match filter.customer_id {
            Some(cid) => stmt.query(params![cid.to_string()]).map_err(map_db_error)?,
            None => stmt.query([]).map_err(map_db_error)?,
        };

        let mut by_customer: HashMap<Uuid, AgingAccum> = HashMap::new();

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let id_str: String = row.get(0).map_err(map_db_error)?;
            let customer_id = parse_uuid(&id_str, "invoice", "customer_id")?;
            let first_name: String = row.get(1).map_err(map_db_error)?;
            let last_name: String = row.get(2).map_err(map_db_error)?;
            let email: String = row.get(3).map_err(map_db_error)?;
            let due_date_str: String = row.get(4).map_err(map_db_error)?;
            let balance_str: String = row.get(5).map_err(map_db_error)?;
            let created_at_str: String = row.get(6).map_err(map_db_error)?;

            let balance = parse_decimal_safe(&balance_str, "invoice", "balance_due")?;
            if balance <= Decimal::ZERO {
                continue;
            }

            let due_date = parse_datetime_safe(&due_date_str, "invoice", "due_date")?;
            let created_at = parse_datetime_safe(&created_at_str, "invoice", "created_at")?;

            let entry = by_customer.entry(customer_id).or_insert_with(|| AgingAccum {
                customer_id,
                customer_name: format!("{} {}", first_name, last_name),
                customer_email: email,
                ..Default::default()
            });

            entry.invoice_count += 1;
            entry.oldest_invoice_date = match entry.oldest_invoice_date {
                Some(existing) if existing <= created_at => Some(existing),
                _ => Some(created_at),
            };

            if due_date >= now {
                entry.current += balance;
            } else if due_date >= cutoff_30 {
                entry.days_1_30 += balance;
            } else if due_date >= cutoff_60 {
                entry.days_31_60 += balance;
            } else if due_date >= cutoff_90 {
                entry.days_61_90 += balance;
            } else {
                entry.days_over_90 += balance;
            }
        }

        let mut results: Vec<CustomerArAging> = by_customer
            .into_values()
            .filter(|entry| {
                if filter.overdue_only.unwrap_or(false) {
                    entry.days_1_30 + entry.days_31_60 + entry.days_61_90 + entry.days_over_90
                        > Decimal::ZERO
                } else {
                    true
                }
            })
            .map(|entry| {
                let total = entry.current
                    + entry.days_1_30
                    + entry.days_31_60
                    + entry.days_61_90
                    + entry.days_over_90;
                CustomerArAging {
                    customer_id: entry.customer_id,
                    customer_name: Some(entry.customer_name),
                    customer_email: Some(entry.customer_email),
                    current: entry.current,
                    days_1_30: entry.days_1_30,
                    days_31_60: entry.days_31_60,
                    days_61_90: entry.days_61_90,
                    days_over_90: entry.days_over_90,
                    total_outstanding: total,
                    invoice_count: entry.invoice_count,
                    oldest_invoice_date: entry.oldest_invoice_date,
                    last_payment_date: None,
                }
            })
            .collect();

        results.sort_by(|a, b| b.total_outstanding.cmp(&a.total_outstanding));

        let offset = filter.offset.unwrap_or(0) as usize;
        let limit = filter.limit.map(|l| l as usize);
        let results = if offset >= results.len() {
            Vec::new()
        } else {
            let mut sliced = results.split_off(offset);
            if let Some(limit) = limit {
                if sliced.len() > limit {
                    sliced.truncate(limit);
                }
            }
            sliced
        };

        Ok(results)
    }

    fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let customer_id = self.get_invoice_customer_id(input.invoice_id.into())?;

        conn.execute(
            "INSERT INTO ar_collection_activities (id, invoice_id, customer_id, activity_type, activity_date, dunning_letter_type, notes, contact_method, contact_result, promise_to_pay_date, promise_to_pay_amount, performed_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id.to_string(),
                input.invoice_id.to_string(),
                customer_id.to_string(),
                input.activity_type.to_string(),
                now.to_rfc3339(),
                input.dunning_letter_type.map(|d| d.to_string()),
                input.notes,
                input.contact_method,
                input.contact_result,
                input.promise_to_pay_date.map(|d| d.to_rfc3339()),
                input.promise_to_pay_amount.map(|a| a.to_string()),
                input.performed_by,
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        Ok(CollectionActivity {
            id,
            invoice_id: input.invoice_id,
            customer_id,
            activity_type: input.activity_type,
            activity_date: now,
            dunning_letter_type: input.dunning_letter_type,
            notes: input.notes,
            contact_method: input.contact_method,
            contact_result: input.contact_result,
            promise_to_pay_date: input.promise_to_pay_date,
            promise_to_pay_amount: input.promise_to_pay_amount,
            performed_by: input.performed_by,
            created_at: now,
        })
    }

    fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, invoice_id, customer_id, activity_type, activity_date, dunning_letter_type, notes, contact_method, contact_result, promise_to_pay_date, promise_to_pay_amount, performed_by, created_at
             FROM ar_collection_activities WHERE 1=1"
        );
        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(inv_id) = &filter.invoice_id {
            sql.push_str(" AND invoice_id = ?");
            params_vec.push(Value::Text(inv_id.to_string()));
        }
        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Value::Text(cust_id.to_string()));
        }
        if let Some(atype) = &filter.activity_type {
            sql.push_str(" AND activity_type = ?");
            params_vec.push(Value::Text(atype.to_string()));
        }

        sql.push_str(" ORDER BY activity_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            params_vec.push(Value::Integer(i64::from(limit)));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(" OFFSET ?");
            params_vec.push(Value::Integer(i64::from(offset)));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_from_iter(params_vec), Self::map_collection_activity_row)
            .map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn update_collection_status(
        &self,
        invoice_id: InvoiceId,
        status: CollectionStatus,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE invoices SET collection_status = ?1 WHERE id = ?2",
            params![status.to_string(), invoice_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        // Return invoices that are overdue and haven't had recent dunning
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, invoice_number, order_id, customer_id, status, issue_date, due_date, subtotal, tax, shipping, discount, total, amount_paid, balance_due, currency, notes, terms, created_at, updated_at
             FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')
               AND CAST(balance_due AS REAL) > 0
               AND due_date < datetime('now')
               AND (last_dunning_date IS NULL OR last_dunning_date < datetime('now', '-7 days'))
             ORDER BY due_date ASC"
        ).map_err(map_db_error)?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Invoice {
                    id: InvoiceId::from(parse_uuid_row(
                        &row.get::<_, String>(0)?,
                        "invoice",
                        "id",
                    )?),
                    invoice_number: row.get(1)?,
                    order_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(2)?,
                        "invoice",
                        "order_id",
                    )?
                    .map(OrderId::from),
                    customer_id: CustomerId::from(parse_uuid_row(
                        &row.get::<_, String>(3)?,
                        "invoice",
                        "customer_id",
                    )?),
                    status: parse_enum_row(&row.get::<_, String>(4)?, "invoice", "status")?,
                    invoice_type: stateset_core::InvoiceType::Standard,
                    invoice_date: parse_datetime_row(
                        &row.get::<_, String>(5)?,
                        "invoice",
                        "invoice_date",
                    )?,
                    due_date: parse_datetime_row(&row.get::<_, String>(6)?, "invoice", "due_date")?,
                    payment_terms: None,
                    subtotal: parse_decimal_row(&row.get::<_, String>(7)?, "invoice", "subtotal")?,
                    tax_amount: parse_decimal_row(
                        &row.get::<_, String>(8)?,
                        "invoice",
                        "tax_amount",
                    )?,
                    tax_rate: None,
                    shipping_amount: parse_decimal_row(
                        &row.get::<_, String>(9)?,
                        "invoice",
                        "shipping_amount",
                    )?,
                    discount_amount: parse_decimal_row(
                        &row.get::<_, String>(10)?,
                        "invoice",
                        "discount_amount",
                    )?,
                    discount_percent: None,
                    total: parse_decimal_row(&row.get::<_, String>(11)?, "invoice", "total")?,
                    amount_paid: parse_decimal_row(
                        &row.get::<_, String>(12)?,
                        "invoice",
                        "amount_paid",
                    )?,
                    balance_due: parse_decimal_row(
                        &row.get::<_, String>(13)?,
                        "invoice",
                        "balance_due",
                    )?,
                    currency: row.get(14)?,
                    billing_name: None,
                    billing_email: None,
                    billing_address: None,
                    billing_city: None,
                    billing_state: None,
                    billing_postal_code: None,
                    billing_country: None,
                    po_number: None,
                    notes: row.get(15)?,
                    terms: row.get(16)?,
                    footer: None,
                    sent_at: None,
                    viewed_at: None,
                    paid_at: None,
                    voided_at: None,
                    items: vec![],
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(17)?,
                        "invoice",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(18)?,
                        "invoice",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn send_dunning_letter(
        &self,
        invoice_id: InvoiceId,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Update invoice dunning info
        conn.execute(
            "UPDATE invoices SET last_dunning_date = datetime('now'), dunning_count = COALESCE(dunning_count, 0) + 1 WHERE id = ?1",
            params![invoice_id.to_string()],
        ).map_err(map_db_error)?;

        // Update collection status based on letter type
        let new_status = match letter_type {
            DunningLetterType::Reminder1 => CollectionStatus::Reminder1Sent,
            DunningLetterType::Reminder2 => CollectionStatus::Reminder2Sent,
            DunningLetterType::Reminder3 => CollectionStatus::Reminder3Sent,
            DunningLetterType::DemandLetter | DunningLetterType::CollectionNotice => {
                CollectionStatus::InCollections
            }
            _ => CollectionStatus::InCollections,
        };

        self.update_collection_status(invoice_id, new_status)?;

        // Log the activity
        self.log_collection_activity(CreateCollectionActivity {
            invoice_id: invoice_id.into(),
            activity_type: CollectionActivityType::DunningLetterSent,
            dunning_letter_type: Some(letter_type),
            notes: Some(format!("Sent {} dunning letter", letter_type)),
            performed_by: sent_by.map(|s| s.to_string()),
            ..Default::default()
        })
    }

    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let write_off_number = generate_write_off_number();
        let customer_id = self.get_invoice_customer_id(input.invoice_id.into())?;

        conn.execute(
            "INSERT INTO ar_write_offs (id, write_off_number, invoice_id, customer_id, amount, reason, notes, write_off_date, approved_by, approved_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id.to_string(),
                write_off_number,
                input.invoice_id.to_string(),
                customer_id.to_string(),
                input.amount.to_string(),
                input.reason.to_string(),
                input.notes,
                now.to_rfc3339(),
                input.approved_by,
                input.approved_by.as_ref().map(|_| now.to_rfc3339()),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        // Update invoice status
        conn.execute(
            "UPDATE invoices SET status = 'written_off', collection_status = 'written_off' WHERE id = ?1",
            params![input.invoice_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(WriteOff {
            id,
            write_off_number,
            invoice_id: input.invoice_id,
            customer_id,
            amount: input.amount,
            reason: input.reason,
            notes: input.notes,
            write_off_date: now,
            approved_by: input.approved_by.clone(),
            approved_at: input.approved_by.map(|_| now),
            reversed_at: None,
            gl_journal_entry_id: None,
            created_at: now,
        })
    }

    fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, write_off_number, invoice_id, customer_id, amount, reason, notes, write_off_date, approved_by, approved_at, reversed_at, gl_journal_entry_id, created_at
             FROM ar_write_offs WHERE id = ?1",
            params![id.to_string()],
            Self::map_write_off_row,
        );

        match result {
            Ok(wo) => Ok(Some(wo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, write_off_number, invoice_id, customer_id, amount, reason, notes, write_off_date, approved_by, approved_at, reversed_at, gl_journal_entry_id, created_at
             FROM ar_write_offs WHERE 1=1"
        );
        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Value::Text(cust_id.to_string()));
        }
        if let Some(inv_id) = &filter.invoice_id {
            sql.push_str(" AND invoice_id = ?");
            params_vec.push(Value::Text(inv_id.to_string()));
        }
        if !filter.include_reversed.unwrap_or(false) {
            sql.push_str(" AND reversed_at IS NULL");
        }

        sql.push_str(" ORDER BY write_off_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            params_vec.push(Value::Integer(i64::from(limit)));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_from_iter(params_vec), Self::map_write_off_row)
            .map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();

        // Get the write-off
        let wo = self.get_write_off(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if wo.reversed_at.is_some() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Write-off already reversed".into(),
            ));
        }

        // Mark as reversed
        conn.execute(
            "UPDATE ar_write_offs SET reversed_at = ?1 WHERE id = ?2",
            params![now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        // Restore invoice status
        conn.execute(
            "UPDATE invoices SET status = 'overdue', collection_status = 'none' WHERE id = ?1",
            params![wo.invoice_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(WriteOff { reversed_at: Some(now), ..wo })
    }

    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let credit_memo_number = generate_credit_memo_number();

        conn.execute(
            "INSERT INTO ar_credit_memos (id, credit_memo_number, customer_id, original_invoice_id, reason, amount, applied_amount, unapplied_amount, status, notes, issue_date, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '0', ?7, 'open', ?8, ?9, ?10, ?11)",
            params![
                id.to_string(),
                credit_memo_number,
                input.customer_id.to_string(),
                input.original_invoice_id.map(|i| i.to_string()),
                input.reason.to_string(),
                input.amount.to_string(),
                input.amount.to_string(), // unapplied = full amount initially
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        Ok(CreditMemo {
            id,
            credit_memo_number,
            customer_id: input.customer_id,
            original_invoice_id: input.original_invoice_id,
            reason: input.reason,
            amount: input.amount,
            applied_amount: Decimal::ZERO,
            unapplied_amount: input.amount,
            status: CreditMemoStatus::Open,
            notes: input.notes,
            issue_date: now,
            gl_journal_entry_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount, applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id, created_at, updated_at
             FROM ar_credit_memos WHERE id = ?1",
            params![id.to_string()],
            Self::map_credit_memo_row,
        );

        match result {
            Ok(cm) => Ok(Some(cm)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount, applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id, created_at, updated_at
             FROM ar_credit_memos WHERE credit_memo_number = ?1",
            params![number],
            Self::map_credit_memo_row,
        );

        match result {
            Ok(cm) => Ok(Some(cm)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount, applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id, created_at, updated_at
             FROM ar_credit_memos WHERE 1=1"
        );
        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Value::Text(cust_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Value::Text(status.to_string()));
        }
        if filter.has_unapplied.unwrap_or(false) {
            sql.push_str(" AND CAST(unapplied_amount AS REAL) > 0");
        }

        sql.push_str(" ORDER BY issue_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            params_vec.push(Value::Integer(i64::from(limit)));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_from_iter(params_vec), Self::map_credit_memo_row)
            .map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        if input.amount <= Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit memo application amount must be greater than zero".into(),
            ));
        }

        let mut conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let cm = self
            .get_credit_memo(input.credit_memo_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        if !cm.can_apply() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit memo cannot be applied".into(),
            ));
        }

        if input.amount > cm.unapplied_amount {
            return Err(stateset_core::CommerceError::ValidationError(
                "Amount exceeds unapplied balance".into(),
            ));
        }

        let invoice_customer_id = self.get_invoice_customer_id(input.invoice_id.into())?;
        if invoice_customer_id != cm.customer_id {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit memo and invoice customer must match".into(),
            ));
        }

        let balance_due: String = conn
            .query_row(
                "SELECT balance_due FROM invoices WHERE id = ?1",
                params![input.invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        let balance_due = parse_decimal_safe(&balance_due, "invoice", "balance_due")?;
        if input.amount > balance_due {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit amount exceeds invoice balance due".into(),
            ));
        }

        let now = Utc::now();
        let app_id = Uuid::new_v4();
        let tx = conn.transaction().map_err(map_db_error)?;

        // Create application record
        tx.execute(
            "INSERT INTO ar_credit_memo_applications (id, credit_memo_id, invoice_id, applied_amount, applied_date, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                app_id.to_string(),
                input.credit_memo_id.to_string(),
                input.invoice_id.to_string(),
                input.amount.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        // Update credit memo
        let new_applied = cm.applied_amount + input.amount;
        let new_unapplied = cm.unapplied_amount - input.amount;
        let new_status = if new_unapplied <= Decimal::ZERO {
            CreditMemoStatus::FullyApplied
        } else {
            CreditMemoStatus::PartiallyApplied
        };

        tx.execute(
            "UPDATE ar_credit_memos SET applied_amount = ?1, unapplied_amount = ?2, status = ?3 WHERE id = ?4",
            params![
                new_applied.to_string(),
                new_unapplied.to_string(),
                new_status.to_string(),
                input.credit_memo_id.to_string()
            ],
        ).map_err(map_db_error)?;

        // Recalculate invoice
        Self::recalculate_invoice_with_conn(&tx, input.invoice_id.into())?;
        tx.commit().map_err(map_db_error)?;

        Ok(CreditMemo {
            applied_amount: new_applied,
            unapplied_amount: new_unapplied,
            status: new_status,
            updated_at: now,
            ..cm
        })
    }

    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let cm = self.get_credit_memo(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if cm.applied_amount > Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Cannot void credit memo with applications".into(),
            ));
        }

        conn.execute(
            "UPDATE ar_credit_memos SET status = 'voided' WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(CreditMemo { status: CreditMemoStatus::Voided, updated_at: Utc::now(), ..cm })
    }

    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer_id),
            has_unapplied: Some(true),
            ..Default::default()
        })
    }

    fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        if input.applications.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();
        let mut applications = Vec::new();
        let mut expected_customer_id: Option<Uuid> = None;

        for app in &input.applications {
            if app.amount <= Decimal::ZERO {
                return Err(stateset_core::CommerceError::ValidationError(
                    "Payment application amount must be greater than zero".into(),
                ));
            }

            let invoice_customer_id = self.get_invoice_customer_id(app.invoice_id.into())?;
            if let Some(expected) = expected_customer_id {
                if expected != invoice_customer_id {
                    return Err(stateset_core::CommerceError::ValidationError(
                        "All invoice applications for a payment must belong to the same customer"
                            .into(),
                    ));
                }
            } else {
                expected_customer_id = Some(invoice_customer_id);
            }

            let balance_due: String = tx
                .query_row(
                    "SELECT balance_due FROM invoices WHERE id = ?1",
                    params![app.invoice_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;
            let balance_due = parse_decimal_safe(&balance_due, "invoice", "balance_due")?;
            if app.amount > balance_due {
                return Err(stateset_core::CommerceError::ValidationError(
                    "Payment application amount exceeds invoice balance due".into(),
                ));
            }

            let app_id = Uuid::new_v4();

            tx.execute(
                "INSERT INTO ar_payment_applications (id, payment_id, invoice_id, applied_amount, applied_date, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    app_id.to_string(),
                    input.payment_id.to_string(),
                    app.invoice_id.to_string(),
                    app.amount.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339()
                ],
            ).map_err(map_db_error)?;

            // Recalculate invoice
            Self::recalculate_invoice_with_conn(&tx, app.invoice_id.into())?;

            applications.push(ArPaymentApplication {
                id: app_id,
                payment_id: input.payment_id,
                invoice_id: app.invoice_id,
                applied_amount: app.amount,
                applied_date: now,
                created_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(applications)
    }

    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, payment_id, invoice_id, applied_amount, applied_date, created_at
             FROM ar_payment_applications WHERE payment_id = ?1",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![payment_id.to_string()], Self::map_payment_application_row)
            .map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Get invoice_id first
        let invoice_id: String = conn
            .query_row(
                "SELECT invoice_id FROM ar_payment_applications WHERE id = ?1",
                params![application_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        // Delete application
        conn.execute(
            "DELETE FROM ar_payment_applications WHERE id = ?1",
            params![application_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Recalculate invoice
        let parsed_invoice_id = parse_uuid_row(&invoice_id, "payment_application", "invoice_id")
            .map_err(map_db_error)?;
        self.recalculate_invoice(parsed_invoice_id.into())?;

        Ok(())
    }

    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>> {
        let aging = match self.get_customer_aging(customer_id)? {
            Some(aging) => aging,
            None => return Ok(None),
        };

        // Get unapplied credits
        let unapplied_credits =
            self.get_unapplied_credits(customer_id)?.iter().map(|cm| cm.unapplied_amount).sum();

        let total_overdue = aging.total_overdue();
        Ok(Some(CustomerArSummary {
            customer_id,
            customer_name: aging.customer_name.clone(),
            total_outstanding: aging.total_outstanding,
            total_overdue,
            credit_limit: None,
            available_credit: None,
            unapplied_credits,
            unapplied_payments: Decimal::ZERO,
            average_days_to_pay: None,
            oldest_open_invoice: aging.oldest_invoice_date,
            last_activity_date: None,
            collection_status: CollectionStatus::None,
        }))
    }

    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();
        let period_start = request.period_start.unwrap_or_else(|| now - chrono::Duration::days(30));
        let period_end = request.period_end.unwrap_or(now);

        // Get customer info
        let (customer_name, customer_email): (String, Option<String>) = conn
            .query_row(
                "SELECT first_name || ' ' || last_name, email FROM customers WHERE id = ?1",
                params![request.customer_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_db_error)?;

        // Get aging
        let aging = self
            .get_customer_aging(request.customer_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        // Build line items from invoices and payments
        let mut line_items: Vec<StatementLineItem> = Vec::new();
        let mut running_balance = Decimal::ZERO;

        // Get invoices
        let mut stmt = conn
            .prepare(
                "SELECT created_at, invoice_number, total FROM invoices
             WHERE customer_id = ?1 AND created_at >= ?2 AND created_at <= ?3
             ORDER BY created_at",
            )
            .map_err(map_db_error)?;

        let inv_rows = stmt
            .query_map(
                params![
                    request.customer_id.to_string(),
                    period_start.to_rfc3339(),
                    period_end.to_rfc3339()
                ],
                |row| {
                    let date: String = row.get(0)?;
                    let number: String = row.get(1)?;
                    let total: String = row.get(2)?;
                    Ok((date, number, total))
                },
            )
            .map_err(map_db_error)?;

        for inv in inv_rows {
            let (date, number, total) = inv.map_err(map_db_error)?;
            let amount = parse_decimal_safe(&total, "statement_invoice", "total")?;
            running_balance += amount;
            line_items.push(StatementLineItem {
                date: parse_datetime_safe(&date, "statement_invoice", "created_at")?,
                transaction_type: StatementTransactionType::Invoice,
                reference_number: number,
                description: "Invoice".into(),
                debit: Some(amount),
                credit: None,
                balance: running_balance,
            });
        }

        // Get payments
        let mut stmt = conn
            .prepare(
                "SELECT pa.applied_date, p.id, pa.applied_amount
             FROM ar_payment_applications pa
             JOIN payments p ON pa.payment_id = p.id
             JOIN invoices i ON pa.invoice_id = i.id
             WHERE i.customer_id = ?1 AND pa.applied_date >= ?2 AND pa.applied_date <= ?3
             ORDER BY pa.applied_date",
            )
            .map_err(map_db_error)?;

        let pay_rows = stmt
            .query_map(
                params![
                    request.customer_id.to_string(),
                    period_start.to_rfc3339(),
                    period_end.to_rfc3339()
                ],
                |row| {
                    let date: String = row.get(0)?;
                    let id: String = row.get(1)?;
                    let amount: String = row.get(2)?;
                    Ok((date, id, amount))
                },
            )
            .map_err(map_db_error)?;

        for pay in pay_rows {
            let (date, id, amount_str) = pay.map_err(map_db_error)?;
            let amount = parse_decimal_safe(&amount_str, "statement_payment", "applied_amount")?;
            running_balance -= amount;
            line_items.push(StatementLineItem {
                date: parse_datetime_safe(&date, "statement_payment", "applied_date")?,
                transaction_type: StatementTransactionType::Payment,
                reference_number: id[..8].to_string(),
                description: "Payment".into(),
                debit: None,
                credit: Some(amount),
                balance: running_balance,
            });
        }

        // Sort by date
        line_items.sort_by(|a, b| a.date.cmp(&b.date));

        // Calculate totals
        let total_invoices: Decimal = line_items
            .iter()
            .filter(|l| matches!(l.transaction_type, StatementTransactionType::Invoice))
            .filter_map(|l| l.debit)
            .sum();
        let total_payments: Decimal = line_items
            .iter()
            .filter(|l| matches!(l.transaction_type, StatementTransactionType::Payment))
            .filter_map(|l| l.credit)
            .sum();

        Ok(CustomerStatement {
            customer_id: request.customer_id,
            customer_name,
            customer_email,
            billing_address: None,
            statement_date: now,
            period_start,
            period_end,
            opening_balance: Decimal::ZERO,
            total_invoices,
            total_payments,
            total_credits: Decimal::ZERO,
            closing_balance: aging.total_outstanding,
            aging,
            line_items,
        })
    }

    fn get_total_outstanding(&self) -> Result<Decimal> {
        let summary = self.get_aging_summary()?;
        Ok(summary.total)
    }

    fn get_dso(&self, days: i32) -> Result<Decimal> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // DSO = (Accounts Receivable / Total Credit Sales) × Number of Days
        let ar_balance = sum_decimal_query(
            &conn,
            "SELECT balance_due FROM invoices WHERE status NOT IN ('paid', 'voided', 'written_off')",
            &[],
            "invoices",
            "balance_due",
        )?;

        let sales_sql = format!(
            "SELECT total FROM invoices WHERE created_at >= datetime('now', '-{} days')",
            days
        );
        let total_sales = sum_decimal_query(&conn, &sales_sql, &[], "invoices", "total")?;

        if total_sales == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        let dso = (ar_balance / total_sales) * Decimal::from(days);
        Ok(dso)
    }

    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let result: rusqlite::Result<f64> = conn.query_row(
            "SELECT AVG(JULIANDAY(pa.applied_date) - JULIANDAY(i.issue_date))
             FROM ar_payment_applications pa
             JOIN invoices i ON pa.invoice_id = i.id
             WHERE i.customer_id = ?1 AND i.status = 'paid'",
            params![customer_id.to_string()],
            |row| row.get(0),
        );

        match result {
            Ok(avg) => Ok(Some(avg as i32)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(rusqlite::Error::InvalidColumnType(_, _, _)) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        let mut summaries = Vec::new();
        for id in ids {
            if let Some(summary) = self.get_customer_summary(id)? {
                summaries.push(summary);
            }
        }
        Ok(summaries)
    }
}
