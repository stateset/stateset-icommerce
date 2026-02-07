//! SQLite implementation of payment repository

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use stateset_core::{
    generate_payment_number, generate_refund_number, validate_batch_size, BatchResult,
    CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund, Payment, PaymentFilter,
    PaymentMethod, PaymentRepository, PaymentTransactionStatus, Refund, RefundStatus, Result,
    UpdatePayment,
};
use uuid::Uuid;

pub struct SqlitePaymentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePaymentRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_payment(row: &Row) -> rusqlite::Result<Payment> {
        Ok(Payment {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "payment", "id")?,
            payment_number: row.get("payment_number")?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "payment",
                "order_id",
            )?,
            invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("invoice_id")?,
                "payment",
                "invoice_id",
            )?,
            customer_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("customer_id")?,
                "payment",
                "customer_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "payment", "status")?,
            payment_method: parse_enum_row(
                &row.get::<_, String>("payment_method")?,
                "payment",
                "payment_method",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "payment", "amount")?,
            currency: row.get("currency")?,
            amount_refunded: parse_decimal_row(
                &row.get::<_, String>("amount_refunded")?,
                "payment",
                "amount_refunded",
            )?,
            external_id: row.get("external_id")?,
            idempotency_key: row.get("idempotency_key")?,
            processor: row.get("processor")?,
            card_brand: match row.get::<_, Option<String>>("card_brand")? {
                Some(value) => Some(parse_enum_row(&value, "payment", "card_brand")?),
                None => None,
            },
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            // Blockchain/Stablecoin fields
            blockchain_network: match row
                .get::<_, Option<String>>("blockchain_network")
                .ok()
                .flatten()
            {
                Some(value) => Some(parse_enum_row(&value, "payment", "blockchain_network")?),
                None => None,
            },
            stablecoin_type: match row
                .get::<_, Option<String>>("stablecoin_type")
                .ok()
                .flatten()
            {
                Some(value) => Some(parse_enum_row(&value, "payment", "stablecoin_type")?),
                None => None,
            },
            from_wallet_address: row.get("from_wallet_address").ok().flatten(),
            to_wallet_address: row.get("to_wallet_address").ok().flatten(),
            tx_hash: row.get("tx_hash").ok().flatten(),
            block_number: row.get("block_number").ok().flatten(),
            confirmations: row.get("confirmations").ok().flatten(),
            token_address: row.get("token_address").ok().flatten(),
            ves_intent_id: row.get("ves_intent_id").ok().flatten(),
            billing_email: row.get("billing_email")?,
            billing_name: row.get("billing_name")?,
            billing_address: row.get("billing_address")?,
            description: row.get("description")?,
            failure_reason: row.get("failure_reason")?,
            failure_code: row.get("failure_code")?,
            metadata: row.get("metadata")?,
            paid_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("paid_at")?,
                "payment",
                "paid_at",
            )?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment",
                "updated_at",
            )?,
        })
    }

    fn row_to_refund(row: &Row) -> rusqlite::Result<Refund> {
        Ok(Refund {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "refund", "id")?,
            refund_number: row.get("refund_number")?,
            payment_id: parse_uuid_row(
                &row.get::<_, String>("payment_id")?,
                "refund",
                "payment_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "refund", "status")?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "refund", "amount")?,
            currency: row.get("currency")?,
            reason: row.get("reason")?,
            external_id: row.get("external_id")?,
            idempotency_key: row.get("idempotency_key")?,
            failure_reason: row.get("failure_reason")?,
            notes: row.get("notes")?,
            refunded_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("refunded_at")?,
                "refund",
                "refunded_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "refund",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "refund",
                "updated_at",
            )?,
        })
    }

    fn row_to_payment_method(row: &Row) -> rusqlite::Result<PaymentMethod> {
        Ok(PaymentMethod {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "payment_method", "id")?,
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "payment_method",
                "customer_id",
            )?,
            method_type: parse_enum_row(
                &row.get::<_, String>("method_type")?,
                "payment_method",
                "method_type",
            )?,
            is_default: row.get::<_, i32>("is_default")? != 0,
            card_brand: match row.get::<_, Option<String>>("card_brand")? {
                Some(value) => Some(parse_enum_row(&value, "payment_method", "card_brand")?),
                None => None,
            },
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            cardholder_name: row.get("cardholder_name")?,
            bank_name: row.get("bank_name")?,
            account_last4: row.get("account_last4")?,
            // Blockchain/Wallet fields
            wallet_address: row.get("wallet_address").ok().flatten(),
            blockchain_network: match row
                .get::<_, Option<String>>("blockchain_network")
                .ok()
                .flatten()
            {
                Some(value) => Some(parse_enum_row(
                    &value,
                    "payment_method",
                    "blockchain_network",
                )?),
                None => None,
            },
            stablecoin_type: match row
                .get::<_, Option<String>>("stablecoin_type")
                .ok()
                .flatten()
            {
                Some(value) => Some(parse_enum_row(&value, "payment_method", "stablecoin_type")?),
                None => None,
            },
            external_id: row.get("external_id")?,
            billing_address: row.get("billing_address")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment_method",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment_method",
                "updated_at",
            )?,
        })
    }

    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<Payment>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE idempotency_key = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([key], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_refund_by_idempotency_key(&self, key: &str) -> Result<Option<Refund>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM refunds WHERE idempotency_key = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([key], Self::row_to_refund);
        match result {
            Ok(refund) => Ok(Some(refund)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }
}

impl PaymentRepository for SqlitePaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key(key)? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let payment_number = generate_payment_number();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    input.idempotency_key,
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
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<Payment>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE id = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE payment_number = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([payment_number], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE external_id = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([external_id], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        let payment = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = chrono::Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), Self::row_to_payment)
            .map_err(map_db_error)?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row.map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.list(PaymentFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
    }

    fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.list(PaymentFilter {
            invoice_id: Some(invoice_id),
            ..Default::default()
        })
    }

    fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        self.update(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Processing),
                ..Default::default()
            },
        )
    }

    fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        let now = chrono::Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE payments SET status = ?, paid_at = ?, updated_at = ? WHERE id = ?",
                params![
                    PaymentTransactionStatus::Completed.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        let now = chrono::Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE payments SET status = ?, failure_reason = ?, failure_code = ?, updated_at = ? WHERE id = ?",
                params![PaymentTransactionStatus::Failed.to_string(), reason, code, now.to_rfc3339(), id.to_string()],
            ).map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: Uuid) -> Result<Payment> {
        self.update(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Cancelled),
                ..Default::default()
            },
        )
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_refund_by_idempotency_key(key)? {
                return Ok(existing);
            }
        }

        // Get payment to determine refund amount
        let payment = self.get(input.payment_id)?.ok_or(CommerceError::NotFound)?;
        let refund_amount = input
            .amount
            .unwrap_or(payment.amount - payment.amount_refunded);

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let refund_number = generate_refund_number();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    refund_number,
                    input.payment_id.to_string(),
                    RefundStatus::Pending.to_string(),
                    refund_amount.to_string(),
                    payment.currency,
                    input.reason,
                    input.external_id,
                    input.idempotency_key,
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM refunds WHERE id = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_refund);
        match result {
            Ok(refund) => Ok(Some(refund)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM refunds WHERE payment_id = ? ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([payment_id.to_string()], Self::row_to_refund)
            .map_err(map_db_error)?;

        let mut refunds = Vec::new();
        for row in rows {
            refunds.push(row.map_err(map_db_error)?);
        }
        Ok(refunds)
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        let refund = self.get_refund(id)?.ok_or(CommerceError::NotFound)?;
        let now = chrono::Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE refunds SET status = ?, refunded_at = ?, updated_at = ? WHERE id = ?",
                params![
                    RefundStatus::Completed.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;

            // Update payment amount_refunded
            conn.execute(
                "UPDATE payments SET amount_refunded = amount_refunded + ?, status = CASE
                 WHEN amount_refunded + ? >= amount THEN 'refunded' ELSE 'partially_refunded' END,
                 updated_at = ? WHERE id = ?",
                params![
                    refund.amount.to_string(),
                    refund.amount.to_string(),
                    now.to_rfc3339(),
                    refund.payment_id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let now = chrono::Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE refunds SET status = ?, failure_reason = ?, updated_at = ? WHERE id = ?",
                params![
                    RefundStatus::Failed.to_string(),
                    reason,
                    now.to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // If setting as default, clear existing default
        if input.is_default.unwrap_or(false) {
            conn.execute(
                "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
                [input.customer_id.to_string()],
            )
            .map_err(map_db_error)?;
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
        )
        .map_err(map_db_error)?;

        let mut stmt = conn
            .prepare("SELECT * FROM payment_methods WHERE id = ?")
            .map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_payment_method)
            .map_err(map_db_error)
    }

    fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payment_methods WHERE customer_id = ? ORDER BY is_default DESC, created_at DESC").map_err(map_db_error)?;
        let rows = stmt
            .query_map([customer_id.to_string()], Self::row_to_payment_method)
            .map_err(map_db_error)?;

        let mut methods = Vec::new();
        for row in rows {
            methods.push(row.map_err(map_db_error)?);
        }
        Ok(methods)
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.execute("DELETE FROM payment_methods WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
            [customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        conn.execute(
            "UPDATE payment_methods SET is_default = 1 WHERE id = ? AND customer_id = ?",
            params![method_id.to_string(), customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;
        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = chrono::Utc::now();
            let payment_number = generate_payment_number();

            tx.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    payment_number.clone(),
                    input.order_id.map(|id| id.to_string()),
                    input.invoice_id.map(|id| id.to_string()),
                    input.customer_id.map(|id| id.to_string()),
                    PaymentTransactionStatus::Pending.to_string(),
                    input.payment_method.to_string(),
                    input.amount.to_string(),
                    input.currency.clone().unwrap_or_else(|| "USD".to_string()),
                    "0",
                    input.external_id.clone(),
                    input.idempotency_key.clone(),
                    input.processor.clone(),
                    input.card_brand.map(|b| b.to_string()),
                    input.card_last4.clone(),
                    input.card_exp_month,
                    input.card_exp_year,
                    input.billing_email.clone(),
                    input.billing_name.clone(),
                    input.billing_address.clone(),
                    input.description.clone(),
                    input.metadata.clone(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;

            results.push(Payment {
                id,
                payment_number,
                order_id: input.order_id,
                invoice_id: input.invoice_id,
                customer_id: input.customer_id,
                status: PaymentTransactionStatus::Pending,
                payment_method: input.payment_method,
                amount: input.amount,
                currency: input.currency.unwrap_or_else(|| "USD".to_string()),
                amount_refunded: rust_decimal::Decimal::ZERO,
                external_id: input.external_id,
                idempotency_key: input.idempotency_key,
                processor: input.processor,
                card_brand: input.card_brand,
                card_last4: input.card_last4,
                card_exp_month: input.card_exp_month,
                card_exp_year: input.card_exp_year,
                // Blockchain/Stablecoin fields
                blockchain_network: input.blockchain_network,
                stablecoin_type: input.stablecoin_type,
                from_wallet_address: input.from_wallet_address,
                to_wallet_address: input.to_wallet_address,
                tx_hash: None,
                block_number: None,
                confirmations: None,
                token_address: input.token_address,
                ves_intent_id: None,
                billing_email: input.billing_email,
                billing_name: input.billing_name,
                billing_address: input.billing_address,
                description: input.description,
                failure_reason: None,
                failure_code: None,
                metadata: input.metadata,
                paid_at: None,
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdatePayment)>) -> Result<BatchResult<Payment>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdatePayment)>) -> Result<Vec<Payment>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = chrono::Utc::now();

            // Get existing payment to merge with updates
            let payment: Payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                    e => map_db_error(e),
                })?;

            tx.execute(
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
            )
            .map_err(map_db_error)?;

            // Fetch the updated payment
            let updated_payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(map_db_error)?;

            results.push(updated_payment);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let conn = self.conn()?;
            match conn.execute("DELETE FROM payments WHERE id = ?", [id.to_string()]) {
                Ok(rows) if rows > 0 => result.record_success(id),
                Ok(_) => {
                    result.record_failure(index, Some(id.to_string()), &CommerceError::NotFound)
                }
                Err(e) => result.record_failure(index, Some(id.to_string()), &map_db_error(e)),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        // Delete refunds associated with these payments first
        let sql = format!("DELETE FROM refunds WHERE payment_id IN ({})", placeholders);
        tx.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        // Delete payments
        let sql = format!("DELETE FROM payments WHERE id IN ({})", placeholders);
        tx.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Payment>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM payments WHERE id IN ({})", placeholders);

        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let payments = stmt
            .query_map(params_refs.as_slice(), Self::row_to_payment)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(payments)
    }
}
