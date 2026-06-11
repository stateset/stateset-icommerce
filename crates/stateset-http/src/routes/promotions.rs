//! Promotion endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::PromotionId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePromotionRequest {
    pub name: String,
    pub description: Option<String>,
    pub promotion_type: String,
    #[schema(value_type = Option<String>)]
    pub discount_value: Option<Decimal>,
    pub code: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub max_uses: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PromotionFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PromotionResponse {
    pub id: String,
    pub name: String,
    pub promotion_type: String,
    pub status: String,
    pub code: Option<String>,
    #[schema(value_type = Option<String>)]
    pub discount_value: Option<Decimal>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PromotionListResponse {
    pub promotions: Vec<PromotionResponse>,
    pub total: usize,
}

fn promo_to_resp(p: &stateset_core::Promotion) -> PromotionResponse {
    PromotionResponse {
        id: p.id.to_string(),
        name: p.name.clone(),
        promotion_type: p.promotion_type.to_string(),
        status: p.status.to_string(),
        code: Some(p.code.clone()),
        discount_value: p.percentage_off,
        created_at: p.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/promotions", post(create_promotion).get(list_promotions))
        .route("/promotions/{id}", get(get_promotion))
        .route("/promotions/{id}/activate", patch(activate_promotion))
        .route("/promotions/{id}/deactivate", patch(deactivate_promotion))
}

#[utoipa::path(post, path = "/api/v1/promotions", tag = "promotions",
    request_body = CreatePromotionRequest,
    responses((status = 201, body = PromotionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_promotion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePromotionRequest>,
) -> Result<(StatusCode, Json<PromotionResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreatePromotion {
        name: req.name,
        description: req.description,
        promotion_type: req
            .promotion_type
            .parse()
            .map_err(|e| HttpError::BadRequest(format!("{e}")))?,
        code: req.code.clone(),
        ..Default::default()
    };
    let p = c.promotions().create(input)?;
    Ok((StatusCode::CREATED, Json(promo_to_resp(&p))))
}

#[utoipa::path(get, path = "/api/v1/promotions", tag = "promotions", params(PromotionFilterParams),
    responses((status = 200, body = PromotionListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_promotions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PromotionFilterParams>,
) -> Result<Json<PromotionListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::PromotionFilter {
        status: params.status.and_then(|s| s.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let promos = c.promotions().list(filter)?;
    let total = promos.len();
    Ok(Json(PromotionListResponse {
        promotions: promos.iter().map(promo_to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/promotions/{id}", tag = "promotions",
    params(("id" = String, Path, description = "Promotion ID")),
    responses((status = 200, body = PromotionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_promotion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PromotionId>,
) -> Result<Json<PromotionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c
        .promotions()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Promotion {id} not found")))?;
    Ok(Json(promo_to_resp(&p)))
}

#[utoipa::path(patch, path = "/api/v1/promotions/{id}/activate", tag = "promotions",
    params(("id" = String, Path, description = "Promotion ID")),
    responses((status = 200, body = PromotionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn activate_promotion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PromotionId>,
) -> Result<Json<PromotionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c.promotions().activate(id)?;
    Ok(Json(promo_to_resp(&p)))
}

#[utoipa::path(patch, path = "/api/v1/promotions/{id}/deactivate", tag = "promotions",
    params(("id" = String, Path, description = "Promotion ID")),
    responses((status = 200, body = PromotionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn deactivate_promotion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PromotionId>,
) -> Result<Json<PromotionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c.promotions().deactivate(id)?;
    Ok(Json(promo_to_resp(&p)))
}
