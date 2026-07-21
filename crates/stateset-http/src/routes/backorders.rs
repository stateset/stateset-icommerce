//! Backorder endpoints (creation, fulfillment, cancellation).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response schemas
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateBackorderRequest {
    pub order_id: String,
    pub order_line_id: Option<String>,
    pub customer_id: String,
    pub sku: String,
    /// Decimal quantity as a string.
    pub quantity: String,
    /// One of `low`, `normal`, `high`, `critical`.
    pub priority: Option<String>,
    /// RFC 3339 expected availability date.
    pub expected_date: Option<String>,
    /// RFC 3339 date promised to the customer.
    pub promised_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct FulfillBackorderRequest {
    /// Decimal quantity as a string.
    pub quantity: String,
    /// One of `inventory`, `purchase_order`, `transfer`, `production`.
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub notes: Option<String>,
    pub fulfilled_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct BackorderFilterParams {
    pub order_id: Option<String>,
    pub customer_id: Option<String>,
    pub sku: Option<String>,
    /// One of `pending`, `partially_fulfilled`, `allocated`, `ready_to_ship`,
    /// `fulfilled`, `cancelled`.
    pub status: Option<String>,
    /// One of `low`, `normal`, `high`, `critical`.
    pub priority: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BackorderResponse {
    pub id: String,
    pub backorder_number: String,
    pub order_id: String,
    pub customer_id: String,
    pub sku: String,
    pub quantity_ordered: String,
    pub quantity_fulfilled: String,
    pub quantity_remaining: String,
    pub status: String,
    pub priority: String,
    pub expected_date: Option<String>,
    pub promised_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BackorderListResponse {
    pub backorders: Vec<BackorderResponse>,
    pub total: usize,
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_datetime(s: &str, what: &str) -> Result<chrono::DateTime<chrono::Utc>, HttpError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn to_resp(b: &stateset_core::Backorder) -> BackorderResponse {
    BackorderResponse {
        id: b.id.to_string(),
        backorder_number: b.backorder_number.clone(),
        order_id: b.order_id.to_string(),
        customer_id: b.customer_id.to_string(),
        sku: b.sku.clone(),
        quantity_ordered: b.quantity_ordered.to_string(),
        quantity_fulfilled: b.quantity_fulfilled.to_string(),
        quantity_remaining: b.quantity_remaining.to_string(),
        status: b.status.to_string(),
        priority: b.priority.to_string(),
        expected_date: b.expected_date.map(|d| d.to_rfc3339()),
        promised_date: b.promised_date.map(|d| d.to_rfc3339()),
        created_at: b.created_at.to_rfc3339(),
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/backorders", post(create).get(list))
        .route("/backorders/{id}", get(get_one))
        .route("/backorders/{id}/fulfill", post(fulfill))
        .route("/backorders/{id}/cancel", post(cancel))
}

#[utoipa::path(post, operation_id = "backorders_create", path = "/api/v1/backorders", tag = "backorders",
    request_body = CreateBackorderRequest,
    responses((status = 201, body = BackorderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateBackorderRequest>,
) -> Result<(StatusCode, Json<BackorderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateBackorder {
        order_id: parse_id::<Uuid>(&req.order_id, "order_id")?,
        order_line_id: req
            .order_line_id
            .as_deref()
            .map(|s| parse_id::<Uuid>(s, "order_line_id"))
            .transpose()?,
        customer_id: parse_id::<Uuid>(&req.customer_id, "customer_id")?,
        sku: req.sku,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        priority: req.priority.as_deref().map(|s| parse_id(s, "priority")).transpose()?,
        expected_date: req
            .expected_date
            .as_deref()
            .map(|s| parse_datetime(s, "expected_date"))
            .transpose()?,
        promised_date: req
            .promised_date
            .as_deref()
            .map(|s| parse_datetime(s, "promised_date"))
            .transpose()?,
        source_location_id: None,
        notes: req.notes,
    };
    let backorder = c.backorder().create_backorder(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&backorder))))
}

#[utoipa::path(get, operation_id = "backorders_list", path = "/api/v1/backorders", tag = "backorders",
    params(BackorderFilterParams),
    responses((status = 200, body = BackorderListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BackorderFilterParams>,
) -> Result<Json<BackorderListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::BackorderFilter {
        order_id: params
            .order_id
            .as_deref()
            .map(|s| parse_id::<Uuid>(s, "order_id"))
            .transpose()?,
        customer_id: params
            .customer_id
            .as_deref()
            .map(|s| parse_id::<Uuid>(s, "customer_id"))
            .transpose()?,
        sku: params.sku.clone(),
        status: params.status.as_deref().map(|s| parse_id(s, "status")).transpose()?,
        priority: params.priority.as_deref().map(|s| parse_id(s, "priority")).transpose()?,
        ..Default::default()
    };
    let total = c.backorder().list_backorders(base.clone())?.len();
    let filter = stateset_core::BackorderFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let backorders = c.backorder().list_backorders(filter)?;
    Ok(Json(BackorderListResponse { backorders: backorders.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "backorders_get_one", path = "/api/v1/backorders/{id}", tag = "backorders",
    params(("id" = String, Path, description = "Backorder ID")),
    responses((status = 200, body = BackorderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackorderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let backorder = c
        .backorder()
        .get_backorder(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Backorder {id} not found")))?;
    Ok(Json(to_resp(&backorder)))
}

#[utoipa::path(post, operation_id = "backorders_fulfill", path = "/api/v1/backorders/{id}/fulfill", tag = "backorders",
    request_body = FulfillBackorderRequest,
    params(("id" = String, Path, description = "Backorder ID")),
    responses((status = 200, body = BackorderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn fulfill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<FulfillBackorderRequest>,
) -> Result<Json<BackorderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let source_type = match req.source_type.as_deref() {
        Some(s) => parse_id(s, "source_type")?,
        None => stateset_core::FulfillmentSourceType::default(),
    };
    let input = stateset_core::FulfillBackorder {
        backorder_id: id,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        source_type,
        source_id: req
            .source_id
            .as_deref()
            .map(|s| parse_id::<Uuid>(s, "source_id"))
            .transpose()?,
        notes: req.notes,
        fulfilled_by: req.fulfilled_by,
    };
    Ok(Json(to_resp(&c.backorder().fulfill_backorder(input)?)))
}

#[utoipa::path(post, operation_id = "backorders_cancel", path = "/api/v1/backorders/{id}/cancel", tag = "backorders",
    params(("id" = String, Path, description = "Backorder ID")),
    responses((status = 200, body = BackorderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackorderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.backorder().cancel_backorder(id)?)))
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

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn create_fulfill_flow() {
        let app = app();
        let body = serde_json::json!({
            "order_id": uuid::Uuid::new_v4().to_string(),
            "customer_id": uuid::Uuid::new_v4().to_string(),
            "sku": "WIDGET-001",
            "quantity": "10",
            "priority": "high"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/backorders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "pending");
        assert_eq!(json["priority"], "high");
        assert_eq!(json["quantity_remaining"], "10");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::post(format!("/backorders/{id}/fulfill"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "quantity": "4", "source_type": "inventory" })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["quantity_fulfilled"], "4");
        assert_eq!(json["quantity_remaining"], "6");
        assert_eq!(json["status"], "partially_fulfilled");
    }

    #[tokio::test]
    async fn list_get_and_cancel() {
        let app = app();
        let order_id = uuid::Uuid::new_v4().to_string();
        let body = serde_json::json!({
            "order_id": order_id,
            "customer_id": uuid::Uuid::new_v4().to_string(),
            "sku": "GADGET-002",
            "quantity": "3"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/backorders")
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
            .oneshot(
                Request::get(format!("/backorders?order_id={order_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_body(resp).await;
        assert_eq!(list["total"], 1);

        let resp = app
            .clone()
            .oneshot(Request::get(format!("/backorders/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(Request::post(format!("/backorders/{id}/cancel")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "cancelled");
    }
}
