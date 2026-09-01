//! PostgreSQL implementation of Accounts Receivable repository

use super::{block_on, map_db_error};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AccountsReceivableRepository, ApplyCreditMemo, ApplyPaymentToInvoices, ArAgingFilter,
    ArAgingSummary, ArPaymentApplication, CollectionActivity, CollectionActivityFilter,
    CollectionActivityType, CollectionStatus, CommerceError, CreateCollectionActivity,
    CreateCreditMemo, CreateWriteOff, CreditMemo, CreditMemoFilter, CreditMemoReason,
    CreditMemoStatus, CurrencyCode, CustomerArAging, CustomerArSummary, CustomerStatement,
    DunningLetterType, GenerateStatementRequest, Invoice, InvoiceId, InvoiceStatus, InvoiceType,
    Result, StatementLineItem, StatementTransactionType, WriteOff, WriteOffFilter, WriteOffReason,
    generate_credit_memo_number, generate_write_off_number,
};
use uuid::Uuid;

/// One statement-visible event: (date, type, reference, description, amount).
type PgStatementActivityRow = (DateTime<Utc>, StatementTransactionType, String, String, Decimal);

#[derive(Debug, Clone)]

pub struct PgAccountsReceivableRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct CollectionActivityRow {
    id: Uuid,
    invoice_id: Uuid,
    customer_id: Uuid,
    activity_type: String,
    activity_date: DateTime<Utc>,
    dunning_letter_type: Option<String>,
    notes: Option<String>,
    contact_method: Option<String>,
    contact_result: Option<String>,
    promise_to_pay_date: Option<DateTime<Utc>>,
    promise_to_pay_amount: Option<Decimal>,
    performed_by: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct WriteOffRow {
    id: Uuid,
    write_off_number: String,
    invoice_id: Uuid,
    customer_id: Uuid,
    amount: Decimal,
    reason: String,
    notes: Option<String>,
    write_off_date: NaiveDate,
    approved_by: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    reversed_at: Option<DateTime<Utc>>,
    gl_journal_entry_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CreditMemoRow {
    id: Uuid,
    credit_memo_number: String,
    customer_id: Uuid,
    original_invoice_id: Option<Uuid>,
    reason: String,
    amount: Decimal,
    applied_amount: Decimal,
    unapplied_amount: Decimal,
    status: String,
    notes: Option<String>,
    issue_date: NaiveDate,
    gl_journal_entry_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentApplicationRow {
    id: Uuid,
    payment_id: Uuid,
    invoice_id: Uuid,
    applied_amount: Decimal,
    applied_date: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CustomerAgingRow {
    customer_id: Uuid,
    customer_name: Option<String>,
    customer_email: Option<String>,
    current_amount: Decimal,
    days_1_30: Decimal,
    days_31_60: Decimal,
    days_61_90: Decimal,
    days_over_90: Decimal,
    total_outstanding: Decimal,
    invoice_count: i64,
    oldest_invoice_date: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct InvoiceRow {
    id: Uuid,
    invoice_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    status: String,
    invoice_type: String,
    invoice_date: DateTime<Utc>,
    due_date: DateTime<Utc>,
    payment_terms: Option<String>,
    currency: CurrencyCode,
    billing_name: Option<String>,
    billing_email: Option<String>,
    billing_address: Option<String>,
    billing_city: Option<String>,
    billing_state: Option<String>,
    billing_postal_code: Option<String>,
    billing_country: Option<String>,
    subtotal: Decimal,
    discount_amount: Decimal,
    discount_percent: Option<Decimal>,
    tax_amount: Decimal,
    tax_rate: Option<Decimal>,
    shipping_amount: Decimal,
    total: Decimal,
    amount_paid: Decimal,
    balance_due: Decimal,
    po_number: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    footer: Option<String>,
    sent_at: Option<DateTime<Utc>>,
    viewed_at: Option<DateTime<Utc>>,
    paid_at: Option<DateTime<Utc>>,
    voided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgAccountsReceivableRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_collection_activity(row: CollectionActivityRow) -> Result<CollectionActivity> {
        let CollectionActivityRow {
            id,
            invoice_id,
            customer_id,
            activity_type,
            activity_date,
            dunning_letter_type,
            notes,
            contact_method,
            contact_result,
            promise_to_pay_date,
            promise_to_pay_amount,
            performed_by,
            created_at,
        } = row;

        let activity_type: CollectionActivityType = activity_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid collection_activity.activity_type '{}': {}",
                activity_type, e
            ))
        })?;
        let dunning_letter_type = match dunning_letter_type {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid collection_activity.dunning_letter_type '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(CollectionActivity {
            id,
            invoice_id,
            customer_id,
            activity_type,
            activity_date,
            dunning_letter_type,
            notes,
            contact_method,
            contact_result,
            promise_to_pay_date,
            promise_to_pay_amount,
            performed_by,
            created_at,
        })
    }

    fn row_to_write_off(row: WriteOffRow) -> Result<WriteOff> {
        let WriteOffRow {
            id,
            write_off_number,
            invoice_id,
            customer_id,
            amount,
            reason,
            notes,
            write_off_date,
            approved_by,
            approved_at,
            reversed_at,
            gl_journal_entry_id,
            created_at,
        } = row;

        let reason: WriteOffReason = reason.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid write_off.reason '{}': {}", reason, e))
        })?;

        Ok(WriteOff {
            id,
            write_off_number,
            invoice_id,
            customer_id,
            amount,
            reason,
            notes,
            write_off_date: from_date(write_off_date),
            approved_by,
            approved_at,
            reversed_at,
            gl_journal_entry_id,
            created_at,
        })
    }

    fn row_to_credit_memo(row: CreditMemoRow) -> Result<CreditMemo> {
        let CreditMemoRow {
            id,
            credit_memo_number,
            customer_id,
            original_invoice_id,
            reason,
            amount,
            applied_amount,
            unapplied_amount,
            status,
            notes,
            issue_date,
            gl_journal_entry_id,
            created_at,
            updated_at,
        } = row;

        let reason: CreditMemoReason = reason.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid credit_memo.reason '{}': {}", reason, e))
        })?;
        let status: CreditMemoStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid credit_memo.status '{}': {}", status, e))
        })?;

        Ok(CreditMemo {
            id,
            credit_memo_number,
            customer_id,
            original_invoice_id,
            reason,
            amount,
            applied_amount,
            unapplied_amount,
            status,
            notes,
            issue_date: from_date(issue_date),
            gl_journal_entry_id,
            created_at,
            updated_at,
        })
    }

    const fn row_to_payment_application(row: PaymentApplicationRow) -> ArPaymentApplication {
        ArPaymentApplication {
            id: row.id,
            payment_id: row.payment_id,
            invoice_id: row.invoice_id,
            applied_amount: row.applied_amount,
            applied_date: row.applied_date,
            created_at: row.created_at,
        }
    }

    fn row_to_invoice(row: InvoiceRow) -> Result<Invoice> {
        let InvoiceRow {
            id,
            invoice_number,
            customer_id,
            order_id,
            status,
            invoice_type,
            invoice_date,
            due_date,
            payment_terms,
            currency,
            billing_name,
            billing_email,
            billing_address,
            billing_city,
            billing_state,
            billing_postal_code,
            billing_country,
            subtotal,
            discount_amount,
            discount_percent,
            tax_amount,
            tax_rate,
            shipping_amount,
            total,
            amount_paid,
            balance_due,
            po_number,
            notes,
            terms,
            footer,
            sent_at,
            viewed_at,
            paid_at,
            voided_at,
            created_at,
            updated_at,
        } = row;

        let status: InvoiceStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid invoice.status '{}': {}", status, e))
        })?;
        let invoice_type: InvoiceType = invoice_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid invoice.invoice_type '{}': {}",
                invoice_type, e
            ))
        })?;

        Ok(Invoice {
            id: id.into(),
            invoice_number,
            customer_id: customer_id.into(),
            order_id: order_id.map(Into::into),
            status,
            invoice_type,
            invoice_date,
            due_date,
            payment_terms,
            currency,
            billing_name,
            billing_email,
            billing_address,
            billing_city,
            billing_state,
            billing_postal_code,
            billing_country,
            subtotal,
            discount_amount,
            discount_percent,
            tax_amount,
            tax_rate,
            shipping_amount,
            total,
            amount_paid,
            balance_due,
            po_number,
            notes,
            terms,
            footer,
            sent_at,
            viewed_at,
            paid_at,
            voided_at,
            items: Vec::new(),
            created_at,
            updated_at,
        })
    }

    async fn get_invoice_customer_id_async(&self, invoice_id: Uuid) -> Result<Uuid> {
        let customer_id: Uuid =
            sqlx::query_scalar("SELECT customer_id FROM invoices WHERE id = $1")
                .bind(invoice_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;
        Ok(customer_id)
    }

    async fn recalculate_invoice_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        invoice_id: Uuid,
    ) -> Result<()> {
        let paid: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(applied_amount), 0) FROM ar_payment_applications WHERE invoice_id = $1",
        )
        .bind(invoice_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let credits: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(applied_amount), 0) FROM ar_credit_memo_applications WHERE invoice_id = $1",
        )
        .bind(invoice_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Direct payments (record_payment) have no ar_payment_applications
        // row; without adding direct_amount_paid back here, this recalculation
        // would REPLACE amount_paid with the application sums and erase them.
        // Read it inside the transaction (FOR UPDATE) so a concurrent
        // record_payment serializes against this recalculation.
        let (total, direct): (Decimal, Decimal) = sqlx::query_as(
            "SELECT total, direct_amount_paid FROM invoices WHERE id = $1 FOR UPDATE",
        )
        .bind(invoice_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let total_applied = direct + paid + credits;
        let balance_due = total - total_applied;
        let status = if balance_due <= Decimal::ZERO {
            "paid"
        } else if total_applied > Decimal::ZERO {
            "partially_paid"
        } else {
            "sent"
        };

        sqlx::query(
            "UPDATE invoices SET amount_paid = $1, balance_due = $2, status = $3 WHERE id = $4",
        )
        .bind(total_applied)
        .bind(balance_due)
        .bind(status)
        .bind(invoice_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_aging_summary_async(&self) -> Result<ArAgingSummary> {
        let (current, days_1_30, days_31_60, days_61_90, days_over_90): (
            Decimal,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
        ) = sqlx::query_as(
            "SELECT
                COALESCE(SUM(CASE WHEN due_date >= NOW() THEN balance_due ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < NOW() AND due_date >= NOW() - INTERVAL '30 days' THEN balance_due ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '30 days' AND due_date >= NOW() - INTERVAL '60 days' THEN balance_due ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '60 days' AND due_date >= NOW() - INTERVAL '90 days' THEN balance_due ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '90 days' THEN balance_due ELSE 0 END), 0)
             FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')
               AND balance_due > 0",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ArAgingSummary {
            current,
            days_1_30,
            days_31_60,
            days_61_90,
            days_over_90,
            total: current + days_1_30 + days_31_60 + days_61_90 + days_over_90,
            as_of_date: Utc::now(),
        })
    }

    pub async fn get_customer_aging_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerArAging>> {
        let customer_row: Option<(String, String, String)> =
            sqlx::query_as("SELECT first_name, last_name, email FROM customers WHERE id = $1")
                .bind(customer_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        let (first_name, last_name, email) = match customer_row {
            Some(row) => row,
            None => return Ok(None),
        };

        let (current, days_1_30, days_31_60, days_61_90, days_over_90, total_outstanding, invoice_count, oldest_invoice_date): (Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, i64, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT
                COALESCE(SUM(CASE WHEN due_date >= NOW() THEN balance_due ELSE 0 END), 0) AS current_amount,
                COALESCE(SUM(CASE WHEN due_date < NOW() AND due_date >= NOW() - INTERVAL '30 days' THEN balance_due ELSE 0 END), 0) AS days_1_30,
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '30 days' AND due_date >= NOW() - INTERVAL '60 days' THEN balance_due ELSE 0 END), 0) AS days_31_60,
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '60 days' AND due_date >= NOW() - INTERVAL '90 days' THEN balance_due ELSE 0 END), 0) AS days_61_90,
                COALESCE(SUM(CASE WHEN due_date < NOW() - INTERVAL '90 days' THEN balance_due ELSE 0 END), 0) AS days_over_90,
                COALESCE(SUM(balance_due), 0) AS total_outstanding,
                COUNT(*) AS invoice_count,
                MIN(created_at) AS oldest_invoice_date
             FROM invoices
             WHERE customer_id = $1
               AND status NOT IN ('paid', 'voided', 'written_off')
               AND balance_due > 0",
        )
        .bind(customer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Some(CustomerArAging {
            customer_id,
            customer_name: Some(format!("{} {}", first_name, last_name)),
            customer_email: Some(email),
            current,
            days_1_30,
            days_31_60,
            days_61_90,
            days_over_90,
            total_outstanding,
            invoice_count: invoice_count as i32,
            oldest_invoice_date,
            last_payment_date: None,
        }))
    }

    pub async fn get_aging_report_async(
        &self,
        filter: ArAgingFilter,
    ) -> Result<Vec<CustomerArAging>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT
                i.customer_id,
                c.first_name || ' ' || c.last_name AS customer_name,
                c.email AS customer_email,
                COALESCE(SUM(CASE WHEN i.due_date >= NOW() THEN i.balance_due ELSE 0 END), 0) AS current_amount,
                COALESCE(SUM(CASE WHEN i.due_date < NOW() AND i.due_date >= NOW() - INTERVAL '30 days' THEN i.balance_due ELSE 0 END), 0) AS days_1_30,
                COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '30 days' AND i.due_date >= NOW() - INTERVAL '60 days' THEN i.balance_due ELSE 0 END), 0) AS days_31_60,
                COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '60 days' AND i.due_date >= NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0) AS days_61_90,
                COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0) AS days_over_90,
                COALESCE(SUM(i.balance_due), 0) AS total_outstanding,
                COUNT(*) AS invoice_count,
                MIN(i.created_at) AS oldest_invoice_date
             FROM invoices i
             LEFT JOIN customers c ON i.customer_id = c.id
             WHERE i.status NOT IN ('paid', 'voided', 'written_off')
               AND i.balance_due > 0",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND i.customer_id = ").push_bind(customer_id);
        }

        builder.push(" GROUP BY i.customer_id, c.first_name, c.last_name, c.email");

        let mut has_having = false;
        if let Some(min_balance) = filter.min_balance {
            builder.push(" HAVING COALESCE(SUM(i.balance_due), 0) >= ").push_bind(min_balance);
            has_having = true;
        }
        if filter.overdue_only.unwrap_or(false) {
            builder.push(if has_having { " AND " } else { " HAVING " });
            builder.push(
                "COALESCE(SUM(CASE WHEN i.due_date < NOW() AND i.due_date >= NOW() - INTERVAL '30 days' THEN i.balance_due ELSE 0 END), 0)
                 + COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '30 days' AND i.due_date >= NOW() - INTERVAL '60 days' THEN i.balance_due ELSE 0 END), 0)
                 + COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '60 days' AND i.due_date >= NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0)
                 + COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0) > 0",
            );
            has_having = true;
        }
        if let Some(bucket) = filter.aging_bucket {
            builder.push(if has_having { " AND " } else { " HAVING " });
            let condition = match bucket {
                stateset_core::AgingBucket::Current => {
                    "COALESCE(SUM(CASE WHEN i.due_date >= NOW() THEN i.balance_due ELSE 0 END), 0) > 0"
                }
                stateset_core::AgingBucket::Days1To30 => {
                    "COALESCE(SUM(CASE WHEN i.due_date < NOW() AND i.due_date >= NOW() - INTERVAL '30 days' THEN i.balance_due ELSE 0 END), 0) > 0"
                }
                stateset_core::AgingBucket::Days31To60 => {
                    "COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '30 days' AND i.due_date >= NOW() - INTERVAL '60 days' THEN i.balance_due ELSE 0 END), 0) > 0"
                }
                stateset_core::AgingBucket::Days61To90 => {
                    "COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '60 days' AND i.due_date >= NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0) > 0"
                }
                stateset_core::AgingBucket::DaysOver90 => {
                    "COALESCE(SUM(CASE WHEN i.due_date < NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END), 0) > 0"
                }
                _ => "1=1",
            };
            builder.push(condition);
        }

        // customer_id is a total-order tiebreaker so equal balances have a
        // stable, unique ordering — otherwise LIMIT/OFFSET pagination can skip or
        // duplicate rows (matches the SQLite backend's sort).
        builder.push(" ORDER BY total_outstanding DESC, i.customer_id ASC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CustomerAgingRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(|row| CustomerArAging {
                customer_id: row.customer_id,
                customer_name: row.customer_name,
                customer_email: row.customer_email,
                current: row.current_amount,
                days_1_30: row.days_1_30,
                days_31_60: row.days_31_60,
                days_61_90: row.days_61_90,
                days_over_90: row.days_over_90,
                total_outstanding: row.total_outstanding,
                invoice_count: row.invoice_count as i32,
                oldest_invoice_date: row.oldest_invoice_date,
                last_payment_date: None,
            })
            .collect())
    }

    pub async fn log_collection_activity_async(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let customer_id = self.get_invoice_customer_id_async(input.invoice_id).await?;

        sqlx::query(
            "INSERT INTO ar_collection_activities (id, invoice_id, customer_id, activity_type, activity_date,
                dunning_letter_type, notes, contact_method, contact_result, promise_to_pay_date,
                promise_to_pay_amount, performed_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(id)
        .bind(input.invoice_id)
        .bind(customer_id)
        .bind(input.activity_type.to_string())
        .bind(now)
        .bind(input.dunning_letter_type.map(|d| d.to_string()))
        .bind(input.notes.clone())
        .bind(input.contact_method.clone())
        .bind(input.contact_result.clone())
        .bind(input.promise_to_pay_date)
        .bind(input.promise_to_pay_amount)
        .bind(input.performed_by.clone())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

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

    pub async fn list_collection_activities_async(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, invoice_id, customer_id, activity_type, activity_date, dunning_letter_type,
                    notes, contact_method, contact_result, promise_to_pay_date, promise_to_pay_amount,
                    performed_by, created_at
             FROM ar_collection_activities WHERE 1=1",
        );

        if let Some(invoice_id) = filter.invoice_id {
            builder.push(" AND invoice_id = ").push_bind(invoice_id);
        }
        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(activity_type) = filter.activity_type {
            builder.push(" AND activity_type = ").push_bind(activity_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND activity_date >= ").push_bind(from_date);
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND activity_date <= ").push_bind(to_date_val);
        }

        builder.push(" ORDER BY activity_date DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CollectionActivityRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_collection_activity).collect::<Result<Vec<_>>>()
    }

    pub async fn update_collection_status_async(
        &self,
        invoice_id: Uuid,
        status: CollectionStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE invoices SET collection_status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(invoice_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_invoices_due_for_dunning_async(&self) -> Result<Vec<Invoice>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT id, invoice_number, customer_id, order_id, status, invoice_type, invoice_date,
                    due_date, payment_terms, currency, billing_name, billing_email, billing_address,
                    billing_city, billing_state, billing_postal_code, billing_country, subtotal,
                    discount_amount, discount_percent, tax_amount, tax_rate, shipping_amount, total,
                    amount_paid, balance_due, po_number, notes, terms, footer, sent_at, viewed_at,
                    paid_at, voided_at, created_at, updated_at
             FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')
               AND balance_due > 0
               AND due_date < NOW()
               AND (last_dunning_date IS NULL OR last_dunning_date < NOW() - INTERVAL '7 days')
             ORDER BY due_date ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_invoice).collect::<Result<Vec<_>>>()
    }

    pub async fn send_dunning_letter_async(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        sqlx::query(
            "UPDATE invoices SET last_dunning_date = NOW(), dunning_count = COALESCE(dunning_count, 0) + 1
             WHERE id = $1",
        )
        .bind(invoice_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let new_status = match letter_type {
            DunningLetterType::Reminder1 => CollectionStatus::Reminder1Sent,
            DunningLetterType::Reminder2 => CollectionStatus::Reminder2Sent,
            DunningLetterType::Reminder3 => CollectionStatus::Reminder3Sent,
            DunningLetterType::DemandLetter | DunningLetterType::CollectionNotice => {
                CollectionStatus::InCollections
            }
            _ => CollectionStatus::InCollections,
        };

        self.update_collection_status_async(invoice_id, new_status).await?;

        self.log_collection_activity_async(CreateCollectionActivity {
            invoice_id,
            activity_type: CollectionActivityType::DunningLetterSent,
            dunning_letter_type: Some(letter_type),
            notes: Some(format!("Sent {} dunning letter", letter_type)),
            performed_by: sent_by.map(|s| s.to_string()),
            ..Default::default()
        })
        .await
    }

    pub async fn create_write_off_async(&self, input: CreateWriteOff) -> Result<WriteOff> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Write-off amount must be greater than zero".into(),
            ));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let write_off_number = generate_write_off_number();

        let approved_at = input.approved_by.as_ref().map(|_| now);

        // Atomic + guarded: mark the invoice written-off and insert the write-off
        // in one transaction. Was two autocommit statements on the pool — no
        // atomicity (a failure between them left them inconsistent) and no guard
        // against double write-off.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Read the invoice under the row lock: a voided/cancelled invoice cannot
        // be written off, and the write-off cannot exceed the balance due. An
        // already written-off invoice is rejected by the guarded UPDATE below.
        let (customer_id, invoice_status, balance_due): (Uuid, String, Decimal) = sqlx::query_as(
            "SELECT customer_id, status, balance_due FROM invoices WHERE id = $1 FOR UPDATE",
        )
        .bind(input.invoice_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        if matches!(invoice_status.as_str(), "voided" | "cancelled") {
            return Err(CommerceError::ValidationError(format!(
                "Cannot write off invoice {} with status {invoice_status}",
                input.invoice_id
            )));
        }
        if input.amount > balance_due {
            return Err(CommerceError::ValidationError(format!(
                "Write-off amount ({}) exceeds invoice balance due ({balance_due})",
                input.amount
            )));
        }

        let updated = sqlx::query(
            "UPDATE invoices SET status = 'written_off', collection_status = 'written_off'
             WHERE id = $1 AND status <> 'written_off'",
        )
        .bind(input.invoice_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("Invoice not found or already written off".into()));
        }

        sqlx::query(
            "INSERT INTO ar_write_offs (id, write_off_number, invoice_id, customer_id, amount, reason,
                notes, write_off_date, approved_by, approved_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(&write_off_number)
        .bind(input.invoice_id)
        .bind(customer_id)
        .bind(input.amount)
        .bind(input.reason.to_string())
        .bind(input.notes.clone())
        .bind(to_date(now))
        .bind(input.approved_by.clone())
        .bind(approved_at)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(WriteOff {
            id,
            write_off_number,
            invoice_id: input.invoice_id,
            customer_id,
            amount: input.amount,
            reason: input.reason,
            notes: input.notes,
            write_off_date: now,
            approved_by: input.approved_by,
            approved_at,
            reversed_at: None,
            gl_journal_entry_id: None,
            created_at: now,
        })
    }

    pub async fn get_write_off_async(&self, id: Uuid) -> Result<Option<WriteOff>> {
        let row = sqlx::query_as::<_, WriteOffRow>(
            "SELECT id, write_off_number, invoice_id, customer_id, amount, reason, notes,
                    write_off_date, approved_by, approved_at, reversed_at, gl_journal_entry_id, created_at
             FROM ar_write_offs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_write_off).transpose()
    }

    pub async fn list_write_offs_async(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, write_off_number, invoice_id, customer_id, amount, reason, notes,
                    write_off_date, approved_by, approved_at, reversed_at, gl_journal_entry_id, created_at
             FROM ar_write_offs WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(invoice_id) = filter.invoice_id {
            builder.push(" AND invoice_id = ").push_bind(invoice_id);
        }
        if let Some(reason) = filter.reason {
            builder.push(" AND reason = ").push_bind(reason.to_string());
        }
        if !filter.include_reversed.unwrap_or(false) {
            builder.push(" AND reversed_at IS NULL");
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND write_off_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND write_off_date <= ").push_bind(to_date(to_date_val));
        }

        builder.push(" ORDER BY write_off_date DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WriteOffRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_write_off).collect::<Result<Vec<_>>>()
    }

    pub async fn reverse_write_off_async(&self, id: Uuid) -> Result<WriteOff> {
        let now = Utc::now();
        let write_off = self.get_write_off_async(id).await?.ok_or(CommerceError::NotFound)?;

        if write_off.reversed_at.is_some() {
            return Err(CommerceError::ValidationError("Write-off already reversed".into()));
        }

        // Atomic + guarded: mark reversed and restore the invoice in one
        // transaction; the reversed_at IS NULL guard makes a concurrent/duplicate
        // reversal a no-op conflict instead of a double restore.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let updated = sqlx::query(
            "UPDATE ar_write_offs SET reversed_at = $1 WHERE id = $2 AND reversed_at IS NULL",
        )
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("Write-off not found or already reversed".into()));
        }

        // Restore the invoice from written_off via the single recalculation
        // source of truth (amount_paid / balance_due / status), then apply the
        // standard overdue rule (due_date < now and still open) rather than
        // unconditionally forcing 'overdue' — the invoice may not be due yet,
        // or payments/credits may cover part (or all) of it.
        sqlx::query("UPDATE invoices SET collection_status = 'none' WHERE id = $1")
            .bind(write_off.invoice_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::recalculate_invoice_tx(&mut tx, write_off.invoice_id).await?;

        sqlx::query(
            "UPDATE invoices SET status = 'overdue'
             WHERE id = $1 AND due_date < NOW() AND status NOT IN ('paid', 'voided')",
        )
        .bind(write_off.invoice_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(WriteOff { reversed_at: Some(now), ..write_off })
    }

    pub async fn create_credit_memo_async(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Credit memo amount must be greater than zero".into(),
            ));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let credit_memo_number = generate_credit_memo_number();

        sqlx::query(
            "INSERT INTO ar_credit_memos (id, credit_memo_number, customer_id, original_invoice_id,
                reason, amount, applied_amount, unapplied_amount, status, notes, issue_date,
                created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, 0, $7, 'open', $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(&credit_memo_number)
        .bind(input.customer_id)
        .bind(input.original_invoice_id)
        .bind(input.reason.to_string())
        .bind(input.amount)
        .bind(input.amount)
        .bind(input.notes.clone())
        .bind(to_date(now))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

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

    pub async fn get_credit_memo_async(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        let row = sqlx::query_as::<_, CreditMemoRow>(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount,
                    applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id,
                    created_at, updated_at
             FROM ar_credit_memos WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_memo).transpose()
    }

    pub async fn get_credit_memo_by_number_async(
        &self,
        number: &str,
    ) -> Result<Option<CreditMemo>> {
        let row = sqlx::query_as::<_, CreditMemoRow>(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount,
                    applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id,
                    created_at, updated_at
             FROM ar_credit_memos WHERE credit_memo_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_memo).transpose()
    }

    pub async fn list_credit_memos_async(
        &self,
        filter: CreditMemoFilter,
    ) -> Result<Vec<CreditMemo>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, credit_memo_number, customer_id, original_invoice_id, reason, amount,
                    applied_amount, unapplied_amount, status, notes, issue_date, gl_journal_entry_id,
                    created_at, updated_at
             FROM ar_credit_memos WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(reason) = filter.reason {
            builder.push(" AND reason = ").push_bind(reason.to_string());
        }
        if let Some(has_unapplied) = filter.has_unapplied {
            if has_unapplied {
                builder.push(" AND unapplied_amount > 0");
            } else {
                builder.push(" AND unapplied_amount <= 0");
            }
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND issue_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND issue_date <= ").push_bind(to_date(to_date_val));
        }

        builder.push(" ORDER BY issue_date DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CreditMemoRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_credit_memo).collect::<Result<Vec<_>>>()
    }

    pub async fn apply_credit_memo_async(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Credit memo application amount must be greater than zero".into(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let cm_row = sqlx::query_as::<_, CreditMemoRow>(
            "SELECT * FROM ar_credit_memos WHERE id = $1 FOR UPDATE",
        )
        .bind(input.credit_memo_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let cm = Self::row_to_credit_memo(cm_row)?;

        if !cm.can_apply() {
            return Err(CommerceError::ValidationError("Credit memo cannot be applied".into()));
        }

        let invoice_customer_id: Uuid =
            sqlx::query_scalar("SELECT customer_id FROM invoices WHERE id = $1")
                .bind(input.invoice_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if invoice_customer_id != cm.customer_id {
            return Err(CommerceError::ValidationError(
                "Credit memo and invoice customer must match".into(),
            ));
        }

        if input.amount > cm.unapplied_amount {
            return Err(CommerceError::ValidationError("Amount exceeds unapplied balance".into()));
        }

        // Lock the invoice row too: the memo FOR UPDATE above only serializes
        // applications of the SAME memo. Two DIFFERENT memos applied to the same
        // invoice would otherwise both read a stale balance and over-apply. Also
        // read the status: a credit memo must not be applied to a terminal invoice.
        let (balance_due, invoice_status): (Decimal, String) =
            sqlx::query_as("SELECT balance_due, status FROM invoices WHERE id = $1 FOR UPDATE")
                .bind(input.invoice_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if matches!(invoice_status.as_str(), "voided" | "cancelled" | "written_off") {
            return Err(CommerceError::ValidationError(format!(
                "Cannot apply credit memo to invoice {} with status {invoice_status}",
                input.invoice_id
            )));
        }
        if input.amount > balance_due {
            return Err(CommerceError::ValidationError(
                "Credit amount exceeds invoice balance due".into(),
            ));
        }

        let now = Utc::now();
        let app_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO ar_credit_memo_applications (id, credit_memo_id, invoice_id, applied_amount,
                applied_date, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(app_id)
        .bind(input.credit_memo_id)
        .bind(input.invoice_id)
        .bind(input.amount)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let new_applied = cm.applied_amount + input.amount;
        let new_unapplied = cm.unapplied_amount - input.amount;
        let new_status = if new_unapplied <= Decimal::ZERO {
            CreditMemoStatus::FullyApplied
        } else {
            CreditMemoStatus::PartiallyApplied
        };

        sqlx::query(
            "UPDATE ar_credit_memos SET applied_amount = $1, unapplied_amount = $2, status = $3
             WHERE id = $4",
        )
        .bind(new_applied)
        .bind(new_unapplied)
        .bind(new_status.to_string())
        .bind(input.credit_memo_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::recalculate_invoice_tx(&mut tx, input.invoice_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(CreditMemo {
            applied_amount: new_applied,
            unapplied_amount: new_unapplied,
            status: new_status,
            updated_at: now,
            ..cm
        })
    }

    pub async fn void_credit_memo_async(&self, id: Uuid) -> Result<CreditMemo> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the memo row so a concurrent apply_credit_memo_async (which also
        // locks it FOR UPDATE) serializes; re-read the applied amount under the
        // lock instead of trusting a value read before the transaction.
        let row = sqlx::query_as::<_, CreditMemoRow>(
            "SELECT * FROM ar_credit_memos WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let cm = Self::row_to_credit_memo(row)?;

        if cm.applied_amount > Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Cannot void credit memo with applications".into(),
            ));
        }
        if cm.status == CreditMemoStatus::Voided {
            return Err(CommerceError::Conflict("Credit memo already voided".into()));
        }

        sqlx::query("UPDATE ar_credit_memos SET status = 'voided' WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(CreditMemo { status: CreditMemoStatus::Voided, updated_at: Utc::now(), ..cm })
    }

    pub async fn get_unapplied_credits_async(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.list_credit_memos_async(CreditMemoFilter {
            customer_id: Some(customer_id),
            has_unapplied: Some(true),
            ..Default::default()
        })
        .await
    }

    pub async fn apply_payment_to_invoices_async(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        if input.applications.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let mut applications = Vec::new();
        let mut expected_customer_id: Option<Uuid> = None;

        // The payment itself bounds the total that can be applied: read its
        // amount and what is already applied inside the transaction, so a
        // payment can never be applied beyond its own value.
        let payment_amount: Decimal =
            sqlx::query_scalar("SELECT amount FROM payments WHERE id = $1")
                .bind(input.payment_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or_else(|| {
                    CommerceError::ValidationError(format!(
                        "Payment {} not found",
                        input.payment_id
                    ))
                })?;
        let existing_applied: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(applied_amount), 0) FROM ar_payment_applications WHERE payment_id = $1",
        )
        .bind(input.payment_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut total_new = Decimal::ZERO;
        for app in &input.applications {
            if app.amount <= Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Payment application amount must be greater than zero".into(),
                ));
            }
            total_new += app.amount;
        }
        if existing_applied + total_new > payment_amount {
            return Err(CommerceError::ValidationError(format!(
                "Payment applications ({total_new}) plus already applied ({existing_applied}) exceed payment amount ({payment_amount})"
            )));
        }

        for app in input.applications {
            let (invoice_customer_id, invoice_status): (Uuid, String) =
                sqlx::query_as("SELECT customer_id, status FROM invoices WHERE id = $1")
                    .bind(app.invoice_id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if matches!(invoice_status.as_str(), "voided" | "cancelled" | "written_off") {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot apply payment to invoice {} with status {invoice_status}",
                    app.invoice_id
                )));
            }
            if let Some(expected) = expected_customer_id {
                if expected != invoice_customer_id {
                    return Err(CommerceError::ValidationError(
                        "All invoice applications for a payment must belong to the same customer"
                            .into(),
                    ));
                }
            } else {
                expected_customer_id = Some(invoice_customer_id);
            }

            // Lock the invoice row so concurrent payments serialize: without
            // FOR UPDATE, two READ COMMITTED transactions both read the same
            // balance and over-apply (the sibling apply_credit_memo_async and
            // AP create_payment_async already lock their rows this way).
            let balance_due: Decimal =
                sqlx::query_scalar("SELECT balance_due FROM invoices WHERE id = $1 FOR UPDATE")
                    .bind(app.invoice_id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if app.amount > balance_due {
                return Err(CommerceError::ValidationError(
                    "Payment application amount exceeds invoice balance due".into(),
                ));
            }

            let app_id = Uuid::new_v4();

            sqlx::query(
                "INSERT INTO ar_payment_applications (id, payment_id, invoice_id, applied_amount,
                    applied_date, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(app_id)
            .bind(input.payment_id)
            .bind(app.invoice_id)
            .bind(app.amount)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            Self::recalculate_invoice_tx(&mut tx, app.invoice_id).await?;

            applications.push(ArPaymentApplication {
                id: app_id,
                payment_id: input.payment_id,
                invoice_id: app.invoice_id,
                applied_amount: app.amount,
                applied_date: now,
                created_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(applications)
    }

    pub async fn get_payment_applications_async(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<ArPaymentApplication>> {
        let rows = sqlx::query_as::<_, PaymentApplicationRow>(
            "SELECT id, payment_id, invoice_id, applied_amount, applied_date, created_at
             FROM ar_payment_applications WHERE payment_id = $1",
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_payment_application).collect())
    }

    pub async fn unapply_payment_async(&self, application_id: Uuid) -> Result<()> {
        // Atomic: read the application, delete it, and recalculate the invoice
        // in one transaction. Previously three autocommit statements — a
        // recalculation failure after the DELETE committed left the application
        // gone while the invoice still showed the old amount_paid/status.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Missing application id -> RowNotFound -> NotFound.
        let invoice_id: Uuid =
            sqlx::query_scalar("SELECT invoice_id FROM ar_payment_applications WHERE id = $1")
                .bind(application_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        sqlx::query("DELETE FROM ar_payment_applications WHERE id = $1")
            .bind(application_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::recalculate_invoice_tx(&mut tx, invoice_id).await?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    pub async fn get_customer_summary_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerArSummary>> {
        let aging = match self.get_customer_aging_async(customer_id).await? {
            Some(aging) => aging,
            None => return Ok(None),
        };

        let unapplied_credits: Decimal = self
            .get_unapplied_credits_async(customer_id)
            .await?
            .iter()
            .map(|cm| cm.unapplied_amount)
            .sum();

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

    pub async fn generate_statement_async(
        &self,
        request: GenerateStatementRequest,
    ) -> Result<CustomerStatement> {
        let now = Utc::now();
        let period_start = request.period_start.unwrap_or_else(|| now - chrono::Duration::days(30));
        let period_end = request.period_end.unwrap_or(now);

        let (customer_name, customer_email): (String, Option<String>) = sqlx::query_as(
            "SELECT first_name || ' ' || last_name, email FROM customers WHERE id = $1",
        )
        .bind(request.customer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let aging = self
            .get_customer_aging_async(request.customer_id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        // Gather statement-visible activity — invoices (debits), payment and
        // credit-memo applications and non-reversed write-offs (credits) — as
        // (date, type, ref, description, amount) rows for one window.
        // Mirrors the SQLite backend.
        async fn collect_activity(
            pool: &sqlx::PgPool,
            customer_id: Uuid,
            from: DateTime<Utc>,
            to: DateTime<Utc>,
        ) -> Result<Vec<PgStatementActivityRow>> {
            let mut rows: Vec<PgStatementActivityRow> = Vec::new();

            let invoices: Vec<(DateTime<Utc>, String, Decimal)> = sqlx::query_as(
                "SELECT created_at, invoice_number, total FROM invoices
                 WHERE customer_id = $1 AND created_at >= $2 AND created_at <= $3",
            )
            .bind(customer_id)
            .bind(from)
            .bind(to)
            .fetch_all(pool)
            .await
            .map_err(map_db_error)?;
            for (date, number, total) in invoices {
                rows.push((
                    date,
                    StatementTransactionType::Invoice,
                    number,
                    "Invoice".into(),
                    total,
                ));
            }

            let payments: Vec<(DateTime<Utc>, Uuid, Decimal)> = sqlx::query_as(
                "SELECT pa.applied_date, p.id, pa.applied_amount
                 FROM ar_payment_applications pa
                 JOIN payments p ON pa.payment_id = p.id
                 JOIN invoices i ON pa.invoice_id = i.id
                 WHERE i.customer_id = $1 AND pa.applied_date >= $2 AND pa.applied_date <= $3",
            )
            .bind(customer_id)
            .bind(from)
            .bind(to)
            .fetch_all(pool)
            .await
            .map_err(map_db_error)?;
            for (date, id, amount) in payments {
                rows.push((
                    date,
                    StatementTransactionType::Payment,
                    id.to_string()[..8].to_string(),
                    "Payment".into(),
                    amount,
                ));
            }

            let credits: Vec<(DateTime<Utc>, String, Decimal)> = sqlx::query_as(
                "SELECT ca.applied_date, cm.credit_memo_number, ca.applied_amount
                 FROM ar_credit_memo_applications ca
                 JOIN ar_credit_memos cm ON ca.credit_memo_id = cm.id
                 JOIN invoices i ON ca.invoice_id = i.id
                 WHERE i.customer_id = $1 AND ca.applied_date >= $2 AND ca.applied_date <= $3",
            )
            .bind(customer_id)
            .bind(from)
            .bind(to)
            .fetch_all(pool)
            .await
            .map_err(map_db_error)?;
            for (date, number, amount) in credits {
                rows.push((
                    date,
                    StatementTransactionType::CreditMemo,
                    number,
                    "Credit Memo".into(),
                    amount,
                ));
            }

            let write_offs: Vec<(DateTime<Utc>, String, Decimal)> = sqlx::query_as(
                "SELECT write_off_date, write_off_number, amount FROM ar_write_offs
                 WHERE customer_id = $1 AND reversed_at IS NULL
                   AND write_off_date >= $2 AND write_off_date <= $3",
            )
            .bind(customer_id)
            .bind(from)
            .bind(to)
            .fetch_all(pool)
            .await
            .map_err(map_db_error)?;
            for (date, number, amount) in write_offs {
                rows.push((
                    date,
                    StatementTransactionType::WriteOff,
                    number,
                    "Write-off".into(),
                    amount,
                ));
            }

            rows.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(rows)
        }

        // Opening balance: everything statement-visible before the period.
        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap_or(period_start);
        let opening_balance =
            collect_activity(&self.pool, request.customer_id, epoch, period_start)
                .await?
                .into_iter()
                // Pre-period rows exactly at period_start belong to the period.
                .filter(|(date, ..)| *date < period_start)
                .map(|(_, kind, _, _, amount)| match kind {
                    StatementTransactionType::Invoice => amount,
                    _ => -amount,
                })
                .sum::<Decimal>();

        let activity =
            collect_activity(&self.pool, request.customer_id, period_start, period_end).await?;

        let mut line_items: Vec<StatementLineItem> = Vec::new();
        let mut running_balance = opening_balance;
        let mut total_invoices = Decimal::ZERO;
        let mut total_payments = Decimal::ZERO;
        let mut total_credits = Decimal::ZERO;
        for (date, kind, reference_number, description, amount) in activity {
            let (debit, credit) = match kind {
                StatementTransactionType::Invoice => {
                    running_balance += amount;
                    total_invoices += amount;
                    (Some(amount), None)
                }
                other => {
                    running_balance -= amount;
                    match other {
                        StatementTransactionType::Payment => total_payments += amount,
                        StatementTransactionType::CreditMemo => total_credits += amount,
                        _ => {}
                    }
                    (None, Some(amount))
                }
            };
            line_items.push(StatementLineItem {
                date,
                transaction_type: kind,
                reference_number,
                description,
                debit,
                credit,
                balance: running_balance,
            });
        }

        Ok(CustomerStatement {
            customer_id: request.customer_id,
            customer_name,
            customer_email,
            billing_address: None,
            statement_date: now,
            period_start,
            period_end,
            opening_balance,
            total_invoices,
            total_payments,
            total_credits,
            // The live aging total; matches the running balance when every
            // balance-affecting event is statement-visible for the window.
            closing_balance: aging.total_outstanding,
            aging,
            line_items,
        })
    }

    pub async fn get_total_outstanding_async(&self) -> Result<Decimal> {
        let summary = self.get_aging_summary_async().await?;
        Ok(summary.total)
    }

    pub async fn get_dso_async(&self, days: i32) -> Result<Decimal> {
        let ar_balance: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(balance_due), 0) FROM invoices
             WHERE status NOT IN ('paid', 'voided', 'written_off')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let total_sales: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM invoices
             WHERE created_at >= NOW() - ($1::text || ' days')::interval",
        )
        .bind(days)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        if total_sales == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        let days_decimal = Decimal::from_i32(days).unwrap_or_default();
        Ok((ar_balance / total_sales) * days_decimal)
    }

    pub async fn get_average_days_to_pay_async(&self, customer_id: Uuid) -> Result<Option<i32>> {
        // Use the FRACTIONAL day difference (total seconds / 86400), matching the
        // SQLite backend's `JULIANDAY(applied) - JULIANDAY(invoice)`. `EXTRACT(DAY
        // FROM interval)` returns only the whole-day component, dropping the hours,
        // which floors each invoice's latency before averaging and diverges.
        // `::double precision`: EXTRACT(EPOCH FROM interval) yields `numeric`
        // (PostgreSQL 14+), and sqlx refuses to decode numeric into f64.
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(EXTRACT(EPOCH FROM (pa.applied_date - i.invoice_date)) / 86400.0)::double precision
             FROM ar_payment_applications pa
             JOIN invoices i ON pa.invoice_id = i.id
             WHERE i.customer_id = $1 AND i.status = 'paid'",
        )
        .bind(customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(avg.map(|v| v as i32))
    }

    pub async fn get_customers_batch_async(
        &self,
        ids: Vec<Uuid>,
    ) -> Result<Vec<CustomerArSummary>> {
        let mut summaries = Vec::new();
        for id in ids {
            if let Some(summary) = self.get_customer_summary_async(id).await? {
                summaries.push(summary);
            }
        }
        Ok(summaries)
    }
}

impl AccountsReceivableRepository for PgAccountsReceivableRepository {
    fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        block_on(self.get_aging_summary_async())
    }

    fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        block_on(self.get_customer_aging_async(customer_id))
    }

    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        block_on(self.get_aging_report_async(filter))
    }

    fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        block_on(self.log_collection_activity_async(input))
    }

    fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        block_on(self.list_collection_activities_async(filter))
    }

    fn update_collection_status(
        &self,
        invoice_id: InvoiceId,
        status: CollectionStatus,
    ) -> Result<()> {
        block_on(self.update_collection_status_async(invoice_id.into_uuid(), status))
    }

    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        block_on(self.get_invoices_due_for_dunning_async())
    }

    fn send_dunning_letter(
        &self,
        invoice_id: InvoiceId,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        block_on(self.send_dunning_letter_async(invoice_id.into_uuid(), letter_type, sent_by))
    }

    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        block_on(self.create_write_off_async(input))
    }

    fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        block_on(self.get_write_off_async(id))
    }

    fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        block_on(self.list_write_offs_async(filter))
    }

    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        block_on(self.reverse_write_off_async(id))
    }

    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        block_on(self.create_credit_memo_async(input))
    }

    fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        block_on(self.get_credit_memo_async(id))
    }

    fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        block_on(self.get_credit_memo_by_number_async(number))
    }

    fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        block_on(self.list_credit_memos_async(filter))
    }

    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        block_on(self.apply_credit_memo_async(input))
    }

    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        block_on(self.void_credit_memo_async(id))
    }

    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        block_on(self.get_unapplied_credits_async(customer_id))
    }

    fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        block_on(self.apply_payment_to_invoices_async(input))
    }

    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>> {
        block_on(self.get_payment_applications_async(payment_id))
    }

    fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        block_on(self.unapply_payment_async(application_id))
    }

    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>> {
        block_on(self.get_customer_summary_async(customer_id))
    }

    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement> {
        block_on(self.generate_statement_async(request))
    }

    fn get_total_outstanding(&self) -> Result<Decimal> {
        block_on(self.get_total_outstanding_async())
    }

    fn get_dso(&self, days: i32) -> Result<Decimal> {
        block_on(self.get_dso_async(days))
    }

    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        block_on(self.get_average_days_to_pay_async(customer_id))
    }

    fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        block_on(self.get_customers_batch_async(ids))
    }
}

fn to_date(dt: DateTime<Utc>) -> NaiveDate {
    dt.date_naive()
}

const fn from_date(date: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
}
