//! Vendor credit endpoints (supplier-owed credits).

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
use stateset_core::VendorCreditId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateVendorCreditRequest {
    pub supplier_id: String,
    pub vendor_return_id: Option<String>,
    /// Credit amount as a string (must be positive).
    pub amount: String,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApplyVendorCreditRequest {
    /// `bill` or `payment_obligation`.
    pub target_type: String,
    pub target_id: String,
    /// Amount to apply as a string.
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct VendorCreditFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct VendorCreditResponse {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub amount: String,
    pub remaining: String,
    pub currency: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct VendorCreditListResponse {
    pub vendor_credits: Vec<VendorCreditResponse>,
    pub total: usize,
}

fn to_resp(c: &stateset_core::VendorCredit) -> VendorCreditResponse {
    VendorCreditResponse {
        id: c.id.to_string(),
        number: c.number.clone(),
        supplier_id: c.supplier_id.to_string(),
        amount: c.amount.to_string(),
        remaining: c.remaining.to_string(),
        currency: c.currency.to_string(),
        status: c.status.to_string(),
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
        .route("/vendor-credits", post(create).get(list))
        .route("/vendor-credits/{id}", get(get_one))
        .route("/vendor-credits/{id}/apply", post(apply))
        .route("/vendor-credits/{id}/cancel", post(cancel))
}

#[utoipa::path(post, operation_id = "vendor_credits_create", path = "/api/v1/vendor-credits", tag = "vendor_credits",
    request_body = CreateVendorCreditRequest,
    responses((status = 201, body = VendorCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateVendorCreditRequest>,
) -> Result<(StatusCode, Json<VendorCreditResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let vendor_return_id = match req.vendor_return_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "vendor_return_id")?),
        None => None,
    };
    let input = stateset_core::CreateVendorCredit {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        vendor_return_id,
        amount: parse_decimal(&req.amount, "amount")?,
        currency: None,
        memo: req.memo,
    };
    let credit = c.vendor_credits().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&credit))))
}

#[utoipa::path(get, operation_id = "vendor_credits_list", path = "/api/v1/vendor-credits", tag = "vendor_credits",
    params(VendorCreditFilterParams),
    responses((status = 200, body = VendorCreditListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<VendorCreditFilterParams>,
) -> Result<Json<VendorCreditListResponse>, HttpError> {
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
        .vendor_credits()
        .list(stateset_core::VendorCreditFilter { supplier_id, status, ..Default::default() })?
        .len();
    let filter = stateset_core::VendorCreditFilter {
        supplier_id,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let credits = c.vendor_credits().list(filter)?;
    Ok(Json(VendorCreditListResponse {
        vendor_credits: credits.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "vendor_credits_get_one", path = "/api/v1/vendor-credits/{id}", tag = "vendor_credits",
    params(("id" = String, Path, description = "Vendor credit ID")),
    responses((status = 200, body = VendorCreditResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorCreditId>,
) -> Result<Json<VendorCreditResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let credit = c
        .vendor_credits()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Vendor credit {id} not found")))?;
    Ok(Json(to_resp(&credit)))
}

#[utoipa::path(post, operation_id = "vendor_credits_apply", path = "/api/v1/vendor-credits/{id}/apply", tag = "vendor_credits",
    request_body = ApplyVendorCreditRequest,
    params(("id" = String, Path, description = "Vendor credit ID")),
    responses((status = 200, body = VendorCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorCreditId>,
    Json(req): Json<ApplyVendorCreditRequest>,
) -> Result<Json<VendorCreditResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::ApplyVendorCredit {
        target_type: parse_id(&req.target_type, "target_type")?,
        target_id: parse_id::<Uuid>(&req.target_id, "target_id")?,
        amount: parse_decimal(&req.amount, "amount")?,
    };
    Ok(Json(to_resp(&c.vendor_credits().apply(id, input)?)))
}

#[utoipa::path(post, operation_id = "vendor_credits_cancel", path = "/api/v1/vendor-credits/{id}/cancel", tag = "vendor_credits",
    params(("id" = String, Path, description = "Vendor credit ID")),
    responses((status = 200, body = VendorCreditResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorCreditId>,
) -> Result<Json<VendorCreditResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.vendor_credits().cancel(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_then_apply() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "amount": "100"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/vendor-credits")
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
            "amount": "40"
        });
        let resp = app
            .oneshot(
                Request::post(format!("/vendor-credits/{id}/apply"))
                    .header("content-type", "application/json")
                    .body(Body::from(apply_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["remaining"], "60");
    }
}
