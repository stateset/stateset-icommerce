//! Payment Obligations.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Payment Obligations
// ============================================================================

pub(crate) fn parse_payment_obligation_status(
    s: &str,
) -> Result<stateset_core::PaymentObligationStatus> {
    s.parse::<stateset_core::PaymentObligationStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid payment obligation status: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePaymentObligationInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    pub currency: Option<String>,
    /// Date string (YYYY-MM-DD)
    pub due_date: String,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationFilterInput {
    pub supplier_id: Option<String>,
    /// Snake-case status
    #[napi(ts_type = "PaymentObligationStatus")]
    pub status: Option<String>,
    /// Date string (YYYY-MM-DD)
    pub due_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub amount_paid: String,
    /// Exact decimal string
    pub outstanding: String,
    pub currency: String,
    /// Date string (YYYY-MM-DD)
    pub due_date: String,
    /// Snake-case status
    #[napi(ts_type = "PaymentObligationStatus")]
    pub status: String,
    pub linked_bill_ids: Vec<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PaymentObligation> for PaymentObligationOutput {
    fn from(o: stateset_core::PaymentObligation) -> Self {
        let outstanding = o.outstanding().to_string();
        Self {
            id: o.id.to_string(),
            number: o.number,
            supplier_id: o.supplier_id.to_string(),
            purchase_order_id: o.purchase_order_id.map(|id| id.to_string()),
            amount: o.amount.to_string(),
            amount_paid: o.amount_paid.to_string(),
            outstanding,
            currency: o.currency.to_string(),
            due_date: o.due_date.to_string(),
            status: o.status.to_string(),
            linked_bill_ids: o.linked_bill_ids.iter().map(ToString::to_string).collect(),
            notes: o.notes,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationDashboardOutput {
    pub open_count: String,
    /// Exact decimal string
    pub total_outstanding: String,
    pub overdue_count: String,
    /// Exact decimal string
    pub overdue_amount: String,
}

impl From<stateset_core::PaymentObligationDashboard> for PaymentObligationDashboardOutput {
    fn from(d: stateset_core::PaymentObligationDashboard) -> Self {
        Self {
            open_count: d.open_count.to_string(),
            total_outstanding: d.total_outstanding.to_string(),
            overdue_count: d.overdue_count.to_string(),
            overdue_amount: d.overdue_amount.to_string(),
        }
    }
}

#[napi]
pub struct PaymentObligations {
    pub(crate) commerce: Handle,
}

#[napi]
impl PaymentObligations {
    /// Whether the payment-obligations backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.payment_obligations().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreatePaymentObligationInput,
    ) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.get()?;
        let obligation = commerce
            .payment_obligations()
            .create(stateset_core::CreatePaymentObligation {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                due_date: parse_naive_date(&input.due_date, "due_date")?,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create payment obligation", e))?;
        Ok(obligation.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PaymentObligationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get payment obligation", e))?;
        Ok(obligation.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PaymentObligationFilterInput>,
    ) -> Result<Vec<PaymentObligationOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PaymentObligationFilter::default()),
            |f| -> Result<stateset_core::PaymentObligationFilter> {
                Ok(stateset_core::PaymentObligationFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f.status.as_deref().map(parse_payment_obligation_status).transpose()?,
                    due_before: f
                        .due_before
                        .as_deref()
                        .map(|d| parse_naive_date(d, "due_before"))
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let obligations = commerce
            .payment_obligations()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list payment obligations", e))?;
        Ok(obligations.into_iter().map(Into::into).collect())
    }

    /// Record a payment against an obligation.
    #[napi]
    pub async fn record_payment(
        &self,
        id: String,
        amount: String,
    ) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .record_payment(uuid.into(), parse_decimal_str(&amount, "amount")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to record payment", e))?;
        Ok(obligation.into())
    }

    /// Set the obligation status (e.g. `scheduled`, `cancelled`).
    #[napi]
    pub async fn set_status(
        &self,
        id: String,
        #[napi(ts_arg_type = "PaymentObligationStatus")] status: String,
    ) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .set_status(uuid.into(), parse_payment_obligation_status(&status)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set payment obligation status", e))?;
        Ok(obligation.into())
    }

    /// Link an AP bill to an obligation.
    #[napi]
    pub async fn link_bill(&self, id: String, bill_id: String) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let bill_uuid = parse_uuid_str(&bill_id, "bill_id")?;
        let obligation = commerce
            .payment_obligations()
            .link_bill(uuid.into(), bill_uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to link bill", e))?;
        Ok(obligation.into())
    }

    /// Aggregate dashboard summary as of the given date (YYYY-MM-DD).
    #[napi]
    pub async fn dashboard(&self, today: String) -> Result<PaymentObligationDashboardOutput> {
        let commerce = self.commerce.get()?;
        let day = parse_naive_date(&today, "today")?;
        let dashboard = commerce.payment_obligations().dashboard(day).map_err(|e| {
            wrap(ErrCode::Internal, "Failed to build payment obligation dashboard", e)
        })?;
        Ok(dashboard.into())
    }
}
