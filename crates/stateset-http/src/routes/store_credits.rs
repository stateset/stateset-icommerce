//! Store credit endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{CurrencyCode, StoreCreditId, StoreCreditReason};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateStoreCreditRequest {
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: stateset_core::CustomerId,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: Option<String>,
    pub reason: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct StoreCreditFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status: Option<String>,
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct StoreCreditResponse {
    pub id: String,
    pub customer_id: String,
    #[schema(value_type = String)]
    pub original_balance: Decimal,
    #[schema(value_type = String)]
    pub current_balance: Decimal,
    pub currency: String,
    pub status: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct StoreCreditListResponse {
    pub store_credits: Vec<StoreCreditResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AdjustStoreCreditRequest {
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub note: Option<String>,
}

fn sc_to_response(sc: &stateset_core::StoreCredit) -> StoreCreditResponse {
    StoreCreditResponse {
        id: sc.id.to_string(),
        customer_id: sc.customer_id.to_string(),
        original_balance: sc.original_balance,
        current_balance: sc.current_balance,
        currency: sc.currency.as_str().to_string(),
        status: sc.status.to_string(),
        reason: sc.reason.to_string(),
        created_at: sc.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/store-credits", post(create_store_credit).get(list_store_credits))
        .route("/store-credits/{id}", get(get_store_credit))
        .route("/store-credits/{id}/adjust", post(adjust_store_credit))
}

#[utoipa::path(post, path = "/api/v1/store-credits", tag = "store_credits",
    request_body = CreateStoreCreditRequest,
    responses((status = 201, body = StoreCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStoreCreditRequest>,
) -> Result<(StatusCode, Json<StoreCreditResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let currency = CurrencyCode::from_str(req.currency.as_deref().unwrap_or("USD"))
        .map_err(|e| HttpError::BadRequest(format!("Invalid currency: {e}")))?;
    let reason = StoreCreditReason::from_str(req.reason.as_deref().unwrap_or("manual"))
        .unwrap_or(StoreCreditReason::Manual);
    let input = stateset_core::CreateStoreCredit {
        customer_id: req.customer_id,
        amount: req.amount,
        currency,
        reason,
        reference_id: None,
        note: req.note,
        expires_at: None,
    };
    let sc = commerce.store_credits().create(input)?;
    Ok((StatusCode::CREATED, Json(sc_to_response(&sc))))
}

#[utoipa::path(get, path = "/api/v1/store-credits", tag = "store_credits",
    params(StoreCreditFilterParams),
    responses((status = 200, body = StoreCreditListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_store_credits(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<StoreCreditFilterParams>,
) -> Result<Json<StoreCreditListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let filter = stateset_core::StoreCreditFilter {
        customer_id: params.customer_id.and_then(|c| c.parse().ok()),
        status: params.status.and_then(|s| s.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let credits = commerce.store_credits().list(filter)?;
    let total = credits.len();
    Ok(Json(StoreCreditListResponse {
        store_credits: credits.iter().map(sc_to_response).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/store-credits/{id}", tag = "store_credits",
    params(("id" = String, Path, description = "Store credit ID")),
    responses((status = 200, body = StoreCreditResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StoreCreditId>,
) -> Result<Json<StoreCreditResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sc = commerce
        .store_credits()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Store credit {id} not found")))?;
    Ok(Json(sc_to_response(&sc)))
}

#[utoipa::path(post, path = "/api/v1/store-credits/{id}/adjust", tag = "store_credits",
    params(("id" = String, Path, description = "Store credit ID")),
    request_body = AdjustStoreCreditRequest,
    responses((status = 200, body = StoreCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn adjust_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StoreCreditId>,
    Json(req): Json<AdjustStoreCreditRequest>,
) -> Result<Json<StoreCreditResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input =
        stateset_core::AdjustStoreCredit { amount: req.amount, note: req.note, reference_id: None };
    let sc = commerce.store_credits().adjust(id, input)?;
    Ok(Json(sc_to_response(&sc)))
}
