//! Accounts receivable endpoints (aging, payment application, credit memos,
//! write-offs, dunning, customer statements).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response schemas
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArAgingResponse {
    pub current: String,
    pub days_1_30: String,
    pub days_31_60: String,
    pub days_61_90: String,
    pub days_over_90: String,
    pub total: String,
    pub as_of_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCustomerAgingResponse {
    pub customer_id: String,
    pub customer_name: Option<String>,
    pub current: String,
    pub days_1_30: String,
    pub days_31_60: String,
    pub days_61_90: String,
    pub days_over_90: String,
    pub total_outstanding: String,
    pub invoice_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCustomerAgingListResponse {
    pub customers: Vec<ArCustomerAgingResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ArAgingReportParams {
    pub customer_id: Option<String>,
    pub overdue_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ArPaymentApplicationLineRequest {
    pub invoice_id: String,
    /// Decimal amount as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApplyArPaymentRequest {
    pub payment_id: String,
    pub applications: Vec<ArPaymentApplicationLineRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArPaymentApplicationResponse {
    pub id: String,
    pub payment_id: String,
    pub invoice_id: String,
    pub applied_amount: String,
    pub applied_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArPaymentApplicationListResponse {
    pub applications: Vec<ArPaymentApplicationResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateArCreditMemoRequest {
    pub customer_id: String,
    pub original_invoice_id: Option<String>,
    /// One of `returned_goods`, `pricing_error`, `overpayment`, `damaged`,
    /// `service_credit`, `goodwill_adjustment`, `other`.
    pub reason: String,
    /// Decimal amount as a string.
    pub amount: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCreditMemoResponse {
    pub id: String,
    pub credit_memo_number: String,
    pub customer_id: String,
    pub reason: String,
    pub amount: String,
    pub applied_amount: String,
    pub unapplied_amount: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCreditMemoListResponse {
    pub credit_memos: Vec<ArCreditMemoResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ArCreditMemoFilterParams {
    pub customer_id: Option<String>,
    /// One of `open`, `partially_applied`, `fully_applied`, `voided`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApplyArCreditMemoRequest {
    pub invoice_id: String,
    /// Decimal amount as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateArWriteOffRequest {
    pub invoice_id: String,
    /// Decimal amount as a string.
    pub amount: String,
    /// One of `uncollectible`, `bankruptcy`, `customer_dispute`, `small_balance`,
    /// `account_closed`, `deceased`, `other`.
    pub reason: String,
    pub notes: Option<String>,
    pub approved_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArWriteOffResponse {
    pub id: String,
    pub write_off_number: String,
    pub invoice_id: String,
    pub customer_id: String,
    pub amount: String,
    pub reason: String,
    pub reversed: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SendArDunningRequest {
    /// One of `reminder_1`, `reminder_2`, `reminder_3`, `demand_letter`, `collection_notice`.
    pub letter_type: String,
    pub sent_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCollectionActivityResponse {
    pub id: String,
    pub invoice_id: String,
    pub customer_id: String,
    pub activity_type: String,
    pub dunning_letter_type: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ArStatementParams {
    /// RFC 3339 period start (defaults to 30 days ago).
    pub period_start: Option<String>,
    /// RFC 3339 period end (defaults to now).
    pub period_end: Option<String>,
    pub include_paid_invoices: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArStatementLineResponse {
    pub date: String,
    pub transaction_type: String,
    pub reference_number: String,
    pub description: String,
    pub debit: Option<String>,
    pub credit: Option<String>,
    pub balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ArCustomerStatementResponse {
    pub customer_id: String,
    pub customer_name: String,
    pub period_start: String,
    pub period_end: String,
    pub opening_balance: String,
    pub total_invoices: String,
    pub total_payments: String,
    pub total_credits: String,
    pub closing_balance: String,
    pub line_items: Vec<ArStatementLineResponse>,
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_datetime(s: &str, what: &str) -> Result<chrono::DateTime<chrono::Utc>, HttpError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn customer_aging_resp(a: &stateset_core::CustomerArAging) -> ArCustomerAgingResponse {
    ArCustomerAgingResponse {
        customer_id: a.customer_id.to_string(),
        customer_name: a.customer_name.clone(),
        current: a.current.to_string(),
        days_1_30: a.days_1_30.to_string(),
        days_31_60: a.days_31_60.to_string(),
        days_61_90: a.days_61_90.to_string(),
        days_over_90: a.days_over_90.to_string(),
        total_outstanding: a.total_outstanding.to_string(),
        invoice_count: a.invoice_count,
    }
}

fn application_resp(a: &stateset_core::ArPaymentApplication) -> ArPaymentApplicationResponse {
    ArPaymentApplicationResponse {
        id: a.id.to_string(),
        payment_id: a.payment_id.to_string(),
        invoice_id: a.invoice_id.to_string(),
        applied_amount: a.applied_amount.to_string(),
        applied_date: a.applied_date.to_rfc3339(),
    }
}

fn credit_memo_resp(m: &stateset_core::CreditMemo) -> ArCreditMemoResponse {
    ArCreditMemoResponse {
        id: m.id.to_string(),
        credit_memo_number: m.credit_memo_number.clone(),
        customer_id: m.customer_id.to_string(),
        reason: m.reason.to_string(),
        amount: m.amount.to_string(),
        applied_amount: m.applied_amount.to_string(),
        unapplied_amount: m.unapplied_amount.to_string(),
        status: m.status.to_string(),
        created_at: m.created_at.to_rfc3339(),
    }
}

fn write_off_resp(w: &stateset_core::WriteOff) -> ArWriteOffResponse {
    ArWriteOffResponse {
        id: w.id.to_string(),
        write_off_number: w.write_off_number.clone(),
        invoice_id: w.invoice_id.to_string(),
        customer_id: w.customer_id.to_string(),
        amount: w.amount.to_string(),
        reason: w.reason.to_string(),
        reversed: w.is_reversed(),
        created_at: w.created_at.to_rfc3339(),
    }
}

fn activity_resp(a: &stateset_core::CollectionActivity) -> ArCollectionActivityResponse {
    ArCollectionActivityResponse {
        id: a.id.to_string(),
        invoice_id: a.invoice_id.to_string(),
        customer_id: a.customer_id.to_string(),
        activity_type: a.activity_type.to_string(),
        dunning_letter_type: a.dunning_letter_type.map(|t| t.to_string()),
        created_at: a.created_at.to_rfc3339(),
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ar/aging", get(aging_summary))
        .route("/ar/aging/customers", get(aging_report))
        .route("/ar/aging/customers/{customer_id}", get(customer_aging))
        .route("/ar/payment-applications", post(apply_payment))
        .route("/ar/payment-applications/{id}/unapply", post(unapply_payment))
        .route("/ar/credit-memos", post(create_credit_memo).get(list_credit_memos))
        .route("/ar/credit-memos/{id}/apply", post(apply_credit_memo))
        .route("/ar/write-offs", post(create_write_off))
        .route("/ar/write-offs/{id}/reverse", post(reverse_write_off))
        .route("/ar/invoices/{invoice_id}/dunning", post(send_dunning))
        .route("/ar/customers/{customer_id}/statement", get(customer_statement))
}

// ============================================================================
// Aging
// ============================================================================

#[utoipa::path(get, operation_id = "ar_aging_summary", path = "/api/v1/ar/aging", tag = "accounts_receivable",
    responses((status = 200, body = ArAgingResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn aging_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ArAgingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let a = c.accounts_receivable().get_aging_summary()?;
    Ok(Json(ArAgingResponse {
        current: a.current.to_string(),
        days_1_30: a.days_1_30.to_string(),
        days_31_60: a.days_31_60.to_string(),
        days_61_90: a.days_61_90.to_string(),
        days_over_90: a.days_over_90.to_string(),
        total: a.total.to_string(),
        as_of_date: a.as_of_date.to_rfc3339(),
    }))
}

#[utoipa::path(get, operation_id = "ar_aging_report", path = "/api/v1/ar/aging/customers", tag = "accounts_receivable",
    params(ArAgingReportParams),
    responses((status = 200, body = ArCustomerAgingListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn aging_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ArAgingReportParams>,
) -> Result<Json<ArCustomerAgingListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let customer_id = match params.customer_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "customer_id")?),
        None => None,
    };
    let filter = stateset_core::ArAgingFilter {
        customer_id,
        overdue_only: params.overdue_only,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let rows = c.accounts_receivable().get_aging_report(filter)?;
    let total = rows.len();
    Ok(Json(ArCustomerAgingListResponse {
        customers: rows.iter().map(customer_aging_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "ar_customer_aging", path = "/api/v1/ar/aging/customers/{customer_id}", tag = "accounts_receivable",
    params(("customer_id" = String, Path, description = "Customer ID")),
    responses((status = 200, body = ArCustomerAgingResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn customer_aging(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<Uuid>,
) -> Result<Json<ArCustomerAgingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let aging = c
        .accounts_receivable()
        .get_customer_aging(customer_id)?
        .ok_or_else(|| HttpError::NotFound(format!("Customer {customer_id} has no AR aging")))?;
    Ok(Json(customer_aging_resp(&aging)))
}

// ============================================================================
// Payment application
// ============================================================================

#[utoipa::path(post, operation_id = "ar_apply_payment", path = "/api/v1/ar/payment-applications", tag = "accounts_receivable",
    request_body = ApplyArPaymentRequest,
    responses((status = 201, body = ArPaymentApplicationListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn apply_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ApplyArPaymentRequest>,
) -> Result<(StatusCode, Json<ArPaymentApplicationListResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut applications = Vec::with_capacity(req.applications.len());
    for a in req.applications {
        applications.push(stateset_core::PaymentApplicationLine {
            invoice_id: parse_id::<Uuid>(&a.invoice_id, "invoice_id")?,
            amount: parse_decimal(&a.amount, "application amount")?,
        });
    }
    let input = stateset_core::ApplyPaymentToInvoices {
        payment_id: parse_id::<Uuid>(&req.payment_id, "payment_id")?,
        applications,
    };
    let applied = c.accounts_receivable().apply_payment_to_invoices(input)?;
    let total = applied.len();
    Ok((
        StatusCode::CREATED,
        Json(ArPaymentApplicationListResponse {
            applications: applied.iter().map(application_resp).collect(),
            total,
        }),
    ))
}

#[utoipa::path(post, operation_id = "ar_unapply_payment", path = "/api/v1/ar/payment-applications/{id}/unapply", tag = "accounts_receivable",
    params(("id" = String, Path, description = "Payment application ID")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn unapply_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.accounts_receivable().unapply_payment(id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Credit memos
// ============================================================================

#[utoipa::path(post, operation_id = "ar_create_credit_memo", path = "/api/v1/ar/credit-memos", tag = "accounts_receivable",
    request_body = CreateArCreditMemoRequest,
    responses((status = 201, body = ArCreditMemoResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_credit_memo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateArCreditMemoRequest>,
) -> Result<(StatusCode, Json<ArCreditMemoResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let original_invoice_id = match req.original_invoice_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "original_invoice_id")?),
        None => None,
    };
    let input = stateset_core::CreateCreditMemo {
        customer_id: parse_id::<Uuid>(&req.customer_id, "customer_id")?,
        original_invoice_id,
        reason: parse_id(&req.reason, "reason")?,
        amount: parse_decimal(&req.amount, "amount")?,
        notes: req.notes,
    };
    let memo = c.accounts_receivable().create_credit_memo(input)?;
    Ok((StatusCode::CREATED, Json(credit_memo_resp(&memo))))
}

#[utoipa::path(get, operation_id = "ar_list_credit_memos", path = "/api/v1/ar/credit-memos", tag = "accounts_receivable",
    params(ArCreditMemoFilterParams),
    responses((status = 200, body = ArCreditMemoListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_credit_memos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ArCreditMemoFilterParams>,
) -> Result<Json<ArCreditMemoListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let customer_id = match params.customer_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "customer_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let filter = stateset_core::CreditMemoFilter {
        customer_id,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let memos = c.accounts_receivable().list_credit_memos(filter)?;
    let total = memos.len();
    Ok(Json(ArCreditMemoListResponse {
        credit_memos: memos.iter().map(credit_memo_resp).collect(),
        total,
    }))
}

#[utoipa::path(post, operation_id = "ar_apply_credit_memo", path = "/api/v1/ar/credit-memos/{id}/apply", tag = "accounts_receivable",
    request_body = ApplyArCreditMemoRequest,
    params(("id" = String, Path, description = "Credit memo ID")),
    responses((status = 200, body = ArCreditMemoResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn apply_credit_memo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ApplyArCreditMemoRequest>,
) -> Result<Json<ArCreditMemoResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::ApplyCreditMemo {
        credit_memo_id: id,
        invoice_id: parse_id::<Uuid>(&req.invoice_id, "invoice_id")?,
        amount: parse_decimal(&req.amount, "amount")?,
    };
    Ok(Json(credit_memo_resp(&c.accounts_receivable().apply_credit_memo(input)?)))
}

// ============================================================================
// Write-offs
// ============================================================================

#[utoipa::path(post, operation_id = "ar_create_write_off", path = "/api/v1/ar/write-offs", tag = "accounts_receivable",
    request_body = CreateArWriteOffRequest,
    responses((status = 201, body = ArWriteOffResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_write_off(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateArWriteOffRequest>,
) -> Result<(StatusCode, Json<ArWriteOffResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateWriteOff {
        invoice_id: parse_id::<Uuid>(&req.invoice_id, "invoice_id")?,
        amount: parse_decimal(&req.amount, "amount")?,
        reason: parse_id(&req.reason, "reason")?,
        notes: req.notes,
        approved_by: req.approved_by,
    };
    let write_off = c.accounts_receivable().create_write_off(input)?;
    Ok((StatusCode::CREATED, Json(write_off_resp(&write_off))))
}

#[utoipa::path(post, operation_id = "ar_reverse_write_off", path = "/api/v1/ar/write-offs/{id}/reverse", tag = "accounts_receivable",
    params(("id" = String, Path, description = "Write-off ID")),
    responses((status = 200, body = ArWriteOffResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn reverse_write_off(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ArWriteOffResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(write_off_resp(&c.accounts_receivable().reverse_write_off(id)?)))
}

// ============================================================================
// Dunning
// ============================================================================

#[utoipa::path(post, operation_id = "ar_send_dunning", path = "/api/v1/ar/invoices/{invoice_id}/dunning", tag = "accounts_receivable",
    request_body = SendArDunningRequest,
    params(("invoice_id" = String, Path, description = "Invoice ID")),
    responses((status = 201, body = ArCollectionActivityResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn send_dunning(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(invoice_id): Path<Uuid>,
    Json(req): Json<SendArDunningRequest>,
) -> Result<(StatusCode, Json<ArCollectionActivityResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let letter_type = parse_id(&req.letter_type, "letter_type")?;
    let activity = c.accounts_receivable().send_dunning_letter(
        invoice_id,
        letter_type,
        req.sent_by.as_deref(),
    )?;
    Ok((StatusCode::CREATED, Json(activity_resp(&activity))))
}

// ============================================================================
// Customer statements
// ============================================================================

#[utoipa::path(get, operation_id = "ar_customer_statement", path = "/api/v1/ar/customers/{customer_id}/statement", tag = "accounts_receivable",
    params(("customer_id" = String, Path, description = "Customer ID"), ArStatementParams),
    responses((status = 200, body = ArCustomerStatementResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn customer_statement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<Uuid>,
    Query(params): Query<ArStatementParams>,
) -> Result<Json<ArCustomerStatementResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let period_start = match params.period_start.as_deref() {
        Some(s) => Some(parse_datetime(s, "period_start")?),
        None => None,
    };
    let period_end = match params.period_end.as_deref() {
        Some(s) => Some(parse_datetime(s, "period_end")?),
        None => None,
    };
    let request = stateset_core::GenerateStatementRequest {
        customer_id,
        period_start,
        period_end,
        include_paid_invoices: params.include_paid_invoices,
    };
    let s = c.accounts_receivable().generate_statement(request)?;
    Ok(Json(ArCustomerStatementResponse {
        customer_id: s.customer_id.to_string(),
        customer_name: s.customer_name,
        period_start: s.period_start.to_rfc3339(),
        period_end: s.period_end.to_rfc3339(),
        opening_balance: s.opening_balance.to_string(),
        total_invoices: s.total_invoices.to_string(),
        total_payments: s.total_payments.to_string(),
        total_credits: s.total_credits.to_string(),
        closing_balance: s.closing_balance.to_string(),
        line_items: s
            .line_items
            .iter()
            .map(|l| ArStatementLineResponse {
                date: l.date.to_rfc3339(),
                transaction_type: l.transaction_type.to_string(),
                reference_number: l.reference_number.clone(),
                description: l.description.clone(),
                debit: l.debit.map(|d| d.to_string()),
                credit: l.credit.map(|d| d.to_string()),
                balance: l.balance.to_string(),
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn aging_summary_on_empty_store() {
        let app = app();
        let resp =
            app.oneshot(Request::get("/ar/aging").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["total"], "0");
        assert_eq!(json["current"], "0");
        assert!(json["as_of_date"].as_str().is_some());
    }

    #[tokio::test]
    async fn create_and_list_credit_memos() {
        let app = app();
        let customer_id = uuid::Uuid::new_v4().to_string();
        let body = serde_json::json!({
            "customer_id": customer_id,
            "reason": "goodwill_adjustment",
            "amount": "25.50",
            "notes": "Goodwill credit"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/ar/credit-memos")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let memo = json_body(resp).await;
        assert_eq!(memo["amount"], "25.50");
        assert_eq!(memo["status"], "open");
        assert_eq!(memo["unapplied_amount"], "25.50");

        let resp = app
            .oneshot(
                Request::get(format!("/ar/credit-memos?customer_id={customer_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_body(resp).await;
        assert_eq!(list["total"], 1);
        assert_eq!(list["credit_memos"][0]["customer_id"], memo["customer_id"]);
    }

    #[tokio::test]
    async fn apply_payment_rejects_bad_payment_id() {
        let app = app();
        let body = serde_json::json!({
            "payment_id": "not-a-uuid",
            "applications": [{ "invoice_id": uuid::Uuid::new_v4().to_string(), "amount": "10" }]
        });
        let resp = app
            .oneshot(
                Request::post("/ar/payment-applications")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
