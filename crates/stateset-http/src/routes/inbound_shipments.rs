//! Inbound shipment endpoints (advance ship notices).

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
use stateset_core::{InboundShipmentId, InboundShipmentItemId, ProductId, WarehouseId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateInboundShipmentItemRequest {
    pub product_id: String,
    pub sku: String,
    /// Expected quantity as a string.
    pub quantity_expected: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateInboundShipmentRequest {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub warehouse_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<CreateInboundShipmentItemRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceiveLineRequest {
    pub item_id: String,
    /// Quantity to receive as a string.
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct InboundShipmentFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InboundShipmentItemResponse {
    pub id: String,
    pub product_id: String,
    pub sku: String,
    pub quantity_expected: String,
    pub quantity_received: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InboundShipmentResponse {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<InboundShipmentItemResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InboundShipmentListResponse {
    pub inbound_shipments: Vec<InboundShipmentResponse>,
    pub total: usize,
}

fn to_resp(s: &stateset_core::InboundShipment) -> InboundShipmentResponse {
    InboundShipmentResponse {
        id: s.id.to_string(),
        number: s.number.clone(),
        supplier_id: s.supplier_id.to_string(),
        status: s.status.to_string(),
        carrier: s.carrier.clone(),
        tracking_number: s.tracking_number.clone(),
        items: s
            .items
            .iter()
            .map(|i| InboundShipmentItemResponse {
                id: i.id.to_string(),
                product_id: i.product_id.to_string(),
                sku: i.sku.clone(),
                quantity_expected: i.quantity_expected.to_string(),
                quantity_received: i.quantity_received.to_string(),
            })
            .collect(),
        created_at: s.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/inbound-shipments", post(create).get(list))
        .route("/inbound-shipments/{id}", get(get_one))
        .route("/inbound-shipments/{id}/in-transit", post(mark_in_transit))
        .route("/inbound-shipments/{id}/arrived", post(mark_arrived))
        .route("/inbound-shipments/{id}/receive", post(receive))
        .route("/inbound-shipments/{id}/cancel", post(cancel))
}

#[utoipa::path(post, path = "/api/v1/inbound-shipments", tag = "inbound_shipments",
    request_body = CreateInboundShipmentRequest,
    responses((status = 201, body = InboundShipmentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateInboundShipmentRequest>,
) -> Result<(StatusCode, Json<InboundShipmentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let purchase_order_id = match req.purchase_order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "purchase_order_id")?),
        None => None,
    };
    let warehouse_id = match req.warehouse_id.as_deref() {
        Some(s) => Some(parse_id::<WarehouseId>(s, "warehouse_id")?),
        None => None,
    };
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::CreateInboundShipmentItem {
            product_id: parse_id::<ProductId>(&i.product_id, "product_id")?,
            sku: i.sku,
            quantity_expected: parse_decimal(&i.quantity_expected, "quantity_expected")?,
        });
    }
    let input = stateset_core::CreateInboundShipment {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        purchase_order_id,
        warehouse_id,
        carrier: req.carrier,
        tracking_number: req.tracking_number,
        expected_at: None,
        items,
        notes: req.notes,
    };
    let s = c.inbound_shipments().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&s))))
}

#[utoipa::path(get, path = "/api/v1/inbound-shipments", tag = "inbound_shipments",
    params(InboundShipmentFilterParams),
    responses((status = 200, body = InboundShipmentListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InboundShipmentFilterParams>,
) -> Result<Json<InboundShipmentListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let supplier_id = match params.supplier_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "supplier_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let total = c
        .inbound_shipments()
        .list(stateset_core::InboundShipmentFilter { supplier_id, status, ..Default::default() })?
        .len();
    let filter = stateset_core::InboundShipmentFilter {
        supplier_id,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let shipments = c.inbound_shipments().list(filter)?;
    Ok(Json(InboundShipmentListResponse {
        inbound_shipments: shipments.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/inbound-shipments/{id}", tag = "inbound_shipments",
    params(("id" = String, Path, description = "Inbound shipment ID")),
    responses((status = 200, body = InboundShipmentResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<InboundShipmentId>,
) -> Result<Json<InboundShipmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .inbound_shipments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Inbound shipment {id} not found")))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(post, path = "/api/v1/inbound-shipments/{id}/in-transit", tag = "inbound_shipments",
    params(("id" = String, Path, description = "Inbound shipment ID")),
    responses((status = 200, body = InboundShipmentResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn mark_in_transit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<InboundShipmentId>,
) -> Result<Json<InboundShipmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.inbound_shipments().mark_in_transit(id)?)))
}

#[utoipa::path(post, path = "/api/v1/inbound-shipments/{id}/arrived", tag = "inbound_shipments",
    params(("id" = String, Path, description = "Inbound shipment ID")),
    responses((status = 200, body = InboundShipmentResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn mark_arrived(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<InboundShipmentId>,
) -> Result<Json<InboundShipmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.inbound_shipments().mark_arrived(id)?)))
}

#[utoipa::path(post, path = "/api/v1/inbound-shipments/{id}/receive", tag = "inbound_shipments",
    request_body = ReceiveLineRequest,
    params(("id" = String, Path, description = "Inbound shipment ID")),
    responses((status = 200, body = InboundShipmentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn receive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<InboundShipmentId>,
    Json(req): Json<ReceiveLineRequest>,
) -> Result<Json<InboundShipmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let item_id: InboundShipmentItemId = parse_id(&req.item_id, "item_id")?;
    let qty = parse_decimal(&req.quantity, "quantity")?;
    Ok(Json(to_resp(&c.inbound_shipments().receive_line(id, item_id, qty)?)))
}

#[utoipa::path(post, path = "/api/v1/inbound-shipments/{id}/cancel", tag = "inbound_shipments",
    params(("id" = String, Path, description = "Inbound shipment ID")),
    responses((status = 200, body = InboundShipmentResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<InboundShipmentId>,
) -> Result<Json<InboundShipmentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.inbound_shipments().cancel(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_and_receive_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "carrier": "DHL",
            "items": [{
                "product_id": uuid::Uuid::new_v4().to_string(),
                "sku": "SKU-1",
                "quantity_expected": "10"
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/inbound-shipments")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = json["id"].as_str().unwrap().to_string();
        let item_id = json["items"][0]["id"].as_str().unwrap().to_string();

        let recv = serde_json::json!({"item_id": item_id, "quantity": "10"});
        let resp = app
            .oneshot(
                Request::post(format!("/inbound-shipments/{id}/receive"))
                    .header("content-type", "application/json")
                    .body(Body::from(recv.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "received");
    }
}
