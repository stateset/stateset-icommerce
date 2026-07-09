//! Customer operational topology snapshot endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::TopologySnapshotId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub(crate) struct CaptureRequest {
    #[serde(default)]
    pub channels_total: u64,
    #[serde(default)]
    pub channels_active: u64,
    #[serde(default)]
    pub warehouses_total: u64,
    #[serde(default)]
    pub products_total: u64,
    #[serde(default)]
    pub open_orders: u64,
    #[serde(default)]
    pub signals: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SnapshotFilterParams {
    pub health: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotResponse {
    pub id: String,
    pub channels_total: u64,
    pub channels_active: u64,
    pub warehouses_total: u64,
    pub products_total: u64,
    pub open_orders: u64,
    pub health: String,
    pub signals: serde_json::Value,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotListResponse {
    pub snapshots: Vec<SnapshotResponse>,
    pub total: usize,
}

fn to_resp(s: &stateset_core::TopologySnapshot) -> SnapshotResponse {
    SnapshotResponse {
        id: s.id.to_string(),
        channels_total: s.channels_total,
        channels_active: s.channels_active,
        warehouses_total: s.warehouses_total,
        products_total: s.products_total,
        open_orders: s.open_orders,
        health: s.health.to_string(),
        signals: s.signals.clone(),
        captured_at: s.captured_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/topology-snapshots", post(capture).get(list))
        .route("/topology-snapshots/latest", get(latest))
        .route("/topology-snapshots/{id}", get(get_one).delete(delete_one))
}

#[utoipa::path(post, operation_id = "topology_snapshots_capture", path = "/api/v1/topology-snapshots", tag = "topology_snapshots",
    request_body = CaptureRequest,
    responses((status = 201, body = SnapshotResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CaptureRequest>,
) -> Result<(StatusCode, Json<SnapshotResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CaptureTopologySnapshot {
        channels_total: req.channels_total,
        channels_active: req.channels_active,
        warehouses_total: req.warehouses_total,
        products_total: req.products_total,
        open_orders: req.open_orders,
        signals: req.signals,
    };
    let s = c.topology_snapshots().capture(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&s))))
}

#[utoipa::path(get, operation_id = "topology_snapshots_list", path = "/api/v1/topology-snapshots", tag = "topology_snapshots",
    params(SnapshotFilterParams),
    responses((status = 200, body = SnapshotListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SnapshotFilterParams>,
) -> Result<Json<SnapshotListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let health = match params.health.as_deref() {
        Some(s) => Some(parse_id(s, "health")?),
        None => None,
    };
    let total = c
        .topology_snapshots()
        .list(stateset_core::TopologySnapshotFilter { health, ..Default::default() })?
        .len();
    let filter = stateset_core::TopologySnapshotFilter {
        health,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let snapshots = c.topology_snapshots().list(filter)?;
    Ok(Json(SnapshotListResponse { snapshots: snapshots.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "topology_snapshots_latest", path = "/api/v1/topology-snapshots/latest", tag = "topology_snapshots",
    responses((status = 200, body = SnapshotResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn latest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SnapshotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .topology_snapshots()
        .latest()?
        .ok_or_else(|| HttpError::NotFound("no topology snapshots captured yet".into()))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(get, operation_id = "topology_snapshots_get_one", path = "/api/v1/topology-snapshots/{id}", tag = "topology_snapshots",
    params(("id" = String, Path, description = "Snapshot ID")),
    responses((status = 200, body = SnapshotResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TopologySnapshotId>,
) -> Result<Json<SnapshotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .topology_snapshots()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Topology snapshot {id} not found")))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(delete, operation_id = "topology_snapshots_delete_one", path = "/api/v1/topology-snapshots/{id}", tag = "topology_snapshots",
    params(("id" = String, Path, description = "Snapshot ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TopologySnapshotId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.topology_snapshots().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn capture_then_latest() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "channels_total": 2, "channels_active": 1,
            "warehouses_total": 1, "products_total": 50, "open_orders": 3
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/topology-snapshots")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["health"], "healthy");

        let resp = app
            .oneshot(Request::get("/topology-snapshots/latest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
