//! Transfer order endpoints (inter-warehouse stock movement).

use crate::error::{ErrorBody, HttpError};
use crate::parsing::parse_id;
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
    pub sku: String,
    pub lot_id: Option<String>,
    pub product_id: String,
    /// Decimal quantity as a string (e.g. `"10"`).
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateTransferOrderRequest {
    pub source_location_id: i32,
    pub destination_location_id: i32,
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
    pub sku: String,
    pub lot_id: Option<String>,
    pub quantity_shipped: String,
    pub quantity_variance: String,
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub quantity_received: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TransferOrderResponse {
    pub source_location_id: Option<i32>,
    pub destination_location_id: Option<i32>,
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
        source_location_id: t.source_location_id,
        destination_location_id: t.destination_location_id,
        id: t.id.to_string(),
        number: t.number.clone(),
        source_warehouse_id: t.source_warehouse_id.to_string(),
        destination_warehouse_id: t.destination_warehouse_id.to_string(),
        status: t.status.to_string(),
        items: t
            .items
            .iter()
            .map(|i| TransferOrderItemResponse {
                sku: i.sku.clone(),
                lot_id: i.lot_id.map(|id| id.to_string()),
                quantity_shipped: i.quantity_shipped.to_string(),
                quantity_variance: i.quantity_variance.to_string(),
                id: i.id.to_string(),
                product_id: i.product_id.to_string(),
                quantity: i.quantity.to_string(),
                quantity_received: i.quantity_received.to_string(),
            })
            .collect(),
        created_at: t.created_at.to_rfc3339(),
    }
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
        .route("/transfer-orders/{id}/complete-receiving", post(complete_receiving))
}

#[utoipa::path(post, operation_id = "transfer_orders_create", path = "/api/v1/transfer-orders", tag = "transfer_orders",
    request_body = CreateTransferOrderRequest,
    responses((status = 201, body = TransferOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTransferOrderRequest>,
) -> Result<(StatusCode, Json<TransferOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
            let mut items = Vec::with_capacity(req.items.len());
            for i in req.items {
                items.push(stateset_core::CreateTransferOrderItem {
                    sku: i.sku,
                    lot_id: i.lot_id.as_deref().map(|id| parse_id(id, "lot_id")).transpose()?,
                    product_id: parse_id(&i.product_id, "product_id")?,
                    quantity: parse_decimal(&i.quantity)?,
                });
            }
            let input = stateset_core::CreateTransferOrder {
                source_location_id: req.source_location_id,
                destination_location_id: req.destination_location_id,
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
        })
        .await
}

#[utoipa::path(get, operation_id = "transfer_orders_list", path = "/api/v1/transfer-orders", tag = "transfer_orders",
    params(TransferOrderFilterParams),
    responses((status = 200, body = TransferOrderListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TransferOrderFilterParams>,
) -> Result<Json<TransferOrderListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
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
        })
        .await
}

#[utoipa::path(get, operation_id = "transfer_orders_get_one", path = "/api/v1/transfer-orders/{id}", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
            let t = c
                .transfer_orders()
                .get(id)?
                .ok_or_else(|| HttpError::NotFound(format!("Transfer order {id} not found")))?;
            Ok(Json(to_resp(&t)))
        })
        .await
}

#[utoipa::path(post, operation_id = "transfer_orders_ship", path = "/api/v1/transfer-orders/{id}/ship", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn ship(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| Ok(Json(to_resp(&c.transfer_orders().ship(id)?))))
        .await
}

#[utoipa::path(post, operation_id = "transfer_orders_receive", path = "/api/v1/transfer-orders/{id}/receive", tag = "transfer_orders",
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
    state
        .run_blocking(tid.as_deref(), move |c| {
            let item_id: TransferOrderItemId = parse_id(&req.item_id, "item_id")?;
            let t = c.transfer_orders().receive_line(id, item_id, parse_decimal(&req.quantity)?)?;
            Ok(Json(to_resp(&t)))
        })
        .await
}

#[utoipa::path(post, operation_id = "transfer_orders_cancel", path = "/api/v1/transfer-orders/{id}/cancel", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| Ok(Json(to_resp(&c.transfer_orders().cancel(id)?))))
        .await
}

#[utoipa::path(post, operation_id = "transfer_orders_complete_receiving", path = "/api/v1/transfer-orders/{id}/complete-receiving", tag = "transfer_orders",
    params(("id" = String, Path, description = "Transfer order ID")),
    responses((status = 200, body = TransferOrderResponse)))]
pub(crate) async fn complete_receiving(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<TransferOrderId>,
) -> Result<Json<TransferOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
            Ok(Json(to_resp(&c.transfer_orders().complete_receiving(id)?)))
        })
        .await
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
        use stateset_core::{
            AdjustLocationInventory, CreateLocation, CreateProduct, CreateProductVariant,
            CreateWarehouse,
        };
        let commerce = Commerce::new(":memory:").unwrap();
        let mut locations = Vec::new();
        for code in ["SRC", "DST"] {
            let warehouse = commerce
                .warehouse()
                .create_warehouse(CreateWarehouse {
                    code: code.into(),
                    name: code.into(),
                    ..Default::default()
                })
                .unwrap();
            locations.push(
                commerce
                    .warehouse()
                    .create_location(CreateLocation {
                        warehouse_id: warehouse.id,
                        ..Default::default()
                    })
                    .unwrap()
                    .id,
            );
        }
        let product = commerce
            .products()
            .create(CreateProduct {
                name: "Transfer stock".into(),
                slug: None,
                description: None,
                attributes: None,
                seo: None,
                product_type: None,
                variants: Some(vec![CreateProductVariant {
                    sku: "TRANSFER-HTTP".into(),
                    price: Decimal::ONE,
                    ..Default::default()
                }]),
            })
            .unwrap();
        commerce
            .warehouse()
            .adjust_inventory(AdjustLocationInventory {
                location_id: locations[0],
                sku: "TRANSFER-HTTP".into(),
                lot_id: None,
                quantity: Decimal::from(10),
                reason: "test stock".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .unwrap();
        let state = AppState::new(commerce);
        let app = router().with_state(state);
        let body = serde_json::json!({
            "source_warehouse_id": uuid::Uuid::new_v4().to_string(),
            "destination_warehouse_id": uuid::Uuid::new_v4().to_string(),
            "source_location_id": locations[0],
            "destination_location_id": locations[1],
            "items": [{"product_id": product.id, "sku": "TRANSFER-HTTP", "quantity": "10"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/transfer-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let id = json["id"].as_str().unwrap();
        for action in ["ship", "complete-receiving"] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(format!("/transfer-orders/{id}/{action}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }
}
