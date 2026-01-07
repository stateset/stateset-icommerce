//! SQLite implementation of Accounts Receivable repository

use crate::sqlite::{map_db_error, parse_decimal};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use stateset_core::{
    AccountsReceivableRepository, ApplyCreditMemo, ApplyPaymentToInvoices,
    ArAgingFilter, ArAgingSummary, ArPaymentApplication, CollectionActivity,
    CollectionActivityFilter, CollectionActivityType, CollectionStatus,
    CreateCollectionActivity, CreateCreditMemo, CreateWriteOff, CreditMemo,
    CreditMemoFilter, CreditMemoStatus, CustomerArAging, CustomerArSummary,
    CustomerStatement, DunningLetterType, GenerateStatementRequest, Invoice,
    Result, StatementLineItem, StatementTransactionType, WriteOff, WriteOffFilter,
    generate_credit_memo_number, generate_write_off_number,
};
use uuid::Uuid;

pub struct SqliteAccountsReceivableRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAccountsReceivableRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn map_collection_activity_row(row: &rusqlite::Row) -> rusqlite::Result<CollectionActivity> {
        Ok(CollectionActivity {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            invoice_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            activity_type: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            activity_date: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
            dunning_letter_type: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
            notes: row.get(6)?,
            contact_method: row.get(7)?,
            contact_result: row.get(8)?,
            promise_to_pay_date: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            promise_to_pay_amount: row.get::<_, Option<String>>(10)?.map(|s| parse_decimal(&s)),
            performed_by: row.get(11)?,
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_write_off_row(row: &rusqlite::Row) -> rusqlite::Result<WriteOff> {
        Ok(WriteOff {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            write_off_number: row.get(1)?,
            invoice_id: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            amount: parse_decimal(&row.get::<_, String>(4)?),
            reason: row.get::<_, String>(5)?.parse().unwrap_or_default(),
            notes: row.get(6)?,
            write_off_date: row.get::<_, String>(7)?.parse().unwrap_or_else(|_| Utc::now()),
            approved_by: row.get(8)?,
            approved_at: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            reversed_at: row.get::<_, Option<String>>(10)?.and_then(|s| s.parse().ok()),
            gl_journal_entry_id: row.get::<_, Option<String>>(11)?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_credit_memo_row(row: &rusqlite::Row) -> rusqlite::Result<CreditMemo> {
        Ok(CreditMemo {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            credit_memo_number: row.get(1)?,
            customer_id: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            original_invoice_id: row.get::<_, Option<String>>(3)?.and_then(|s| s.parse().ok()),
            reason: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            amount: parse_decimal(&row.get::<_, String>(5)?),
            applied_amount: parse_decimal(&row.get::<_, String>(6)?),
            unapplied_amount: parse_decimal(&row.get::<_, String>(7)?),
            status: row.get::<_, String>(8)?.parse().unwrap_or_default(),
            notes: row.get(9)?,
            issue_date: row.get::<_, String>(10)?.parse().unwrap_or_else(|_| Utc::now()),
            gl_journal_entry_id: row.get::<_, Option<String>>(11)?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(13)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_payment_application_row(row: &rusqlite::Row) -> rusqlite::Result<ArPaymentApplication> {
        Ok(ArPaymentApplication {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            payment_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
            invoice_id: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            applied_amount: parse_decimal(&row.get::<_, String>(3)?),
            applied_date: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
            created_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn get_invoice_customer_id(&self, invoice_id: Uuid) -> Result<Uuid> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let customer_id: String = conn.query_row(
            "SELECT customer_id FROM invoices WHERE id = ?1",
            params![invoice_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;
        Ok(customer_id.parse().unwrap_or_default())
    }

    fn recalculate_invoice(&self, invoice_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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

        let paid_dec = parse_decimal(&paid);
        let credits_dec = parse_decimal(&credits);
        let total_applied = paid_dec + credits_dec;

        // Get invoice total
        let total: String = conn.query_row(
            "SELECT total FROM invoices WHERE id = ?1",
            params![invoice_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        let total_dec = parse_decimal(&total);
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
        ).map_err(map_db_error)?;

        Ok(())
    }
}

impl AccountsReceivableRepository for SqliteAccountsReceivableRepository {
    fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (current, days_1_30, days_31_60, days_61_90, days_over_90): (String, String, String, String, String) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN due_date >= datetime('now') THEN CAST(balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < datetime('now') AND due_date >= datetime('now', '-30 days') THEN CAST(balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < datetime('now', '-30 days') AND due_date >= datetime('now', '-60 days') THEN CAST(balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < datetime('now', '-60 days') AND due_date >= datetime('now', '-90 days') THEN CAST(balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < datetime('now', '-90 days') THEN CAST(balance_due AS REAL) ELSE 0 END), 0)
            FROM invoices
            WHERE status NOT IN ('paid', 'voided', 'written_off')
              AND CAST(balance_due AS REAL) > 0",
            [],
            |row| Ok((
                row.get::<_, f64>(0)?.to_string(),
                row.get::<_, f64>(1)?.to_string(),
                row.get::<_, f64>(2)?.to_string(),
                row.get::<_, f64>(3)?.to_string(),
                row.get::<_, f64>(4)?.to_string(),
            )),
        ).map_err(map_db_error)?;

        let current_dec = parse_decimal(&current);
        let days_1_30_dec = parse_decimal(&days_1_30);
        let days_31_60_dec = parse_decimal(&days_31_60);
        let days_61_90_dec = parse_decimal(&days_61_90);
        let days_over_90_dec = parse_decimal(&days_over_90);

        Ok(ArAgingSummary {
            current: current_dec,
            days_1_30: days_1_30_dec,
            days_31_60: days_31_60_dec,
            days_61_90: days_61_90_dec,
            days_over_90: days_over_90_dec,
            total: current_dec + days_1_30_dec + days_31_60_dec + days_61_90_dec + days_over_90_dec,
            as_of_date: Utc::now(),
        })
    }

    fn get_customer_aging(&self, customer_id: Uuid) -> Result<CustomerArAging> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let row = conn.query_row(
            "SELECT
                i.customer_id,
                c.first_name || ' ' || c.last_name,
                c.email,
                COALESCE(SUM(CASE WHEN i.due_date >= datetime('now') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now') AND i.due_date >= datetime('now', '-30 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-30 days') AND i.due_date >= datetime('now', '-60 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-60 days') AND i.due_date >= datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0),
                COUNT(*),
                MIN(i.created_at)
            FROM invoices i
            LEFT JOIN customers c ON i.customer_id = c.id
            WHERE i.customer_id = ?1
              AND i.status NOT IN ('paid', 'voided', 'written_off')
              AND CAST(i.balance_due AS REAL) > 0
            GROUP BY i.customer_id",
            params![customer_id.to_string()],
            |row| {
                let current: f64 = row.get(3)?;
                let days_1_30: f64 = row.get(4)?;
                let days_31_60: f64 = row.get(5)?;
                let days_61_90: f64 = row.get(6)?;
                let days_over_90: f64 = row.get(7)?;

                Ok(CustomerArAging {
                    customer_id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    customer_name: row.get(1)?,
                    customer_email: row.get(2)?,
                    current: Decimal::from_f64_retain(current).unwrap_or_default(),
                    days_1_30: Decimal::from_f64_retain(days_1_30).unwrap_or_default(),
                    days_31_60: Decimal::from_f64_retain(days_31_60).unwrap_or_default(),
                    days_61_90: Decimal::from_f64_retain(days_61_90).unwrap_or_default(),
                    days_over_90: Decimal::from_f64_retain(days_over_90).unwrap_or_default(),
                    total_outstanding: Decimal::from_f64_retain(current + days_1_30 + days_31_60 + days_61_90 + days_over_90).unwrap_or_default(),
                    invoice_count: row.get(8)?,
                    oldest_invoice_date: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
                    last_payment_date: None,
                })
            },
        ).map_err(map_db_error)?;

        Ok(row)
    }

    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT
                i.customer_id,
                c.first_name || ' ' || c.last_name,
                c.email,
                COALESCE(SUM(CASE WHEN i.due_date >= datetime('now') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0) as current_amt,
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now') AND i.due_date >= datetime('now', '-30 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0) as days_1_30,
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-30 days') AND i.due_date >= datetime('now', '-60 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0) as days_31_60,
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-60 days') AND i.due_date >= datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0) as days_61_90,
                COALESCE(SUM(CASE WHEN i.due_date < datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END), 0) as days_over_90,
                COUNT(*) as invoice_count,
                MIN(i.created_at) as oldest
            FROM invoices i
            LEFT JOIN customers c ON i.customer_id = c.id
            WHERE i.status NOT IN ('paid', 'voided', 'written_off')
              AND CAST(i.balance_due AS REAL) > 0"
        );

        if let Some(cid) = &filter.customer_id {
            sql.push_str(&format!(" AND i.customer_id = '{}'", cid));
        }

        sql.push_str(" GROUP BY i.customer_id");

        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" HAVING (days_1_30 + days_31_60 + days_61_90 + days_over_90) > 0");
        }

        sql.push_str(" ORDER BY (current_amt + days_1_30 + days_31_60 + days_61_90 + days_over_90) DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map([], |row| {
            let current: f64 = row.get(3)?;
            let days_1_30: f64 = row.get(4)?;
            let days_31_60: f64 = row.get(5)?;
            let days_61_90: f64 = row.get(6)?;
            let days_over_90: f64 = row.get(7)?;

            Ok(CustomerArAging {
                customer_id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                customer_name: row.get(1)?,
                customer_email: row.get(2)?,
                current: Decimal::from_f64_retain(current).unwrap_or_default(),
                days_1_30: Decimal::from_f64_retain(days_1_30).unwrap_or_default(),
                days_31_60: Decimal::from_f64_retain(days_31_60).unwrap_or_default(),
                days_61_90: Decimal::from_f64_retain(days_61_90).unwrap_or_default(),
                days_over_90: Decimal::from_f64_retain(days_over_90).unwrap_or_default(),
                total_outstanding: Decimal::from_f64_retain(current + days_1_30 + days_31_60 + days_61_90 + days_over_90).unwrap_or_default(),
                invoice_count: row.get(8)?,
                oldest_invoice_date: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
                last_payment_date: None,
            })
        }).map_err(map_db_error)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn log_collection_activity(&self, input: CreateCollectionActivity) -> Result<CollectionActivity> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let customer_id = self.get_invoice_customer_id(input.invoice_id)?;

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

    fn list_collection_activities(&self, filter: CollectionActivityFilter) -> Result<Vec<CollectionActivity>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, invoice_id, customer_id, activity_type, activity_date, dunning_letter_type, notes, contact_method, contact_result, promise_to_pay_date, promise_to_pay_amount, performed_by, created_at
             FROM ar_collection_activities WHERE 1=1"
        );

        if let Some(inv_id) = &filter.invoice_id {
            sql.push_str(&format!(" AND invoice_id = '{}'", inv_id));
        }
        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(&format!(" AND customer_id = '{}'", cust_id));
        }
        if let Some(atype) = &filter.activity_type {
            sql.push_str(&format!(" AND activity_type = '{}'", atype));
        }

        sql.push_str(" ORDER BY activity_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map([], Self::map_collection_activity_row).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn update_collection_status(&self, invoice_id: Uuid, status: CollectionStatus) -> Result<()> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE invoices SET collection_status = ?1 WHERE id = ?2",
            params![status.to_string(), invoice_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        // Return invoices that are overdue and haven't had recent dunning
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, invoice_number, order_id, customer_id, status, issue_date, due_date, subtotal, tax, shipping, discount, total, amount_paid, balance_due, currency, notes, terms, created_at, updated_at
             FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')
               AND CAST(balance_due AS REAL) > 0
               AND due_date < datetime('now')
               AND (last_dunning_date IS NULL OR last_dunning_date < datetime('now', '-7 days'))
             ORDER BY due_date ASC"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map([], |row| {
            Ok(Invoice {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                invoice_number: row.get(1)?,
                order_id: row.get::<_, Option<String>>(2)?.and_then(|s| s.parse().ok()),
                customer_id: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                status: row.get::<_, String>(4)?.parse().unwrap_or_default(),
                invoice_type: stateset_core::InvoiceType::Standard,
                invoice_date: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                due_date: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
                payment_terms: None,
                subtotal: parse_decimal(&row.get::<_, String>(7)?),
                tax_amount: parse_decimal(&row.get::<_, String>(8)?),
                tax_rate: None,
                shipping_amount: parse_decimal(&row.get::<_, String>(9)?),
                discount_amount: parse_decimal(&row.get::<_, String>(10)?),
                discount_percent: None,
                total: parse_decimal(&row.get::<_, String>(11)?),
                amount_paid: parse_decimal(&row.get::<_, String>(12)?),
                balance_due: parse_decimal(&row.get::<_, String>(13)?),
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
                created_at: row.get::<_, String>(17)?.parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<_, String>(18)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        }).map_err(map_db_error)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn send_dunning_letter(&self, invoice_id: Uuid, letter_type: DunningLetterType, sent_by: Option<&str>) -> Result<CollectionActivity> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
            DunningLetterType::DemandLetter | DunningLetterType::CollectionNotice => CollectionStatus::InCollections,
        };

        self.update_collection_status(invoice_id, new_status)?;

        // Log the activity
        self.log_collection_activity(CreateCollectionActivity {
            invoice_id,
            activity_type: CollectionActivityType::DunningLetterSent,
            dunning_letter_type: Some(letter_type),
            notes: Some(format!("Sent {} dunning letter", letter_type)),
            performed_by: sent_by.map(|s| s.to_string()),
            ..Default::default()
        })
    }

    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let write_off_number = generate_write_off_number();
        let customer_id = self.get_invoice_customer_id(input.invoice_id)?;

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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, write_off_number, invoice_id, customer_id, amount, reason, notes, write_off_date, approved_by, approved_at, reversed_at, gl_journal_entry_id, created_at
             FROM ar_write_offs WHERE 1=1"
        );

        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(&format!(" AND customer_id = '{}'", cust_id));
        }
        if let Some(inv_id) = &filter.invoice_id {
            sql.push_str(&format!(" AND invoice_id = '{}'", inv_id));
        }
        if !filter.include_reversed.unwrap_or(false) {
            sql.push_str(" AND reversed_at IS NULL");
        }

        sql.push_str(" ORDER BY write_off_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map([], Self::map_write_off_row).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();

        // Get the write-off
        let wo = self.get_write_off(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if wo.reversed_at.is_some() {
            return Err(stateset_core::CommerceError::ValidationError("Write-off already reversed".into()));
        }

        // Mark as reversed
        conn.execute(
            "UPDATE ar_write_offs SET reversed_at = ?1 WHERE id = ?2",
            params![now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        // Restore invoice status
        conn.execute(
            "UPDATE invoices SET status = 'overdue', collection_status = 'none' WHERE id = ?1",
            params![wo.invoice_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(WriteOff {
            reversed_at: Some(now),
            ..wo
        })
    }

    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount, applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id, created_at, updated_at
             FROM ar_credit_memos WHERE 1=1"
        );

        if let Some(cust_id) = &filter.customer_id {
            sql.push_str(&format!(" AND customer_id = '{}'", cust_id));
        }
        if let Some(status) = &filter.status {
            sql.push_str(&format!(" AND status = '{}'", status));
        }
        if filter.has_unapplied.unwrap_or(false) {
            sql.push_str(" AND CAST(unapplied_amount AS REAL) > 0");
        }

        sql.push_str(" ORDER BY issue_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map([], Self::map_credit_memo_row).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let cm = self.get_credit_memo(input.credit_memo_id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if !cm.can_apply() {
            return Err(stateset_core::CommerceError::ValidationError("Credit memo cannot be applied".into()));
        }

        if input.amount > cm.unapplied_amount {
            return Err(stateset_core::CommerceError::ValidationError("Amount exceeds unapplied balance".into()));
        }

        let now = Utc::now();
        let app_id = Uuid::new_v4();

        // Create application record
        conn.execute(
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

        conn.execute(
            "UPDATE ar_credit_memos SET applied_amount = ?1, unapplied_amount = ?2, status = ?3 WHERE id = ?4",
            params![
                new_applied.to_string(),
                new_unapplied.to_string(),
                new_status.to_string(),
                input.credit_memo_id.to_string()
            ],
        ).map_err(map_db_error)?;

        // Recalculate invoice
        self.recalculate_invoice(input.invoice_id)?;

        Ok(CreditMemo {
            applied_amount: new_applied,
            unapplied_amount: new_unapplied,
            status: new_status,
            updated_at: now,
            ..cm
        })
    }

    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let cm = self.get_credit_memo(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if cm.applied_amount > Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError("Cannot void credit memo with applications".into()));
        }

        conn.execute(
            "UPDATE ar_credit_memos SET status = 'voided' WHERE id = ?1",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        Ok(CreditMemo {
            status: CreditMemoStatus::Voided,
            updated_at: Utc::now(),
            ..cm
        })
    }

    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer_id),
            has_unapplied: Some(true),
            ..Default::default()
        })
    }

    fn apply_payment_to_invoices(&self, input: ApplyPaymentToInvoices) -> Result<Vec<ArPaymentApplication>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();
        let mut applications = Vec::new();

        for app in input.applications {
            let app_id = Uuid::new_v4();

            conn.execute(
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
            self.recalculate_invoice(app.invoice_id)?;

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
    }

    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, payment_id, invoice_id, applied_amount, applied_date, created_at
             FROM ar_payment_applications WHERE payment_id = ?1"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map(params![payment_id.to_string()], Self::map_payment_application_row).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Get invoice_id first
        let invoice_id: String = conn.query_row(
            "SELECT invoice_id FROM ar_payment_applications WHERE id = ?1",
            params![application_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        // Delete application
        conn.execute(
            "DELETE FROM ar_payment_applications WHERE id = ?1",
            params![application_id.to_string()],
        ).map_err(map_db_error)?;

        // Recalculate invoice
        self.recalculate_invoice(invoice_id.parse().unwrap_or_default())?;

        Ok(())
    }

    fn get_customer_summary(&self, customer_id: Uuid) -> Result<CustomerArSummary> {
        let aging = self.get_customer_aging(customer_id)?;

        // Get unapplied credits
        let unapplied_credits = self.get_unapplied_credits(customer_id)?
            .iter()
            .map(|cm| cm.unapplied_amount)
            .sum();

        let total_overdue = aging.total_overdue();
        Ok(CustomerArSummary {
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
        })
    }

    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();
        let period_start = request.period_start.unwrap_or_else(|| now - chrono::Duration::days(30));
        let period_end = request.period_end.unwrap_or(now);

        // Get customer info
        let (customer_name, customer_email): (String, Option<String>) = conn.query_row(
            "SELECT first_name || ' ' || last_name, email FROM customers WHERE id = ?1",
            params![request.customer_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        // Get aging
        let aging = self.get_customer_aging(request.customer_id)?;

        // Build line items from invoices and payments
        let mut line_items: Vec<StatementLineItem> = Vec::new();
        let mut running_balance = Decimal::ZERO;

        // Get invoices
        let mut stmt = conn.prepare(
            "SELECT created_at, invoice_number, total FROM invoices
             WHERE customer_id = ?1 AND created_at >= ?2 AND created_at <= ?3
             ORDER BY created_at"
        ).map_err(map_db_error)?;

        let inv_rows = stmt.query_map(
            params![request.customer_id.to_string(), period_start.to_rfc3339(), period_end.to_rfc3339()],
            |row| {
                let date: String = row.get(0)?;
                let number: String = row.get(1)?;
                let total: String = row.get(2)?;
                Ok((date, number, total))
            }
        ).map_err(map_db_error)?;

        for inv in inv_rows {
            let (date, number, total) = inv.map_err(map_db_error)?;
            let amount = parse_decimal(&total);
            running_balance += amount;
            line_items.push(StatementLineItem {
                date: date.parse().unwrap_or(now),
                transaction_type: StatementTransactionType::Invoice,
                reference_number: number,
                description: "Invoice".into(),
                debit: Some(amount),
                credit: None,
                balance: running_balance,
            });
        }

        // Get payments
        let mut stmt = conn.prepare(
            "SELECT pa.applied_date, p.id, pa.applied_amount
             FROM ar_payment_applications pa
             JOIN payments p ON pa.payment_id = p.id
             JOIN invoices i ON pa.invoice_id = i.id
             WHERE i.customer_id = ?1 AND pa.applied_date >= ?2 AND pa.applied_date <= ?3
             ORDER BY pa.applied_date"
        ).map_err(map_db_error)?;

        let pay_rows = stmt.query_map(
            params![request.customer_id.to_string(), period_start.to_rfc3339(), period_end.to_rfc3339()],
            |row| {
                let date: String = row.get(0)?;
                let id: String = row.get(1)?;
                let amount: String = row.get(2)?;
                Ok((date, id, amount))
            }
        ).map_err(map_db_error)?;

        for pay in pay_rows {
            let (date, id, amount_str) = pay.map_err(map_db_error)?;
            let amount = parse_decimal(&amount_str);
            running_balance -= amount;
            line_items.push(StatementLineItem {
                date: date.parse().unwrap_or(now),
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
        let total_invoices: Decimal = line_items.iter()
            .filter(|l| matches!(l.transaction_type, StatementTransactionType::Invoice))
            .filter_map(|l| l.debit)
            .sum();
        let total_payments: Decimal = line_items.iter()
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
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // DSO = (Accounts Receivable / Total Credit Sales) × Number of Days
        let ar_balance: f64 = conn.query_row(
            "SELECT COALESCE(SUM(CAST(balance_due AS REAL)), 0) FROM invoices WHERE status NOT IN ('paid', 'voided', 'written_off')",
            [],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        let total_sales: f64 = conn.query_row(
            &format!("SELECT COALESCE(SUM(CAST(total AS REAL)), 0) FROM invoices WHERE created_at >= datetime('now', '-{} days')", days),
            [],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        if total_sales == 0.0 {
            return Ok(Decimal::ZERO);
        }

        let dso = (ar_balance / total_sales) * days as f64;
        Ok(Decimal::from_f64_retain(dso).unwrap_or_default())
    }

    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        ids.into_iter().map(|id| self.get_customer_summary(id)).collect()
    }
}
