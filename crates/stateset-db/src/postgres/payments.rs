//! PostgreSQL implementation of payment repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    generate_payment_number, generate_refund_number, CardBrand, CommerceError, CreatePayment,
    CreatePaymentMethod, CreateRefund, Payment, PaymentFilter, PaymentMethod, PaymentMethodType,
    PaymentRepository, PaymentTransactionStatus, Refund, RefundStatus, Result, UpdatePayment,
};
use uuid::Uuid;

/// PostgreSQL payment repository
#[derive(Clone)]
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
    currency: String,
    amount_refunded: Decimal,
    external_id: Option<String>,
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
    currency: String,
    reason: Option<String>,
    external_id: Option<String>,
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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_status(s: &str) -> PaymentTransactionStatus {
        s.parse().unwrap_or_default()
    }

    fn parse_method_type(s: &str) -> PaymentMethodType {
        s.parse().unwrap_or_default()
    }

    fn parse_card_brand(s: &str) -> CardBrand {
        s.parse().unwrap_or_default()
    }

    fn parse_refund_status(s: &str) -> RefundStatus {
        s.parse().unwrap_or_default()
    }

    fn row_to_payment(row: PaymentRow) -> Payment {
        Payment {
            id: row.id,
            payment_number: row.payment_number,
            order_id: row.order_id,
            invoice_id: row.invoice_id,
            customer_id: row.customer_id,
            status: Self::parse_status(&row.status),
            payment_method: Self::parse_method_type(&row.payment_method),
            amount: row.amount,
            currency: row.currency,
            amount_refunded: row.amount_refunded,
            external_id: row.external_id,
            processor: row.processor,
            card_brand: row.card_brand.map(|s| Self::parse_card_brand(&s)),
            card_last4: row.card_last4,
            card_exp_month: row.card_exp_month,
            card_exp_year: row.card_exp_year,
            billing_email: row.billing_email,
            billing_name: row.billing_name,
            billing_address: row.billing_address,
            description: row.description,
            failure_reason: row.failure_reason,
            failure_code: row.failure_code,
            metadata: row.metadata,
            paid_at: row.paid_at,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_refund(row: RefundRow) -> Refund {
        Refund {
            id: row.id,
            refund_number: row.refund_number,
            payment_id: row.payment_id,
            status: Self::parse_refund_status(&row.status),
            amount: row.amount,
            currency: row.currency,
            reason: row.reason,
            external_id: row.external_id,
            failure_reason: row.failure_reason,
            notes: row.notes,
            refunded_at: row.refunded_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_payment_method(row: PaymentMethodRow) -> PaymentMethod {
        PaymentMethod {
            id: row.id,
            customer_id: row.customer_id,
            method_type: Self::parse_method_type(&row.method_type),
            is_default: row.is_default,
            card_brand: row.card_brand.map(|s| Self::parse_card_brand(&s)),
            card_last4: row.card_last4,
            card_exp_month: row.card_exp_month,
            card_exp_year: row.card_exp_year,
            cardholder_name: row.cardholder_name,
            bank_name: row.bank_name,
            account_last4: row.account_last4,
            external_id: row.external_id,
            billing_address: row.billing_address,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    /// Create payment (async)
    pub async fn create_async(&self, input: CreatePayment) -> Result<Payment> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let payment_number = generate_payment_number();

        sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, processor,
             card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
             billing_address, description, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)"
        )
        .bind(id)
        .bind(&payment_number)
        .bind(input.order_id)
        .bind(input.invoice_id)
        .bind(input.customer_id)
        .bind(PaymentTransactionStatus::Pending.to_string())
        .bind(input.payment_method.to_string())
        .bind(input.amount)
        .bind(input.currency.unwrap_or_else(|| "USD".to_string()))
        .bind(Decimal::ZERO)
        .bind(&input.external_id)
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
             amount, currency, amount_refunded, external_id, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_payment))
    }

    /// Get payment by number (async)
    pub async fn get_by_number_async(&self, payment_number: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE payment_number = $1"
        )
        .bind(payment_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_payment))
    }

    /// Get payment by external ID (async)
    pub async fn get_by_external_id_async(&self, external_id: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE external_id = $1"
        )
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_payment))
    }

    /// Update payment (async)
    pub async fn update_async(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        let payment = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE payments SET status = $1, external_id = $2, failure_reason = $3,
             failure_code = $4, metadata = $5, updated_at = $6 WHERE id = $7"
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
             amount, currency, amount_refunded, external_id, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE 1=1"
        );
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
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

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

        let mut q = sqlx::query_as::<_, PaymentRow>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_payment).collect())
    }

    /// Get payments for order (async)
    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter { order_id: Some(order_id), ..Default::default() }).await
    }

    /// Get payments for invoice (async)
    pub async fn for_invoice_async(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter { invoice_id: Some(invoice_id), ..Default::default() }).await
    }

    /// Mark payment as processing (async)
    pub async fn mark_processing_async(&self, id: Uuid) -> Result<Payment> {
        self.update_async(id, UpdatePayment { status: Some(PaymentTransactionStatus::Processing), ..Default::default() }).await
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
    pub async fn mark_failed_async(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
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
        self.update_async(id, UpdatePayment { status: Some(PaymentTransactionStatus::Cancelled), ..Default::default() }).await
    }

    /// Create refund (async)
    pub async fn create_refund_async(&self, input: CreateRefund) -> Result<Refund> {
        let payment = self.get_async(input.payment_id).await?.ok_or(CommerceError::NotFound)?;
        let refund_amount = input.amount.unwrap_or(payment.amount - payment.amount_refunded);

        let id = Uuid::new_v4();
        let now = Utc::now();
        let refund_number = generate_refund_number();

        sqlx::query(
            "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
        )
        .bind(id)
        .bind(&refund_number)
        .bind(input.payment_id)
        .bind(RefundStatus::Pending.to_string())
        .bind(refund_amount)
        .bind(&payment.currency)
        .bind(&input.reason)
        .bind(&input.external_id)
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
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_refund))
    }

    /// Get refunds for payment (async)
    pub async fn get_refunds_async(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        let rows = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE payment_id = $1 ORDER BY created_at DESC"
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_refund).collect())
    }

    /// Complete refund (async)
    pub async fn complete_refund_async(&self, id: Uuid) -> Result<Refund> {
        let refund = self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query("UPDATE refunds SET status = $1, refunded_at = $2, updated_at = $3 WHERE id = $4")
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
             updated_at = $3 WHERE id = $4"
        )
        .bind(refund.amount)
        .bind(refund.amount)
        .bind(now)
        .bind(refund.payment_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Fail refund (async)
    pub async fn fail_refund_async(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let now = Utc::now();

        sqlx::query("UPDATE refunds SET status = $1, failure_reason = $2, updated_at = $3 WHERE id = $4")
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
    pub async fn create_payment_method_async(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // If setting as default, clear existing default
        if input.is_default.unwrap_or(false) {
            sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
                .bind(input.customer_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        sqlx::query(
            "INSERT INTO payment_methods (id, customer_id, method_type, is_default, card_brand,
             card_last4, card_exp_month, card_exp_year, cardholder_name, bank_name, account_last4,
             external_id, billing_address, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
        )
        .bind(id)
        .bind(input.customer_id)
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

        Ok(Self::row_to_payment_method(row))
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

        Ok(rows.into_iter().map(Self::row_to_payment_method).collect())
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
    pub async fn set_default_payment_method_async(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
            .bind(customer_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("UPDATE payment_methods SET is_default = true WHERE id = $1 AND customer_id = $2")
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
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }
}

impl PaymentRepository for PgPaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Payment>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(payment_number))
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        tokio::runtime::Handle::current().block_on(self.get_by_external_id_async(external_id))
    }

    fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        tokio::runtime::Handle::current().block_on(self.for_order_async(order_id))
    }

    fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        tokio::runtime::Handle::current().block_on(self.for_invoice_async(invoice_id))
    }

    fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.mark_processing_async(id))
    }

    fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.mark_completed_async(id))
    }

    fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.mark_failed_async(id, reason, code))
    }

    fn cancel(&self, id: Uuid) -> Result<Payment> {
        tokio::runtime::Handle::current().block_on(self.cancel_async(id))
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        tokio::runtime::Handle::current().block_on(self.create_refund_async(input))
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        tokio::runtime::Handle::current().block_on(self.get_refund_async(id))
    }

    fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        tokio::runtime::Handle::current().block_on(self.get_refunds_async(payment_id))
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        tokio::runtime::Handle::current().block_on(self.complete_refund_async(id))
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        tokio::runtime::Handle::current().block_on(self.fail_refund_async(id, reason))
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        tokio::runtime::Handle::current().block_on(self.create_payment_method_async(input))
    }

    fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        tokio::runtime::Handle::current().block_on(self.get_payment_methods_async(customer_id))
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_payment_method_async(id))
    }

    fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.set_default_payment_method_async(customer_id, method_id))
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}
