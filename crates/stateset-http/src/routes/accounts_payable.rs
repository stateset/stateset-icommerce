//! Accounts payable endpoints (supplier bills, payments, payment runs, aging).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response schemas
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateApBillItemRequest {
    pub description: String,
    pub account_code: Option<String>,
    /// Decimal quantity as a string.
    pub quantity: String,
    /// Decimal unit price as a string.
    pub unit_price: String,
    /// Optional decimal tax rate as a string.
    pub tax_rate: Option<String>,
    /// Purchase order line this bill line references (for three-way matching).
    pub po_line_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateApBillRequest {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// RFC 3339 due date, e.g. `2026-08-01T00:00:00Z`.
    pub due_date: String,
    pub payment_terms: Option<String>,
    pub reference_number: Option<String>,
    pub memo: Option<String>,
    pub items: Vec<CreateApBillItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApBillResponse {
    pub id: String,
    pub bill_number: String,
    pub supplier_id: String,
    pub status: String,
    pub due_date: String,
    pub subtotal: String,
    pub tax_amount: String,
    pub total_amount: String,
    pub amount_paid: String,
    pub amount_due: String,
    pub currency: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApBillListResponse {
    pub bills: Vec<ApBillResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ApBillFilterParams {
    pub supplier_id: Option<String>,
    /// One of `draft`, `pending`, `approved`, `partially_paid`, `paid`, `overdue`, `cancelled`, `disputed`.
    pub status: Option<String>,
    pub overdue_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApPaymentAllocationRequest {
    pub bill_id: String,
    /// Decimal amount as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateApPaymentRequest {
    pub supplier_id: String,
    /// One of `check`, `ach`, `wire`, `credit_card`, `cash`, `other`.
    pub payment_method: String,
    /// Decimal amount as a string.
    pub amount: String,
    pub reference_number: Option<String>,
    pub bank_account: Option<String>,
    pub check_number: Option<String>,
    pub memo: Option<String>,
    #[serde(default)]
    pub allocations: Vec<ApPaymentAllocationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApPaymentResponse {
    pub id: String,
    pub payment_number: String,
    pub supplier_id: String,
    pub payment_method: String,
    pub amount: String,
    pub currency: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateApPaymentRunRequest {
    /// RFC 3339 payment date.
    pub payment_date: String,
    /// One of `check`, `ach`, `wire`, `credit_card`, `cash`, `other`.
    pub payment_method: String,
    pub bill_ids: Vec<String>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApprovePaymentRunRequest {
    pub approved_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApPaymentRunResponse {
    pub id: String,
    pub run_number: String,
    pub status: String,
    pub payment_date: String,
    pub payment_method: String,
    pub total_amount: String,
    pub payment_count: i32,
    pub approved_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApAgingResponse {
    pub current: String,
    pub days_1_30: String,
    pub days_31_60: String,
    pub days_61_90: String,
    pub days_over_90: String,
    pub total: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ThreeWayMatchParams {
    /// Relative tolerance percentage as a decimal string (e.g. `"5"` = 5%).
    /// Defaults to `0` (exact matching).
    pub tolerance_percent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ThreeWayMatchLineResponse {
    pub po_line_id: Option<String>,
    pub bill_item_id: String,
    pub description: String,
    pub ordered_quantity: Option<String>,
    pub ordered_unit_cost: Option<String>,
    pub received_quantity: String,
    pub billed_quantity: String,
    pub billed_unit_cost: String,
    pub quantity_variance: String,
    pub price_variance: String,
    pub matched: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ThreeWayMatchResponse {
    pub bill_id: String,
    /// One of `not_required`, `pending`, `matched`, `variance`.
    pub match_status: String,
    /// Number of lines with variances (present when `match_status` is `variance`).
    pub variance_line_count: Option<u64>,
    pub tolerance_percent: String,
    pub lines: Vec<ThreeWayMatchLineResponse>,
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

fn parse_datetime(s: &str, what: &str) -> Result<DateTime<Utc>, HttpError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn bill_resp(b: &stateset_core::Bill) -> ApBillResponse {
    ApBillResponse {
        id: b.id.to_string(),
        bill_number: b.bill_number.clone(),
        supplier_id: b.supplier_id.to_string(),
        status: b.status.to_string(),
        due_date: b.due_date.to_rfc3339(),
        subtotal: b.subtotal.to_string(),
        tax_amount: b.tax_amount.to_string(),
        total_amount: b.total_amount.to_string(),
        amount_paid: b.amount_paid.to_string(),
        amount_due: b.amount_due.to_string(),
        currency: b.currency.to_string(),
        created_at: b.created_at.to_rfc3339(),
    }
}

fn payment_resp(p: &stateset_core::BillPayment) -> ApPaymentResponse {
    ApPaymentResponse {
        id: p.id.to_string(),
        payment_number: p.payment_number.clone(),
        supplier_id: p.supplier_id.to_string(),
        payment_method: p.payment_method.to_string(),
        amount: p.amount.to_string(),
        currency: p.currency.to_string(),
        status: p.status.to_string(),
        created_at: p.created_at.to_rfc3339(),
    }
}

fn run_resp(r: &stateset_core::PaymentRun) -> ApPaymentRunResponse {
    ApPaymentRunResponse {
        id: r.id.to_string(),
        run_number: r.run_number.clone(),
        status: r.status.to_string(),
        payment_date: r.payment_date.to_rfc3339(),
        payment_method: r.payment_method.to_string(),
        total_amount: r.total_amount.to_string(),
        payment_count: r.payment_count,
        approved_by: r.approved_by.clone(),
        created_at: r.created_at.to_rfc3339(),
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ap/bills", post(create_bill).get(list_bills))
        .route("/ap/bills/{id}", get(get_bill))
        .route("/ap/bills/{id}/approve", post(approve_bill))
        .route("/ap/bills/{id}/cancel", post(cancel_bill))
        .route("/ap/bills/{id}/dispute", post(dispute_bill))
        .route("/ap/bills/{id}/three-way-match", get(three_way_match_bill))
        .route("/ap/payments", post(create_payment))
        .route("/ap/payments/{id}/void", post(void_payment))
        .route("/ap/payment-runs", post(create_payment_run))
        .route("/ap/payment-runs/{id}/approve", post(approve_payment_run))
        .route("/ap/payment-runs/{id}/process", post(process_payment_run))
        .route("/ap/payment-runs/{id}/cancel", post(cancel_payment_run))
        .route("/ap/aging", get(aging))
}

// ============================================================================
// Bill handlers
// ============================================================================

#[utoipa::path(post, operation_id = "ap_create_bill", path = "/api/v1/ap/bills", tag = "accounts_payable",
    request_body = CreateApBillRequest,
    responses((status = 201, body = ApBillResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateApBillRequest>,
) -> Result<(StatusCode, Json<ApBillResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let purchase_order_id = match req.purchase_order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "purchase_order_id")?),
        None => None,
    };
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        let tax_rate = match i.tax_rate.as_deref() {
            Some(s) => Some(parse_decimal(s, "tax_rate")?),
            None => None,
        };
        items.push(stateset_core::CreateBillItem {
            description: i.description,
            account_code: i.account_code,
            quantity: parse_decimal(&i.quantity, "quantity")?,
            unit_price: parse_decimal(&i.unit_price, "unit_price")?,
            tax_rate,
            po_line_id: match i.po_line_id.as_deref() {
                Some(s) => Some(parse_id::<Uuid>(s, "po_line_id")?),
                None => None,
            },
        });
    }
    let input = stateset_core::CreateBill {
        bill_number: None,
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        purchase_order_id,
        bill_date: None,
        due_date: parse_datetime(&req.due_date, "due_date")?,
        payment_terms: req.payment_terms,
        currency: None,
        reference_number: req.reference_number,
        memo: req.memo,
        items,
    };
    let bill = c.accounts_payable().create_bill(input)?;
    Ok((StatusCode::CREATED, Json(bill_resp(&bill))))
}

#[utoipa::path(get, operation_id = "ap_list_bills", path = "/api/v1/ap/bills", tag = "accounts_payable",
    params(ApBillFilterParams),
    responses((status = 200, body = ApBillListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_bills(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ApBillFilterParams>,
) -> Result<Json<ApBillListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let supplier_id = match params.supplier_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "supplier_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::BillFilter {
        supplier_id,
        status,
        overdue_only: params.overdue_only,
        ..Default::default()
    };
    let total = c.accounts_payable().count_bills(base.clone())?;
    let filter = stateset_core::BillFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let bills = c.accounts_payable().list_bills(filter)?;
    Ok(Json(ApBillListResponse { bills: bills.iter().map(bill_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "ap_get_bill", path = "/api/v1/ap/bills/{id}", tag = "accounts_payable",
    params(("id" = String, Path, description = "Bill ID")),
    responses((status = 200, body = ApBillResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApBillResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let bill = c
        .accounts_payable()
        .get_bill(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Bill {id} not found")))?;
    Ok(Json(bill_resp(&bill)))
}

#[utoipa::path(post, operation_id = "ap_approve_bill", path = "/api/v1/ap/bills/{id}/approve", tag = "accounts_payable",
    params(("id" = String, Path, description = "Bill ID")),
    responses((status = 200, body = ApBillResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn approve_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApBillResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(bill_resp(&c.accounts_payable().approve_bill(id)?)))
}

#[utoipa::path(post, operation_id = "ap_cancel_bill", path = "/api/v1/ap/bills/{id}/cancel", tag = "accounts_payable",
    params(("id" = String, Path, description = "Bill ID")),
    responses((status = 200, body = ApBillResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApBillResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(bill_resp(&c.accounts_payable().cancel_bill(id)?)))
}

#[utoipa::path(post, operation_id = "ap_dispute_bill", path = "/api/v1/ap/bills/{id}/dispute", tag = "accounts_payable",
    params(("id" = String, Path, description = "Bill ID")),
    responses((status = 200, body = ApBillResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn dispute_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApBillResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(bill_resp(&c.accounts_payable().dispute_bill(id)?)))
}

#[utoipa::path(get, operation_id = "ap_three_way_match_bill", path = "/api/v1/ap/bills/{id}/three-way-match", tag = "accounts_payable",
    params(("id" = String, Path, description = "Bill ID"), ThreeWayMatchParams),
    responses((status = 200, body = ThreeWayMatchResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn three_way_match_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(params): Query<ThreeWayMatchParams>,
) -> Result<Json<ThreeWayMatchResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let tolerance = match params.tolerance_percent.as_deref() {
        Some(s) => Some(parse_decimal(s, "tolerance_percent")?),
        None => None,
    };
    let result = c.accounts_payable().three_way_match(id, tolerance)?;

    let (match_status, variance_line_count) = match result.match_status {
        stateset_core::MatchStatus::NotRequired => ("not_required", None),
        stateset_core::MatchStatus::Pending => ("pending", None),
        stateset_core::MatchStatus::Matched => ("matched", None),
        stateset_core::MatchStatus::Variance { variance_line_count } => {
            ("variance", Some(variance_line_count as u64))
        }
        _ => ("unknown", None),
    };

    Ok(Json(ThreeWayMatchResponse {
        bill_id: id.to_string(),
        match_status: match_status.to_string(),
        variance_line_count,
        tolerance_percent: result.tolerance_percent.to_string(),
        lines: result
            .lines
            .into_iter()
            .map(|l| ThreeWayMatchLineResponse {
                po_line_id: l.po_line_id.map(|id| id.to_string()),
                bill_item_id: l.bill_item_id.to_string(),
                description: l.description,
                ordered_quantity: l.ordered_quantity.map(|d| d.to_string()),
                ordered_unit_cost: l.ordered_unit_cost.map(|d| d.to_string()),
                received_quantity: l.received_quantity.to_string(),
                billed_quantity: l.billed_quantity.to_string(),
                billed_unit_cost: l.billed_unit_cost.to_string(),
                quantity_variance: l.quantity_variance.to_string(),
                price_variance: l.price_variance.to_string(),
                matched: l.matched,
                issues: l.issues,
            })
            .collect(),
    }))
}

// ============================================================================
// Payment handlers
// ============================================================================

#[utoipa::path(post, operation_id = "ap_create_payment", path = "/api/v1/ap/payments", tag = "accounts_payable",
    request_body = CreateApPaymentRequest,
    responses((status = 201, body = ApPaymentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateApPaymentRequest>,
) -> Result<(StatusCode, Json<ApPaymentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut allocations = Vec::with_capacity(req.allocations.len());
    for a in req.allocations {
        allocations.push(stateset_core::PaymentAllocationInput {
            bill_id: parse_id::<Uuid>(&a.bill_id, "bill_id")?,
            amount: parse_decimal(&a.amount, "allocation amount")?,
        });
    }
    let input = stateset_core::CreateBillPayment {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        payment_date: None,
        payment_method: parse_id(&req.payment_method, "payment_method")?,
        amount: parse_decimal(&req.amount, "amount")?,
        currency: None,
        reference_number: req.reference_number,
        bank_account: req.bank_account,
        check_number: req.check_number,
        memo: req.memo,
        allocations,
    };
    let payment = c.accounts_payable().create_payment(input)?;
    Ok((StatusCode::CREATED, Json(payment_resp(&payment))))
}

#[utoipa::path(post, operation_id = "ap_void_payment", path = "/api/v1/ap/payments/{id}/void", tag = "accounts_payable",
    params(("id" = String, Path, description = "Payment ID")),
    responses((status = 200, body = ApPaymentResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn void_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApPaymentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(payment_resp(&c.accounts_payable().void_payment(id)?)))
}

// ============================================================================
// Payment run handlers
// ============================================================================

#[utoipa::path(post, operation_id = "ap_create_payment_run", path = "/api/v1/ap/payment-runs", tag = "accounts_payable",
    request_body = CreateApPaymentRunRequest,
    responses((status = 201, body = ApPaymentRunResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_payment_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateApPaymentRunRequest>,
) -> Result<(StatusCode, Json<ApPaymentRunResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut bill_ids = Vec::with_capacity(req.bill_ids.len());
    for id in &req.bill_ids {
        bill_ids.push(parse_id::<Uuid>(id, "bill_id")?);
    }
    let input = stateset_core::CreatePaymentRun {
        payment_date: parse_datetime(&req.payment_date, "payment_date")?,
        payment_method: parse_id(&req.payment_method, "payment_method")?,
        bill_ids,
        notes: req.notes,
        created_by: req.created_by,
    };
    let run = c.accounts_payable().create_payment_run(input)?;
    Ok((StatusCode::CREATED, Json(run_resp(&run))))
}

#[utoipa::path(post, operation_id = "ap_approve_payment_run", path = "/api/v1/ap/payment-runs/{id}/approve", tag = "accounts_payable",
    request_body = ApprovePaymentRunRequest,
    params(("id" = String, Path, description = "Payment run ID")),
    responses((status = 200, body = ApPaymentRunResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn approve_payment_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ApprovePaymentRunRequest>,
) -> Result<Json<ApPaymentRunResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(run_resp(&c.accounts_payable().approve_payment_run(id, &req.approved_by)?)))
}

#[utoipa::path(post, operation_id = "ap_process_payment_run", path = "/api/v1/ap/payment-runs/{id}/process", tag = "accounts_payable",
    params(("id" = String, Path, description = "Payment run ID")),
    responses((status = 200, body = ApPaymentRunResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn process_payment_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApPaymentRunResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(run_resp(&c.accounts_payable().process_payment_run(id)?)))
}

#[utoipa::path(post, operation_id = "ap_cancel_payment_run", path = "/api/v1/ap/payment-runs/{id}/cancel", tag = "accounts_payable",
    params(("id" = String, Path, description = "Payment run ID")),
    responses((status = 200, body = ApPaymentRunResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_payment_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApPaymentRunResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(run_resp(&c.accounts_payable().cancel_payment_run(id)?)))
}

// ============================================================================
// Aging
// ============================================================================

#[utoipa::path(get, operation_id = "ap_aging", path = "/api/v1/ap/aging", tag = "accounts_payable",
    responses((status = 200, body = ApAgingResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn aging(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApAgingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let a = c.accounts_payable().get_aging_summary()?;
    Ok(Json(ApAgingResponse {
        current: a.current.to_string(),
        days_1_30: a.days_1_30.to_string(),
        days_31_60: a.days_31_60.to_string(),
        days_61_90: a.days_61_90.to_string(),
        days_over_90: a.days_over_90.to_string(),
        total: a.total.to_string(),
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

    async fn create_bill(app: &Router) -> serde_json::Value {
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "due_date": (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
            "items": [{ "description": "Widgets", "quantity": "10", "unit_price": "5" }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/ap/bills")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        json_body(resp).await
    }

    #[tokio::test]
    async fn create_approve_and_list_bills() {
        let app = app();
        let bill = create_bill(&app).await;
        assert_eq!(bill["total_amount"], "50");
        assert_eq!(bill["status"], "draft");
        let id = bill["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::post(format!("/ap/bills/{id}/approve")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let approved = json_body(resp).await;
        assert_eq!(approved["status"], "approved");

        let resp = app
            .oneshot(Request::get("/ap/bills?status=approved").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_body(resp).await;
        assert_eq!(list["total"], 1);
        assert_eq!(list["bills"][0]["id"], id.as_str());
    }

    #[tokio::test]
    async fn pay_approved_bill_then_check_aging() {
        let app = app();
        let bill = create_bill(&app).await;
        let id = bill["id"].as_str().unwrap().to_string();
        let supplier_id = bill["supplier_id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::post(format!("/ap/bills/{id}/approve")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = serde_json::json!({
            "supplier_id": supplier_id,
            "payment_method": "check",
            "amount": "50",
            "allocations": [{ "bill_id": id, "amount": "50" }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/ap/payments")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let payment = json_body(resp).await;
        assert_eq!(payment["amount"], "50");

        let resp = app
            .clone()
            .oneshot(Request::get(format!("/ap/bills/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let paid = json_body(resp).await;
        assert_eq!(paid["status"], "paid");
        assert_eq!(paid["amount_due"], "0");

        let resp =
            app.oneshot(Request::get("/ap/aging").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let aging = json_body(resp).await;
        assert_eq!(aging["total"], "0");
    }

    #[tokio::test]
    async fn three_way_match_reports_matched_and_variance() {
        use rust_decimal::Decimal;

        // Set up supplier, PO, and a fully-received receipt via the embedded API.
        let commerce = Commerce::new(":memory:").expect("in-memory Commerce");
        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name: "Acme Supplies".into(),
                ..Default::default()
            })
            .unwrap();
        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![stateset_core::CreatePurchaseOrderItem {
                    sku: "WIDGET-001".into(),
                    name: "Widget".into(),
                    quantity: "10".parse::<Decimal>().unwrap(),
                    unit_cost: "5".parse::<Decimal>().unwrap(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let po_id: uuid::Uuid = po.id.into();
        let po_line_id = po.items[0].id;

        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: "WH-1".into(),
                name: "Main".into(),
                warehouse_type: Default::default(),
                address: stateset_core::WarehouseAddress {
                    street1: "1 Dock St".into(),
                    street2: None,
                    city: "Reno".into(),
                    state: "NV".into(),
                    postal_code: "89501".into(),
                    country: "US".into(),
                    phone: None,
                },
                timezone: None,
            })
            .unwrap();

        let receipt = commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt {
                receipt_type: stateset_core::ReceiptType::PurchaseOrder,
                reference_type: Some("purchase_order".into()),
                reference_id: Some(po_id),
                supplier_id: Some(supplier.id),
                warehouse_id: warehouse.id,
                items: vec![stateset_core::CreateReceiptItem {
                    sku: "WIDGET-001".into(),
                    po_line_id: Some(po_line_id),
                    expected_quantity: "10".parse::<Decimal>().unwrap(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let receipt_items = commerce.receiving().get_receipt_items(receipt.id).unwrap();
        commerce.receiving().start_receiving(receipt.id).unwrap();
        commerce
            .receiving()
            .receive_items(stateset_core::ReceiveItems {
                receipt_id: receipt.id,
                items: vec![stateset_core::ReceiveItemLine {
                    receipt_item_id: receipt_items[0].id,
                    quantity_received: "10".parse::<Decimal>().unwrap(),
                    quantity_rejected: None,
                    rejection_reason: None,
                    lot_number: None,
                    serial_numbers: None,
                    expiration_date: None,
                    notes: None,
                }],
                receiving_location_id: None,
                received_by: None,
            })
            .unwrap();

        let state = AppState::new(commerce);
        let app = router().with_state(state);

        // Bill that matches the PO exactly.
        let body = serde_json::json!({
            "supplier_id": supplier.id.to_string(),
            "purchase_order_id": po_id.to_string(),
            "due_date": (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
            "items": [{
                "description": "Widget",
                "quantity": "10",
                "unit_price": "5",
                "po_line_id": po_line_id.to_string()
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/ap/bills")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bill = json_body(resp).await;
        let bill_id = bill["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/ap/bills/{bill_id}/three-way-match"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let matched = json_body(resp).await;
        assert_eq!(matched["match_status"], "matched");
        assert_eq!(matched["lines"][0]["received_quantity"], "10");
        assert_eq!(matched["lines"][0]["matched"], true);

        // Over-billed bill triggers a variance even with 5% tolerance.
        let body = serde_json::json!({
            "supplier_id": supplier.id.to_string(),
            "purchase_order_id": po_id.to_string(),
            "due_date": (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
            "items": [{
                "description": "Widget",
                "quantity": "12",
                "unit_price": "5",
                "po_line_id": po_line_id.to_string()
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/ap/bills")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bill2 = json_body(resp).await;
        let bill2_id = bill2["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/ap/bills/{bill2_id}/three-way-match?tolerance_percent=5"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let variance = json_body(resp).await;
        assert_eq!(variance["match_status"], "variance");
        assert_eq!(variance["variance_line_count"], 1);
        assert_eq!(variance["tolerance_percent"], "5");
    }

    #[tokio::test]
    async fn three_way_match_not_required_without_po() {
        let app = app();
        let bill = create_bill(&app).await;
        let id = bill["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::get(format!("/ap/bills/{id}/three-way-match"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["match_status"], "not_required");
        assert_eq!(body["lines"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn create_bill_rejects_bad_supplier_id() {
        let app = app();
        let body = serde_json::json!({
            "supplier_id": "not-a-uuid",
            "due_date": chrono::Utc::now().to_rfc3339(),
            "items": []
        });
        let resp = app
            .oneshot(
                Request::post("/ap/bills")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
