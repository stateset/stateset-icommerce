//! Prepayment endpoints (advance payments to suppliers).

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
use stateset_core::PrepaymentId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePrepaymentRequest {
    pub supplier_id: String,
    /// Prepaid amount as a string (must be positive).
    pub amount: String,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApplyPrepaymentRequest {
    /// `bill` or `payment_obligation`.
    pub target_type: String,
    pub target_id: String,
    /// Amount to apply as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PrepaymentFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PrepaymentResponse {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub amount: String,
    pub remaining: String,
    pub currency: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PrepaymentListResponse {
    pub prepayments: Vec<PrepaymentResponse>,
    pub total: usize,
}

fn to_resp(p: &stateset_core::Prepayment) -> PrepaymentResponse {
    PrepaymentResponse {
        id: p.id.to_string(),
        number: p.number.clone(),
        supplier_id: p.supplier_id.to_string(),
        amount: p.amount.to_string(),
        remaining: p.remaining.to_string(),
        currency: p.currency.to_string(),
        status: p.status.to_string(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/prepayments", post(create).get(list))
        .route("/prepayments/{id}", get(get_one))
        .route("/prepayments/{id}/apply", post(apply))
        .route("/prepayments/{id}/refund", post(refund))
}

#[utoipa::path(post, path = "/api/v1/prepayments", tag = "prepayments",
    request_body = CreatePrepaymentRequest,
    responses((status = 201, body = PrepaymentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePrepaymentRequest>,
) -> Result<(StatusCode, Json<PrepaymentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreatePrepayment {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        amount: parse_decimal(&req.amount, "amount")?,
        currency: None,
        method: req.method,
        reference: req.reference,
        memo: req.memo,
    };
    let p = c.prepayments().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&p))))
}

#[utoipa::path(get, path = "/api/v1/prepayments", tag = "prepayments",
    params(PrepaymentFilterParams),
    responses((status = 200, body = PrepaymentListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PrepaymentFilterParams>,
) -> Result<Json<PrepaymentListResponse>, HttpError> {
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
    let total = c
        .prepayments()
        .list(stateset_core::PrepaymentFilter { supplier_id, status, ..Default::default() })?
        .len();
    let filter = stateset_core::PrepaymentFilter {
        supplier_id,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let prepayments = c.prepayments().list(filter)?;
    Ok(Json(PrepaymentListResponse {
        prepayments: prepayments.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/prepayments/{id}", tag = "prepayments",
    params(("id" = String, Path, description = "Prepayment ID")),
    responses((status = 200, body = PrepaymentResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrepaymentId>,
) -> Result<Json<PrepaymentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c
        .prepayments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Prepayment {id} not found")))?;
    Ok(Json(to_resp(&p)))
}

#[utoipa::path(post, path = "/api/v1/prepayments/{id}/apply", tag = "prepayments",
    request_body = ApplyPrepaymentRequest,
    params(("id" = String, Path, description = "Prepayment ID")),
    responses((status = 200, body = PrepaymentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrepaymentId>,
    Json(req): Json<ApplyPrepaymentRequest>,
) -> Result<Json<PrepaymentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::ApplyPrepayment {
        target_type: parse_id(&req.target_type, "target_type")?,
        target_id: parse_id::<Uuid>(&req.target_id, "target_id")?,
        amount: parse_decimal(&req.amount, "amount")?,
    };
    Ok(Json(to_resp(&c.prepayments().apply(id, input)?)))
}

#[utoipa::path(post, path = "/api/v1/prepayments/{id}/refund", tag = "prepayments",
    params(("id" = String, Path, description = "Prepayment ID")),
    responses((status = 200, body = PrepaymentResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn refund(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrepaymentId>,
) -> Result<Json<PrepaymentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.prepayments().refund(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_apply_refund() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "amount": "100",
            "method": "wire"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/prepayments")
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

        let apply_body = serde_json::json!({
            "target_type": "bill",
            "target_id": uuid::Uuid::new_v4().to_string(),
            "amount": "30"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/prepayments/{id}/apply"))
                    .header("content-type", "application/json")
                    .body(Body::from(apply_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["remaining"], "70");

        let resp = app
            .oneshot(
                Request::post(format!("/prepayments/{id}/refund")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "refunded");
    }
}
