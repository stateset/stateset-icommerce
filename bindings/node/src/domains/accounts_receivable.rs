//! Accounts Receivable API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Accounts Receivable API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ArAgingSummaryOutput {
    /// Exact decimal string.
    pub current: String,
    /// Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money.
    pub current_exact: String,
    /// Exact decimal string.
    pub days_1_30: String,
    /// Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_1_30_exact: String,
    /// Exact decimal string.
    pub days_31_60: String,
    /// Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_31_60_exact: String,
    /// Exact decimal string.
    pub days_61_90: String,
    /// Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_61_90_exact: String,
    /// Exact decimal string.
    pub days_over_90: String,
    /// Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_over_90_exact: String,
    /// Exact decimal string.
    pub total: String,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
}

impl TryFrom<stateset_core::ArAgingSummary> for ArAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ArAgingSummary) -> Result<Self> {
        let current = a.current.to_string();
        let current_exact = current.clone();
        let days_1_30 = a.days_1_30.to_string();
        let days_1_30_exact = days_1_30.clone();
        let days_31_60 = a.days_31_60.to_string();
        let days_31_60_exact = days_31_60.clone();
        let days_61_90 = a.days_61_90.to_string();
        let days_61_90_exact = days_61_90.clone();
        let days_over_90 = a.days_over_90.to_string();
        let days_over_90_exact = days_over_90.clone();
        let total = a.total.to_string();
        let total_exact = total.clone();
        Ok(Self {
            current,
            current_exact,
            days_1_30,
            days_1_30_exact,
            days_31_60,
            days_31_60_exact,
            days_61_90,
            days_61_90_exact,
            days_over_90,
            days_over_90_exact,
            total,
            total_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCreditMemoInput {
    pub customer_id: String,
    pub original_invoice_id: Option<String>,
    #[napi(ts_type = "CreditMemoReasonInput")]
    pub reason: String,
    pub amount: String,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditMemoOutput {
    pub id: String,
    pub credit_memo_number: String,
    pub customer_id: String,
    /// Exact decimal string.
    pub amount: String,
    /// Exact base-10 amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_exact: String,
    #[napi(ts_type = "CreditMemoStatus")]
    pub status: String,
    #[napi(ts_type = "CreditMemoReason")]
    pub reason: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::CreditMemo> for CreditMemoOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditMemo) -> Result<Self> {
        let amount = c.amount.to_string();
        let amount_exact = amount.clone();
        Ok(Self {
            id: c.id.to_string(),
            credit_memo_number: c.credit_memo_number,
            customer_id: c.customer_id.to_string(),
            amount,
            amount_exact,
            status: format!("{:?}", c.status),
            reason: format!("{:?}", c.reason),
            created_at: c.created_at.to_rfc3339(),
        })
    }
}

pub(crate) fn parse_credit_memo_reason(s: &str) -> Result<stateset_core::CreditMemoReason> {
    Ok(match s.to_lowercase().as_str() {
        "returned_goods" | "returnedgoods" | "return" => {
            stateset_core::CreditMemoReason::ReturnedGoods
        }
        "pricing_error" | "pricingerror" | "billing_error" => {
            stateset_core::CreditMemoReason::PricingError
        }
        "overpayment" => stateset_core::CreditMemoReason::Overpayment,
        "damaged" => stateset_core::CreditMemoReason::Damaged,
        "service_credit" | "servicecredit" => stateset_core::CreditMemoReason::ServiceCredit,
        "goodwill" | "goodwill_adjustment" => stateset_core::CreditMemoReason::GoodwillAdjustment,
        "other" => stateset_core::CreditMemoReason::Other,
        _ => {
            return Err(unknown_variant(
                "credit memo reason",
                s,
                &[
                    "returned_goods",
                    "return",
                    "pricing_error",
                    "billing_error",
                    "overpayment",
                    "damaged",
                    "service_credit",
                    "goodwill",
                    "goodwill_adjustment",
                    "other",
                ],
            ));
        }
    })
}

/// Optional filters for `AccountsReceivable.listCreditMemos`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreditMemoFilterInput {
    pub customer_id: Option<String>,
    /// The rendered form (`PartiallyApplied`) or the engine's snake_case (`partially_applied`).
    #[napi(ts_type = "CreditMemoStatusInput")]
    pub status: Option<String>,
    /// The rendered form (`ReturnedGoods`) or the engine's snake_case (`returned_goods`).
    #[napi(ts_type = "CreditMemoReasonFilter")]
    pub reason: Option<String>,
    pub has_unapplied: Option<bool>,
    /// RFC 3339 timestamp.
    pub from_date: Option<String>,
    /// RFC 3339 timestamp.
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<CreditMemoFilterInput> for stateset_core::CreditMemoFilter {
    type Error = Error;

    fn try_from(f: CreditMemoFilterInput) -> Result<Self> {
        Ok(Self {
            customer_id: parse_optional_id(f.customer_id, "customer")?,
            status: parse_optional_enum(f.status, "credit memo status")?,
            reason: parse_optional_enum(f.reason, "credit memo reason")?,
            has_unapplied: f.has_unapplied,
            from_date: parse_optional_datetime(f.from_date, "from date")?,
            to_date: parse_optional_datetime(f.to_date, "to date")?,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct AccountsReceivable {
    pub(crate) commerce: Handle,
}

#[napi]
impl AccountsReceivable {
    #[napi]
    pub async fn apply_payment_to_invoices(
        &self,
        input: ApplyArPaymentInput,
    ) -> Result<Vec<ArPaymentApplicationOutput>> {
        self.commerce
            .get()?
            .accounts_receivable()
            .apply_payment_to_invoices(ar_workflow_input(input)?)
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply receipt", e))
    }
    #[napi]
    pub async fn get_payment_applications(
        &self,
        payment_id: String,
    ) -> Result<Vec<ArPaymentApplicationOutput>> {
        self.commerce
            .get()?
            .accounts_receivable()
            .get_payment_applications(ar_workflow_id(&payment_id)?)
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list applications", e))
    }
    #[napi]
    pub async fn unapply_payment(&self, id: String) -> Result<()> {
        self.commerce
            .get()?
            .accounts_receivable()
            .unapply_payment(ar_workflow_id(&id)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to unapply receipt", e))
    }
    #[napi]
    pub async fn apply_credit_memo(&self, input: ApplyCreditMemoInput) -> Result<CreditMemoOutput> {
        convert_output(
            self.commerce
                .get()?
                .accounts_receivable()
                .apply_credit_memo(ar_workflow_input(input)?)
                .map_err(|e| wrap(ErrCode::Internal, "Failed to apply credit memo", e))?,
        )
    }
    #[napi]
    pub async fn create_write_off(
        &self,
        input: CreateWriteOffInput,
        actor: String,
    ) -> Result<ArWriteOffOutput> {
        let mut request: stateset_core::CreateWriteOff = ar_workflow_input(input)?;
        request.approved_by = Some(actor);
        self.commerce
            .get()?
            .accounts_receivable()
            .create_write_off(request)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to write off invoice", e))
    }
    #[napi]
    pub async fn reverse_write_off(&self, id: String) -> Result<ArWriteOffOutput> {
        self.commerce
            .get()?
            .accounts_receivable()
            .reverse_write_off(ar_workflow_id(&id)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reverse write-off", e))
    }
    #[napi]
    pub async fn list_write_offs(
        &self,
        filter: Option<ArWriteOffFilterInput>,
    ) -> Result<Vec<ArWriteOffOutput>> {
        self.commerce
            .get()?
            .accounts_receivable()
            .list_write_offs(ar_workflow_input(filter.unwrap_or_default())?)
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list write-offs", e))
    }

    /// Get AR aging summary
    #[napi]
    pub async fn get_aging_summary(&self) -> Result<ArAgingSummaryOutput> {
        let commerce = self.commerce.get()?;
        let aging = commerce
            .accounts_receivable()
            .get_aging_summary()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get aging", e))?;
        convert_output(aging)
    }

    /// Get total outstanding
    #[napi]
    pub async fn get_total_outstanding(&self) -> Result<String> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .accounts_receivable()
            .get_total_outstanding()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get total", e))?;
        Ok(total.to_string())
    }

    /// Get Days Sales Outstanding (DSO)
    #[napi]
    pub async fn get_dso(&self, days: i32) -> Result<String> {
        let commerce = self.commerce.get()?;
        let dso = commerce
            .accounts_receivable()
            .get_dso(days)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get DSO", e))?;
        Ok(dso.to_string())
    }

    /// Create a credit memo
    #[napi]
    pub async fn create_credit_memo(
        &self,
        input: CreateCreditMemoInput,
    ) -> Result<CreditMemoOutput> {
        let commerce = self.commerce.get()?;
        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .create_credit_memo(stateset_core::CreateCreditMemo {
                customer_id,
                original_invoice_id: parse_optional_id(
                    input.original_invoice_id,
                    "original invoice",
                )?,
                reason: parse_credit_memo_reason(&input.reason)?,
                amount: parse_decimal_str(&input.amount, "credit memo amount")?,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create credit memo", e))?;
        convert_output(memo)
    }

    /// Get a credit memo by ID
    #[napi]
    pub async fn get_credit_memo(&self, id: String) -> Result<Option<CreditMemoOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .get_credit_memo(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get credit memo", e))?;
        convert_optional_output(memo)
    }

    /// List credit memos, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_credit_memos(
        &self,
        filter: Option<CreditMemoFilterInput>,
    ) -> Result<Vec<CreditMemoOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::CreditMemoFilter = filter.unwrap_or_default().try_into()?;
        let memos = commerce
            .accounts_receivable()
            .list_credit_memos(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list credit memos", e))?;
        convert_outputs(memos)
    }

    /// Void a credit memo
    #[napi]
    pub async fn void_credit_memo(&self, id: String) -> Result<CreditMemoOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .void_credit_memo(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to void credit memo", e))?;
        convert_output(memo)
    }

    /// Get unapplied credits for a customer
    #[napi]
    pub async fn get_unapplied_credits(
        &self,
        customer_id: String,
    ) -> Result<Vec<CreditMemoOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let memos = commerce
            .accounts_receivable()
            .get_unapplied_credits(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get credits", e))?;
        convert_outputs(memos)
    }
}

fn ar_workflow_input<T: serde::de::DeserializeOwned>(value: impl Serialize) -> Result<T> {
    let value = serde_json::to_value(value)
        .map_err(|e| wrap(ErrCode::Validation, "Invalid finance input", e))?;
    serde_json::from_value(value).map_err(|e| wrap(ErrCode::Validation, "Invalid finance input", e))
}
fn ar_workflow_id(id: &str) -> Result<uuid::Uuid> {
    id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid finance UUID"))
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyArPaymentInput {
    pub payment_id: String,
    pub applications: Vec<ArPaymentLineInput>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ArPaymentLineInput {
    pub invoice_id: String,
    pub amount: String,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyCreditMemoInput {
    pub credit_memo_id: String,
    pub invoice_id: String,
    pub amount: String,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWriteOffInput {
    pub invoice_id: String,
    pub amount: String,
    pub reason: String,
    pub notes: Option<String>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ArWriteOffFilterInput {
    pub customer_id: Option<String>,
    pub invoice_id: Option<String>,
    pub reason: Option<String>,
    pub include_reversed: Option<bool>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ArPaymentApplicationOutput {
    pub id: String,
    pub payment_id: String,
    pub invoice_id: String,
    pub applied_amount: String,
    pub applied_date: String,
    pub created_at: String,
}
impl From<stateset_core::ArPaymentApplication> for ArPaymentApplicationOutput {
    fn from(v: stateset_core::ArPaymentApplication) -> Self {
        Self {
            id: v.id.to_string(),
            payment_id: v.payment_id.to_string(),
            invoice_id: v.invoice_id.to_string(),
            applied_amount: v.applied_amount.to_string(),
            applied_date: v.applied_date.to_rfc3339(),
            created_at: v.created_at.to_rfc3339(),
        }
    }
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ArWriteOffOutput {
    pub id: String,
    pub invoice_id: String,
    pub customer_id: String,
    pub amount: String,
    pub reason: String,
    pub write_off_number: String,
    pub notes: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub reversed_at: Option<String>,
    pub gl_journal_entry_id: Option<String>,
    pub write_off_date: String,
    pub created_at: String,
}
impl From<stateset_core::WriteOff> for ArWriteOffOutput {
    fn from(v: stateset_core::WriteOff) -> Self {
        Self {
            id: v.id.to_string(),
            invoice_id: v.invoice_id.to_string(),
            customer_id: v.customer_id.to_string(),
            amount: v.amount.to_string(),
            reason: v.reason.to_string(),
            write_off_number: v.write_off_number,
            notes: v.notes,
            approved_by: v.approved_by,
            approved_at: v.approved_at.map(|x| x.to_rfc3339()),
            reversed_at: v.reversed_at.map(|x| x.to_rfc3339()),
            gl_journal_entry_id: v.gl_journal_entry_id.map(|x| x.to_string()),
            write_off_date: v.write_off_date.to_rfc3339(),
            created_at: v.created_at.to_rfc3339(),
        }
    }
}
