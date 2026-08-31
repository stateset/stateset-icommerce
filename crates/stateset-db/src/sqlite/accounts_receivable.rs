//! SQLite implementation of Accounts Receivable repository

use crate::sqlite::parse_helpers::{
    parse_datetime as parse_datetime_safe, parse_decimal as parse_decimal_safe,
};
use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_uuid, parse_uuid_opt_row, parse_uuid_row,
    sum_decimal_query, with_immediate_transaction,
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
    #[must_use]
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
        // Sum applications exactly in Rust: `SUM()` over a TEXT decimal column
        // coerces to float/int in SQLite (and returns a non-TEXT type), which is
        // both lossy and mistyped on read. `sum_decimal_query` reads each stored
        // decimal and adds them with `rust_decimal`.
        let inv = invoice_id.to_string();
        let paid_dec = sum_decimal_query(
            conn,
            "SELECT applied_amount FROM ar_payment_applications WHERE invoice_id = ?1",
            &[&inv as &dyn rusqlite::ToSql],
            "invoice",
            "paid_amount",
        )?;
        let credits_dec = sum_decimal_query(
            conn,
            "SELECT applied_amount FROM ar_credit_memo_applications WHERE invoice_id = ?1",
            &[&inv as &dyn rusqlite::ToSql],
            "invoice",
            "credits_amount",
        )?;
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

        // The customer exists (checked above); an empty open-invoice set is a
        // zero-filled aging, not NotFound — matching the Postgres backend and
        // keeping get_customer_summary / generate_statement working for paid-up
        // customers.
        Ok(Some(CustomerArAging {
            customer_id,
            customer_name: Some(format!("{first_name} {last_name}")),
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
            customer_name: Option<String>,
            customer_email: Option<String>,
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
            // first_name/last_name/email come from a LEFT JOIN, so they are NULL
            // when an invoice references a customer that no longer exists. Read
            // them as optional rather than crashing the whole report on one
            // orphaned invoice.
            let first_name: Option<String> = row.get(1).map_err(map_db_error)?;
            let last_name: Option<String> = row.get(2).map_err(map_db_error)?;
            let email: Option<String> = row.get(3).map_err(map_db_error)?;
            let due_date_str: String = row.get(4).map_err(map_db_error)?;
            let balance_str: String = row.get(5).map_err(map_db_error)?;
            let created_at_str: String = row.get(6).map_err(map_db_error)?;

            let balance = parse_decimal_safe(&balance_str, "invoice", "balance_due")?;
            if balance <= Decimal::ZERO {
                continue;
            }

            let due_date = parse_datetime_safe(&due_date_str, "invoice", "due_date")?;
            let created_at = parse_datetime_safe(&created_at_str, "invoice", "created_at")?;

            let customer_name = match (first_name, last_name) {
                (Some(f), Some(l)) => Some(format!("{f} {l}")),
                (Some(f), None) => Some(f),
                (None, Some(l)) => Some(l),
                (None, None) => None,
            };
            let entry = by_customer.entry(customer_id).or_insert_with(|| AgingAccum {
                customer_id,
                customer_name,
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
                // overdue_only: the customer must have some past-due balance.
                if filter.overdue_only.unwrap_or(false)
                    && entry.days_1_30 + entry.days_31_60 + entry.days_61_90 + entry.days_over_90
                        <= Decimal::ZERO
                {
                    return false;
                }
                // min_balance: total outstanding must meet the threshold. Mirrors
                // the Postgres `HAVING SUM(balance_due) >= min_balance`, applied
                // before pagination so the offset/limit see the filtered set.
                if let Some(min_balance) = filter.min_balance {
                    let total = entry.current
                        + entry.days_1_30
                        + entry.days_31_60
                        + entry.days_61_90
                        + entry.days_over_90;
                    if total < min_balance {
                        return false;
                    }
                }
                // aging_bucket: the customer must have a positive balance in the
                // requested bucket (matches the Postgres per-bucket HAVING).
                if let Some(bucket) = filter.aging_bucket {
                    let has_bucket_balance = match bucket {
                        stateset_core::AgingBucket::Current => entry.current > Decimal::ZERO,
                        stateset_core::AgingBucket::Days1To30 => entry.days_1_30 > Decimal::ZERO,
                        stateset_core::AgingBucket::Days31To60 => entry.days_31_60 > Decimal::ZERO,
                        stateset_core::AgingBucket::Days61To90 => entry.days_61_90 > Decimal::ZERO,
                        stateset_core::AgingBucket::DaysOver90 => {
                            entry.days_over_90 > Decimal::ZERO
                        }
                        // Unknown bucket: don't filter (matches Postgres `1=1`).
                        _ => true,
                    };
                    if !has_bucket_balance {
                        return false;
                    }
                }
                true
            })
            .map(|entry| {
                let total = entry.current
                    + entry.days_1_30
                    + entry.days_31_60
                    + entry.days_61_90
                    + entry.days_over_90;
                CustomerArAging {
                    customer_id: entry.customer_id,
                    customer_name: entry.customer_name,
                    customer_email: entry.customer_email,
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

        // Sort by outstanding balance, then customer_id as a total-order
        // tiebreaker: customers with equal balances must have a stable, unique
        // ordering, otherwise the offset/limit pagination below (and the Postgres
        // ORDER BY) can skip or duplicate rows across pages.
        results.sort_by(|a, b| {
            b.total_outstanding
                .cmp(&a.total_outstanding)
                .then_with(|| a.customer_id.cmp(&b.customer_id))
        });

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

        // `balance_due` is a TEXT decimal, so it is not filtered in SQL (a
        // CAST(... AS REAL) comparison coerces to IEEE-754 floats); the exact
        // `Decimal` filter is applied after parsing, below.
        let mut stmt = conn.prepare(
            "SELECT id, invoice_number, order_id, customer_id, status, invoice_date, due_date, subtotal, tax_amount, shipping_amount, discount_amount, total, amount_paid, balance_due, currency, notes, terms, created_at, updated_at
             FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')
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

        let invoices = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)?;
        // Only invoices that still owe money are due for dunning; compare on
        // the exact parsed Decimal rather than a float-coerced SQL cast.
        Ok(invoices.into_iter().filter(|invoice| invoice.balance_due > Decimal::ZERO).collect())
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
            notes: Some(format!("Sent {letter_type} dunning letter")),
            performed_by: sent_by.map(std::string::ToString::to_string),
            ..Default::default()
        })
    }

    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        if input.amount <= Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Write-off amount must be greater than zero".into(),
            ));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let write_off_number = generate_write_off_number();

        // Atomic: mark the invoice written-off and insert the write-off row in one
        // immediate transaction. Previously two autocommit statements — a crash
        // between them left the write-off and the invoice status inconsistent. The
        // guarded status flip also refuses to double-write-off an invoice.
        let customer_id = with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite = |e: stateset_core::CommerceError| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            };

            let (customer_str, invoice_status, balance_str): (String, String, String) = tx
                .query_row(
                    "SELECT customer_id, status, balance_due FROM invoices WHERE id = ?1",
                    params![input.invoice_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            let customer_id = parse_uuid_row(&customer_str, "invoice", "customer_id")?;

            // A voided/cancelled invoice cannot be written off; an already
            // written-off invoice is rejected by the guarded UPDATE below.
            if matches!(invoice_status.as_str(), "voided" | "cancelled") {
                return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(format!(
                    "Cannot write off invoice {} with status {invoice_status}",
                    input.invoice_id
                ))));
            }

            let balance_due =
                parse_decimal_safe(&balance_str, "invoice", "balance_due").map_err(to_rusqlite)?;
            if input.amount > balance_due {
                return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(format!(
                    "Write-off amount ({}) exceeds invoice balance due ({balance_due})",
                    input.amount
                ))));
            }

            let rows = tx.execute(
                "UPDATE invoices SET status = 'written_off', collection_status = 'written_off'
                 WHERE id = ?1 AND status != 'written_off'",
                params![input.invoice_id.to_string()],
            )?;
            if rows == 0 {
                return Err(to_rusqlite(stateset_core::CommerceError::Conflict(
                    "Invoice not found or already written off".into(),
                )));
            }

            tx.execute(
                "INSERT INTO ar_write_offs (id, write_off_number, invoice_id, customer_id, amount, reason, notes, write_off_date, approved_by, approved_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id.to_string(),
                    write_off_number,
                    input.invoice_id.to_string(),
                    customer_id.to_string(),
                    input.amount.to_string(),
                    input.reason.to_string(),
                    &input.notes,
                    now.to_rfc3339(),
                    &input.approved_by,
                    input.approved_by.as_ref().map(|_| now.to_rfc3339()),
                    now.to_rfc3339()
                ],
            )?;

            Ok(customer_id)
        })?;

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
        let now = Utc::now();

        // Get the write-off (cheap early-out; re-checked under the lock below).
        let wo = self.get_write_off(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if wo.reversed_at.is_some() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Write-off already reversed".into(),
            ));
        }

        // Atomic + guarded: mark reversed and restore the invoice in one immediate
        // transaction. The guard (reversed_at IS NULL) makes a concurrent/duplicate
        // reversal a no-op conflict instead of a double restore.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite = |e: stateset_core::CommerceError| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            };

            let rows = tx.execute(
                "UPDATE ar_write_offs SET reversed_at = ?1 WHERE id = ?2 AND reversed_at IS NULL",
                params![now.to_rfc3339(), id.to_string()],
            )?;
            if rows == 0 {
                return Err(to_rusqlite(stateset_core::CommerceError::Conflict(
                    "Write-off not found or already reversed".into(),
                )));
            }

            tx.execute(
                "UPDATE invoices SET status = 'overdue', collection_status = 'none' WHERE id = ?1",
                params![wo.invoice_id.to_string()],
            )?;

            Ok(())
        })?;

        Ok(WriteOff { reversed_at: Some(now), ..wo })
    }

    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        if input.amount <= Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit memo amount must be greater than zero".into(),
            ));
        }

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
        // `unapplied_amount` is a TEXT decimal, so the has_unapplied filter is
        // applied below on the exact parsed `Decimal` (a SQL CAST(... AS REAL)
        // comparison coerces to IEEE-754 floats), and the LIMIT after it so
        // filtering never eats into the page.
        let has_unapplied = filter.has_unapplied.unwrap_or(false);

        sql.push_str(" ORDER BY issue_date DESC");

        if !has_unapplied {
            if let Some(limit) = filter.limit {
                sql.push_str(" LIMIT ?");
                params_vec.push(Value::Integer(i64::from(limit)));
            }
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_from_iter(params_vec), Self::map_credit_memo_row)
            .map_err(map_db_error)?;
        let mut memos = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)?;
        if has_unapplied {
            memos.retain(|memo| memo.unapplied_amount > Decimal::ZERO);
            if let Some(limit) = filter.limit {
                memos.truncate(limit as usize);
            }
        }
        Ok(memos)
    }

    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        if input.amount <= Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Credit memo application amount must be greater than zero".into(),
            ));
        }

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

        let now = Utc::now();
        let app_id = Uuid::new_v4();
        let cm_id_str = input.credit_memo_id.to_string();
        let invoice_id_str = input.invoice_id.to_string();

        // Apply atomically. The authoritative unapplied balance and the invoice
        // balance are re-read INSIDE the immediate (write-locked) transaction and
        // the memo update is guarded on the re-read unapplied amount, so two
        // concurrent applications of the same memo cannot both validate against a
        // stale balance and double-apply (the pre-transaction checks above stay as
        // cheap early-outs). Mirrors the pattern used by orders/inventory/GL.
        let (new_applied, new_unapplied, new_status) = with_immediate_transaction(
            &self.pool,
            |tx| {
                let to_rusqlite = |e: stateset_core::CommerceError| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                };

                // Re-read the memo's current amounts AND status under the write
                // lock: a concurrent void_credit_memo (also an immediate txn) may
                // have voided it since the pre-transaction can_apply() check.
                let (applied_str, unapplied_str, status_str): (String, String, String) = tx
                    .query_row(
                        "SELECT applied_amount, unapplied_amount, status FROM ar_credit_memos WHERE id = ?1",
                        params![cm_id_str],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )?;
                if status_str == "voided" {
                    return Err(to_rusqlite(stateset_core::CommerceError::Conflict(
                        "Credit memo was voided concurrently".into(),
                    )));
                }
                let cur_applied = parse_decimal_safe(&applied_str, "credit_memo", "applied_amount")
                    .map_err(to_rusqlite)?;
                let cur_unapplied =
                    parse_decimal_safe(&unapplied_str, "credit_memo", "unapplied_amount")
                        .map_err(to_rusqlite)?;

                if input.amount > cur_unapplied {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        "Amount exceeds unapplied balance".into(),
                    )));
                }

                // Re-read the invoice balance and status under the same lock: a
                // credit memo must not be applied to a terminal invoice.
                let (balance_str, invoice_status): (String, String) = tx.query_row(
                    "SELECT balance_due, status FROM invoices WHERE id = ?1",
                    params![invoice_id_str],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if matches!(invoice_status.as_str(), "voided" | "cancelled" | "written_off") {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        format!(
                            "Cannot apply credit memo to invoice {} with status {invoice_status}",
                            input.invoice_id
                        ),
                    )));
                }
                let balance_due = parse_decimal_safe(&balance_str, "invoice", "balance_due")
                    .map_err(to_rusqlite)?;
                if input.amount > balance_due {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        "Credit amount exceeds invoice balance due".into(),
                    )));
                }

                let new_applied = cur_applied + input.amount;
                let new_unapplied = cur_unapplied - input.amount;
                let new_status = if new_unapplied <= Decimal::ZERO {
                    CreditMemoStatus::FullyApplied
                } else {
                    CreditMemoStatus::PartiallyApplied
                };

                tx.execute(
                    "INSERT INTO ar_credit_memo_applications (id, credit_memo_id, invoice_id, applied_amount, applied_date, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        app_id.to_string(),
                        cm_id_str,
                        invoice_id_str,
                        input.amount.to_string(),
                        now.to_rfc3339(),
                        now.to_rfc3339()
                    ],
                )?;

                // Guard on the unapplied amount we just read: if a concurrent
                // writer already moved it, this affects 0 rows -> conflict.
                let rows = tx.execute(
                    "UPDATE ar_credit_memos SET applied_amount = ?1, unapplied_amount = ?2, status = ?3
                     WHERE id = ?4 AND unapplied_amount = ?5",
                    params![
                        new_applied.to_string(),
                        new_unapplied.to_string(),
                        new_status.to_string(),
                        cm_id_str,
                        unapplied_str
                    ],
                )?;
                if rows == 0 {
                    return Err(to_rusqlite(stateset_core::CommerceError::Conflict(
                        "Credit memo was modified concurrently".into(),
                    )));
                }

                Self::recalculate_invoice_with_conn(tx, input.invoice_id.into())
                    .map_err(to_rusqlite)?;

                Ok((new_applied, new_unapplied, new_status))
            },
        )?;

        Ok(CreditMemo {
            applied_amount: new_applied,
            unapplied_amount: new_unapplied,
            status: new_status,
            updated_at: now,
            ..cm
        })
    }

    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        let cm = self.get_credit_memo(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        // Atomic + guarded: re-read the applied amount under the write lock so a
        // concurrent apply_credit_memo (also an immediate txn) cannot slip an
        // application in between the check and the void; the status guard blocks a
        // double void.
        with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite = |e: stateset_core::CommerceError| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            };

            let applied_str: String = tx.query_row(
                "SELECT applied_amount FROM ar_credit_memos WHERE id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )?;
            let applied = parse_decimal_safe(&applied_str, "credit_memo", "applied_amount")
                .map_err(to_rusqlite)?;
            if applied > Decimal::ZERO {
                return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                    "Cannot void credit memo with applications".into(),
                )));
            }

            let rows = tx.execute(
                "UPDATE ar_credit_memos SET status = 'voided' WHERE id = ?1 AND status != 'voided'",
                params![id.to_string()],
            )?;
            if rows == 0 {
                return Err(to_rusqlite(stateset_core::CommerceError::Conflict(
                    "Credit memo not found or already voided".into(),
                )));
            }
            Ok(())
        })?;

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

        let now = Utc::now();

        // Immediate (write-locked) transaction so concurrent payments against the
        // same invoice serialize: each application re-reads the invoice balance
        // under the lock, so a second payment sees the balance the first already
        // reduced and cannot over-apply. `with_immediate_transaction` also retries
        // on SQLITE_BUSY, which a raw deferred `super::begin_immediate(&mut conn)` did not.
        let applications = with_immediate_transaction(&self.pool, |tx| {
            let to_rusqlite = |e: stateset_core::CommerceError| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            };
            let mut applications = Vec::new();
            let mut expected_customer_id: Option<Uuid> = None;

            // The payment itself bounds the total that can be applied: read its
            // amount and what is already applied under the same write lock, so a
            // payment can never be applied beyond its own value.
            let payment_id_str = input.payment_id.to_string();
            let payment_amount_str: String = match tx.query_row(
                "SELECT amount FROM payments WHERE id = ?1",
                params![payment_id_str],
                |row| row.get(0),
            ) {
                Ok(amount) => amount,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        format!("Payment {} not found", input.payment_id),
                    )));
                }
                Err(e) => return Err(e),
            };
            let payment_amount = parse_decimal_safe(&payment_amount_str, "payment", "amount")
                .map_err(to_rusqlite)?;
            let existing_applied = sum_decimal_query(
                tx,
                "SELECT applied_amount FROM ar_payment_applications WHERE payment_id = ?1",
                &[&payment_id_str],
                "payment_application",
                "applied_amount",
            )
            .map_err(to_rusqlite)?;

            let mut total_new = Decimal::ZERO;
            for app in &input.applications {
                if app.amount <= Decimal::ZERO {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        "Payment application amount must be greater than zero".into(),
                    )));
                }
                total_new += app.amount;
            }
            if existing_applied + total_new > payment_amount {
                return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(format!(
                    "Payment applications ({total_new}) plus already applied ({existing_applied}) exceed payment amount ({payment_amount})"
                ))));
            }

            for app in &input.applications {
                // Read the customer id and status on the transaction's own
                // connection — a nested `self.pool.get()` here would take a
                // second pooled connection while this one is held, deadlocking
                // the pool under concurrency.
                let (customer_str, invoice_status): (String, String) = tx.query_row(
                    "SELECT customer_id, status FROM invoices WHERE id = ?1",
                    params![app.invoice_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if matches!(invoice_status.as_str(), "voided" | "cancelled" | "written_off") {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        format!(
                            "Cannot apply payment to invoice {} with status {invoice_status}",
                            app.invoice_id
                        ),
                    )));
                }
                let invoice_customer_id = parse_uuid_row(&customer_str, "invoice", "customer_id")?;
                if let Some(expected) = expected_customer_id {
                    if expected != invoice_customer_id {
                        return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                            "All invoice applications for a payment must belong to the same customer"
                                .into(),
                        )));
                    }
                } else {
                    expected_customer_id = Some(invoice_customer_id);
                }

                let balance_str: String = tx.query_row(
                    "SELECT balance_due FROM invoices WHERE id = ?1",
                    params![app.invoice_id.to_string()],
                    |row| row.get(0),
                )?;
                let balance_due = parse_decimal_safe(&balance_str, "invoice", "balance_due")
                    .map_err(to_rusqlite)?;
                if app.amount > balance_due {
                    return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                        "Payment application amount exceeds invoice balance due".into(),
                    )));
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
                )?;

                Self::recalculate_invoice_with_conn(tx, app.invoice_id.into())
                    .map_err(to_rusqlite)?;

                applications.push(ArPaymentApplication {
                    id: app_id,
                    payment_id: input.payment_id,
                    invoice_id: app.invoice_id,
                    applied_amount: app.amount,
                    applied_date: now,
                    created_at: now,
                });
            }

            Ok(applications)
        })?;

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
            "SELECT total FROM invoices WHERE created_at >= datetime('now', '-{days} days')"
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
            "SELECT AVG(JULIANDAY(pa.applied_date) - JULIANDAY(i.invoice_date))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AccountsReceivableRepository, AgingBucket, ApplyCreditMemo, ArAgingFilter,
        CreateCreditMemo, CreditMemoFilter, CreditMemoReason, CreditMemoStatus,
    };

    fn fresh_repo() -> SqliteAccountsReceivableRepository {
        SqliteDatabase::in_memory().expect("in-memory").accounts_receivable()
    }

    fn make_memo(
        repo: &SqliteAccountsReceivableRepository,
        customer_id: Uuid,
        amount: Decimal,
        reason: CreditMemoReason,
    ) -> CreditMemo {
        repo.create_credit_memo(CreateCreditMemo {
            customer_id,
            original_invoice_id: None,
            reason,
            amount,
            notes: Some("test".into()),
        })
        .expect("create memo")
    }

    fn seed_payment(repo: &SqliteAccountsReceivableRepository, id: Uuid, number: &str) {
        let conn = repo.pool.get().expect("conn");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO payments (id, payment_number, amount, created_at, updated_at)
             VALUES (?1, ?2, '100', ?3, ?3)",
            params![id.to_string(), number, now],
        )
        .expect("seed payment");
    }

    fn seed_paid_invoice(
        repo: &SqliteAccountsReceivableRepository,
        id: Uuid,
        number: &str,
        customer_id: Uuid,
        invoice_date: chrono::DateTime<Utc>,
    ) {
        let conn = repo.pool.get().expect("conn");
        let d = invoice_date.to_rfc3339();
        conn.execute(
            "INSERT INTO invoices (id, invoice_number, customer_id, status, invoice_date,
                due_date, total, amount_paid, balance_due, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'paid', ?4, ?4, '100', '100', '0', ?4, ?4)",
            params![id.to_string(), number, customer_id.to_string(), d],
        )
        .expect("seed paid invoice");
    }

    fn seed_application(
        repo: &SqliteAccountsReceivableRepository,
        payment_id: Uuid,
        invoice_id: Uuid,
        applied_date: chrono::DateTime<Utc>,
    ) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO ar_payment_applications (id, payment_id, invoice_id, applied_amount, applied_date, created_at)
             VALUES (?1, ?2, ?3, '100', ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                payment_id.to_string(),
                invoice_id.to_string(),
                applied_date.to_rfc3339(),
                Utc::now().to_rfc3339(),
            ],
        )
        .expect("seed application");
    }

    #[test]
    fn average_days_to_pay_uses_fractional_days() {
        let repo = fresh_repo();
        let customer = Uuid::new_v4();
        let payment = Uuid::new_v4();
        seed_payment(&repo, payment, "PAY-ADP");

        let base = Utc::now();
        let inv1 = Uuid::new_v4();
        let inv2 = Uuid::new_v4();
        seed_paid_invoice(&repo, inv1, "INV-ADP-1", customer, base);
        seed_paid_invoice(&repo, inv2, "INV-ADP-2", customer, base);
        // Paid at +10.5 and +11.5 days.
        seed_application(&repo, payment, inv1, base + chrono::Duration::hours(10 * 24 + 12));
        seed_application(&repo, payment, inv2, base + chrono::Duration::hours(11 * 24 + 12));

        // Fractional per-invoice days: AVG(10.5, 11.5) = 11.0 -> 11. Postgres's
        // whole-day EXTRACT(DAY) truncation would give AVG(10, 11) = 10.5 -> 10.
        assert_eq!(repo.get_average_days_to_pay(customer).expect("avg"), Some(11));
    }

    /// Insert an overdue, unpaid invoice directly so balance filters can be
    /// exercised with exact TEXT decimal values.
    fn insert_overdue_invoice(
        repo: &SqliteAccountsReceivableRepository,
        number: &str,
        balance_due: &str,
    ) {
        let conn = repo.pool.get().expect("conn");
        let now = Utc::now();
        let due = now - chrono::Duration::days(30);
        conn.execute(
            "INSERT INTO invoices (id, invoice_number, customer_id, status, invoice_date,
                due_date, total, amount_paid, balance_due, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'sent', ?4, ?5, ?6, '0', ?6, ?4, ?4)",
            params![
                Uuid::new_v4().to_string(),
                number,
                Uuid::new_v4().to_string(),
                now.to_rfc3339(),
                due.to_rfc3339(),
                balance_due,
            ],
        )
        .expect("insert invoice");
    }

    #[test]
    fn dunning_keeps_tiny_positive_balances_and_drops_zero_or_negative() {
        let repo = fresh_repo();
        insert_overdue_invoice(&repo, "INV-DUN-1", "0.005");
        insert_overdue_invoice(&repo, "INV-DUN-2", "0.00");
        // Smallest positive value a Decimal can represent (scale 28).
        insert_overdue_invoice(&repo, "INV-DUN-3", "0.0000000000000000000000000001");
        insert_overdue_invoice(&repo, "INV-DUN-4", "-0.01");

        let due = repo.get_invoices_due_for_dunning().expect("ok");
        let numbers: Vec<&str> = due.iter().map(|i| i.invoice_number.as_str()).collect();
        assert!(numbers.contains(&"INV-DUN-1"), "tiny positive balance is due for dunning");
        assert!(numbers.contains(&"INV-DUN-3"), "1e-28 balance is still positive");
        assert!(!numbers.contains(&"INV-DUN-2"), "zero balance must not be dunned");
        assert!(!numbers.contains(&"INV-DUN-4"), "credit balance must not be dunned");

        let tiny = due.iter().find(|i| i.invoice_number == "INV-DUN-1").expect("found");
        assert_eq!(tiny.balance_due, dec!(0.005), "balance must round-trip exactly");
    }

    /// Insert an unpaid invoice for a specific customer with a chosen due-date
    /// offset (positive = days in the past, negative = days in the future), so
    /// aging-bucket filters can be exercised deterministically.
    fn insert_invoice_due_days(
        repo: &SqliteAccountsReceivableRepository,
        customer_id: Uuid,
        number: &str,
        balance_due: &str,
        due_days_ago: i64,
    ) {
        let conn = repo.pool.get().expect("conn");
        let now = Utc::now();
        let due = now - chrono::Duration::days(due_days_ago);
        conn.execute(
            "INSERT INTO invoices (id, invoice_number, customer_id, status, invoice_date,
                due_date, total, amount_paid, balance_due, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'sent', ?4, ?5, ?6, '0', ?6, ?4, ?4)",
            params![
                Uuid::new_v4().to_string(),
                number,
                customer_id.to_string(),
                now.to_rfc3339(),
                due.to_rfc3339(),
                balance_due,
            ],
        )
        .expect("insert invoice");
    }

    #[test]
    fn get_aging_report_honors_min_balance() {
        // Each insert_overdue_invoice creates a distinct customer.
        let repo = fresh_repo();
        insert_overdue_invoice(&repo, "INV-SMALL", "100.00");
        insert_overdue_invoice(&repo, "INV-BIG", "5000.00");

        let all = repo.get_aging_report(ArAgingFilter::default()).expect("all");
        assert_eq!(all.len(), 2, "both customers appear without a min_balance filter");

        let filtered = repo
            .get_aging_report(ArAgingFilter { min_balance: Some(dec!(1000)), ..Default::default() })
            .expect("filtered");
        assert_eq!(filtered.len(), 1, "only the >= 1000 customer must pass min_balance");
        assert_eq!(filtered[0].total_outstanding, dec!(5000.00));
    }

    #[test]
    fn get_aging_report_honors_aging_bucket() {
        let repo = fresh_repo();
        let current_cust = Uuid::new_v4();
        let over90_cust = Uuid::new_v4();
        // Not yet due (5 days in the future) => Current bucket.
        insert_invoice_due_days(&repo, current_cust, "INV-CUR", "200.00", -5);
        // 120 days overdue => DaysOver90 bucket.
        insert_invoice_due_days(&repo, over90_cust, "INV-OLD", "300.00", 120);

        let over90 = repo
            .get_aging_report(ArAgingFilter {
                aging_bucket: Some(AgingBucket::DaysOver90),
                ..Default::default()
            })
            .expect("over90");
        assert_eq!(over90.len(), 1, "only the >90-day customer matches DaysOver90: {over90:?}");
        assert_eq!(over90[0].customer_id, over90_cust);

        let current = repo
            .get_aging_report(ArAgingFilter {
                aging_bucket: Some(AgingBucket::Current),
                ..Default::default()
            })
            .expect("current");
        assert_eq!(current.len(), 1, "only the not-yet-due customer matches Current: {current:?}");
        assert_eq!(current[0].customer_id, current_cust);
    }

    /// Insert a bare customer row (id + required NOT NULL columns).
    fn insert_customer(repo: &SqliteAccountsReceivableRepository, id: Uuid, email: &str) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO customers (id, email, first_name, last_name) VALUES (?1, ?2, 'Paid', 'Up')",
            params![id.to_string(), email],
        )
        .expect("insert customer");
    }

    #[test]
    fn get_customer_aging_returns_zeros_for_existing_customer_without_invoices() {
        // A customer that exists but owes nothing must return a zero-filled aging
        // (matching Postgres), NOT NotFound — the truly-missing case is the only
        // one that is None. SQLite previously returned Err(NotFound) here, which
        // also broke get_customer_summary / generate_statement for paid-up
        // customers.
        let repo = fresh_repo();
        let existing = Uuid::new_v4();
        insert_customer(&repo, existing, "paidup@example.com");

        let aging = repo
            .get_customer_aging(existing)
            .expect("an existing customer must not error")
            .expect("an existing customer must return Some, even with no open invoices");
        assert_eq!(aging.invoice_count, 0);
        assert_eq!(aging.total_outstanding, dec!(0));
        assert_eq!(aging.current, dec!(0));
        assert!(aging.oldest_invoice_date.is_none());

        // A truly-unknown customer is still None (not an error).
        let missing =
            repo.get_customer_aging(Uuid::new_v4()).expect("unknown customer is not error");
        assert!(missing.is_none(), "unknown customer must be None");
    }

    #[test]
    fn has_unapplied_filter_is_exact_and_limit_applies_after_filtering() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let m1 = make_memo(&repo, cust, dec!(10), CreditMemoReason::ReturnedGoods);
        let m2 = make_memo(&repo, cust, dec!(20), CreditMemoReason::ReturnedGoods);
        let m3 = make_memo(&repo, cust, dec!(30), CreditMemoReason::ReturnedGoods);

        {
            let conn = repo.pool.get().expect("conn");
            // Fully-applied memos whose zero balance is stored non-canonically,
            // plus one memo with a sub-cent positive balance.
            for (id, unapplied) in [(m1.id, "0.00"), (m2.id, "0"), (m3.id, "0.005")] {
                conn.execute(
                    "UPDATE ar_credit_memos SET unapplied_amount = ?1 WHERE id = ?2",
                    params![unapplied, id.to_string()],
                )
                .expect("update memo");
            }
        }

        let got = repo
            .list_credit_memos(CreditMemoFilter {
                customer_id: Some(cust),
                has_unapplied: Some(true),
                limit: Some(1),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(got.len(), 1, "only one memo has an unapplied balance");
        assert_eq!(got[0].id, m3.id);
        assert_eq!(got[0].unapplied_amount, dec!(0.005));
    }

    #[test]
    fn empty_aging_summary_is_zero() {
        let repo = fresh_repo();
        let summary = repo.get_aging_summary().expect("ok");
        assert_eq!(summary.total, dec!(0));
        assert_eq!(summary.current, dec!(0));
        assert_eq!(summary.days_over_90, dec!(0));
    }

    #[test]
    fn empty_aging_report_returns_empty() {
        let repo = fresh_repo();
        let rows = repo.get_aging_report(ArAgingFilter::default()).expect("ok");
        assert!(rows.is_empty());
    }

    #[test]
    fn empty_total_outstanding_is_zero() {
        let repo = fresh_repo();
        assert_eq!(repo.get_total_outstanding().expect("ok"), dec!(0));
    }

    #[test]
    fn empty_dso_is_zero() {
        let repo = fresh_repo();
        assert_eq!(repo.get_dso(30).expect("ok"), dec!(0));
    }

    #[test]
    fn average_days_to_pay_for_unknown_customer_is_none() {
        let repo = fresh_repo();
        assert!(repo.get_average_days_to_pay(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn create_credit_memo_starts_open_with_full_unapplied() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let memo = make_memo(&repo, cust, dec!(100), CreditMemoReason::ReturnedGoods);
        assert_eq!(memo.amount, dec!(100));
        assert_eq!(memo.applied_amount, dec!(0));
        assert_eq!(memo.unapplied_amount, dec!(100));
        assert_eq!(memo.status, CreditMemoStatus::Open);
        assert!(memo.credit_memo_number.starts_with("CM-"));
    }

    #[test]
    fn get_credit_memo_round_trips() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let memo = make_memo(&repo, cust, dec!(50), CreditMemoReason::PricingError);
        let by_id = repo.get_credit_memo(memo.id).expect("ok").expect("found");
        assert_eq!(by_id.id, memo.id);
        let by_num =
            repo.get_credit_memo_by_number(&memo.credit_memo_number).expect("ok").expect("found");
        assert_eq!(by_num.id, memo.id);
        assert!(repo.get_credit_memo_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn list_credit_memos_filters_by_customer() {
        let repo = fresh_repo();
        let cust_a = Uuid::new_v4();
        let cust_b = Uuid::new_v4();
        make_memo(&repo, cust_a, dec!(10), CreditMemoReason::Damaged);
        make_memo(&repo, cust_a, dec!(20), CreditMemoReason::Overpayment);
        make_memo(&repo, cust_b, dec!(30), CreditMemoReason::ReturnedGoods);

        let for_a = repo
            .list_credit_memos(CreditMemoFilter { customer_id: Some(cust_a), ..Default::default() })
            .expect("list");
        assert_eq!(for_a.len(), 2);
        assert!(for_a.iter().all(|m| m.customer_id == cust_a));
    }

    #[test]
    fn list_credit_memos_filters_by_status() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let to_void = make_memo(&repo, cust, dec!(10), CreditMemoReason::ReturnedGoods);
        let _staying_open = make_memo(&repo, cust, dec!(20), CreditMemoReason::ReturnedGoods);
        repo.void_credit_memo(to_void.id).expect("void");

        let opens = repo
            .list_credit_memos(CreditMemoFilter {
                status: Some(CreditMemoStatus::Open),
                ..Default::default()
            })
            .expect("opens");
        let voided = repo
            .list_credit_memos(CreditMemoFilter {
                status: Some(CreditMemoStatus::Voided),
                ..Default::default()
            })
            .expect("voided");
        assert_eq!(opens.len(), 1);
        assert_eq!(voided.len(), 1);
    }

    #[test]
    fn void_credit_memo_rejects_when_applied_and_guards_double_void() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let invoice_id = insert_invoice_for(&repo, cust, "INV-VCM", "1000");

        // A memo with an application cannot be voided.
        let memo = make_memo(&repo, cust, dec!(100), CreditMemoReason::ReturnedGoods);
        repo.apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id,
            amount: dec!(40),
        })
        .expect("apply");
        assert!(repo.void_credit_memo(memo.id).is_err(), "cannot void a memo with applications");

        // A fresh unapplied memo voids once; a second void is rejected, and
        // applying a voided memo is rejected.
        let memo2 = make_memo(&repo, cust, dec!(50), CreditMemoReason::ReturnedGoods);
        repo.void_credit_memo(memo2.id).expect("first void");
        assert!(repo.void_credit_memo(memo2.id).is_err(), "double void rejected");
        assert!(
            repo.apply_credit_memo(ApplyCreditMemo {
                credit_memo_id: memo2.id,
                invoice_id,
                amount: dec!(10),
            })
            .is_err(),
            "cannot apply a voided memo"
        );
    }

    #[test]
    fn void_credit_memo_changes_status() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let memo = make_memo(&repo, cust, dec!(15), CreditMemoReason::ReturnedGoods);
        let voided = repo.void_credit_memo(memo.id).expect("void");
        assert_eq!(voided.status, CreditMemoStatus::Voided);
    }

    #[test]
    fn get_unapplied_credits_filters_by_customer() {
        let repo = fresh_repo();
        let cust_a = Uuid::new_v4();
        let cust_b = Uuid::new_v4();
        make_memo(&repo, cust_a, dec!(40), CreditMemoReason::ReturnedGoods);
        make_memo(&repo, cust_b, dec!(50), CreditMemoReason::ReturnedGoods);

        let unapplied = repo.get_unapplied_credits(cust_a).expect("ok");
        assert_eq!(unapplied.len(), 1);
        assert_eq!(unapplied[0].customer_id, cust_a);
    }

    #[test]
    fn apply_credit_memo_to_nonexistent_invoice_errors() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let memo = make_memo(&repo, cust, dec!(10), CreditMemoReason::ReturnedGoods);
        let result = repo.apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id: Uuid::new_v4(),
            amount: dec!(5),
        });
        assert!(result.is_err(), "should fail without an invoice");
    }

    /// Insert an unpaid invoice for a specific customer and return its id, so
    /// credit-memo application (which requires the invoice/memo customers to
    /// match) can be exercised.
    fn insert_invoice_for(
        repo: &SqliteAccountsReceivableRepository,
        customer_id: Uuid,
        number: &str,
        balance_due: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let conn = repo.pool.get().expect("conn");
        let now = Utc::now();
        conn.execute(
            "INSERT INTO invoices (id, invoice_number, customer_id, status, invoice_date,
                due_date, total, amount_paid, balance_due, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'sent', ?4, ?4, ?5, '0', ?5, ?4, ?4)",
            params![id.to_string(), number, customer_id.to_string(), now.to_rfc3339(), balance_due],
        )
        .expect("insert invoice");
        id
    }

    #[test]
    fn aging_report_orders_ties_by_customer_id() {
        // Five customers with identical outstanding balances must come back in a
        // stable, unique order (by customer_id) — otherwise the report's order,
        // and the offset/limit pagination built on it, is non-deterministic.
        let repo = fresh_repo();
        let mut ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        for (i, id) in ids.iter().enumerate() {
            {
                let conn = repo.pool.get().expect("conn");
                conn.execute(
                    "INSERT INTO customers (id, email, first_name, last_name)
                     VALUES (?1, ?2, 'Test', 'Customer')",
                    params![id.to_string(), format!("tie-{i}@example.com")],
                )
                .expect("insert customer");
            }
            insert_invoice_for(&repo, *id, &format!("INV-TIE-{i}"), "100");
        }

        let report = repo.get_aging_report(ArAgingFilter::default()).expect("report");
        let returned: Vec<Uuid> = report.iter().map(|r| r.customer_id).collect();
        ids.sort();
        assert_eq!(
            returned, ids,
            "equal-balance customers must be ordered by ascending customer_id"
        );
    }

    #[test]
    fn aging_report_tolerates_invoice_with_missing_customer() {
        // An invoice can reference a customer that no longer exists (deleted).
        // The report LEFT JOINs customers, so that row's name/email come back
        // NULL — it must surface the balance with an empty name rather than
        // erroring out and taking down the whole report.
        let repo = fresh_repo();
        let orphan = Uuid::new_v4();
        insert_invoice_for(&repo, orphan, "INV-ORPHAN", "250");

        let report = repo
            .get_aging_report(ArAgingFilter::default())
            .expect("report must not crash on an invoice whose customer is missing");
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].customer_id, orphan);
        assert_eq!(report[0].customer_name, None, "missing customer must yield no name");
        assert_eq!(report[0].customer_email, None);
        assert_eq!(report[0].total_outstanding, dec!(250));
    }

    #[test]
    fn apply_credit_memo_is_atomic_under_concurrency() {
        use std::sync::{Arc, Barrier};

        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        // A memo with exactly 100 unapplied, against an invoice whose balance is
        // large enough that only the unapplied balance can bind.
        let memo = make_memo(&repo, cust, dec!(100), CreditMemoReason::ReturnedGoods);
        let invoice_id = insert_invoice_for(&repo, cust, "INV-RACE", "100000");

        const THREADS: usize = 10;
        let barrier = Arc::new(Barrier::new(THREADS));

        // All threads apply the full 100 at once. Before the fix (balance read
        // outside the transaction) several could validate against the stale
        // unapplied=100 and double-apply; now exactly one may win.
        let successes = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let repo = &repo;
                    s.spawn(move || {
                        barrier.wait();
                        repo.apply_credit_memo(ApplyCreditMemo {
                            credit_memo_id: memo.id,
                            invoice_id,
                            amount: dec!(100),
                        })
                        .is_ok()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).filter(|&ok| ok).count()
        });

        // Safety invariant: the memo can be applied AT MOST once (never
        // double-applied). Under extreme lock contention the sole winner can fail
        // transiently (the caller retries), so zero successes is acceptable; two
        // or more would be the double-apply bug this guards against.
        assert!(
            successes <= 1,
            "the 100-unit memo was applied more than once across {THREADS} threads"
        );
        let after = repo.get_credit_memo(memo.id).expect("get").expect("memo");
        assert_eq!(
            after.applied_amount,
            dec!(100) * Decimal::from(successes as u64),
            "applied_amount must reflect exactly the successful application"
        );
        assert_eq!(after.unapplied_amount, dec!(100) - after.applied_amount);
        let expected_status =
            if successes == 1 { CreditMemoStatus::FullyApplied } else { CreditMemoStatus::Open };
        assert_eq!(after.status, expected_status);
    }

    fn insert_payment(
        repo: &SqliteAccountsReceivableRepository,
        number: &str,
        amount: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let conn = repo.pool.get().expect("conn");
        let now = Utc::now();
        conn.execute(
            "INSERT INTO payments (id, payment_number, amount, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id.to_string(), number, amount, now.to_rfc3339()],
        )
        .expect("insert payment");
        id
    }

    #[test]
    fn apply_payment_to_invoice_is_atomic_under_concurrency() {
        use stateset_core::{ApplyPaymentToInvoices, PaymentApplicationLine};
        use std::sync::{Arc, Barrier};

        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        // Invoice with a balance of exactly 100.
        let invoice_id = insert_invoice_for(&repo, cust, "INV-PAY-RACE", "100");
        // One payment parent row (FK target); all attempts apply against it.
        let payment_id = insert_payment(&repo, "PAY-RACE", "1000");

        const THREADS: usize = 10;
        let barrier = Arc::new(Barrier::new(THREADS));

        // Each thread applies a distinct payment of the full 100 to the same
        // invoice. Only one may succeed; the rest must see the reduced balance.
        let successes = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let repo = &repo;
                    s.spawn(move || {
                        barrier.wait();
                        repo.apply_payment_to_invoices(ApplyPaymentToInvoices {
                            payment_id,
                            applications: vec![PaymentApplicationLine {
                                invoice_id,
                                amount: dec!(100),
                            }],
                        })
                        .map(|apps| !apps.is_empty())
                        .unwrap_or(false)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).filter(|&ok| ok).count()
        });

        // Safety invariant: at most one full payment can bind to the 100-balance
        // invoice (no double-apply). Under extreme lock contention the winner can
        // fail transiently (retryable), so zero is acceptable; two or more is the
        // bug this guards against.
        assert!(
            successes <= 1,
            "more than one full payment bound to the invoice across {THREADS} threads"
        );
        let balance: String = {
            let conn = repo.pool.get().expect("conn");
            conn.query_row(
                "SELECT balance_due FROM invoices WHERE id = ?1",
                params![invoice_id.to_string()],
                |r| r.get(0),
            )
            .expect("balance")
        };
        let balance = parse_decimal_safe(&balance, "invoice", "balance_due").expect("parse");
        assert_eq!(
            balance,
            dec!(100) - dec!(100) * Decimal::from(successes as u64),
            "invoice balance must reflect exactly the successful payment"
        );
    }

    #[test]
    fn write_off_is_atomic_and_guards_double_write_off() {
        use stateset_core::{CreateWriteOff, WriteOffReason};
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let invoice_id = insert_invoice_for(&repo, cust, "INV-WO", "100");

        let wo = repo
            .create_write_off(CreateWriteOff {
                invoice_id,
                amount: dec!(100),
                reason: WriteOffReason::Uncollectible,
                notes: None,
                approved_by: None,
            })
            .expect("first write-off");
        assert_eq!(wo.invoice_id, invoice_id);

        // Writing off an already-written-off invoice must be rejected.
        let second = repo.create_write_off(CreateWriteOff {
            invoice_id,
            amount: dec!(100),
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        });
        assert!(second.is_err(), "cannot write off an already-written-off invoice");
    }

    #[test]
    fn reverse_write_off_guards_double_reverse() {
        use stateset_core::{CreateWriteOff, WriteOffReason};
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let invoice_id = insert_invoice_for(&repo, cust, "INV-WO-REV", "100");

        let wo = repo
            .create_write_off(CreateWriteOff {
                invoice_id,
                amount: dec!(100),
                reason: WriteOffReason::Uncollectible,
                notes: None,
                approved_by: None,
            })
            .expect("write-off");

        repo.reverse_write_off(wo.id).expect("first reverse succeeds");
        let second = repo.reverse_write_off(wo.id);
        assert!(second.is_err(), "cannot reverse an already-reversed write-off");
    }

    #[test]
    fn apply_credit_memo_rejects_second_application_beyond_unapplied() {
        let repo = fresh_repo();
        let cust = Uuid::new_v4();
        let memo = make_memo(&repo, cust, dec!(100), CreditMemoReason::ReturnedGoods);
        let invoice_id = insert_invoice_for(&repo, cust, "INV-SEQ", "100000");

        repo.apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id,
            amount: dec!(100),
        })
        .expect("first application succeeds");

        // The memo is now fully applied; a second application must fail and must
        // not move the balance.
        let second = repo.apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id,
            amount: dec!(1),
        });
        assert!(second.is_err(), "cannot apply beyond the unapplied balance");
        let after = repo.get_credit_memo(memo.id).expect("get").expect("memo");
        assert_eq!(after.applied_amount, dec!(100));
        assert_eq!(after.unapplied_amount, dec!(0));
    }

    #[test]
    fn customer_summary_for_unknown_customer_is_none() {
        let repo = fresh_repo();
        assert!(repo.get_customer_summary(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn get_payment_applications_empty_for_unknown_payment() {
        let repo = fresh_repo();
        let apps = repo.get_payment_applications(Uuid::new_v4()).expect("ok");
        assert!(apps.is_empty());
    }

    #[test]
    fn get_invoices_due_for_dunning_empty_on_fresh_db() {
        let repo = fresh_repo();
        let invoices = repo.get_invoices_due_for_dunning().expect("ok");
        assert!(invoices.is_empty());
    }

    #[test]
    fn get_customers_batch_returns_only_existing() {
        let repo = fresh_repo();
        let result = repo.get_customers_batch(vec![Uuid::new_v4(), Uuid::new_v4()]).expect("ok");
        assert!(result.is_empty());
    }
}
