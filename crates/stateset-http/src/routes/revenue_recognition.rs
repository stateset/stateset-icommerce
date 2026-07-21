//! Revenue recognition (ASC 606) endpoints.

use crate::dto::{decode_cursor, encode_cursor, finalize_page, overfetch_limit};
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
use stateset_core::RecognitionMethod;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateObligationRequest {
    pub description: String,
    /// Decimal standalone selling price as a string.
    pub standalone_selling_price: Option<String>,
    /// Decimal allocated amount as a string.
    pub allocated_amount: String,
    /// One of `point_in_time`, `ratable_over_time`, `milestone`.
    pub recognition_method: String,
    /// ISO date; required for `ratable_over_time`.
    pub start: Option<String>,
    /// ISO date; required for `ratable_over_time`.
    pub end: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateRevenueContractRequest {
    pub contract_number: Option<String>,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// Decimal transaction price as a string.
    pub transaction_price: String,
    /// ISO date (`YYYY-MM-DD`).
    pub effective_date: String,
    pub obligations: Vec<CreateObligationRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub(crate) struct UpdateRevenueContractRequest {
    /// One of `draft`, `active`, `completed`, `cancelled`.
    pub status: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// ISO date (`YYYY-MM-DD`).
    pub effective_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecognizeRevenueRequest {
    /// Recognize deferred entries with a period start on or before this ISO date.
    pub through: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct RevenueContractFilterParams {
    pub customer_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PerformanceObligationResponse {
    pub id: String,
    pub contract_id: String,
    pub description: String,
    pub allocated_amount: String,
    pub recognized_amount: String,
    pub deferred_amount: String,
    pub recognition_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevenueContractResponse {
    pub id: String,
    pub contract_number: String,
    pub customer_id: String,
    pub status: String,
    pub transaction_price: String,
    pub total_recognized: String,
    pub deferred_balance: String,
    pub currency: String,
    pub effective_date: String,
    pub obligations: Vec<PerformanceObligationResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevenueContractListResponse {
    pub revenue_contracts: Vec<RevenueContractResponse>,
    pub total: usize,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevenueScheduleEntryResponse {
    pub period: u32,
    pub period_start: String,
    pub amount: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevenueScheduleResponse {
    pub obligation_id: String,
    pub total_amount: String,
    pub recognized_total: String,
    pub deferred_total: String,
    pub entries: Vec<RevenueScheduleEntryResponse>,
}

const fn method_name(m: RecognitionMethod) -> &'static str {
    match m {
        RecognitionMethod::PointInTime => "point_in_time",
        RecognitionMethod::RatableOverTime { .. } => "ratable_over_time",
        RecognitionMethod::Milestone => "milestone",
        _ => "other",
    }
}

fn obligation_resp(o: &stateset_core::PerformanceObligation) -> PerformanceObligationResponse {
    PerformanceObligationResponse {
        id: o.id.to_string(),
        contract_id: o.contract_id.to_string(),
        description: o.description.clone(),
        allocated_amount: o.allocated_amount.to_string(),
        recognized_amount: o.recognized_amount.to_string(),
        deferred_amount: o.deferred_amount().to_string(),
        recognition_method: method_name(o.recognition_method).to_string(),
    }
}

fn to_resp(c: &stateset_core::RevenueContract) -> RevenueContractResponse {
    RevenueContractResponse {
        id: c.id.to_string(),
        contract_number: c.contract_number.clone(),
        customer_id: c.customer_id.to_string(),
        status: c.status.to_string(),
        transaction_price: c.transaction_price.to_string(),
        total_recognized: c.total_recognized().to_string(),
        deferred_balance: c.deferred_balance().to_string(),
        currency: c.currency.to_string(),
        effective_date: c.effective_date.to_string(),
        obligations: c.obligations.iter().map(obligation_resp).collect(),
        created_at: c.created_at.to_rfc3339(),
    }
}

fn schedule_resp(s: &stateset_core::RevenueSchedule) -> RevenueScheduleResponse {
    RevenueScheduleResponse {
        obligation_id: s.obligation_id.to_string(),
        total_amount: s.total_amount.to_string(),
        recognized_total: s.recognized_total().to_string(),
        deferred_total: s.deferred_total().to_string(),
        entries: s
            .entries
            .iter()
            .map(|e| RevenueScheduleEntryResponse {
                period: e.period,
                period_start: e.period_start.to_string(),
                amount: e.amount.to_string(),
                status: e.status.to_string(),
            })
            .collect(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_date(s: &str, what: &str) -> Result<NaiveDate, HttpError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_method(req: &CreateObligationRequest) -> Result<RecognitionMethod, HttpError> {
    match req.recognition_method.as_str() {
        "point_in_time" => Ok(RecognitionMethod::PointInTime),
        "milestone" => Ok(RecognitionMethod::Milestone),
        "ratable_over_time" => {
            let start = req.start.as_deref().ok_or_else(|| {
                HttpError::BadRequest("start is required for ratable_over_time".into())
            })?;
            let end = req.end.as_deref().ok_or_else(|| {
                HttpError::BadRequest("end is required for ratable_over_time".into())
            })?;
            Ok(RecognitionMethod::RatableOverTime {
                start: parse_date(start, "start")?,
                end: parse_date(end, "end")?,
            })
        }
        other => Err(HttpError::BadRequest(format!("invalid recognition_method: {other}"))),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/revenue-contracts", post(create_contract).get(list_contracts))
        .route("/revenue-contracts/{id}", get(get_contract).put(update_contract))
        .route("/revenue-contracts/{id}/obligations", get(list_obligations))
        .route("/revenue-obligations/{id}/schedule", post(generate_schedule).get(get_schedule))
        .route("/revenue-obligations/{id}/recognize", post(recognize))
}

#[utoipa::path(post, operation_id = "revenue_recognition_create_contract", path = "/api/v1/revenue-contracts", tag = "revenue_recognition",
    request_body = CreateRevenueContractRequest,
    responses((status = 201, body = RevenueContractResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRevenueContractRequest>,
) -> Result<(StatusCode, Json<RevenueContractResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let order_id = match req.order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "order_id")?),
        None => None,
    };
    let invoice_id = match req.invoice_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "invoice_id")?),
        None => None,
    };
    let mut obligations = Vec::with_capacity(req.obligations.len());
    for ob in &req.obligations {
        let standalone_selling_price = match ob.standalone_selling_price.as_deref() {
            Some(s) => Some(parse_decimal(s, "standalone_selling_price")?),
            None => None,
        };
        obligations.push(stateset_core::CreatePerformanceObligation {
            description: ob.description.clone(),
            standalone_selling_price,
            allocated_amount: parse_decimal(&ob.allocated_amount, "allocated_amount")?,
            recognition_method: parse_method(ob)?,
        });
    }
    let input = stateset_core::CreateRevenueContract {
        contract_number: req.contract_number,
        customer_id: parse_id::<Uuid>(&req.customer_id, "customer_id")?,
        order_id,
        invoice_id,
        transaction_price: parse_decimal(&req.transaction_price, "transaction_price")?,
        currency: None,
        effective_date: parse_date(&req.effective_date, "effective_date")?,
        obligations,
    };
    let contract = c.revenue_recognition().create_contract(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&contract))))
}

#[utoipa::path(get, operation_id = "revenue_recognition_list_contracts", path = "/api/v1/revenue-contracts", tag = "revenue_recognition",
    params(RevenueContractFilterParams),
    responses((status = 200, body = RevenueContractListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_contracts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<RevenueContractFilterParams>,
) -> Result<Json<RevenueContractListResponse>, HttpError> {
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
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };
    let total = c
        .revenue_recognition()
        .list_contracts(stateset_core::RevenueContractFilter {
            customer_id,
            status,
            ..Default::default()
        })?
        .len();
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let filter = stateset_core::RevenueContractFilter {
        customer_id,
        status,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
        ..Default::default()
    };
    let mut contracts = c.revenue_recognition().list_contracts(filter)?;
    let has_more = finalize_page(&mut contracts, limit);
    let next_cursor = if has_more {
        contracts.last().map(|ct| encode_cursor(&ct.created_at.to_rfc3339(), &ct.id.to_string()))
    } else {
        None
    };
    Ok(Json(RevenueContractListResponse {
        revenue_contracts: contracts.iter().map(to_resp).collect(),
        total,
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "revenue_recognition_get_contract", path = "/api/v1/revenue-contracts/{id}", tag = "revenue_recognition",
    params(("id" = String, Path, description = "Revenue contract ID")),
    responses((status = 200, body = RevenueContractResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<RevenueContractResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let contract = c
        .revenue_recognition()
        .get_contract(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Revenue contract {id} not found")))?;
    Ok(Json(to_resp(&contract)))
}

#[utoipa::path(put, operation_id = "revenue_recognition_update_contract", path = "/api/v1/revenue-contracts/{id}", tag = "revenue_recognition",
    request_body = UpdateRevenueContractRequest,
    params(("id" = String, Path, description = "Revenue contract ID")),
    responses((status = 200, body = RevenueContractResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRevenueContractRequest>,
) -> Result<Json<RevenueContractResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match req.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let order_id = match req.order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "order_id")?),
        None => None,
    };
    let invoice_id = match req.invoice_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "invoice_id")?),
        None => None,
    };
    let effective_date = match req.effective_date.as_deref() {
        Some(s) => Some(parse_date(s, "effective_date")?),
        None => None,
    };
    let input =
        stateset_core::UpdateRevenueContract { order_id, invoice_id, status, effective_date };
    Ok(Json(to_resp(&c.revenue_recognition().update_contract(id, input)?)))
}

#[utoipa::path(get, operation_id = "revenue_recognition_list_obligations", path = "/api/v1/revenue-contracts/{id}/obligations", tag = "revenue_recognition",
    params(("id" = String, Path, description = "Revenue contract ID")),
    responses((status = 200, body = [PerformanceObligationResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_obligations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PerformanceObligationResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let obligations = c.revenue_recognition().list_obligations(id)?;
    Ok(Json(obligations.iter().map(obligation_resp).collect()))
}

#[utoipa::path(post, operation_id = "revenue_recognition_generate_schedule", path = "/api/v1/revenue-obligations/{id}/schedule", tag = "revenue_recognition",
    params(("id" = String, Path, description = "Performance obligation ID")),
    responses((status = 200, body = RevenueScheduleResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn generate_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<RevenueScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(schedule_resp(&c.revenue_recognition().generate_schedule(id)?)))
}

#[utoipa::path(get, operation_id = "revenue_recognition_get_schedule", path = "/api/v1/revenue-obligations/{id}/schedule", tag = "revenue_recognition",
    params(("id" = String, Path, description = "Performance obligation ID")),
    responses((status = 200, body = RevenueScheduleResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<RevenueScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .revenue_recognition()
        .get_schedule(id)?
        .ok_or_else(|| HttpError::NotFound(format!("No schedule generated for obligation {id}")))?;
    Ok(Json(schedule_resp(&s)))
}

#[utoipa::path(post, operation_id = "revenue_recognition_recognize", path = "/api/v1/revenue-obligations/{id}/recognize", tag = "revenue_recognition",
    request_body = RecognizeRevenueRequest,
    params(("id" = String, Path, description = "Performance obligation ID")),
    responses((status = 200, body = RevenueScheduleResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn recognize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<RecognizeRevenueRequest>,
) -> Result<Json<RevenueScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let through = parse_date(&req.through, "through")?;
    Ok(Json(schedule_resp(&c.revenue_recognition().recognize_period(id, through)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn json_of(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn create_activate_schedule_and_recognize_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "customer_id": uuid::Uuid::new_v4().to_string(),
            "transaction_price": "1200",
            "effective_date": "2026-01-01",
            "obligations": [
                {
                    "description": "Annual support",
                    "allocated_amount": "1200",
                    "recognition_method": "ratable_over_time",
                    "start": "2026-01-01",
                    "end": "2026-12-31"
                }
            ]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/revenue-contracts")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_of(resp).await;
        assert_eq!(json["status"], "draft");
        assert_eq!(json["deferred_balance"], "1200");
        let id = json["id"].as_str().unwrap().to_string();
        let ob_id = json["obligations"][0]["id"].as_str().unwrap().to_string();

        // Activate.
        let resp = app
            .clone()
            .oneshot(
                Request::put(format!("/revenue-contracts/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"status": "active"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "active");

        // Generate the schedule.
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/revenue-obligations/{ob_id}/schedule"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["total_amount"], "1200");
        assert_eq!(json["entries"].as_array().unwrap().len(), 12);

        // Recognize the entire year -> contract completes.
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/revenue-obligations/{ob_id}/recognize"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"through": "2026-12-31"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["recognized_total"], "1200");
        assert_eq!(json["deferred_total"], "0");

        let resp = app
            .oneshot(Request::get(format!("/revenue-contracts/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let json = json_of(resp).await;
        assert_eq!(json["status"], "completed");
        assert_eq!(json["total_recognized"], "1200");
        assert_eq!(json["deferred_balance"], "0");
    }

    #[tokio::test]
    async fn misallocated_contract_is_bad_request() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "customer_id": uuid::Uuid::new_v4().to_string(),
            "transaction_price": "1000",
            "effective_date": "2026-01-01",
            "obligations": [
                {
                    "description": "License",
                    "allocated_amount": "900",
                    "recognition_method": "point_in_time"
                }
            ]
        });
        let resp = app
            .oneshot(
                Request::post("/revenue-contracts")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn list_revenue_contracts_has_more_and_next_cursor() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        for i in 0..3 {
            let _ = i;
            let body = serde_json::json!({
                "customer_id": uuid::Uuid::new_v4().to_string(),
                "transaction_price": "100",
                "effective_date": "2026-01-01",
                "obligations": [{
                    "description": "One-time",
                    "allocated_amount": "100",
                    "recognition_method": "point_in_time"
                }]
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/revenue-contracts")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        let resp = app
            .clone()
            .oneshot(Request::get("/revenue-contracts?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["revenue_contracts"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/revenue-contracts?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["revenue_contracts"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_revenue_contracts_invalid_cursor_returns_400() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .oneshot(
                Request::get("/revenue-contracts?after=!!!invalid!!!").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
