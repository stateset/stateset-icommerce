//! Manufacturing (BOM + Work Order) endpoints.

use axum::{Json, Router, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, routing::{get, patch, post}};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateBomRequest {
    pub name: String, pub product_id: Option<String>, pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWorkOrderRequest {
    #[schema(value_type = String, format = "uuid")]
    pub bom_id: Uuid,
    pub planned_quantity: Option<f64>,
    pub priority: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct BomFilterParams { pub limit: Option<u32>, pub offset: Option<u32> }

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct WorkOrderFilterParams { pub limit: Option<u32>, pub offset: Option<u32>, pub status: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomResponse { pub id: String, pub bom_number: String, pub name: String, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomListResponse { pub boms: Vec<BomResponse>, pub total: usize }

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderResponse {
    pub id: String, pub work_order_number: String, pub status: String,
    #[schema(value_type = String)]
    pub planned_quantity: Decimal,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderListResponse { pub work_orders: Vec<WorkOrderResponse>, pub total: usize }

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/boms", post(create_bom).get(list_boms))
        .route("/boms/{id}", get(get_bom))
        .route("/work-orders", post(create_work_order).get(list_work_orders))
        .route("/work-orders/{id}", get(get_work_order))
        .route("/work-orders/{id}/start", patch(start_work_order))
        .route("/work-orders/{id}/complete", patch(complete_work_order))
}

#[utoipa::path(post, path = "/api/v1/boms", tag = "manufacturing",
    request_body = CreateBomRequest,
    responses((status = 201, body = BomResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_bom(
    State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateBomRequest>,
) -> Result<(StatusCode, Json<BomResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateBom {
        name: req.name, product_id: req.product_id.and_then(|p| p.parse().ok()),
        description: req.description, ..Default::default()
    };
    let b = c.bom().create(input)?;
    Ok((StatusCode::CREATED, Json(BomResponse {
        id: b.id.to_string(), bom_number: b.bom_number, name: b.name,
        status: b.status.to_string(), created_at: b.created_at.to_rfc3339(),
    })))
}

#[utoipa::path(get, path = "/api/v1/boms", tag = "manufacturing", params(BomFilterParams),
    responses((status = 200, body = BomListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_boms(
    State(state): State<AppState>, headers: HeaderMap, Query(params): Query<BomFilterParams>,
) -> Result<Json<BomListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::BomFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)), ..Default::default()
    };
    let boms = c.bom().list(filter)?;
    let total = boms.len();
    Ok(Json(BomListResponse { boms: boms.into_iter().map(|b| BomResponse {
        id: b.id.to_string(), bom_number: b.bom_number, name: b.name,
        status: b.status.to_string(), created_at: b.created_at.to_rfc3339(),
    }).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/boms/{id}", tag = "manufacturing",
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 200, body = BomResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_bom(
    State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>,
) -> Result<Json<BomResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let b = c.bom().get(id)?.ok_or_else(|| HttpError::NotFound(format!("BOM {id} not found")))?;
    Ok(Json(BomResponse {
        id: b.id.to_string(), bom_number: b.bom_number, name: b.name,
        status: b.status.to_string(), created_at: b.created_at.to_rfc3339(),
    }))
}

#[utoipa::path(post, path = "/api/v1/work-orders", tag = "manufacturing",
    request_body = CreateWorkOrderRequest,
    responses((status = 201, body = WorkOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_work_order(
    State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateWorkOrderRequest>,
) -> Result<(StatusCode, Json<WorkOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateWorkOrder {
        bom_id: req.bom_id,
        planned_quantity: req.planned_quantity.map(|q| Decimal::try_from(q).unwrap_or_default()),
        notes: req.notes, ..Default::default()
    };
    let wo = c.work_orders().create(input)?;
    Ok((StatusCode::CREATED, Json(WorkOrderResponse {
        id: wo.id.to_string(), work_order_number: wo.work_order_number,
        status: wo.status.to_string(), planned_quantity: wo.planned_quantity,
        created_at: wo.created_at.to_rfc3339(),
    })))
}

#[utoipa::path(get, path = "/api/v1/work-orders", tag = "manufacturing", params(WorkOrderFilterParams),
    responses((status = 200, body = WorkOrderListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_work_orders(
    State(state): State<AppState>, headers: HeaderMap, Query(params): Query<WorkOrderFilterParams>,
) -> Result<Json<WorkOrderListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::WorkOrderFilter {
        status: params.status.and_then(|s| s.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)), ..Default::default()
    };
    let orders = c.work_orders().list(filter)?;
    let total = orders.len();
    Ok(Json(WorkOrderListResponse { work_orders: orders.into_iter().map(|wo| WorkOrderResponse {
        id: wo.id.to_string(), work_order_number: wo.work_order_number,
        status: wo.status.to_string(), planned_quantity: wo.planned_quantity,
        created_at: wo.created_at.to_rfc3339(),
    }).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/work-orders/{id}", tag = "manufacturing",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_work_order(
    State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let wo = c.work_orders().get(id)?.ok_or_else(|| HttpError::NotFound(format!("Work order {id} not found")))?;
    Ok(Json(WorkOrderResponse {
        id: wo.id.to_string(), work_order_number: wo.work_order_number,
        status: wo.status.to_string(), planned_quantity: wo.planned_quantity,
        created_at: wo.created_at.to_rfc3339(),
    }))
}

#[utoipa::path(patch, path = "/api/v1/work-orders/{id}/start", tag = "manufacturing",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start_work_order(
    State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let wo = c.work_orders().start(id)?;
    Ok(Json(WorkOrderResponse {
        id: wo.id.to_string(), work_order_number: wo.work_order_number,
        status: wo.status.to_string(), planned_quantity: wo.planned_quantity,
        created_at: wo.created_at.to_rfc3339(),
    }))
}

#[utoipa::path(patch, path = "/api/v1/work-orders/{id}/complete", tag = "manufacturing",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_work_order(
    State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let wo = c.work_orders().complete(id, Decimal::ZERO)?;
    Ok(Json(WorkOrderResponse {
        id: wo.id.to_string(), work_order_number: wo.work_order_number,
        status: wo.status.to_string(), planned_quantity: wo.planned_quantity,
        created_at: wo.created_at.to_rfc3339(),
    }))
}
