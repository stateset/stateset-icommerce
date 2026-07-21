//! Work order endpoints (manufacturing execution).

use crate::dto::{decode_cursor, encode_cursor, finalize_page, overfetch_limit};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::ProductId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWorkOrderTaskRequest {
    pub task_name: String,
    pub sequence: Option<i32>,
    /// Decimal estimated hours as a string.
    pub estimated_hours: Option<String>,
    pub assigned_to: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWorkOrderRequest {
    pub product_id: String,
    /// Decimal quantity as a string.
    pub quantity_to_build: String,
    pub bom_id: Option<String>,
    pub work_center_id: Option<String>,
    pub assigned_to: Option<String>,
    /// One of `low`, `normal`, `high`, `urgent`.
    pub priority: Option<String>,
    /// RFC 3339 timestamp.
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp.
    pub scheduled_end: Option<String>,
    pub notes: Option<String>,
    pub tasks: Option<Vec<CreateWorkOrderTaskRequest>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateWorkOrderRequest {
    pub work_center_id: Option<String>,
    pub assigned_to: Option<String>,
    /// One of `low`, `normal`, `high`, `urgent`.
    pub priority: Option<String>,
    /// Decimal quantity as a string.
    pub quantity_to_build: Option<String>,
    /// RFC 3339 timestamp.
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp.
    pub scheduled_end: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompleteWorkOrderRequest {
    /// Decimal quantity completed as a string.
    pub quantity_completed: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompleteWorkOrderTaskRequest {
    /// Decimal actual hours as a string.
    pub actual_hours: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct WorkOrderFilterParams {
    pub product_id: Option<String>,
    pub bom_id: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub work_center_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderResponse {
    pub id: String,
    pub work_order_number: String,
    pub product_id: String,
    pub bom_id: Option<String>,
    pub status: String,
    pub priority: String,
    pub quantity_to_build: String,
    pub quantity_completed: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderListResponse {
    pub work_orders: Vec<WorkOrderResponse>,
    pub total: u64,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderTaskResponse {
    pub id: String,
    pub work_order_id: String,
    pub sequence: i32,
    pub task_name: String,
    pub status: String,
    pub estimated_hours: Option<String>,
    pub actual_hours: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WorkOrderTaskListResponse {
    pub tasks: Vec<WorkOrderTaskResponse>,
    pub total: usize,
}

fn to_resp(wo: &stateset_core::WorkOrder) -> WorkOrderResponse {
    WorkOrderResponse {
        id: wo.id.to_string(),
        work_order_number: wo.work_order_number.clone(),
        product_id: wo.product_id.to_string(),
        bom_id: wo.bom_id.map(|id| id.to_string()),
        status: wo.status.to_string(),
        priority: wo.priority.to_string(),
        quantity_to_build: wo.quantity_to_build.to_string(),
        quantity_completed: wo.quantity_completed.to_string(),
        notes: wo.notes.clone(),
        created_at: wo.created_at.to_rfc3339(),
    }
}

fn task_to_resp(t: &stateset_core::WorkOrderTask) -> WorkOrderTaskResponse {
    WorkOrderTaskResponse {
        id: t.id.to_string(),
        work_order_id: t.work_order_id.to_string(),
        sequence: t.sequence,
        task_name: t.task_name.clone(),
        status: t.status.to_string(),
        estimated_hours: t.estimated_hours.map(|h| h.to_string()),
        actual_hours: t.actual_hours.map(|h| h.to_string()),
        notes: t.notes.clone(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_opt_id<T: std::str::FromStr>(s: Option<&str>, what: &str) -> Result<Option<T>, HttpError> {
    s.map(|v| parse_id(v, what)).transpose()
}

fn parse_opt_decimal(s: Option<&str>, what: &str) -> Result<Option<Decimal>, HttpError> {
    s.map(|v| parse_decimal(v, what)).transpose()
}

fn parse_opt_datetime(s: Option<&str>, what: &str) -> Result<Option<DateTime<Utc>>, HttpError> {
    s.map(|v| {
        v.parse::<DateTime<Utc>>()
            .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {v}")))
    })
    .transpose()
}

fn to_task_input(
    t: CreateWorkOrderTaskRequest,
) -> Result<stateset_core::CreateWorkOrderTask, HttpError> {
    Ok(stateset_core::CreateWorkOrderTask {
        sequence: t.sequence,
        task_name: t.task_name,
        estimated_hours: parse_opt_decimal(t.estimated_hours.as_deref(), "estimated_hours")?,
        assigned_to: parse_opt_id::<Uuid>(t.assigned_to.as_deref(), "assigned_to")?,
        notes: t.notes,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/work-orders", post(create).get(list))
        .route("/work-orders/{id}", get(get_one).put(update))
        .route("/work-orders/{id}/start", post(start))
        .route("/work-orders/{id}/complete", post(complete))
        .route("/work-orders/{id}/hold", post(hold))
        .route("/work-orders/{id}/resume", post(resume))
        .route("/work-orders/{id}/cancel", post(cancel))
        .route("/work-orders/{id}/tasks", post(add_task).get(list_tasks))
        .route("/work-orders/tasks/{task_id}/start", post(start_task))
        .route("/work-orders/tasks/{task_id}/complete", post(complete_task))
}

#[utoipa::path(post, operation_id = "work_orders_create", path = "/api/v1/work-orders", tag = "work_orders",
    request_body = CreateWorkOrderRequest,
    responses((status = 201, body = WorkOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWorkOrderRequest>,
) -> Result<(StatusCode, Json<WorkOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let priority = match req.priority.as_deref() {
        Some(s) => Some(parse_id(s, "priority")?),
        None => None,
    };
    let tasks = match req.tasks {
        Some(tasks) => Some(tasks.into_iter().map(to_task_input).collect::<Result<Vec<_>, _>>()?),
        None => None,
    };
    let input = stateset_core::CreateWorkOrder {
        product_id: parse_id::<ProductId>(&req.product_id, "product_id")?,
        bom_id: parse_opt_id::<Uuid>(req.bom_id.as_deref(), "bom_id")?,
        work_center_id: req.work_center_id,
        assigned_to: parse_opt_id::<Uuid>(req.assigned_to.as_deref(), "assigned_to")?,
        priority,
        quantity_to_build: parse_decimal(&req.quantity_to_build, "quantity_to_build")?,
        scheduled_start: parse_opt_datetime(req.scheduled_start.as_deref(), "scheduled_start")?,
        scheduled_end: parse_opt_datetime(req.scheduled_end.as_deref(), "scheduled_end")?,
        notes: req.notes,
        tasks,
    };
    let wo = c.work_orders().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&wo))))
}

#[utoipa::path(get, operation_id = "work_orders_list", path = "/api/v1/work-orders", tag = "work_orders",
    params(WorkOrderFilterParams),
    responses((status = 200, body = WorkOrderListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WorkOrderFilterParams>,
) -> Result<Json<WorkOrderListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = parse_opt_id::<ProductId>(params.product_id.as_deref(), "product_id")?;
    let bom_id = parse_opt_id::<Uuid>(params.bom_id.as_deref(), "bom_id")?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let priority = match params.priority.as_deref() {
        Some(s) => Some(parse_id(s, "priority")?),
        None => None,
    };
    let base = stateset_core::WorkOrderFilter {
        product_id,
        bom_id,
        status,
        priority,
        work_center_id: params.work_center_id.clone(),
        ..Default::default()
    };
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };
    let total = c.work_orders().count(base.clone())?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let filter = stateset_core::WorkOrderFilter {
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
        ..base
    };
    let mut orders = c.work_orders().list(filter)?;
    let has_more = finalize_page(&mut orders, limit);
    let next_cursor = if has_more {
        orders.last().map(|wo| encode_cursor(&wo.created_at.to_rfc3339(), &wo.id.to_string()))
    } else {
        None
    };
    Ok(Json(WorkOrderListResponse {
        work_orders: orders.iter().map(to_resp).collect(),
        total,
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "work_orders_get_one", path = "/api/v1/work-orders/{id}", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let wo = c
        .work_orders()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Work order {id} not found")))?;
    Ok(Json(to_resp(&wo)))
}

#[utoipa::path(put, operation_id = "work_orders_update", path = "/api/v1/work-orders/{id}", tag = "work_orders",
    request_body = UpdateWorkOrderRequest,
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkOrderRequest>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let priority = match req.priority.as_deref() {
        Some(s) => Some(parse_id(s, "priority")?),
        None => None,
    };
    let input = stateset_core::UpdateWorkOrder {
        work_center_id: req.work_center_id,
        assigned_to: parse_opt_id::<Uuid>(req.assigned_to.as_deref(), "assigned_to")?,
        priority,
        quantity_to_build: parse_opt_decimal(
            req.quantity_to_build.as_deref(),
            "quantity_to_build",
        )?,
        scheduled_start: parse_opt_datetime(req.scheduled_start.as_deref(), "scheduled_start")?,
        scheduled_end: parse_opt_datetime(req.scheduled_end.as_deref(), "scheduled_end")?,
        notes: req.notes,
        ..Default::default()
    };
    Ok(Json(to_resp(&c.work_orders().update(id, input)?)))
}

#[utoipa::path(post, operation_id = "work_orders_start", path = "/api/v1/work-orders/{id}/start", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.work_orders().start(id)?)))
}

#[utoipa::path(post, operation_id = "work_orders_complete", path = "/api/v1/work-orders/{id}/complete", tag = "work_orders",
    request_body = CompleteWorkOrderRequest,
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteWorkOrderRequest>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let qty = parse_decimal(&req.quantity_completed, "quantity_completed")?;
    Ok(Json(to_resp(&c.work_orders().complete(id, qty)?)))
}

#[utoipa::path(post, operation_id = "work_orders_hold", path = "/api/v1/work-orders/{id}/hold", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn hold(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.work_orders().hold(id)?)))
}

#[utoipa::path(post, operation_id = "work_orders_resume", path = "/api/v1/work-orders/{id}/resume", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.work_orders().resume(id)?)))
}

#[utoipa::path(post, operation_id = "work_orders_cancel", path = "/api/v1/work-orders/{id}/cancel", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.work_orders().cancel(id)?)))
}

#[utoipa::path(post, operation_id = "work_orders_add_task", path = "/api/v1/work-orders/{id}/tasks", tag = "work_orders",
    request_body = CreateWorkOrderTaskRequest,
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 201, body = WorkOrderTaskResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateWorkOrderTaskRequest>,
) -> Result<(StatusCode, Json<WorkOrderTaskResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let task = c.work_orders().add_task(id, to_task_input(req)?)?;
    Ok((StatusCode::CREATED, Json(task_to_resp(&task))))
}

#[utoipa::path(get, operation_id = "work_orders_list_tasks", path = "/api/v1/work-orders/{id}/tasks", tag = "work_orders",
    params(("id" = String, Path, description = "Work order ID")),
    responses((status = 200, body = WorkOrderTaskListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkOrderTaskListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let tasks = c.work_orders().get_tasks(id)?;
    Ok(Json(WorkOrderTaskListResponse {
        total: tasks.len(),
        tasks: tasks.iter().map(task_to_resp).collect(),
    }))
}

#[utoipa::path(post, operation_id = "work_orders_start_task", path = "/api/v1/work-orders/tasks/{task_id}/start", tag = "work_orders",
    params(("task_id" = String, Path, description = "Work order task ID")),
    responses((status = 200, body = WorkOrderTaskResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
) -> Result<Json<WorkOrderTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(task_to_resp(&c.work_orders().start_task(task_id)?)))
}

#[utoipa::path(post, operation_id = "work_orders_complete_task", path = "/api/v1/work-orders/tasks/{task_id}/complete", tag = "work_orders",
    request_body = CompleteWorkOrderTaskRequest,
    params(("task_id" = String, Path, description = "Work order task ID")),
    responses((status = 200, body = WorkOrderTaskResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    Json(req): Json<CompleteWorkOrderTaskRequest>,
) -> Result<Json<WorkOrderTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let actual_hours = parse_opt_decimal(req.actual_hours.as_deref(), "actual_hours")?;
    Ok(Json(task_to_resp(&c.work_orders().complete_task(task_id, actual_hours)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn create_start_complete_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "quantity_to_build": "100",
            "priority": "high",
            "tasks": [{"task_name": "Assembly", "sequence": 1, "estimated_hours": "2"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/work-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "planned");
        assert_eq!(json["priority"], "high");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::post(format!("/work-orders/{id}/start")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "in_progress");

        let resp = app
            .oneshot(
                Request::post(format!("/work-orders/{id}/complete"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"quantity_completed": "100"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "completed");
        assert_eq!(json["quantity_completed"], "100");
    }

    #[tokio::test]
    async fn list_and_tasks() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "quantity_to_build": "5"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/work-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let id = json_body(resp).await["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::get("/work-orders?status=planned").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["total"], 1);

        let task = serde_json::json!({"task_name": "QC", "sequence": 2});
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/work-orders/{id}/tasks"))
                    .header("content-type", "application/json")
                    .body(Body::from(task.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .oneshot(Request::get(format!("/work-orders/{id}/tasks")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["total"], 1);
    }

    #[tokio::test]
    async fn list_work_orders_has_more_and_next_cursor() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        for i in 0..3 {
            let _ = i;
            let body = serde_json::json!({
                "product_id": uuid::Uuid::new_v4().to_string(),
                "quantity_to_build": "10"
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/work-orders")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        let resp = app
            .clone()
            .oneshot(Request::get("/work-orders?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["work_orders"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/work-orders?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["work_orders"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_work_orders_invalid_cursor_returns_400() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .oneshot(Request::get("/work-orders?after=!!!invalid!!!").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
