//! Warranty endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWarrantyRequest {
    #[schema(value_type = String, format = "uuid")]
    pub order_id: stateset_core::OrderId,
    #[schema(value_type = String, format = "uuid")]
    pub product_id: stateset_core::ProductId,
    pub warranty_type: Option<String>,
    pub duration_months: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct WarrantyFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status: Option<String>,
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WarrantyResponse {
    pub id: String,
    pub warranty_number: String,
    pub order_id: String,
    pub product_id: String,
    pub warranty_type: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WarrantyListResponse {
    pub warranties: Vec<WarrantyResponse>,
    pub total: usize,
}

fn warranty_to_resp(w: &stateset_core::Warranty) -> WarrantyResponse {
    WarrantyResponse {
        id: w.id.to_string(),
        warranty_number: w.warranty_number.clone(),
        order_id: w.order_id.map(|id| id.to_string()).unwrap_or_default(),
        product_id: w.product_id.map(|id| id.to_string()).unwrap_or_default(),
        warranty_type: w.warranty_type.to_string(),
        status: w.status.to_string(),
        created_at: w.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/warranties", post(create_warranty).get(list_warranties))
        .route("/warranties/{id}", get(get_warranty))
}

#[utoipa::path(post, path = "/api/v1/warranties", tag = "warranties",
    request_body = CreateWarrantyRequest,
    responses((status = 201, body = WarrantyResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_warranty(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWarrantyRequest>,
) -> Result<(StatusCode, Json<WarrantyResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateWarranty {
        order_id: Some(req.order_id),
        product_id: Some(req.product_id),
        warranty_type: req.warranty_type.and_then(|t| t.parse().ok()),
        duration_months: req.duration_months,
        ..Default::default()
    };
    let w = c.warranties().create(input)?;
    Ok((StatusCode::CREATED, Json(warranty_to_resp(&w))))
}

#[utoipa::path(get, path = "/api/v1/warranties", tag = "warranties", params(WarrantyFilterParams),
    responses((status = 200, body = WarrantyListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_warranties(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WarrantyFilterParams>,
) -> Result<Json<WarrantyListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::WarrantyFilter {
        status: params.status.and_then(|s| s.parse().ok()),
        customer_id: params.customer_id.and_then(|c| c.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let warranties = c.warranties().list(filter)?;
    let total = warranties.len();
    Ok(Json(WarrantyListResponse {
        warranties: warranties.iter().map(warranty_to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/warranties/{id}", tag = "warranties",
    params(("id" = String, Path, description = "Warranty ID")),
    responses((status = 200, body = WarrantyResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_warranty(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WarrantyResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let w = c
        .warranties()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Warranty {id} not found")))?;
    Ok(Json(warranty_to_resp(&w)))
}
