//! Receiving endpoints (goods receipts, receive items, put-away tasks).

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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response bodies
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateReceiptItemRequest {
    pub sku: String,
    pub description: Option<String>,
    pub po_line_id: Option<String>,
    /// Decimal expected quantity as a string.
    pub expected_quantity: String,
    /// Decimal unit cost as a string.
    pub unit_cost: Option<String>,
    pub lot_number: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateReceiptRequest {
    /// One of `purchase_order`, `transfer`, `return`, `adjustment`, `production`, `other`.
    pub receipt_type: Option<String>,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub supplier_id: Option<String>,
    pub warehouse_id: i32,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// RFC 3339 timestamp.
    pub expected_date: Option<String>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub items: Vec<CreateReceiptItemRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceiveItemLineRequest {
    pub receipt_item_id: String,
    /// Decimal quantity received as a string.
    pub quantity_received: String,
    /// Decimal quantity rejected as a string.
    pub quantity_rejected: Option<String>,
    pub rejection_reason: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceiveItemsRequest {
    pub items: Vec<ReceiveItemLineRequest>,
    pub receiving_location_id: Option<i32>,
    pub received_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePutAwayRequest {
    pub receipt_id: String,
    pub receipt_item_id: String,
    pub sku: String,
    pub from_location_id: Option<i32>,
    pub to_location_id: i32,
    /// Decimal quantity as a string.
    pub quantity: String,
    pub lot_id: Option<String>,
    pub assigned_to: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompletePutAwayRequest {
    pub actual_location_id: Option<i32>,
    pub completed_by: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ReceiptFilterParams {
    pub warehouse_id: Option<i32>,
    pub receipt_type: Option<String>,
    pub status: Option<String>,
    pub supplier_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PutAwayFilterParams {
    pub receipt_id: Option<String>,
    pub warehouse_id: Option<i32>,
    pub status: Option<String>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ReceiptResponse {
    pub id: String,
    pub receipt_number: String,
    pub receipt_type: String,
    pub status: String,
    pub warehouse_id: i32,
    pub supplier_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub expected_quantity: String,
    pub received_quantity: String,
    pub put_away_quantity: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ReceiptListResponse {
    pub receipts: Vec<ReceiptResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ReceiptItemResponse {
    pub id: String,
    pub receipt_id: String,
    pub line_number: i32,
    pub sku: String,
    pub description: Option<String>,
    pub expected_quantity: String,
    pub received_quantity: String,
    pub rejected_quantity: String,
    pub unit_cost: Option<String>,
    pub lot_number: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ReceiptItemListResponse {
    pub items: Vec<ReceiptItemResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PutAwayResponse {
    pub id: String,
    pub receipt_id: String,
    pub receipt_item_id: String,
    pub sku: String,
    pub from_location_id: Option<i32>,
    pub to_location_id: i32,
    pub quantity: String,
    pub status: String,
    pub assigned_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PutAwayListResponse {
    pub put_aways: Vec<PutAwayResponse>,
    pub total: u64,
}

// ============================================================================
// Conversions / helpers
// ============================================================================

fn receipt_resp(r: &stateset_core::Receipt) -> ReceiptResponse {
    ReceiptResponse {
        id: r.id.to_string(),
        receipt_number: r.receipt_number.clone(),
        receipt_type: r.receipt_type.to_string(),
        status: r.status.to_string(),
        warehouse_id: r.warehouse_id,
        supplier_id: r.supplier_id.map(|s| s.to_string()),
        carrier: r.carrier.clone(),
        tracking_number: r.tracking_number.clone(),
        expected_quantity: r.expected_quantity.to_string(),
        received_quantity: r.received_quantity.to_string(),
        put_away_quantity: r.put_away_quantity.to_string(),
        created_at: r.created_at.to_rfc3339(),
    }
}

fn receipt_item_resp(i: &stateset_core::ReceiptItem) -> ReceiptItemResponse {
    ReceiptItemResponse {
        id: i.id.to_string(),
        receipt_id: i.receipt_id.to_string(),
        line_number: i.line_number,
        sku: i.sku.clone(),
        description: i.description.clone(),
        expected_quantity: i.expected_quantity.to_string(),
        received_quantity: i.received_quantity.to_string(),
        rejected_quantity: i.rejected_quantity.to_string(),
        unit_cost: i.unit_cost.map(|c| c.to_string()),
        lot_number: i.lot_number.clone(),
        status: i.status.to_string(),
    }
}

fn put_away_resp(p: &stateset_core::PutAway) -> PutAwayResponse {
    PutAwayResponse {
        id: p.id.to_string(),
        receipt_id: p.receipt_id.to_string(),
        receipt_item_id: p.receipt_item_id.to_string(),
        sku: p.sku.clone(),
        from_location_id: p.from_location_id,
        to_location_id: p.to_location_id,
        quantity: p.quantity.to_string(),
        status: p.status.to_string(),
        assigned_to: p.assigned_to.clone(),
        created_at: p.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_opt_decimal(s: Option<&str>, what: &str) -> Result<Option<Decimal>, HttpError> {
    s.map(|v| parse_decimal(v, what)).transpose()
}

fn parse_opt<T: std::str::FromStr>(s: Option<&str>, what: &str) -> Result<Option<T>, HttpError> {
    s.map(|v| parse_id::<T>(v, what)).transpose()
}

fn parse_opt_datetime(s: Option<&str>, what: &str) -> Result<Option<DateTime<Utc>>, HttpError> {
    s.map(|v| {
        DateTime::parse_from_rfc3339(v)
            .map(|d| d.with_timezone(&Utc))
            .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {v}")))
    })
    .transpose()
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/receipts", post(create_receipt).get(list_receipts))
        .route("/receipts/{id}", get(get_receipt))
        .route("/receipts/{id}/items", get(list_receipt_items))
        .route("/receipts/{id}/start", post(start_receiving))
        .route("/receipts/{id}/receive", post(receive_items))
        .route("/receipts/{id}/complete", post(complete_receiving))
        .route("/receipts/{id}/cancel", post(cancel_receipt))
        .route("/put-aways", post(create_put_away).get(list_put_aways))
        .route("/put-aways/{id}", get(get_put_away))
        .route("/put-aways/{id}/complete", post(complete_put_away))
}

// ============================================================================
// Receipt handlers
// ============================================================================

#[utoipa::path(post, operation_id = "receiving_receipt_create", path = "/api/v1/receipts", tag = "receiving",
    request_body = CreateReceiptRequest,
    responses((status = 201, body = ReceiptResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateReceiptRequest>,
) -> Result<(StatusCode, Json<ReceiptResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let receipt_type = match req.receipt_type.as_deref() {
        Some(s) => parse_id(s, "receipt_type")?,
        None => stateset_core::ReceiptType::default(),
    };
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::CreateReceiptItem {
            sku: i.sku,
            description: i.description,
            po_line_id: parse_opt(i.po_line_id.as_deref(), "po_line_id")?,
            expected_quantity: parse_decimal(&i.expected_quantity, "expected_quantity")?,
            unit_cost: parse_opt_decimal(i.unit_cost.as_deref(), "unit_cost")?,
            lot_number: i.lot_number,
            expiration_date: None,
            notes: i.notes,
        });
    }
    let r = c.receiving().create_receipt(stateset_core::CreateReceipt {
        receipt_number: None,
        receipt_type,
        reference_type: req.reference_type,
        reference_id: parse_opt(req.reference_id.as_deref(), "reference_id")?,
        supplier_id: parse_opt(req.supplier_id.as_deref(), "supplier_id")?,
        warehouse_id: req.warehouse_id,
        carrier: req.carrier,
        tracking_number: req.tracking_number,
        expected_date: parse_opt_datetime(req.expected_date.as_deref(), "expected_date")?,
        notes: req.notes,
        created_by: req.created_by,
        items,
    })?;
    Ok((StatusCode::CREATED, Json(receipt_resp(&r))))
}

#[utoipa::path(get, operation_id = "receiving_receipt_list", path = "/api/v1/receipts", tag = "receiving",
    params(ReceiptFilterParams),
    responses((status = 200, body = ReceiptListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_receipts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ReceiptFilterParams>,
) -> Result<Json<ReceiptListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::ReceiptFilter {
        warehouse_id: params.warehouse_id,
        receipt_type: parse_opt(params.receipt_type.as_deref(), "receipt_type")?,
        status: parse_opt(params.status.as_deref(), "status")?,
        supplier_id: parse_opt(params.supplier_id.as_deref(), "supplier_id")?,
        ..Default::default()
    };
    let total = c.receiving().count_receipts(base.clone())?;
    let receipts = c.receiving().list_receipts(stateset_core::ReceiptFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(ReceiptListResponse { receipts: receipts.iter().map(receipt_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "receiving_receipt_get_one", path = "/api/v1/receipts/{id}", tag = "receiving",
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let r = c
        .receiving()
        .get_receipt(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Receipt {id} not found")))?;
    Ok(Json(receipt_resp(&r)))
}

#[utoipa::path(get, operation_id = "receiving_receipt_items", path = "/api/v1/receipts/{id}/items", tag = "receiving",
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptItemListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_receipt_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptItemListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let items = c.receiving().get_receipt_items(id)?;
    Ok(Json(ReceiptItemListResponse {
        total: items.len(),
        items: items.iter().map(receipt_item_resp).collect(),
    }))
}

#[utoipa::path(post, operation_id = "receiving_receipt_start", path = "/api/v1/receipts/{id}/start", tag = "receiving",
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start_receiving(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(receipt_resp(&c.receiving().start_receiving(id)?)))
}

#[utoipa::path(post, operation_id = "receiving_receipt_receive_items", path = "/api/v1/receipts/{id}/receive", tag = "receiving",
    request_body = ReceiveItemsRequest,
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn receive_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReceiveItemsRequest>,
) -> Result<Json<ReceiptResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::ReceiveItemLine {
            receipt_item_id: parse_id(&i.receipt_item_id, "receipt_item_id")?,
            quantity_received: parse_decimal(&i.quantity_received, "quantity_received")?,
            quantity_rejected: parse_opt_decimal(
                i.quantity_rejected.as_deref(),
                "quantity_rejected",
            )?,
            rejection_reason: i.rejection_reason,
            lot_number: i.lot_number,
            serial_numbers: i.serial_numbers,
            expiration_date: None,
            notes: i.notes,
        });
    }
    let r = c.receiving().receive_items(stateset_core::ReceiveItems {
        receipt_id: id,
        items,
        receiving_location_id: req.receiving_location_id,
        received_by: req.received_by,
    })?;
    Ok(Json(receipt_resp(&r)))
}

#[utoipa::path(post, operation_id = "receiving_receipt_complete", path = "/api/v1/receipts/{id}/complete", tag = "receiving",
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_receiving(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(receipt_resp(&c.receiving().complete_receiving(id)?)))
}

#[utoipa::path(post, operation_id = "receiving_receipt_cancel", path = "/api/v1/receipts/{id}/cancel", tag = "receiving",
    params(("id" = String, Path, description = "Receipt ID")),
    responses((status = 200, body = ReceiptResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(receipt_resp(&c.receiving().cancel_receipt(id)?)))
}

// ============================================================================
// Put-away handlers
// ============================================================================

#[utoipa::path(post, operation_id = "receiving_put_away_create", path = "/api/v1/put-aways", tag = "receiving",
    request_body = CreatePutAwayRequest,
    responses((status = 201, body = PutAwayResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePutAwayRequest>,
) -> Result<(StatusCode, Json<PutAwayResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c.receiving().create_put_away(stateset_core::CreatePutAway {
        receipt_id: parse_id(&req.receipt_id, "receipt_id")?,
        receipt_item_id: parse_id(&req.receipt_item_id, "receipt_item_id")?,
        sku: req.sku,
        from_location_id: req.from_location_id,
        to_location_id: req.to_location_id,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        lot_id: parse_opt(req.lot_id.as_deref(), "lot_id")?,
        assigned_to: req.assigned_to,
        notes: req.notes,
    })?;
    Ok((StatusCode::CREATED, Json(put_away_resp(&p))))
}

#[utoipa::path(get, operation_id = "receiving_put_away_list", path = "/api/v1/put-aways", tag = "receiving",
    params(PutAwayFilterParams),
    responses((status = 200, body = PutAwayListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_put_aways(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PutAwayFilterParams>,
) -> Result<Json<PutAwayListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::PutAwayFilter {
        receipt_id: parse_opt(params.receipt_id.as_deref(), "receipt_id")?,
        warehouse_id: params.warehouse_id,
        status: parse_opt(params.status.as_deref(), "status")?,
        assigned_to: params.assigned_to.clone(),
        ..Default::default()
    };
    let total = c.receiving().count_put_aways(base.clone())?;
    let put_aways = c.receiving().list_put_aways(stateset_core::PutAwayFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(PutAwayListResponse {
        put_aways: put_aways.iter().map(put_away_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "receiving_put_away_get_one", path = "/api/v1/put-aways/{id}", tag = "receiving",
    params(("id" = String, Path, description = "Put-away task ID")),
    responses((status = 200, body = PutAwayResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PutAwayResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c
        .receiving()
        .get_put_away(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Put-away {id} not found")))?;
    Ok(Json(put_away_resp(&p)))
}

#[utoipa::path(post, operation_id = "receiving_put_away_complete", path = "/api/v1/put-aways/{id}/complete", tag = "receiving",
    request_body = CompletePutAwayRequest,
    params(("id" = String, Path, description = "Put-away task ID")),
    responses((status = 200, body = PutAwayResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CompletePutAwayRequest>,
) -> Result<Json<PutAwayResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let p = c.receiving().complete_put_away(stateset_core::CompletePutAway {
        put_away_id: id,
        actual_location_id: req.actual_location_id,
        completed_by: req.completed_by,
        notes: req.notes,
    })?;
    Ok(Json(put_away_resp(&p)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn json_of(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn create_start_and_receive_flow() {
        let commerce = Commerce::new(":memory:").expect("in-memory Commerce");
        // Receipts have a FOREIGN KEY to warehouses; seed one first.
        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: "WH-R".into(),
                name: "Receiving WH".into(),
                ..Default::default()
            })
            .expect("seed warehouse");
        let state = AppState::new(commerce);
        let app = router().with_state(state);

        // Create receipt
        let body = serde_json::json!({
            "receipt_type": "purchase_order",
            "warehouse_id": warehouse.id,
            "carrier": "UPS",
            "items": [{"sku": "WIDGET-001", "expected_quantity": "50", "unit_cost": "10.00"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/receipts")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let receipt = json_of(resp).await;
        assert_eq!(receipt["status"], "expected");
        assert_eq!(receipt["expected_quantity"], "50");
        let id = receipt["id"].as_str().unwrap().to_string();

        // Start receiving
        let resp = app
            .clone()
            .oneshot(Request::post(format!("/receipts/{id}/start")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "in_progress");

        // Look up item id
        let resp = app
            .clone()
            .oneshot(Request::get(format!("/receipts/{id}/items")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items = json_of(resp).await;
        assert_eq!(items["total"], 1);
        let item_id = items["items"][0]["id"].as_str().unwrap().to_string();

        // Receive full quantity
        let body = serde_json::json!({
            "items": [{"receipt_item_id": item_id, "quantity_received": "50"}],
            "received_by": "tester"
        });
        let resp = app
            .oneshot(
                Request::post(format!("/receipts/{id}/receive"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let receipt = json_of(resp).await;
        assert_eq!(receipt["received_quantity"], "50");
    }

    #[tokio::test]
    async fn unknown_receipt_is_not_found() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let id = uuid::Uuid::new_v4();
        let resp = app
            .oneshot(Request::get(format!("/receipts/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
