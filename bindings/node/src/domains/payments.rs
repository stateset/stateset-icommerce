//! Payments API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Payments API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePaymentInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    pub idempotency_key: Option<String>,
    /// Float amount. Optional: send `amount_exact` instead for exact money.
    pub amount: Option<f64>,
    /// Exact base-10 amount. Takes precedence over `amount` when present.
    pub amount_exact: Option<String>,
    pub currency: Option<String>,
    /// Defaults to `credit_card`.
    #[napi(ts_type = "PaymentMethodType")]
    pub payment_method: Option<String>,
}

/// Exact-money payment input for agent and financial integrations.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePaymentExactInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub amount: String,
    pub currency: Option<String>,
    /// Defaults to `credit_card`.
    #[napi(ts_type = "PaymentMethodType")]
    pub payment_method: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentOutput {
    pub id: String,
    pub payment_number: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    pub idempotency_key: Option<String>,
    /// @deprecated Use the `amountExact` twin; float money will be removed in 2.0.
    pub amount: f64,
    /// Exact base-10 amount. Prefer this field for all calculations.
    pub amount_exact: String,
    pub currency: String,
    #[napi(ts_type = "PaymentTransactionStatus")]
    pub status: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Payment> for PaymentOutput {
    type Error = Error;

    fn try_from(p: stateset_core::Payment) -> Result<Self> {
        let (amount, amount_exact) = money_pair(p.amount, "payment amount")?;
        Ok(Self {
            id: p.id.to_string(),
            payment_number: p.payment_number,
            order_id: p.order_id.map(|id| id.to_string()),
            invoice_id: p.invoice_id.map(|id| id.to_string()),
            customer_id: p.customer_id.map(|id| id.to_string()),
            idempotency_key: p.idempotency_key,
            amount,
            amount_exact,
            currency: p.currency.to_string(),
            status: format!("{}", p.status),
            version: p.version,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRefundInput {
    pub payment_id: String,
    /// Float amount. Optional: send `amount_exact` instead for exact money.
    pub amount: Option<f64>,
    /// Exact base-10 amount. Takes precedence over `amount` when present.
    pub amount_exact: Option<String>,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
}

/// Exact-money refund input for agent and financial integrations.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRefundExactInput {
    pub payment_id: String,
    pub amount: String,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RefundOutput {
    pub id: String,
    pub refund_number: String,
    pub payment_id: String,
    /// @deprecated Use the `amountExact` twin; float money will be removed in 2.0.
    pub amount: f64,
    /// Exact base-10 amount. Prefer this field for all calculations.
    pub amount_exact: String,
    #[napi(ts_type = "RefundStatus")]
    pub status: String,
    pub reason: Option<String>,
    pub created_at: String,
    pub idempotency_key: Option<String>,
}

impl TryFrom<stateset_core::Refund> for RefundOutput {
    type Error = Error;

    fn try_from(r: stateset_core::Refund) -> Result<Self> {
        let (amount, amount_exact) = money_pair(r.amount, "refund amount")?;
        Ok(Self {
            id: r.id.to_string(),
            refund_number: r.refund_number,
            payment_id: r.payment_id.to_string(),
            amount,
            amount_exact,
            status: format!("{}", r.status),
            reason: r.reason,
            created_at: r.created_at.to_rfc3339(),
            idempotency_key: r.idempotency_key,
        })
    }
}

#[napi]
pub struct Payments {
    pub(crate) commerce: Handle,
}

#[napi]
impl Payments {
    #[napi]
    pub async fn create(&self, input: CreatePaymentInput) -> Result<PaymentOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = input
            .customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;

        let invoice_id = input
            .invoice_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid invoice UUID"))?;

        let payment_method = input
            .payment_method
            .map(|method| {
                method.parse::<stateset_core::PaymentMethodType>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid payment method '{method}'"))
                })
            })
            .transpose()?
            .unwrap_or_default();

        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id,
                invoice_id,
                customer_id,
                idempotency_key: input.idempotency_key,
                amount: money_input(input.amount_exact.as_deref(), input.amount, "payment amount")?,
                currency: parse_optional_currency(input.currency)?,
                payment_method,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create payment", e))?;

        convert_output(payment)
    }

    /// Create a payment without any floating-point conversion.
    #[napi]
    pub async fn create_exact(&self, input: CreatePaymentExactInput) -> Result<PaymentOutput> {
        let commerce = self.commerce.get()?;
        let customer_id = input
            .customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;
        let invoice_id = input
            .invoice_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid invoice UUID"))?;
        let currency = input
            .currency
            .unwrap_or_else(|| "USD".to_string())
            .parse::<CurrencyCode>()
            .map_err(|error| wrap(ErrCode::Validation, "Invalid currency", error))?;
        // `MoneyWireError` is a stateset-primitives error, not a `CommerceError`,
        // so `classify_cause` cannot recognise it and the fallback code is what
        // the caller sees. A malformed `amount` is bad input, not a bug in the
        // binding: `Internal` reported it as INTERNAL/500.
        let money = stateset_core::Money::from_decimal_str(&input.amount, currency)
            .map_err(|error| from_cause(ErrCode::Validation, error))?;
        let payment_method = input
            .payment_method
            .map(|method| {
                method.parse::<stateset_core::PaymentMethodType>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid payment method '{method}'"))
                })
            })
            .transpose()?
            .unwrap_or_default();
        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id,
                invoice_id,
                customer_id,
                idempotency_key: input.idempotency_key,
                amount: money.amount(),
                currency: Some(money.currency()),
                payment_method,
                ..Default::default()
            })
            .map_err(|error| wrap(ErrCode::Internal, "Failed to create payment", error))?;
        convert_output(payment)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PaymentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let payment = commerce
            .payments()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get payment", e))?;

        convert_optional_output(payment)
    }

    /// List payments, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every payment).
    #[napi]
    pub async fn list(&self, filter: Option<PaymentFilterInput>) -> Result<Vec<PaymentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = payment_filter_from_input(filter)?;
        let payments = commerce
            .payments()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list payments", e))?;

        convert_outputs(payments)
    }

    #[napi]
    pub async fn mark_completed(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_completed(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete payment", e))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn mark_failed(
        &self,
        id: String,
        reason: String,
        code: Option<String>,
    ) -> Result<PaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_failed(uuid.into(), &reason, code.as_deref())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to fail payment", e))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let payment = commerce
            .payments()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel payment", e))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn create_refund(&self, input: CreateRefundInput) -> Result<RefundOutput> {
        let commerce = self.commerce.get()?;
        let payment_id = input
            .payment_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid payment UUID"))?;

        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id,
                amount: Some(money_input(
                    input.amount_exact.as_deref(),
                    input.amount,
                    "refund amount",
                )?),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create refund", e))?;

        convert_output(refund)
    }

    /// Create a refund without any floating-point conversion.
    #[napi]
    pub async fn create_refund_exact(&self, input: CreateRefundExactInput) -> Result<RefundOutput> {
        let commerce = self.commerce.get()?;
        let payment_id = input
            .payment_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid payment UUID"))?;
        let payment = commerce
            .payments()
            .get(payment_id)
            .map_err(|error| wrap(ErrCode::Internal, "Failed to get payment", error))?
            .ok_or_else(|| coded(ErrCode::NotFound, "Payment not found"))?;
        // As above: a malformed refund `amount` is VALIDATION, not INTERNAL.
        let money = stateset_core::Money::from_decimal_str(&input.amount, payment.currency)
            .map_err(|error| from_cause(ErrCode::Validation, error))?;
        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id,
                amount: Some(money.amount()),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
                ..Default::default()
            })
            .map_err(|error| wrap(ErrCode::Internal, "Failed to create refund", error))?;
        convert_output(refund)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .payments()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count payments", e))?;

        Ok(count as u32)
    }
}
