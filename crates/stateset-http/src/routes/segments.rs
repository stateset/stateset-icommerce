//! Customer segment endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::{CustomerId, SegmentId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateSegmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub segment_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SegmentFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SegmentResponse {
    pub id: String,
    pub name: String,
    pub segment_type: String,
    pub member_count: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SegmentListResponse {
    pub segments: Vec<SegmentResponse>,
    pub total: usize,
}

fn seg_to_resp(s: &stateset_core::Segment) -> SegmentResponse {
    SegmentResponse {
        id: s.id.to_string(),
        name: s.name.clone(),
        segment_type: s.segment_type.to_string(),
        member_count: s.member_count,
        created_at: s.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/segments", post(create_segment).get(list_segments))
        .route("/segments/{id}", get(get_segment).delete(delete_segment))
        .route("/segments/{id}/members/{customer_id}", post(add_member).delete(remove_member))
}

#[utoipa::path(post, path = "/api/v1/segments", tag = "segments",
    request_body = CreateSegmentRequest,
    responses((status = 201, body = SegmentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_segment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSegmentRequest>,
) -> Result<(StatusCode, Json<SegmentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateSegment {
        name: req.name,
        description: req.description,
        segment_type: req
            .segment_type
            .and_then(|t| t.parse().ok())
            .unwrap_or(stateset_core::SegmentType::Static),
        rules: vec![],
    };
    let s = c.segments().create(input)?;
    Ok((StatusCode::CREATED, Json(seg_to_resp(&s))))
}

#[utoipa::path(get, path = "/api/v1/segments", tag = "segments", params(SegmentFilterParams),
    responses((status = 200, body = SegmentListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_segments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SegmentFilterParams>,
) -> Result<Json<SegmentListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::SegmentFilter {
        name: params.search,
        segment_type: None,
        offset: Some(params.offset.unwrap_or(0)),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
    };
    let segs = c.segments().list(filter)?;
    let total = segs.len();
    Ok(Json(SegmentListResponse { segments: segs.iter().map(seg_to_resp).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/segments/{id}", tag = "segments",
    params(("id" = String, Path, description = "Segment ID")),
    responses((status = 200, body = SegmentResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_segment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SegmentId>,
) -> Result<Json<SegmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .segments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Segment {id} not found")))?;
    Ok(Json(seg_to_resp(&s)))
}

#[utoipa::path(delete, path = "/api/v1/segments/{id}", tag = "segments",
    params(("id" = String, Path, description = "Segment ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_segment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SegmentId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.segments().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/v1/segments/{id}/members/{customer_id}", tag = "segments",
    params(("id" = String, Path, description = "Segment ID"),
        ("customer_id" = String, Path, description = "Customer ID")),
    responses((status = 204, description = "Member added")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, cid)): Path<(SegmentId, CustomerId)>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.segments().add_member(id, cid)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/segments/{id}/members/{customer_id}", tag = "segments",
    params(("id" = String, Path, description = "Segment ID"),
        ("customer_id" = String, Path, description = "Customer ID")),
    responses((status = 204, description = "Member removed")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, cid)): Path<(SegmentId, CustomerId)>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.segments().remove_member(id, cid)?;
    Ok(StatusCode::NO_CONTENT)
}
