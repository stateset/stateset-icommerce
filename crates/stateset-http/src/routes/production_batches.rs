//! Production batch endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::ProductionBatchId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateProductionBatchRequest {
    pub name: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub work_order_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct WorkOrdersRequest {
    pub work_order_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ProductionBatchFilterParams {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ProductionBatchResponse {
    pub id: String,
    pub name: String,
    pub status: String,
    pub work_order_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ProductionBatchListResponse {
    pub production_batches: Vec<ProductionBatchResponse>,
    pub total: usize,
}

fn to_resp(b: &stateset_core::ProductionBatch) -> ProductionBatchResponse {
    ProductionBatchResponse {
        id: b.id.to_string(),
        name: b.name.clone(),
        status: b.status.to_string(),
        work_order_ids: b.work_order_ids.iter().map(ToString::to_string).collect(),
        created_at: b.created_at.to_rfc3339(),
    }
}

fn parse_uuids(values: &[String]) -> Result<Vec<Uuid>, HttpError> {
    values
        .iter()
        .map(|s| {
            s.parse::<Uuid>()
                .map_err(|_| HttpError::BadRequest(format!("invalid work order id: {s}")))
        })
        .collect()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/production-batches", post(create).get(list))
        .route("/production-batches/{id}", get(get_one).delete(delete_one))
        .route("/production-batches/{id}/work-orders", post(add_work_orders))
}

#[utoipa::path(post, operation_id = "production_batches_create", path = "/api/v1/production-batches", tag = "production_batches",
    request_body = CreateProductionBatchRequest,
    responses((status = 201, body = ProductionBatchResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateProductionBatchRequest>,
) -> Result<(StatusCode, Json<ProductionBatchResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateProductionBatch {
        name: req.name,
        vendor_id: None,
        work_order_ids: parse_uuids(&req.work_order_ids)?,
        notes: req.notes,
        scheduled_start: None,
        scheduled_end: None,
    };
    let b = c.production_batches().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&b))))
}

#[utoipa::path(get, operation_id = "production_batches_list", path = "/api/v1/production-batches", tag = "production_batches",
    params(ProductionBatchFilterParams),
    responses((status = 200, body = ProductionBatchListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ProductionBatchFilterParams>,
) -> Result<Json<ProductionBatchListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => {
            Some(s.parse().map_err(|_| HttpError::BadRequest(format!("invalid status: {s}")))?)
        }
        None => None,
    };
    let total = c
        .production_batches()
        .list(stateset_core::ProductionBatchFilter { status, ..Default::default() })?
        .len();
    let filter = stateset_core::ProductionBatchFilter {
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let batches = c.production_batches().list(filter)?;
    Ok(Json(ProductionBatchListResponse {
        production_batches: batches.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "production_batches_get_one", path = "/api/v1/production-batches/{id}", tag = "production_batches",
    params(("id" = String, Path, description = "Production batch ID")),
    responses((status = 200, body = ProductionBatchResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductionBatchId>,
) -> Result<Json<ProductionBatchResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let b = c
        .production_batches()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Production batch {id} not found")))?;
    Ok(Json(to_resp(&b)))
}

#[utoipa::path(delete, operation_id = "production_batches_delete_one", path = "/api/v1/production-batches/{id}", tag = "production_batches",
    params(("id" = String, Path, description = "Production batch ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductionBatchId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.production_batches().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "production_batches_add_work_orders", path = "/api/v1/production-batches/{id}/work-orders", tag = "production_batches",
    request_body = WorkOrdersRequest,
    params(("id" = String, Path, description = "Production batch ID")),
    responses((status = 200, body = ProductionBatchResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_work_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductionBatchId>,
    Json(req): Json<WorkOrdersRequest>,
) -> Result<Json<ProductionBatchResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let b = c.production_batches().add_work_orders(id, parse_uuids(&req.work_order_ids)?)?;
    Ok(Json(to_resp(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_and_get_batch() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::post("/production-batches")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"name":"Batch 1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "planned");
    }
}
