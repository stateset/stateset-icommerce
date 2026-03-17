//! PostgreSQL implementation of payment repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund, CurrencyCode,
    CustomerId, InvoiceId, OrderId, Payment, PaymentFilter, PaymentId, PaymentMethod,
    PaymentMethodType, PaymentRepository, PaymentTransactionStatus, Refund, RefundStatus, Result,
    UpdatePayment, generate_payment_number, generate_refund_number, validate_batch_size,
};
use uuid::Uuid;

/// PostgreSQL payment repository
#[derive(Debug, Clone)]
pub struct PgPaymentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PaymentRow {
    id: Uuid,
    payment_number: String,
    order_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    status: String,
    payment_method: String,
    amount: Decimal,
    currency: CurrencyCode,
    amount_refunded: Decimal,
    external_id: Option<String>,
    idempotency_key: Option<String>,
    processor: Option<String>,
    card_brand: Option<String>,
    card_last4: Option<String>,
    card_exp_month: Option<i32>,
    card_exp_year: Option<i32>,
    billing_email: Option<String>,
    billing_name: Option<String>,
    billing_address: Option<String>,
    description: Option<String>,
    failure_reason: Option<String>,
    failure_code: Option<String>,
    metadata: Option<String>,
    paid_at: Option<DateTime<Utc>>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RefundRow {
    id: Uuid,
    refund_number: String,
    payment_id: Uuid,
    status: String,
    amount: Decimal,
    currency: CurrencyCode,
    reason: Option<String>,
    external_id: Option<String>,
    idempotency_key: Option<String>,
    failure_reason: Option<String>,
    notes: Option<String>,
    refunded_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentMethodRow {
    id: Uuid,
    customer_id: Uuid,
    method_type: String,
    is_default: bool,
    card_brand: Option<String>,
    card_last4: Option<String>,
    card_exp_month: Option<i32>,
    card_exp_year: Option<i32>,
    cardholder_name: Option<String>,
    bank_name: Option<String>,
    account_last4: Option<String>,
    external_id: Option<String>,
    billing_address: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPaymentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_payment(row: PaymentRow) -> Result<Payment> {
        let PaymentRow {
            id,
            payment_number,
            order_id,
            invoice_id,
            customer_id,
            status,
            payment_method,
            amount,
            currency,
            amount_refunded,
            external_id,
            idempotency_key,
            processor,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            billing_email,
            billing_name,
            billing_address,
            description,
            failure_reason,
            failure_code,
            metadata,
            paid_at,
            version,
            created_at,
            updated_at,
        } = row;

        let status: PaymentTransactionStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid payment.status '{}': {}", status, e))
        })?;
        let payment_method: PaymentMethodType = payment_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment.payment_method '{}': {}",
                payment_method, e
            ))
        })?;
        let card_brand = match card_brand {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment.card_brand '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(Payment {
            id: PaymentId::from(id),
            payment_number,
            order_id: order_id.map(OrderId::from),
            invoice_id,
            customer_id: customer_id.map(CustomerId::from),
            status,
            payment_method,
            amount,
            currency,
            amount_refunded,
            external_id,
            idempotency_key,
            processor,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            blockchain_network: None,
            stablecoin_type: None,
            from_wallet_address: None,
            to_wallet_address: None,
            tx_hash: None,
            block_number: None,
            confirmations: None,
            token_address: None,
            ves_intent_id: None,
            billing_email,
            billing_name,
            billing_address,
            description,
            failure_reason,
            failure_code,
            metadata,
            paid_at,
            version,
            created_at,
            updated_at,
        })
    }

    fn row_to_refund(row: RefundRow) -> Result<Refund> {
        let RefundRow {
            id,
            refund_number,
            payment_id,
            status,
            amount,
            currency,
            reason,
            external_id,
            idempotency_key,
            failure_reason,
            notes,
            refunded_at,
            created_at,
            updated_at,
        } = row;

        let status: RefundStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid refund.status '{}': {}", status, e))
        })?;

        Ok(Refund {
            id,
            refund_number,
            payment_id: PaymentId::from(payment_id),
            status,
            amount,
            currency,
            reason,
            external_id,
            idempotency_key,
            failure_reason,
            notes,
            refunded_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_payment_method(row: PaymentMethodRow) -> Result<PaymentMethod> {
        let PaymentMethodRow {
            id,
            customer_id,
            method_type,
            is_default,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            cardholder_name,
            bank_name,
            account_last4,
            external_id,
            billing_address,
            created_at,
            updated_at,
        } = row;

        let method_type: PaymentMethodType = method_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment_method.method_type '{}': {}",
                method_type, e
            ))
        })?;
        let card_brand = match card_brand {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment_method.card_brand '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(PaymentMethod {
            id,
            customer_id: CustomerId::from(customer_id),
            method_type,
            is_default,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            cardholder_name,
            bank_name,
            account_last4,
            wallet_address: None,
            blockchain_network: None,
            stablecoin_type: None,
            external_id,
            billing_address,
            created_at,
            updated_at,
        })
    }

    /// Create payment (async)
    pub async fn create_async(&self, input: CreatePayment) -> Result<Payment> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key_async(key).await? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let payment_number = generate_payment_number();

        sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
             card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
             billing_address, description, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
        )
        .bind(id)
        .bind(&payment_number)
        .bind(input.order_id.map(|oid| oid.into_uuid()))
        .bind(input.invoice_id)
        .bind(input.customer_id.map(|cid| cid.into_uuid()))
        .bind(PaymentTransactionStatus::Pending.to_string())
        .bind(input.payment_method.to_string())
        .bind(input.amount)
        .bind(input.currency.unwrap_or(CurrencyCode::USD))
        .bind(Decimal::ZERO)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.processor)
        .bind(input.card_brand.map(|b| b.to_string()))
        .bind(&input.card_last4)
        .bind(input.card_exp_month)
        .bind(input.card_exp_year)
        .bind(&input.billing_email)
        .bind(&input.billing_name)
        .bind(&input.billing_address)
        .bind(&input.description)
        .bind(&input.metadata)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get payment by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Get payment by number (async)
    pub async fn get_by_number_async(&self, payment_number: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE payment_number = $1"
        )
        .bind(payment_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Get payment by external ID (async)
    pub async fn get_by_external_id_async(&self, external_id: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE external_id = $1"
        )
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    async fn get_by_idempotency_key_async(&self, key: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE idempotency_key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Update payment (async)
    pub async fn update_async(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        let payment = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE payments SET status = $1, external_id = $2, failure_reason = $3,
             failure_code = $4, metadata = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(input.status.unwrap_or(payment.status).to_string())
        .bind(input.external_id.or(payment.external_id))
        .bind(input.failure_reason.or(payment.failure_reason))
        .bind(input.failure_code.or(payment.failure_code))
        .bind(input.metadata.or(payment.metadata))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List payments (async)
    pub async fn list_async(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE 1=1"
        );
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.invoice_id.is_some() {
            query.push_str(&format!(" AND invoice_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PaymentRow>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id.into_uuid());
        }
        if let Some(invoice_id) = filter.invoice_id {
            q = q.bind(invoice_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(rows.len());
        for row in rows {
            payments.push(Self::row_to_payment(row)?);
        }
        Ok(payments)
    }

    /// Get payments for order (async)
    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter {
            order_id: Some(OrderId::from(order_id)),
            ..Default::default()
        })
        .await
    }

    /// Get payments for invoice (async)
    pub async fn for_invoice_async(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter { invoice_id: Some(invoice_id), ..Default::default() }).await
    }

    /// Mark payment as processing (async)
    pub async fn mark_processing_async(&self, id: Uuid) -> Result<Payment> {
        self.update_async(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Processing),
                ..Default::default()
            },
        )
        .await
    }

    /// Mark payment as completed (async)
    pub async fn mark_completed_async(&self, id: Uuid) -> Result<Payment> {
        let now = Utc::now();

        sqlx::query("UPDATE payments SET status = $1, paid_at = $2, updated_at = $3 WHERE id = $4")
            .bind(PaymentTransactionStatus::Completed.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Mark payment as failed (async)
    pub async fn mark_failed_async(
        &self,
        id: Uuid,
        reason: &str,
        code: Option<&str>,
    ) -> Result<Payment> {
        let now = Utc::now();

        sqlx::query("UPDATE payments SET status = $1, failure_reason = $2, failure_code = $3, updated_at = $4 WHERE id = $5")
            .bind(PaymentTransactionStatus::Failed.to_string())
            .bind(reason)
            .bind(code)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel payment (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<Payment> {
        self.update_async(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Cancelled),
                ..Default::default()
            },
        )
        .await
    }

    /// Create refund (async)
    pub async fn create_refund_async(&self, input: CreateRefund) -> Result<Refund> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_refund_by_idempotency_key_async(key).await? {
                return Ok(existing);
            }
        }

        let raw_payment_id = input.payment_id.into_uuid();
        let payment = self.get_async(raw_payment_id).await?.ok_or(CommerceError::NotFound)?;
        let refund_amount = input.amount.unwrap_or(payment.amount - payment.amount_refunded);

        let id = Uuid::new_v4();
        let now = Utc::now();
        let refund_number = generate_refund_number();

        sqlx::query(
            "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
        )
        .bind(id)
        .bind(&refund_number)
        .bind(raw_payment_id)
        .bind(RefundStatus::Pending.to_string())
        .bind(refund_amount)
        .bind(payment.currency)
        .bind(&input.reason)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get refund by ID (async)
    pub async fn get_refund_async(&self, id: Uuid) -> Result<Option<Refund>> {
        let row = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_refund).transpose()
    }

    async fn get_refund_by_idempotency_key_async(&self, key: &str) -> Result<Option<Refund>> {
        let row = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE idempotency_key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_refund).transpose()
    }

    /// Get refunds for payment (async)
    pub async fn get_refunds_async(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        let rows = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE payment_id = $1 ORDER BY created_at DESC"
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut refunds = Vec::with_capacity(rows.len());
        for row in rows {
            refunds.push(Self::row_to_refund(row)?);
        }
        Ok(refunds)
    }

    /// Complete refund (async)
    pub async fn complete_refund_async(&self, id: Uuid) -> Result<Refund> {
        let refund = self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE refunds SET status = $1, refunded_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(RefundStatus::Completed.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Update payment amount_refunded
        sqlx::query(
            "UPDATE payments SET amount_refunded = amount_refunded + $1, status = CASE
             WHEN amount_refunded + $2 >= amount THEN 'refunded' ELSE 'partially_refunded' END,
             updated_at = $3 WHERE id = $4",
        )
        .bind(refund.amount)
        .bind(refund.amount)
        .bind(now)
        .bind(refund.payment_id.into_uuid())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Fail refund (async)
    pub async fn fail_refund_async(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE refunds SET status = $1, failure_reason = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(RefundStatus::Failed.to_string())
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create payment method (async)
    pub async fn create_payment_method_async(
        &self,
        input: CreatePaymentMethod,
    ) -> Result<PaymentMethod> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // If setting as default, clear existing default
        if input.is_default.unwrap_or(false) {
            sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
                .bind(input.customer_id.into_uuid())
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        sqlx::query(
            "INSERT INTO payment_methods (id, customer_id, method_type, is_default, card_brand,
             card_last4, card_exp_month, card_exp_year, cardholder_name, bank_name, account_last4,
             external_id, billing_address, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(id)
        .bind(input.customer_id.into_uuid())
        .bind(input.method_type.to_string())
        .bind(input.is_default.unwrap_or(false))
        .bind(input.card_brand.map(|b| b.to_string()))
        .bind(&input.card_last4)
        .bind(input.card_exp_month)
        .bind(input.card_exp_year)
        .bind(&input.cardholder_name)
        .bind(&input.bank_name)
        .bind(&input.account_last4)
        .bind(&input.external_id)
        .bind(&input.billing_address)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, PaymentMethodRow>(
            "SELECT id, customer_id, method_type, is_default, card_brand, card_last4, card_exp_month,
             card_exp_year, cardholder_name, bank_name, account_last4, external_id, billing_address,
             created_at, updated_at FROM payment_methods WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_payment_method(row)
    }

    /// Get payment methods for customer (async)
    pub async fn get_payment_methods_async(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        let rows = sqlx::query_as::<_, PaymentMethodRow>(
            "SELECT id, customer_id, method_type, is_default, card_brand, card_last4, card_exp_month,
             card_exp_year, cardholder_name, bank_name, account_last4, external_id, billing_address,
             created_at, updated_at FROM payment_methods WHERE customer_id = $1
             ORDER BY is_default DESC, created_at DESC"
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut methods = Vec::with_capacity(rows.len());
        for row in rows {
            methods.push(Self::row_to_payment_method(row)?);
        }
        Ok(methods)
    }

    /// Delete payment method (async)
    pub async fn delete_payment_method_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM payment_methods WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Set default payment method (async)
    pub async fn set_default_payment_method_async(
        &self,
        customer_id: Uuid,
        method_id: Uuid,
    ) -> Result<()> {
        sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
            .bind(customer_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE payment_methods SET is_default = true WHERE id = $1 AND customer_id = $2",
        )
        .bind(method_id)
        .bind(customer_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Count payments (async)
    pub async fn count_async(&self, filter: PaymentFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM payments WHERE 1=1");
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.invoice_id.is_some() {
            query.push_str(&format!(" AND invoice_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id.into_uuid());
        }
        if let Some(invoice_id) = filter.invoice_id {
            q = q.bind(invoice_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    /// Delete payment (async) - hard delete
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // Delete associated refunds first
        sqlx::query("DELETE FROM refunds WHERE payment_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM payments WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple payments - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreatePayment>,
    ) -> Result<BatchResult<Payment>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple payments - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreatePayment>,
    ) -> Result<Vec<Payment>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let payment_number = generate_payment_number();

            sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
            )
            .bind(id)
            .bind(&payment_number)
            .bind(input.order_id.map(|oid| oid.into_uuid()))
            .bind(input.invoice_id)
            .bind(input.customer_id.map(|cid| cid.into_uuid()))
            .bind(PaymentTransactionStatus::Pending.to_string())
            .bind(input.payment_method.to_string())
            .bind(input.amount)
            .bind(input.currency.unwrap_or(CurrencyCode::USD))
            .bind(Decimal::ZERO)
            .bind(&input.external_id)
            .bind(&input.idempotency_key)
            .bind(&input.processor)
            .bind(input.card_brand.map(|b| b.to_string()))
            .bind(&input.card_last4)
            .bind(input.card_exp_month)
            .bind(input.card_exp_year)
            .bind(&input.billing_email)
            .bind(&input.billing_name)
            .bind(&input.billing_address)
            .bind(&input.description)
            .bind(&input.metadata)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            payments.push(Payment {
                id: PaymentId::from(id),
                payment_number,
                order_id: input.order_id,
                invoice_id: input.invoice_id,
                customer_id: input.customer_id,
                status: PaymentTransactionStatus::Pending,
                payment_method: input.payment_method,
                amount: input.amount,
                currency: input.currency.unwrap_or(CurrencyCode::USD),
                amount_refunded: Decimal::ZERO,
                external_id: input.external_id,
                idempotency_key: input.idempotency_key,
                processor: input.processor,
                card_brand: input.card_brand,
                card_last4: input.card_last4,
                card_exp_month: input.card_exp_month,
                card_exp_year: input.card_exp_year,
                blockchain_network: None,
                stablecoin_type: None,
                from_wallet_address: None,
                to_wallet_address: None,
                tx_hash: None,
                block_number: None,
                confirmations: None,
                token_address: None,
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

        tx.commit().await.map_err(map_db_error)?;
        Ok(payments)
    }

    /// Update multiple payments - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.update_async(raw_id, input).await {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple payments - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<Vec<Payment>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let raw_id = id.into_uuid();
            let payment = sqlx::query_as::<_, PaymentRow>(
                "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                 amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
                 card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
                 description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
                 FROM payments WHERE id = $1"
            )
            .bind(raw_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .map(Self::row_to_payment)
            .transpose()?
            .ok_or(CommerceError::NotFound)?;

            let now = Utc::now();

            sqlx::query(
                "UPDATE payments SET status = $1, external_id = $2, failure_reason = $3,
                 failure_code = $4, metadata = $5, updated_at = $6 WHERE id = $7",
            )
            .bind(input.status.unwrap_or(payment.status).to_string())
            .bind(input.external_id.or(payment.external_id))
            .bind(input.failure_reason.or(payment.failure_reason))
            .bind(input.failure_code.or(payment.failure_code))
            .bind(input.metadata.or(payment.metadata))
            .bind(now)
            .bind(raw_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch the updated payment
            let updated_row = sqlx::query_as::<_, PaymentRow>(
                "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                 amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
                 card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
                 description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
                 FROM payments WHERE id = $1"
            )
            .bind(raw_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            payments.push(Self::row_to_payment(updated_row)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(payments)
    }

    /// Delete multiple payments - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.delete_async(raw_id).await {
                Ok(()) => result.record_success(raw_id),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple payments - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<PaymentId>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete associated refunds first (foreign key constraint)
        sqlx::query("DELETE FROM refunds WHERE payment_id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete payments
        sqlx::query("DELETE FROM payments WHERE id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple payments by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        let rows = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = ANY($1)"
        )
        .bind(&raw_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut payments = Vec::with_capacity(rows.len());
        for row in rows {
            payments.push(Self::row_to_payment(row)?);
        }
        Ok(payments)
    }
}

impl PaymentRepository for PgPaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PaymentId) -> Result<Option<Payment>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        super::block_on(self.get_by_number_async(payment_number))
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        super::block_on(self.get_by_external_id_async(external_id))
    }

    fn update(&self, id: PaymentId, input: UpdatePayment) -> Result<Payment> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        super::block_on(self.list_async(filter))
    }

    fn for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        super::block_on(self.for_order_async(order_id.into_uuid()))
    }

    fn for_invoice(&self, invoice_id: InvoiceId) -> Result<Vec<Payment>> {
        super::block_on(self.for_invoice_async(invoice_id.into_uuid()))
    }

    fn mark_processing(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.mark_processing_async(id.into_uuid()))
    }

    fn mark_completed(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.mark_completed_async(id.into_uuid()))
    }

    fn mark_failed(&self, id: PaymentId, reason: &str, code: Option<&str>) -> Result<Payment> {
        super::block_on(self.mark_failed_async(id.into_uuid(), reason, code))
    }

    fn cancel(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.cancel_async(id.into_uuid()))
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        super::block_on(self.create_refund_async(input))
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        super::block_on(self.get_refund_async(id))
    }

    fn get_refunds(&self, payment_id: PaymentId) -> Result<Vec<Refund>> {
        super::block_on(self.get_refunds_async(payment_id.into_uuid()))
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        super::block_on(self.complete_refund_async(id))
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        super::block_on(self.fail_refund_async(id, reason))
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        super::block_on(self.create_payment_method_async(input))
    }

    fn get_payment_methods(&self, customer_id: CustomerId) -> Result<Vec<PaymentMethod>> {
        super::block_on(self.get_payment_methods_async(customer_id.into_uuid()))
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_payment_method_async(id))
    }

    fn set_default_payment_method(&self, customer_id: CustomerId, method_id: Uuid) -> Result<()> {
        super::block_on(self.set_default_payment_method_async(customer_id.into_uuid(), method_id))
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<Vec<Payment>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<PaymentId>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>> {
        super::block_on(self.get_batch_async(ids))
    }
}
