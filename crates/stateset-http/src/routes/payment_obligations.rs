//! Payment obligation endpoints (scheduled AP payments).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::PaymentObligationId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePaymentObligationRequest {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Amount owed as a string (must be positive).
    pub amount: String,
    /// Due date in ISO `YYYY-MM-DD` form.
    pub due_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecordPaymentRequest {
    /// Payment amount as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct LinkBillRequest {
    pub bill_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetStatusRequest {
    /// One of `pending`, `scheduled`, `partially_paid`, `paid`, `cancelled`.
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PaymentObligationFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub due_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PaymentObligationResponse {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub amount: String,
    pub amount_paid: String,
    pub outstanding: String,
    pub currency: String,
    pub due_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PaymentObligationListResponse {
    pub payment_obligations: Vec<PaymentObligationResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct DashboardResponse {
    pub open_count: u64,
    pub total_outstanding: String,
    pub overdue_count: u64,
    pub overdue_amount: String,
}

fn to_resp(o: &stateset_core::PaymentObligation) -> PaymentObligationResponse {
    PaymentObligationResponse {
        id: o.id.to_string(),
        number: o.number.clone(),
        supplier_id: o.supplier_id.to_string(),
        amount: o.amount.to_string(),
        amount_paid: o.amount_paid.to_string(),
        outstanding: o.outstanding().to_string(),
        currency: o.currency.to_string(),
        due_date: o.due_date.to_string(),
        status: o.status.to_string(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_date(s: &str) -> Result<NaiveDate, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid date (expected YYYY-MM-DD): {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payment-obligations", post(create).get(list))
        .route("/payment-obligations/dashboard", get(dashboard))
        .route("/payment-obligations/{id}", get(get_one))
        .route("/payment-obligations/{id}/payments", post(record_payment))
        .route("/payment-obligations/{id}/status", post(set_status))
        .route("/payment-obligations/{id}/bills", post(link_bill))
}

#[utoipa::path(post, path = "/api/v1/payment-obligations", tag = "payment_obligations",
    request_body = CreatePaymentObligationRequest,
    responses((status = 201, body = PaymentObligationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePaymentObligationRequest>,
) -> Result<(StatusCode, Json<PaymentObligationResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let purchase_order_id = match req.purchase_order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "purchase_order_id")?),
        None => None,
    };
    let input = stateset_core::CreatePaymentObligation {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        purchase_order_id,
        amount: parse_decimal(&req.amount, "amount")?,
        currency: None,
        due_date: parse_date(&req.due_date)?,
        notes: req.notes,
    };
    let o = c.payment_obligations().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&o))))
}

#[utoipa::path(get, path = "/api/v1/payment-obligations", tag = "payment_obligations",
    params(PaymentObligationFilterParams),
    responses((status = 200, body = PaymentObligationListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaymentObligationFilterParams>,
) -> Result<Json<PaymentObligationListResponse>, HttpError> {
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
    let due_before = match params.due_before.as_deref() {
        Some(s) => Some(parse_date(s)?),
        None => None,
    };
    let total = c
        .payment_obligations()
        .list(stateset_core::PaymentObligationFilter {
            supplier_id,
            status,
            due_before,
            ..Default::default()
        })?
        .len();
    let filter = stateset_core::PaymentObligationFilter {
        supplier_id,
        status,
        due_before,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let obligations = c.payment_obligations().list(filter)?;
    Ok(Json(PaymentObligationListResponse {
        payment_obligations: obligations.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/payment-obligations/dashboard", tag = "payment_obligations",
    responses((status = 200, body = DashboardResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let today = chrono::Utc::now().date_naive();
    let d = c.payment_obligations().dashboard(today)?;
    Ok(Json(DashboardResponse {
        open_count: d.open_count,
        total_outstanding: d.total_outstanding.to_string(),
        overdue_count: d.overdue_count,
        overdue_amount: d.overdue_amount.to_string(),
    }))
}

#[utoipa::path(get, path = "/api/v1/payment-obligations/{id}", tag = "payment_obligations",
    params(("id" = String, Path, description = "Payment obligation ID")),
    responses((status = 200, body = PaymentObligationResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentObligationId>,
) -> Result<Json<PaymentObligationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let o = c
        .payment_obligations()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Payment obligation {id} not found")))?;
    Ok(Json(to_resp(&o)))
}

#[utoipa::path(post, path = "/api/v1/payment-obligations/{id}/payments", tag = "payment_obligations",
    request_body = RecordPaymentRequest,
    params(("id" = String, Path, description = "Payment obligation ID")),
    responses((status = 200, body = PaymentObligationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn record_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentObligationId>,
    Json(req): Json<RecordPaymentRequest>,
) -> Result<Json<PaymentObligationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(
        &c.payment_obligations().record_payment(id, parse_decimal(&req.amount, "amount")?)?,
    )))
}

#[utoipa::path(post, path = "/api/v1/payment-obligations/{id}/status", tag = "payment_obligations",
    request_body = SetStatusRequest,
    params(("id" = String, Path, description = "Payment obligation ID")),
    responses((status = 200, body = PaymentObligationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentObligationId>,
    Json(req): Json<SetStatusRequest>,
) -> Result<Json<PaymentObligationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = parse_id(&req.status, "status")?;
    Ok(Json(to_resp(&c.payment_obligations().set_status(id, status)?)))
}

#[utoipa::path(post, path = "/api/v1/payment-obligations/{id}/bills", tag = "payment_obligations",
    request_body = LinkBillRequest,
    params(("id" = String, Path, description = "Payment obligation ID")),
    responses((status = 200, body = PaymentObligationResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn link_bill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentObligationId>,
    Json(req): Json<LinkBillRequest>,
) -> Result<Json<PaymentObligationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let bill_id = parse_id::<Uuid>(&req.bill_id, "bill_id")?;
    Ok(Json(to_resp(&c.payment_obligations().link_bill(id, bill_id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_pay_and_dashboard() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "amount": "100",
            "due_date": "2026-07-01"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/payment-obligations")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/payment-obligations/{id}/payments"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"amount": "100"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "paid");
        assert_eq!(json["outstanding"], "0");

        let resp = app
            .oneshot(Request::get("/payment-obligations/dashboard").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
