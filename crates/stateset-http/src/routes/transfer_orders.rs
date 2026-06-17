//! Transfer order endpoints (inter-warehouse stock movement).

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
use stateset_core::{TransferOrderId, TransferOrderItemId, WarehouseId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateTransferOrderItemRequest {
    pub product_id: String,
    /// Decimal quantity as a string (e.g. `"10"`).
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateTransferOrderRequest {
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    pub items: Vec<CreateTransferOrderItemRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceiveLineRequest {
    pub item_id: String,
    /// Decimal quantity as a string (e.g. `"4"`).
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TransferOrderFilterParams {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TransferOrderItemResponse {
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub quantity_received: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TransferOrderResponse {
    pub id: String,
    pub number: String,
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    pub status: String,
    pub items: Vec<TransferOrderItemResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TransferOrderListResponse {
    pub transfer_orders: Vec<TransferOrderResponse>,
    pub total: usize,
}

fn to_resp(t: &stateset_core::TransferOrder) -> TransferOrderResponse {
    TransferOrderResponse {
        id: t.id.to_string(),
        number: t.number.clone(),
        source_warehouse_id: t.source_warehouse_id.to_string(),
        destination_warehouse_id: t.destination_warehouse_id.to_string(),
        status: t.status.to_string(),
        items: t
            .items
            .iter()
            .map(|i| TransferOrderItemResponse {
                id: i.id.to_string(),
                product_id: i.product_id.to_string(),
                quantity: i.quantity.to_string(),
                quantity_received: i.quantity_received.to_string(),
            })
            .collect(),
        created_at: t.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid quantity: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/transfer-orders", post(create).get(list))
        .route("/transfer-orders/{id}", get(get_one))
        .route("/transfer-orders/{id}/ship", post(ship))
        .route("/transfer-orders/{id}/receive", post(receive))
        .route("/transfer-orders/{id}/cancel", post(cancel))
}

#[utoipa::path(post, path = "/api/v1/transfer-orders", tag = "transfer_orders",
    request_body = CreateTransferOrderRequest,
    responses((status = 201, body = TransferOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTransferOrderRequest>,
) -> Result<(StatusCode, Json<TransferOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::CreateTransferOrderItem {
            product_id: parse_id(&i.product_id, "product_id")?,
            quantity: parse_decimal(&i.quantity)?,
        });
    }
    let input = stateset_core::CreateTransferOrder {
        source_warehouse_id: parse_id::<WarehouseId>(
            &req.source_warehouse_id,
            "source_warehouse_id",
        )?,
        destination_warehouse_id: parse_id::<WarehouseId>(
            &req.destination_warehouse_id,
            "destination_warehouse_id",
        )?,
        items,
        expected_at: None,
        notes: req.notes,
    };
    let t = c.transfer_orders().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&t))))
}

#[utoipa::path(get, path = "/api/v1/transfer-orders", tag = "transfer_orders",
    params(TransferOrderFilterParams),
    responses((status = 200, body = TransferOrderListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TransferOrderFilterParams>,
) -> Result<Json<TransferOrderListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let total = c
        .transfer_orders()
        .list(stateset_core::TransferOrderFilter { status, ..Default::default() })?
        .len();
    let filter = stateset_core::TransferOrderFilter {
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let orders = c.transfer_orders().list(filter)?;
    Ok(Json(TransferOrderListResponse {
        transfer_orders: orders.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/transfer-orders/{id}", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let t = c
        .transfer_orders()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Transfer order {id} not found")))?;
    Ok(Json(to_resp(&t)))
}

#[utoipa::path(post, path = "/api/v1/transfer-orders/{id}/ship", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn ship(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.transfer_orders().ship(id)?)))
}

#[utoipa::path(post, path = "/api/v1/transfer-orders/{id}/receive", tag = "transfer_orders",
    request_body = ReceiveLineRequest,
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn receive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
    Json(req): Json<ReceiveLineRequest>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let item_id: TransferOrderItemId = parse_id(&req.item_id, "item_id")?;
    let t = c.transfer_orders().receive_line(id, item_id, parse_decimal(&req.quantity)?)?;
    Ok(Json(to_resp(&t)))
}

#[utoipa::path(post, path = "/api/v1/transfer-orders/{id}/cancel", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.transfer_orders().cancel(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_transfer_order() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "source_warehouse_id": uuid::Uuid::new_v4().to_string(),
            "destination_warehouse_id": uuid::Uuid::new_v4().to_string(),
            "items": [{"product_id": uuid::Uuid::new_v4().to_string(), "quantity": "10"}]
        });
        let resp = app
            .oneshot(
                Request::post("/transfer-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
}
