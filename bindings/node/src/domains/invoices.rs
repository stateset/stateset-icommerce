//! Invoices API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Invoices API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInvoiceItemInput {
    pub description: String,
    pub quantity: String,
    /// Exact decimal unit price; `unit_price_exact` remains an alias.
    pub unit_price: Option<String>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    pub sku: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInvoiceInput {
    pub currency: Option<String>,
    pub due_date: Option<String>,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub items: Vec<CreateInvoiceItemInput>,
    pub billing_email: Option<String>,
    pub billing_name: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InvoiceOutput {
    pub currency: String,
    pub balance_due: String,
    pub id: String,
    pub invoice_number: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    #[napi(ts_type = "InvoiceStatus")]
    pub status: String,
    /// Exact decimal string.
    pub subtotal: String,
    /// Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub subtotal_exact: String,
    /// Exact decimal string.
    pub tax_amount: String,
    /// Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_amount_exact: String,
    /// Exact decimal string.
    pub total: String,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
    /// Exact decimal string.
    pub amount_paid: String,
    /// Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_paid_exact: String,
    pub due_date: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Invoice> for InvoiceOutput {
    type Error = Error;

    fn try_from(inv: stateset_core::Invoice) -> Result<Self> {
        let subtotal = inv.subtotal.to_string();
        let subtotal_exact = subtotal.clone();
        let tax_amount = inv.tax_amount.to_string();
        let tax_amount_exact = tax_amount.clone();
        let total = inv.total.to_string();
        let total_exact = total.clone();
        let amount_paid = inv.amount_paid.to_string();
        let amount_paid_exact = amount_paid.clone();
        Ok(Self {
            currency: inv.currency.to_string(),
            balance_due: inv.balance_due.to_string(),
            id: inv.id.to_string(),
            invoice_number: inv.invoice_number,
            customer_id: inv.customer_id.to_string(),
            order_id: inv.order_id.map(|id| id.to_string()),
            status: format!("{}", inv.status),
            subtotal,
            subtotal_exact,
            tax_amount,
            tax_amount_exact,
            total,
            total_exact,
            amount_paid,
            amount_paid_exact,
            due_date: inv.due_date.to_rfc3339(),
            created_at: inv.created_at.to_rfc3339(),
            updated_at: inv.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordPaymentInput {
    /// Exact decimal amount; `amount_exact` remains an alias.
    pub amount: Option<String>,
    /// Exact base-10 amount. Takes precedence over `amount` when present.
    pub amount_exact: Option<String>,
    /// Stable nonzero UUID for retry-safe recording of this invoice payment.
    pub payment_id: Option<String>,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
}

#[napi]
pub struct Invoices {
    pub(crate) commerce: Handle,
}

#[napi]
impl Invoices {
    /// This binding consumes paymentId atomically and rejects amount conflicts.
    #[napi]
    pub fn supports_payment_idempotency(&self) -> bool {
        true
    }

    #[napi]
    pub async fn create(&self, input: CreateInvoiceInput) -> Result<InvoiceOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;

        let items: Vec<stateset_core::CreateInvoiceItem> = input
            .items
            .into_iter()
            .map(|i| {
                Ok(stateset_core::CreateInvoiceItem {
                    description: i.description,
                    quantity: parse_decimal_str(&i.quantity, "invoice item quantity")?,
                    unit_price: finance_money_input(
                        i.unit_price_exact.as_deref(),
                        i.unit_price,
                        "invoice item unit price",
                    )?,
                    sku: i.sku,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let invoice = commerce
            .invoices()
            .create(stateset_core::CreateInvoice {
                customer_id,
                currency: parse_optional_enum(input.currency, "currency")?,
                due_date: parse_optional_datetime(input.due_date, "due date")?,
                order_id,
                items,
                billing_email: input.billing_email,
                billing_name: input.billing_name,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create invoice", e))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<InvoiceOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get invoice", e))?;

        convert_optional_output(invoice)
    }

    /// List invoices, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every invoice).
    #[napi]
    pub async fn list(&self, filter: Option<InvoiceFilterInput>) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.get()?;
        let filter = invoice_filter_from_input(filter)?;
        let invoices = commerce
            .invoices()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list invoices", e))?;

        convert_outputs(invoices)
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to send invoice", e))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn void(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to void invoice", e))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn record_payment(
        &self,
        id: String,
        input: RecordPaymentInput,
    ) -> Result<InvoiceOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .record_payment(
                uuid,
                stateset_core::RecordInvoicePayment {
                    amount: finance_money_input(
                        input.amount_exact.as_deref(),
                        input.amount,
                        "invoice payment amount",
                    )?,
                    payment_id: input
                        .payment_id
                        .map(|value| {
                            value
                                .parse::<uuid::Uuid>()
                                .map_err(|_| coded(ErrCode::Validation, "Invalid payment UUID"))
                        })
                        .transpose()?,
                    payment_method: input.payment_method,
                    reference: input.reference,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to record payment", e))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn get_overdue(&self) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.get()?;
        let invoices = commerce
            .invoices()
            .get_overdue()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get overdue invoices", e))?;

        convert_outputs(invoices)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .invoices()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count invoices", e))?;

        Ok(count as u32)
    }
}
