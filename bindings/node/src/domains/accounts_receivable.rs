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
    /// @deprecated Use the `currentExact` twin; float money will be removed in 2.0.
    pub current: f64,
    /// Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money.
    pub current_exact: String,
    /// @deprecated Use the `days130Exact` twin; float money will be removed in 2.0.
    pub days_1_30: f64,
    /// Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_1_30_exact: String,
    /// @deprecated Use the `days3160Exact` twin; float money will be removed in 2.0.
    pub days_31_60: f64,
    /// Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_31_60_exact: String,
    /// @deprecated Use the `days6190Exact` twin; float money will be removed in 2.0.
    pub days_61_90: f64,
    /// Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_61_90_exact: String,
    /// @deprecated Use the `daysOver90Exact` twin; float money will be removed in 2.0.
    pub days_over_90: f64,
    /// Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_over_90_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
}

impl TryFrom<stateset_core::ArAgingSummary> for ArAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ArAgingSummary) -> Result<Self> {
        let (current, current_exact) = money_pair(a.current, "AR aging current")?;
        let (days_1_30, days_1_30_exact) = money_pair(a.days_1_30, "AR aging 1-30 days")?;
        let (days_31_60, days_31_60_exact) = money_pair(a.days_31_60, "AR aging 31-60 days")?;
        let (days_61_90, days_61_90_exact) = money_pair(a.days_61_90, "AR aging 61-90 days")?;
        let (days_over_90, days_over_90_exact) =
            money_pair(a.days_over_90, "AR aging over 90 days")?;
        let (total, total_exact) = money_pair(a.total, "AR aging total")?;
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
    pub amount: f64,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditMemoOutput {
    pub id: String,
    pub credit_memo_number: String,
    pub customer_id: String,
    /// @deprecated Use the `amountExact` twin; float money will be removed in 2.0.
    pub amount: f64,
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
        let (amount, amount_exact) = money_pair(c.amount, "credit memo amount")?;
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
    pub async fn get_total_outstanding(&self) -> Result<f64> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .accounts_receivable()
            .get_total_outstanding()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get total", e))?;
        to_f64_checked(total, "accounts receivable total outstanding")
    }

    /// Get Days Sales Outstanding (DSO)
    #[napi]
    pub async fn get_dso(&self, days: i32) -> Result<f64> {
        let commerce = self.commerce.get()?;
        let dso = commerce
            .accounts_receivable()
            .get_dso(days)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get DSO", e))?;
        to_f64_checked(dso, "days sales outstanding")
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
                amount: decimal_from_f64(input.amount, "credit memo amount")?,
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
