//! SQLite implementation of payment repository

use super::{map_db_error, parse_decimal};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use stateset_core::{
    CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund,
    Payment, PaymentFilter, PaymentMethod, PaymentRepository,
    PaymentTransactionStatus, Refund, RefundStatus, Result, UpdatePayment,
    generate_payment_number, generate_refund_number,
};
use uuid::Uuid;

pub struct SqlitePaymentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePaymentRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_payment(row: &Row) -> rusqlite::Result<Payment> {
        Ok(Payment {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            payment_number: row.get("payment_number")?,
            order_id: row.get::<_, Option<String>>("order_id")?.and_then(|s| s.parse().ok()),
            invoice_id: row.get::<_, Option<String>>("invoice_id")?.and_then(|s| s.parse().ok()),
            customer_id: row.get::<_, Option<String>>("customer_id")?.and_then(|s| s.parse().ok()),
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            payment_method: row.get::<_, String>("payment_method")?.parse().unwrap_or_default(),
            amount: parse_decimal(&row.get::<_, String>("amount")?),
            currency: row.get("currency")?,
            amount_refunded: parse_decimal(&row.get::<_, String>("amount_refunded")?),
            external_id: row.get("external_id")?,
            processor: row.get("processor")?,
            card_brand: row.get::<_, Option<String>>("card_brand")?.and_then(|s| s.parse().ok()),
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            billing_email: row.get("billing_email")?,
            billing_name: row.get("billing_name")?,
            billing_address: row.get("billing_address")?,
            description: row.get("description")?,
            failure_reason: row.get("failure_reason")?,
            failure_code: row.get("failure_code")?,
            metadata: row.get("metadata")?,
            paid_at: row.get::<_, Option<String>>("paid_at")?.and_then(|s| s.parse().ok()),
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_refund(row: &Row) -> rusqlite::Result<Refund> {
        Ok(Refund {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            refund_number: row.get("refund_number")?,
            payment_id: row.get::<_, String>("payment_id")?.parse().unwrap_or_default(),
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            amount: parse_decimal(&row.get::<_, String>("amount")?),
            currency: row.get("currency")?,
            reason: row.get("reason")?,
            external_id: row.get("external_id")?,
            failure_reason: row.get("failure_reason")?,
            notes: row.get("notes")?,
            refunded_at: row.get::<_, Option<String>>("refunded_at")?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_payment_method(row: &Row) -> rusqlite::Result<PaymentMethod> {
        Ok(PaymentMethod {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            method_type: row.get::<_, String>("method_type")?.parse().unwrap_or_default(),
            is_default: row.get::<_, i32>("is_default")? != 0,
            card_brand: row.get::<_, Option<String>>("card_brand")?.and_then(|s| s.parse().ok()),
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            cardholder_name: row.get("cardholder_name")?,
            bank_name: row.get("bank_name")?,
            account_last4: row.get("account_last4")?,
            external_id: row.get("external_id")?,
            billing_address: row.get("billing_address")?,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }
}

impl PaymentRepository for SqlitePaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let payment_number = generate_payment_number();

        conn.execute(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, processor,
             card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
             billing_address, description, metadata, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                payment_number,
                input.order_id.map(|id| id.to_string()),
                input.invoice_id.map(|id| id.to_string()),
                input.customer_id.map(|id| id.to_string()),
                PaymentTransactionStatus::Pending.to_string(),
                input.payment_method.to_string(),
                input.amount.to_string(),
                input.currency.unwrap_or_else(|| "USD".to_string()),
                "0",
                input.external_id,
                input.processor,
                input.card_brand.map(|b| b.to_string()),
                input.card_last4,
                input.card_exp_month,
                input.card_exp_year,
                input.billing_email,
                input.billing_name,
                input.billing_address,
                input.description,
                input.metadata,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payments WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payments WHERE payment_number = ?").map_err(map_db_error)?;
        let result = stmt.query_row([payment_number], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payments WHERE external_id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([external_id], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        let payment = self.get(id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE payments SET status = ?, external_id = ?, failure_reason = ?,
             failure_code = ?, metadata = ?, updated_at = ? WHERE id = ?",
            params![
                input.status.unwrap_or(payment.status).to_string(),
                input.external_id.or(payment.external_id),
                input.failure_reason.or(payment.failure_reason),
                input.failure_code.or(payment.failure_code),
                input.metadata.or(payment.metadata),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT * FROM payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
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
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_payment).map_err(map_db_error)?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row.map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.list(PaymentFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.list(PaymentFilter { invoice_id: Some(invoice_id), ..Default::default() })
    }

    fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        self.update(id, UpdatePayment { status: Some(PaymentTransactionStatus::Processing), ..Default::default() })
    }

    fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE payments SET status = ?, paid_at = ?, updated_at = ? WHERE id = ?",
            params![PaymentTransactionStatus::Completed.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE payments SET status = ?, failure_reason = ?, failure_code = ?, updated_at = ? WHERE id = ?",
            params![PaymentTransactionStatus::Failed.to_string(), reason, code, now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: Uuid) -> Result<Payment> {
        self.update(id, UpdatePayment { status: Some(PaymentTransactionStatus::Cancelled), ..Default::default() })
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Get payment to determine refund amount
        let payment = self.get(input.payment_id)?.ok_or(CommerceError::NotFound)?;
        let refund_amount = input.amount.unwrap_or(payment.amount - payment.amount_refunded);

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let refund_number = generate_refund_number();

        conn.execute(
            "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                refund_number,
                input.payment_id.to_string(),
                RefundStatus::Pending.to_string(),
                refund_amount.to_string(),
                payment.currency,
                input.reason,
                input.external_id,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM refunds WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_refund);
        match result {
            Ok(refund) => Ok(Some(refund)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM refunds WHERE payment_id = ? ORDER BY created_at DESC").map_err(map_db_error)?;
        let rows = stmt.query_map([payment_id.to_string()], Self::row_to_refund).map_err(map_db_error)?;

        let mut refunds = Vec::new();
        for row in rows {
            refunds.push(row.map_err(map_db_error)?);
        }
        Ok(refunds)
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        let refund = self.get_refund(id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE refunds SET status = ?, refunded_at = ?, updated_at = ? WHERE id = ?",
            params![RefundStatus::Completed.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        // Update payment amount_refunded
        conn.execute(
            "UPDATE payments SET amount_refunded = amount_refunded + ?, status = CASE
             WHEN amount_refunded + ? >= amount THEN 'refunded' ELSE 'partially_refunded' END,
             updated_at = ? WHERE id = ?",
            params![refund.amount.to_string(), refund.amount.to_string(), now.to_rfc3339(), refund.payment_id.to_string()],
        ).map_err(map_db_error)?;

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE refunds SET status = ?, failure_reason = ?, updated_at = ? WHERE id = ?",
            params![RefundStatus::Failed.to_string(), reason, now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // If setting as default, clear existing default
        if input.is_default.unwrap_or(false) {
            conn.execute(
                "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
                [input.customer_id.to_string()],
            ).map_err(map_db_error)?;
        }

        conn.execute(
            "INSERT INTO payment_methods (id, customer_id, method_type, is_default, card_brand,
             card_last4, card_exp_month, card_exp_year, cardholder_name, bank_name, account_last4,
             external_id, billing_address, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                input.customer_id.to_string(),
                input.method_type.to_string(),
                input.is_default.unwrap_or(false) as i32,
                input.card_brand.map(|b| b.to_string()),
                input.card_last4,
                input.card_exp_month,
                input.card_exp_year,
                input.cardholder_name,
                input.bank_name,
                input.account_last4,
                input.external_id,
                input.billing_address,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        let mut stmt = conn.prepare("SELECT * FROM payment_methods WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_payment_method).map_err(map_db_error)
    }

    fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payment_methods WHERE customer_id = ? ORDER BY is_default DESC, created_at DESC").map_err(map_db_error)?;
        let rows = stmt.query_map([customer_id.to_string()], Self::row_to_payment_method).map_err(map_db_error)?;

        let mut methods = Vec::new();
        for row in rows {
            methods.push(row.map_err(map_db_error)?);
        }
        Ok(methods)
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.execute("DELETE FROM payment_methods WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        Ok(())
    }

    fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
            [customer_id.to_string()],
        ).map_err(map_db_error)?;

        conn.execute(
            "UPDATE payment_methods SET is_default = 1 WHERE id = ? AND customer_id = ?",
            params![method_id.to_string(), customer_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }
}
