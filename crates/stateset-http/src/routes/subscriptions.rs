//! Subscription endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::SubscriptionId;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateSubscriptionRequest {
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: stateset_core::CustomerId,
    #[schema(value_type = String, format = "uuid")]
    pub plan_id: Uuid,
    pub payment_method_id: Option<String>,
    pub skip_trial: Option<bool>,
    pub coupon_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SubscriptionFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status: Option<String>,
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SubscriptionResponse {
    pub id: String,
    pub customer_id: String,
    pub status: String,
    pub billing_interval: String,
    #[schema(value_type = String)]
    pub price: Decimal,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SubscriptionListResponse {
    pub subscriptions: Vec<SubscriptionResponse>,
    pub total: usize,
}

fn sub_to_response(s: &stateset_core::Subscription) -> SubscriptionResponse {
    SubscriptionResponse {
        id: s.id.to_string(),
        customer_id: s.customer_id.to_string(),
        status: s.status.to_string(),
        billing_interval: s.billing_interval.to_string(),
        price: s.price,
        currency: s.currency.as_str().to_string(),
        created_at: s.created_at.to_rfc3339(),
        updated_at: s.updated_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subscriptions", post(create_subscription).get(list_subscriptions))
        .route("/subscriptions/{id}", get(get_subscription))
        .route("/subscriptions/{id}/pause", patch(pause_subscription))
        .route("/subscriptions/{id}/resume", patch(resume_subscription))
        .route("/subscriptions/{id}/cancel", patch(cancel_subscription))
}

#[utoipa::path(post, path = "/api/v1/subscriptions", tag = "subscriptions",
    request_body = CreateSubscriptionRequest,
    responses((status = 201, body = SubscriptionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<SubscriptionResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input = stateset_core::CreateSubscription {
        customer_id: req.customer_id,
        plan_id: req.plan_id,
        payment_method_id: req.payment_method_id,
        skip_trial: req.skip_trial,
        coupon_code: req.coupon_code,
        ..Default::default()
    };
    let sub = commerce.subscriptions().subscribe(input)?;
    Ok((StatusCode::CREATED, Json(sub_to_response(&sub))))
}

#[utoipa::path(get, path = "/api/v1/subscriptions", tag = "subscriptions",
    params(SubscriptionFilterParams),
    responses((status = 200, body = SubscriptionListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SubscriptionFilterParams>,
) -> Result<Json<SubscriptionListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let filter = stateset_core::SubscriptionFilter {
        customer_id: params.customer_id.and_then(|c| c.parse().ok()),
        status: params.status.and_then(|s| s.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let subs = commerce.subscriptions().list(filter)?;
    let total = subs.len();
    Ok(Json(SubscriptionListResponse {
        subscriptions: subs.iter().map(sub_to_response).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/subscriptions/{id}", tag = "subscriptions",
    params(("id" = String, Path, description = "Subscription ID")),
    responses((status = 200, body = SubscriptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SubscriptionId>,
) -> Result<Json<SubscriptionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sub = commerce
        .subscriptions()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Subscription {id} not found")))?;
    Ok(Json(sub_to_response(&sub)))
}

#[utoipa::path(patch, path = "/api/v1/subscriptions/{id}/pause", tag = "subscriptions",
    params(("id" = String, Path, description = "Subscription ID")),
    responses((status = 200, body = SubscriptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn pause_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SubscriptionId>,
) -> Result<Json<SubscriptionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sub = commerce.subscriptions().pause(id, stateset_core::PauseSubscription::default())?;
    Ok(Json(sub_to_response(&sub)))
}

#[utoipa::path(patch, path = "/api/v1/subscriptions/{id}/resume", tag = "subscriptions",
    params(("id" = String, Path, description = "Subscription ID")),
    responses((status = 200, body = SubscriptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn resume_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SubscriptionId>,
) -> Result<Json<SubscriptionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sub = commerce.subscriptions().resume(id)?;
    Ok(Json(sub_to_response(&sub)))
}

#[utoipa::path(patch, path = "/api/v1/subscriptions/{id}/cancel", tag = "subscriptions",
    params(("id" = String, Path, description = "Subscription ID")),
    responses((status = 200, body = SubscriptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SubscriptionId>,
) -> Result<Json<SubscriptionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sub = commerce.subscriptions().cancel(id, stateset_core::CancelSubscription::default())?;
    Ok(Json(sub_to_response(&sub)))
}
