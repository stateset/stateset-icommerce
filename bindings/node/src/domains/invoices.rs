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
    pub quantity: f64,
    /// Float unit price. Optional: send `unit_price_exact` instead for exact money.
    pub unit_price: Option<f64>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    pub sku: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInvoiceInput {
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
    pub id: String,
    pub invoice_number: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    #[napi(ts_type = "InvoiceStatus")]
    pub status: String,
    /// @deprecated Use the `subtotalExact` twin; float money will be removed in 2.0.
    pub subtotal: f64,
    /// Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub subtotal_exact: String,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    /// Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_amount_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
    /// @deprecated Use the `amountPaidExact` twin; float money will be removed in 2.0.
    pub amount_paid: f64,
    /// Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_paid_exact: String,
    pub due_date: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Invoice> for InvoiceOutput {
    type Error = Error;

    fn try_from(inv: stateset_core::Invoice) -> Result<Self> {
        let (subtotal, subtotal_exact) = money_pair(inv.subtotal, "invoice subtotal")?;
        let (tax_amount, tax_amount_exact) = money_pair(inv.tax_amount, "invoice tax amount")?;
        let (total, total_exact) = money_pair(inv.total, "invoice total")?;
        let (amount_paid, amount_paid_exact) = money_pair(inv.amount_paid, "invoice amount paid")?;
        Ok(Self {
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
    /// Float amount. Optional: send `amount_exact` instead for exact money.
    pub amount: Option<f64>,
    /// Exact base-10 amount. Takes precedence over `amount` when present.
    pub amount_exact: Option<String>,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
}

#[napi]
pub struct Invoices {
    pub(crate) commerce: Handle,
}

#[napi]
impl Invoices {
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
                    quantity: decimal_from_f64(i.quantity, "invoice item quantity")?,
                    unit_price: money_input(
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
                    amount: money_input(
                        input.amount_exact.as_deref(),
                        input.amount,
                        "invoice payment amount",
                    )?,
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
