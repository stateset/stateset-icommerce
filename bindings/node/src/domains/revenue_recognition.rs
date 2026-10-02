//! Revenue Recognition  (all monetary values cross as exact decimal strings).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Revenue Recognition  (all monetary values cross as exact decimal strings)
// ============================================================================

pub(crate) fn parse_recognition_method(
    method: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<stateset_core::RecognitionMethod> {
    match method {
        "point_in_time" => Ok(stateset_core::RecognitionMethod::PointInTime),
        "ratable_over_time" => {
            let start = start.ok_or_else(|| {
                coded(ErrCode::Validation, "ratable_over_time requires recognition_start")
            })?;
            let end = end.ok_or_else(|| {
                coded(ErrCode::Validation, "ratable_over_time requires recognition_end")
            })?;
            Ok(stateset_core::RecognitionMethod::RatableOverTime {
                start: parse_iso_date(start, "recognition_start")?,
                end: parse_iso_date(end, "recognition_end")?,
            })
        }
        "milestone" => Ok(stateset_core::RecognitionMethod::Milestone),
        _ => Err(coded(
            ErrCode::Validation,
            "Invalid recognition method (expected point_in_time, ratable_over_time, or milestone)",
        )),
    }
}

pub(crate) fn recognition_method_parts(
    method: stateset_core::RecognitionMethod,
) -> (String, Option<String>, Option<String>) {
    match method {
        stateset_core::RecognitionMethod::PointInTime => ("point_in_time".to_string(), None, None),
        stateset_core::RecognitionMethod::RatableOverTime { start, end } => {
            ("ratable_over_time".to_string(), Some(start.to_string()), Some(end.to_string()))
        }
        stateset_core::RecognitionMethod::Milestone => ("milestone".to_string(), None, None),
        _ => ("unknown".to_string(), None, None),
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePerformanceObligationInput {
    pub description: String,
    /// Exact decimal string
    pub standalone_selling_price: Option<String>,
    /// Exact decimal string; obligations must sum to the transaction price
    pub allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    #[napi(ts_type = "RecognitionMethod")]
    pub recognition_method: String,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    pub recognition_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRevenueContractInput {
    /// Optional contract number; auto-generated when omitted (RC-...)
    pub contract_number: Option<String>,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// Exact decimal string
    pub transaction_price: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: String,
    pub obligations: Vec<CreatePerformanceObligationInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateRevenueContractInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// draft, active, completed, cancelled (transition-guarded)
    #[napi(ts_type = "RevenueContractStatus")]
    pub status: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueContractFilterInput {
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// draft, active, completed, cancelled
    #[napi(ts_type = "RevenueContractStatus")]
    pub status: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_from: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_to: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PerformanceObligationOutput {
    pub id: String,
    pub contract_id: String,
    pub description: String,
    /// Exact decimal string
    pub standalone_selling_price: Option<String>,
    /// Exact decimal string
    pub allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    #[napi(ts_type = "RecognitionMethodOutput")]
    pub recognition_method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_end: Option<String>,
    /// Exact decimal string
    pub recognized_amount: String,
    /// Exact decimal string: allocated_amount - recognized_amount
    pub deferred_amount: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PerformanceObligation> for PerformanceObligationOutput {
    fn from(o: stateset_core::PerformanceObligation) -> Self {
        let deferred = o.deferred_amount();
        let (method, start, end) = recognition_method_parts(o.recognition_method);
        Self {
            id: o.id.to_string(),
            contract_id: o.contract_id.to_string(),
            description: o.description,
            standalone_selling_price: o.standalone_selling_price.map(|d| d.to_string()),
            allocated_amount: o.allocated_amount.to_string(),
            recognition_method: method,
            recognition_start: start,
            recognition_end: end,
            recognized_amount: o.recognized_amount.to_string(),
            deferred_amount: deferred.to_string(),
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueContractOutput {
    pub id: String,
    pub contract_number: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// Exact decimal string
    pub transaction_price: String,
    pub currency: String,
    /// draft, active, completed, cancelled
    #[napi(ts_type = "RevenueContractStatus")]
    pub status: String,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: String,
    pub obligations: Vec<PerformanceObligationOutput>,
    /// Exact decimal string: total recognized across obligations
    pub total_recognized: String,
    /// Exact decimal string: transaction_price - total_recognized
    pub deferred_balance: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::RevenueContract> for RevenueContractOutput {
    fn from(c: stateset_core::RevenueContract) -> Self {
        let total_recognized = c.total_recognized();
        let deferred_balance = c.deferred_balance();
        Self {
            id: c.id.to_string(),
            contract_number: c.contract_number,
            customer_id: c.customer_id.to_string(),
            order_id: c.order_id.map(|id| id.to_string()),
            invoice_id: c.invoice_id.map(|id| id.to_string()),
            transaction_price: c.transaction_price.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            effective_date: c.effective_date.to_string(),
            obligations: c.obligations.into_iter().map(Into::into).collect(),
            total_recognized: total_recognized.to_string(),
            deferred_balance: deferred_balance.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueScheduleEntryOutput {
    pub period: u32,
    /// ISO date (YYYY-MM-DD): first day of the entry's month
    pub period_start: String,
    /// Exact decimal string
    pub amount: String,
    /// deferred or recognized
    #[napi(ts_type = "RevenueEntryStatus")]
    pub status: String,
}

impl From<stateset_core::RevenueScheduleEntry> for RevenueScheduleEntryOutput {
    fn from(e: stateset_core::RevenueScheduleEntry) -> Self {
        Self {
            period: e.period,
            period_start: e.period_start.to_string(),
            amount: e.amount.to_string(),
            status: format!("{}", e.status),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueScheduleOutput {
    pub obligation_id: String,
    /// point_in_time, ratable_over_time, milestone
    #[napi(ts_type = "RecognitionMethodOutput")]
    pub method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_end: Option<String>,
    pub entries: Vec<RevenueScheduleEntryOutput>,
    /// Exact decimal string
    pub total_amount: String,
    /// Exact decimal string: sum of recognized entries
    pub recognized_total: String,
    /// Exact decimal string: sum of deferred entries
    pub deferred_total: String,
}

impl From<stateset_core::RevenueSchedule> for RevenueScheduleOutput {
    fn from(s: stateset_core::RevenueSchedule) -> Self {
        let recognized_total = s.recognized_total();
        let deferred_total = s.deferred_total();
        let (method, start, end) = recognition_method_parts(s.method);
        Self {
            obligation_id: s.obligation_id.to_string(),
            method,
            recognition_start: start,
            recognition_end: end,
            entries: s.entries.into_iter().map(Into::into).collect(),
            total_amount: s.total_amount.to_string(),
            recognized_total: recognized_total.to_string(),
            deferred_total: deferred_total.to_string(),
        }
    }
}

#[napi]
pub struct RevenueRecognition {
    pub(crate) commerce: Handle,
}

#[napi]
impl RevenueRecognition {
    /// Whether the revenue-recognition backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.revenue_recognition().is_supported())
    }

    #[napi]
    pub async fn create_contract(
        &self,
        input: CreateRevenueContractInput,
    ) -> Result<RevenueContractOutput> {
        let commerce = self.commerce.get()?;
        let customer_id: uuid::Uuid = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let currency = input
            .currency
            .map(|s| {
                s.parse::<CurrencyCode>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid currency code"))
            })
            .transpose()?;
        let obligations = input
            .obligations
            .into_iter()
            .map(|o| -> Result<stateset_core::CreatePerformanceObligation> {
                Ok(stateset_core::CreatePerformanceObligation {
                    description: o.description,
                    standalone_selling_price: o
                        .standalone_selling_price
                        .as_deref()
                        .map(|s| parse_decimal_str(s, "standalone_selling_price"))
                        .transpose()?,
                    allocated_amount: parse_decimal_str(&o.allocated_amount, "allocated_amount")?,
                    recognition_method: parse_recognition_method(
                        &o.recognition_method,
                        o.recognition_start.as_deref(),
                        o.recognition_end.as_deref(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let contract = commerce
            .revenue_recognition()
            .create_contract(stateset_core::CreateRevenueContract {
                contract_number: input.contract_number,
                customer_id,
                order_id: parse_optional_uuid(input.order_id, "order_id")?,
                invoice_id: parse_optional_uuid(input.invoice_id, "invoice_id")?,
                transaction_price: parse_decimal_str(
                    &input.transaction_price,
                    "transaction_price",
                )?,
                currency,
                effective_date: parse_iso_date(&input.effective_date, "effective_date")?,
                obligations,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create revenue contract", e))?;
        Ok(contract.into())
    }

    #[napi]
    pub async fn get_contract(&self, id: String) -> Result<Option<RevenueContractOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let contract = commerce
            .revenue_recognition()
            .get_contract(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get revenue contract", e))?;
        Ok(contract.map(Into::into))
    }

    #[napi]
    pub async fn list_contracts(
        &self,
        filter: Option<RevenueContractFilterInput>,
    ) -> Result<Vec<RevenueContractOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::RevenueContractFilter::default()),
            |f| -> Result<stateset_core::RevenueContractFilter> {
                Ok(stateset_core::RevenueContractFilter {
                    customer_id: parse_optional_uuid(f.customer_id, "customer_id")?,
                    order_id: parse_optional_uuid(f.order_id, "order_id")?,
                    invoice_id: parse_optional_uuid(f.invoice_id, "invoice_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::RevenueContractStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid revenue contract status")
                            })
                        })
                        .transpose()?,
                    effective_from: f
                        .effective_from
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_from"))
                        .transpose()?,
                    effective_to: f
                        .effective_to
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_to"))
                        .transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let contracts = commerce
            .revenue_recognition()
            .list_contracts(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list revenue contracts", e))?;
        Ok(contracts.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn update_contract(
        &self,
        id: String,
        input: UpdateRevenueContractInput,
    ) -> Result<RevenueContractOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let status = input
            .status
            .map(|s| {
                s.parse::<stateset_core::RevenueContractStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid revenue contract status"))
            })
            .transpose()?;
        let contract = commerce
            .revenue_recognition()
            .update_contract(
                uuid,
                stateset_core::UpdateRevenueContract {
                    order_id: parse_optional_uuid(input.order_id, "order_id")?,
                    invoice_id: parse_optional_uuid(input.invoice_id, "invoice_id")?,
                    status,
                    effective_date: input
                        .effective_date
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_date"))
                        .transpose()?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update revenue contract", e))?;
        Ok(contract.into())
    }

    /// List the performance obligations under a contract.
    #[napi]
    pub async fn list_obligations(
        &self,
        contract_id: String,
    ) -> Result<Vec<PerformanceObligationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            contract_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let obligations = commerce
            .revenue_recognition()
            .list_obligations(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list obligations", e))?;
        Ok(obligations.into_iter().map(Into::into).collect())
    }

    /// Generate and persist the recognition schedule for an obligation.
    #[napi]
    pub async fn generate_schedule(&self, obligation_id: String) -> Result<RevenueScheduleOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .generate_schedule(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to generate schedule", e))?;
        Ok(schedule.into())
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    #[napi]
    pub async fn get_schedule(
        &self,
        obligation_id: String,
    ) -> Result<Option<RevenueScheduleOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .get_schedule(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get schedule", e))?;
        Ok(schedule.map(Into::into))
    }

    /// Recognize deferred entries with a period start on or before `through`
    /// (ISO date, YYYY-MM-DD).
    #[napi]
    pub async fn recognize(
        &self,
        obligation_id: String,
        through: String,
    ) -> Result<RevenueScheduleOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let through = parse_iso_date(&through, "through")?;
        let schedule = commerce
            .revenue_recognition()
            .recognize_period(uuid, through)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to recognize revenue", e))?;
        Ok(schedule.into())
    }
}
