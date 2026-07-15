//! Sales / fulfillment channel endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::ChannelId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateChannelRequest {
    pub name: String,
    /// One of `sales_channel`, `fulfillment_channel`, `end_to_end_channel`.
    pub channel_type: String,
    pub integration: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateChannelRequest {
    pub name: Option<String>,
    pub integration: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct LockChannelRequest {
    pub locked: bool,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ChannelFilterParams {
    pub channel_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ChannelResponse {
    pub id: String,
    pub name: String,
    pub channel_type: String,
    pub integration: Option<String>,
    pub status: String,
    pub api_locked: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ChannelListResponse {
    pub channels: Vec<ChannelResponse>,
    pub total: usize,
}

fn to_resp(c: &stateset_core::Channel) -> ChannelResponse {
    ChannelResponse {
        id: c.id.to_string(),
        name: c.name.clone(),
        channel_type: c.channel_type.to_string(),
        integration: c.integration.clone(),
        status: c.status.to_string(),
        api_locked: c.api_locked,
        created_at: c.created_at.to_rfc3339(),
    }
}

fn parse_channel_type(s: &str) -> Result<stateset_core::ChannelType, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid channel_type: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/channels", post(create).get(list))
        .route("/channels/{id}", get(get_one).put(update).delete(delete_one))
        .route("/channels/{id}/lock", post(set_lock))
}

#[utoipa::path(post, operation_id = "channels_create", path = "/api/v1/channels", tag = "channels",
    request_body = CreateChannelRequest,
    responses((status = 201, body = ChannelResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateChannelRequest>,
) -> Result<(StatusCode, Json<ChannelResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateChannel {
        name: req.name,
        channel_type: parse_channel_type(&req.channel_type)?,
        integration: req.integration,
        default_warehouse_id: None,
        tags: Vec::new(),
        metadata: serde_json::Value::Null,
    };
    let ch = c.channels().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&ch))))
}

#[utoipa::path(get, operation_id = "channels_list", path = "/api/v1/channels", tag = "channels",
    params(ChannelFilterParams),
    responses((status = 200, body = ChannelListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChannelFilterParams>,
) -> Result<Json<ChannelListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;

    let channel_type = match params.channel_type.as_deref() {
        Some(s) => Some(parse_channel_type(s)?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => {
            Some(s.parse().map_err(|_| HttpError::BadRequest(format!("invalid status: {s}")))?)
        }
        None => None,
    };

    let total = c
        .channels()
        .list(stateset_core::ChannelFilter { channel_type, status, ..Default::default() })?
        .len();
    let filter = stateset_core::ChannelFilter {
        channel_type,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let channels = c.channels().list(filter)?;
    Ok(Json(ChannelListResponse { channels: channels.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "channels_get_one", path = "/api/v1/channels/{id}", tag = "channels",
    params(("id" = String, Path, description = "Channel ID")),
    responses((status = 200, body = ChannelResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ChannelId>,
) -> Result<Json<ChannelResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let ch = c
        .channels()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Channel {id} not found")))?;
    Ok(Json(to_resp(&ch)))
}

#[utoipa::path(put, operation_id = "channels_update", path = "/api/v1/channels/{id}", tag = "channels",
    request_body = UpdateChannelRequest,
    params(("id" = String, Path, description = "Channel ID")),
    responses((status = 200, body = ChannelResponse), (status = 403, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ChannelId>,
    Json(req): Json<UpdateChannelRequest>,
) -> Result<Json<ChannelResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match req.status.as_deref() {
        Some(s) => {
            Some(s.parse().map_err(|_| HttpError::BadRequest(format!("invalid status: {s}")))?)
        }
        None => None,
    };
    let input = stateset_core::UpdateChannel {
        name: req.name,
        integration: req.integration,
        status,
        ..Default::default()
    };
    let ch = c.channels().update(id, input)?;
    Ok(Json(to_resp(&ch)))
}

#[utoipa::path(delete, operation_id = "channels_delete_one", path = "/api/v1/channels/{id}", tag = "channels",
    params(("id" = String, Path, description = "Channel ID")),
    responses((status = 204, description = "Deleted"), (status = 403, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ChannelId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.channels().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "channels_set_lock", path = "/api/v1/channels/{id}/lock", tag = "channels",
    request_body = LockChannelRequest,
    params(("id" = String, Path, description = "Channel ID")),
    responses((status = 200, body = ChannelResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ChannelId>,
    Json(req): Json<LockChannelRequest>,
) -> Result<Json<ChannelResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let ch = c.channels().set_lock(id, req.locked)?;
    Ok(Json(to_resp(&ch)))
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

    #[tokio::test]
    async fn create_then_list_channel() {
        let app = app();
        let body = serde_json::json!({"name":"Shopify","channel_type":"sales_channel"});
        let resp = app
            .clone()
            .oneshot(
                Request::post("/channels")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp =
            app.oneshot(Request::get("/channels").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"], 1);
    }

    #[tokio::test]
    async fn invalid_channel_type_is_400() {
        let app = app();
        let body = serde_json::json!({"name":"X","channel_type":"bogus"});
        let resp = app
            .oneshot(
                Request::post("/channels")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
