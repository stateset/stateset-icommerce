//! Purchase order and supplier endpoints (procurement).

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

// === Request/response types ===

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePurchaseOrderItemRequest {
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub supplier_sku: Option<String>,
    /// Decimal quantity as a string.
    pub quantity: String,
    pub unit_of_measure: Option<String>,
    /// Decimal unit cost as a string.
    pub unit_cost: String,
    /// Decimal tax amount as a string.
    pub tax_amount: Option<String>,
    /// Decimal discount amount as a string.
    pub discount_amount: Option<String>,
    /// RFC 3339 timestamp.
    pub expected_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePurchaseOrderRequest {
    pub supplier_id: String,
    /// RFC 3339 timestamp (defaults to now).
    pub order_date: Option<String>,
    /// RFC 3339 timestamp.
    pub expected_date: Option<String>,
    pub ship_to_address: Option<String>,
    pub ship_to_city: Option<String>,
    pub ship_to_state: Option<String>,
    pub ship_to_postal_code: Option<String>,
    pub ship_to_country: Option<String>,
    /// One of `due_on_receipt`, `net_15`, `net_30`, `net_45`, `net_60`, `net_90`,
    /// `2_10_net_30`, `prepaid`, `cash_on_delivery`, `letter_of_credit`.
    pub payment_terms: Option<String>,
    /// ISO 4217 currency code.
    pub currency: Option<String>,
    /// Decimal tax amount as a string.
    pub tax_amount: Option<String>,
    /// Decimal shipping cost as a string.
    pub shipping_cost: Option<String>,
    /// Decimal discount amount as a string.
    pub discount_amount: Option<String>,
    pub notes: Option<String>,
    pub supplier_notes: Option<String>,
    pub items: Vec<CreatePurchaseOrderItemRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdatePurchaseOrderRequest {
    /// RFC 3339 timestamp.
    pub expected_date: Option<String>,
    pub ship_to_address: Option<String>,
    pub ship_to_city: Option<String>,
    pub ship_to_state: Option<String>,
    pub ship_to_postal_code: Option<String>,
    pub ship_to_country: Option<String>,
    pub payment_terms: Option<String>,
    /// Decimal tax amount as a string.
    pub tax_amount: Option<String>,
    /// Decimal shipping cost as a string.
    pub shipping_cost: Option<String>,
    /// Decimal discount amount as a string.
    pub discount_amount: Option<String>,
    pub notes: Option<String>,
    pub supplier_notes: Option<String>,
    pub supplier_reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApprovePurchaseOrderRequest {
    /// Name/ID of the approver.
    pub approved_by: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct AcknowledgePurchaseOrderRequest {
    /// Optional reference number from the supplier.
    #[serde(default)]
    pub supplier_reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceivePurchaseOrderItemRequest {
    /// PO line item ID.
    pub item_id: String,
    /// Decimal quantity received as a string.
    pub quantity_received: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceivePurchaseOrderRequest {
    pub items: Vec<ReceivePurchaseOrderItemRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PurchaseOrderFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurchaseOrderItemResponse {
    pub id: String,
    pub purchase_order_id: String,
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub supplier_sku: Option<String>,
    pub quantity_ordered: String,
    pub quantity_received: String,
    pub unit_cost: String,
    pub line_total: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurchaseOrderResponse {
    pub id: String,
    pub po_number: String,
    pub supplier_id: String,
    pub status: String,
    pub payment_terms: String,
    pub currency: String,
    pub subtotal: String,
    pub tax_amount: String,
    pub shipping_cost: String,
    pub discount_amount: String,
    pub total: String,
    pub supplier_reference: Option<String>,
    pub approved_by: Option<String>,
    pub items: Vec<PurchaseOrderItemResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurchaseOrderListResponse {
    pub purchase_orders: Vec<PurchaseOrderResponse>,
    pub total: u64,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurchaseOrderItemsResponse {
    pub items: Vec<PurchaseOrderItemResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateSupplierRequest {
    pub name: String,
    pub supplier_code: Option<String>,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub tax_id: Option<String>,
    pub payment_terms: Option<String>,
    /// ISO 4217 currency code.
    pub currency: Option<String>,
    pub lead_time_days: Option<i32>,
    /// Decimal minimum order amount as a string.
    pub minimum_order: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateSupplierRequest {
    pub name: Option<String>,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub tax_id: Option<String>,
    pub payment_terms: Option<String>,
    /// ISO 4217 currency code.
    pub currency: Option<String>,
    pub lead_time_days: Option<i32>,
    /// Decimal minimum order amount as a string.
    pub minimum_order: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SupplierFilterParams {
    pub name: Option<String>,
    pub country: Option<String>,
    pub active_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SupplierResponse {
    pub id: String,
    pub supplier_code: String,
    pub name: String,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub payment_terms: String,
    pub currency: String,
    pub lead_time_days: Option<i32>,
    pub minimum_order: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SupplierListResponse {
    pub suppliers: Vec<SupplierResponse>,
    pub total: u64,
}

// === Helpers ===

fn to_item_resp(i: &stateset_core::PurchaseOrderItem) -> PurchaseOrderItemResponse {
    PurchaseOrderItemResponse {
        id: i.id.to_string(),
        purchase_order_id: i.purchase_order_id.to_string(),
        product_id: i.product_id.map(|p| p.to_string()),
        sku: i.sku.clone(),
        name: i.name.clone(),
        supplier_sku: i.supplier_sku.clone(),
        quantity_ordered: i.quantity_ordered.to_string(),
        quantity_received: i.quantity_received.to_string(),
        unit_cost: i.unit_cost.to_string(),
        line_total: i.line_total.to_string(),
        created_at: i.created_at.to_rfc3339(),
    }
}

fn to_resp(po: &stateset_core::PurchaseOrder) -> PurchaseOrderResponse {
    PurchaseOrderResponse {
        id: po.id.to_string(),
        po_number: po.po_number.clone(),
        supplier_id: po.supplier_id.to_string(),
        status: po.status.to_string(),
        payment_terms: po.payment_terms.to_string(),
        currency: po.currency.to_string(),
        subtotal: po.subtotal.to_string(),
        tax_amount: po.tax_amount.to_string(),
        shipping_cost: po.shipping_cost.to_string(),
        discount_amount: po.discount_amount.to_string(),
        total: po.total.to_string(),
        supplier_reference: po.supplier_reference.clone(),
        approved_by: po.approved_by.clone(),
        items: po.items.iter().map(to_item_resp).collect(),
        created_at: po.created_at.to_rfc3339(),
    }
}

fn to_supplier_resp(s: &stateset_core::Supplier) -> SupplierResponse {
    SupplierResponse {
        id: s.id.to_string(),
        supplier_code: s.supplier_code.clone(),
        name: s.name.clone(),
        contact_name: s.contact_name.clone(),
        email: s.email.clone(),
        phone: s.phone.clone(),
        country: s.country.clone(),
        payment_terms: s.payment_terms.to_string(),
        currency: s.currency.to_string(),
        lead_time_days: s.lead_time_days,
        minimum_order: s.minimum_order.map(|m| m.to_string()),
        is_active: s.is_active,
        created_at: s.created_at.to_rfc3339(),
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

fn parse_datetime(s: &str, what: &str) -> Result<DateTime<Utc>, HttpError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_opt_datetime(s: Option<&str>, what: &str) -> Result<Option<DateTime<Utc>>, HttpError> {
    s.map(|v| parse_datetime(v, what)).transpose()
}

fn parse_opt_terms(s: Option<&str>) -> Result<Option<stateset_core::PaymentTerms>, HttpError> {
    s.map(|v| parse_id(v, "payment_terms")).transpose()
}

fn parse_opt_currency(s: Option<&str>) -> Result<Option<stateset_core::CurrencyCode>, HttpError> {
    s.map(|v| parse_id(v, "currency")).transpose()
}

fn to_item_input(
    i: CreatePurchaseOrderItemRequest,
) -> Result<stateset_core::CreatePurchaseOrderItem, HttpError> {
    Ok(stateset_core::CreatePurchaseOrderItem {
        product_id: i
            .product_id
            .as_deref()
            .map(|s| parse_id::<ProductId>(s, "product_id"))
            .transpose()?,
        sku: i.sku,
        name: i.name,
        supplier_sku: i.supplier_sku,
        quantity: parse_decimal(&i.quantity, "quantity")?,
        unit_of_measure: i.unit_of_measure,
        unit_cost: parse_decimal(&i.unit_cost, "unit_cost")?,
        tax_amount: parse_opt_decimal(i.tax_amount.as_deref(), "tax_amount")?,
        discount_amount: parse_opt_decimal(i.discount_amount.as_deref(), "discount_amount")?,
        expected_date: parse_opt_datetime(i.expected_date.as_deref(), "expected_date")?,
        notes: i.notes,
    })
}

// === Router ===

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/purchase-orders", post(create).get(list))
        .route("/purchase-orders/{id}", get(get_one).put(update))
        .route("/purchase-orders/{id}/submit", post(submit))
        .route("/purchase-orders/{id}/approve", post(approve))
        .route("/purchase-orders/{id}/send", post(send))
        .route("/purchase-orders/{id}/acknowledge", post(acknowledge))
        .route("/purchase-orders/{id}/hold", post(hold))
        .route("/purchase-orders/{id}/complete", post(complete))
        .route("/purchase-orders/{id}/cancel", post(cancel))
        .route("/purchase-orders/{id}/receive", post(receive))
        .route("/purchase-orders/{id}/items", get(get_items))
        .route("/suppliers", post(create_supplier).get(list_suppliers))
        .route("/suppliers/{id}", get(get_supplier).put(update_supplier).delete(delete_supplier))
}

// === Purchase order handlers ===

#[utoipa::path(post, operation_id = "purchase_orders_create", path = "/api/v1/purchase-orders", tag = "purchase_orders",
    request_body = CreatePurchaseOrderRequest,
    responses((status = 201, body = PurchaseOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePurchaseOrderRequest>,
) -> Result<(StatusCode, Json<PurchaseOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(to_item_input(i)?);
    }
    let input = stateset_core::CreatePurchaseOrder {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        order_date: parse_opt_datetime(req.order_date.as_deref(), "order_date")?,
        expected_date: parse_opt_datetime(req.expected_date.as_deref(), "expected_date")?,
        ship_to_address: req.ship_to_address,
        ship_to_city: req.ship_to_city,
        ship_to_state: req.ship_to_state,
        ship_to_postal_code: req.ship_to_postal_code,
        ship_to_country: req.ship_to_country,
        payment_terms: parse_opt_terms(req.payment_terms.as_deref())?,
        currency: parse_opt_currency(req.currency.as_deref())?,
        tax_amount: parse_opt_decimal(req.tax_amount.as_deref(), "tax_amount")?,
        shipping_cost: parse_opt_decimal(req.shipping_cost.as_deref(), "shipping_cost")?,
        discount_amount: parse_opt_decimal(req.discount_amount.as_deref(), "discount_amount")?,
        notes: req.notes,
        supplier_notes: req.supplier_notes,
        items,
    };
    let po = c.purchase_orders().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&po))))
}

#[utoipa::path(get, operation_id = "purchase_orders_list", path = "/api/v1/purchase-orders", tag = "purchase_orders",
    params(PurchaseOrderFilterParams),
    responses((status = 200, body = PurchaseOrderListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PurchaseOrderFilterParams>,
) -> Result<Json<PurchaseOrderListResponse>, HttpError> {
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
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };
    let total = c.purchase_orders().count(stateset_core::PurchaseOrderFilter {
        supplier_id,
        status,
        ..Default::default()
    })?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let filter = stateset_core::PurchaseOrderFilter {
        supplier_id,
        status,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
        ..Default::default()
    };
    let mut orders = c.purchase_orders().list(filter)?;
    let has_more = finalize_page(&mut orders, limit);
    let next_cursor = if has_more {
        orders.last().map(|po| encode_cursor(&po.order_date.to_rfc3339(), &po.id.to_string()))
    } else {
        None
    };
    Ok(Json(PurchaseOrderListResponse {
        purchase_orders: orders.iter().map(to_resp).collect(),
        total,
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "purchase_orders_get_one", path = "/api/v1/purchase-orders/{id}", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let po = c
        .purchase_orders()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Purchase order {id} not found")))?;
    Ok(Json(to_resp(&po)))
}

#[utoipa::path(put, operation_id = "purchase_orders_update", path = "/api/v1/purchase-orders/{id}", tag = "purchase_orders",
    request_body = UpdatePurchaseOrderRequest,
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePurchaseOrderRequest>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::UpdatePurchaseOrder {
        expected_date: parse_opt_datetime(req.expected_date.as_deref(), "expected_date")?,
        ship_to_address: req.ship_to_address,
        ship_to_city: req.ship_to_city,
        ship_to_state: req.ship_to_state,
        ship_to_postal_code: req.ship_to_postal_code,
        ship_to_country: req.ship_to_country,
        payment_terms: parse_opt_terms(req.payment_terms.as_deref())?,
        tax_amount: parse_opt_decimal(req.tax_amount.as_deref(), "tax_amount")?,
        shipping_cost: parse_opt_decimal(req.shipping_cost.as_deref(), "shipping_cost")?,
        discount_amount: parse_opt_decimal(req.discount_amount.as_deref(), "discount_amount")?,
        notes: req.notes,
        supplier_notes: req.supplier_notes,
        supplier_reference: req.supplier_reference,
    };
    Ok(Json(to_resp(&c.purchase_orders().update(id, input)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_submit", path = "/api/v1/purchase-orders/{id}/submit", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().submit(id)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_approve", path = "/api/v1/purchase-orders/{id}/approve", tag = "purchase_orders",
    request_body = ApprovePurchaseOrderRequest,
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ApprovePurchaseOrderRequest>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().approve(id, &req.approved_by)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_send", path = "/api/v1/purchase-orders/{id}/send", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn send(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().send(id)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_acknowledge", path = "/api/v1/purchase-orders/{id}/acknowledge", tag = "purchase_orders",
    request_body = AcknowledgePurchaseOrderRequest,
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn acknowledge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<AcknowledgePurchaseOrderRequest>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().acknowledge(id, req.supplier_reference.as_deref())?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_hold", path = "/api/v1/purchase-orders/{id}/hold", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn hold(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().hold(id)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_complete", path = "/api/v1/purchase-orders/{id}/complete", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().complete(id)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_cancel", path = "/api/v1/purchase-orders/{id}/cancel", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purchase_orders().cancel(id)?)))
}

#[utoipa::path(post, operation_id = "purchase_orders_receive", path = "/api/v1/purchase-orders/{id}/receive", tag = "purchase_orders",
    request_body = ReceivePurchaseOrderRequest,
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderResponse), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn receive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReceivePurchaseOrderRequest>,
) -> Result<Json<PurchaseOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::ReceivePurchaseOrderItem {
            item_id: parse_id::<Uuid>(&i.item_id, "item_id")?,
            quantity_received: parse_decimal(&i.quantity_received, "quantity_received")?,
            notes: i.notes,
        });
    }
    let input = stateset_core::ReceivePurchaseOrderItems { items, notes: req.notes };
    Ok(Json(to_resp(&c.purchase_orders().receive(id, input)?)))
}

#[utoipa::path(get, operation_id = "purchase_orders_get_items", path = "/api/v1/purchase-orders/{id}/items", tag = "purchase_orders",
    params(("id" = String, Path, description = "Purchase order ID")),
    responses((status = 200, body = PurchaseOrderItemsResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseOrderItemsResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let items = c.purchase_orders().get_items(id)?;
    let total = items.len();
    Ok(Json(PurchaseOrderItemsResponse { items: items.iter().map(to_item_resp).collect(), total }))
}

// === Supplier handlers ===

#[utoipa::path(post, operation_id = "purchase_orders_create_supplier", path = "/api/v1/suppliers", tag = "purchase_orders",
    request_body = CreateSupplierRequest,
    responses((status = 201, body = SupplierResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_supplier(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSupplierRequest>,
) -> Result<(StatusCode, Json<SupplierResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateSupplier {
        name: req.name,
        supplier_code: req.supplier_code,
        contact_name: req.contact_name,
        email: req.email,
        phone: req.phone,
        website: req.website,
        address: req.address,
        city: req.city,
        state: req.state,
        postal_code: req.postal_code,
        country: req.country,
        tax_id: req.tax_id,
        payment_terms: parse_opt_terms(req.payment_terms.as_deref())?,
        currency: parse_opt_currency(req.currency.as_deref())?,
        lead_time_days: req.lead_time_days,
        minimum_order: parse_opt_decimal(req.minimum_order.as_deref(), "minimum_order")?,
        notes: req.notes,
    };
    let s = c.purchase_orders().create_supplier(input)?;
    Ok((StatusCode::CREATED, Json(to_supplier_resp(&s))))
}

#[utoipa::path(get, operation_id = "purchase_orders_list_suppliers", path = "/api/v1/suppliers", tag = "purchase_orders",
    params(SupplierFilterParams),
    responses((status = 200, body = SupplierListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_suppliers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SupplierFilterParams>,
) -> Result<Json<SupplierListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let total = c.purchase_orders().count_suppliers(stateset_core::SupplierFilter {
        name: params.name.clone(),
        country: params.country.clone(),
        active_only: params.active_only,
        ..Default::default()
    })?;
    let filter = stateset_core::SupplierFilter {
        name: params.name,
        country: params.country,
        active_only: params.active_only,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let suppliers = c.purchase_orders().list_suppliers(filter)?;
    Ok(Json(SupplierListResponse {
        suppliers: suppliers.iter().map(to_supplier_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "purchase_orders_get_supplier", path = "/api/v1/suppliers/{id}", tag = "purchase_orders",
    params(("id" = String, Path, description = "Supplier ID")),
    responses((status = 200, body = SupplierResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_supplier(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SupplierResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .purchase_orders()
        .get_supplier(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Supplier {id} not found")))?;
    Ok(Json(to_supplier_resp(&s)))
}

#[utoipa::path(put, operation_id = "purchase_orders_update_supplier", path = "/api/v1/suppliers/{id}", tag = "purchase_orders",
    request_body = UpdateSupplierRequest,
    params(("id" = String, Path, description = "Supplier ID")),
    responses((status = 200, body = SupplierResponse), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_supplier(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateSupplierRequest>,
) -> Result<Json<SupplierResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::UpdateSupplier {
        name: req.name,
        contact_name: req.contact_name,
        email: req.email,
        phone: req.phone,
        website: req.website,
        address: req.address,
        city: req.city,
        state: req.state,
        postal_code: req.postal_code,
        country: req.country,
        tax_id: req.tax_id,
        payment_terms: parse_opt_terms(req.payment_terms.as_deref())?,
        currency: parse_opt_currency(req.currency.as_deref())?,
        lead_time_days: req.lead_time_days,
        minimum_order: parse_opt_decimal(req.minimum_order.as_deref(), "minimum_order")?,
        is_active: req.is_active,
        notes: req.notes,
    };
    Ok(Json(to_supplier_resp(&c.purchase_orders().update_supplier(id, input)?)))
}

#[utoipa::path(delete, operation_id = "purchase_orders_delete_supplier", path = "/api/v1/suppliers/{id}", tag = "purchase_orders",
    params(("id" = String, Path, description = "Supplier ID")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_supplier(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.purchase_orders().delete_supplier(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    async fn create_supplier_via_api(app: &Router) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::post("/suppliers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "Acme Supplies",
                            "email": "orders@acme.test",
                            "payment_terms": "net_30"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["payment_terms"], "net_30");
        json["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_submit_approve_send_flow() {
        let app = app();
        let supplier_id = create_supplier_via_api(&app).await;

        let body = serde_json::json!({
            "supplier_id": supplier_id,
            "items": [{
                "sku": "RAW-001",
                "name": "Raw Material A",
                "quantity": "10",
                "unit_cost": "2.50"
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/purchase-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "draft");
        let subtotal: Decimal = json["subtotal"].as_str().unwrap().parse().unwrap();
        assert_eq!(subtotal, Decimal::new(2500, 2));
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/purchase-orders/{id}/submit")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["status"], "pending_approval");

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/purchase-orders/{id}/approve"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"approved_by": "admin"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "approved");
        assert_eq!(json["approved_by"], "admin");

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/purchase-orders/{id}/send")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["status"], "sent");

        let resp = app
            .oneshot(
                Request::get(format!("/purchase-orders/{id}/items")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["items"][0]["sku"], "RAW-001");
    }

    #[tokio::test]
    async fn supplier_crud_and_po_list() {
        let app = app();
        let supplier_id = create_supplier_via_api(&app).await;

        // Get supplier.
        let resp = app
            .clone()
            .oneshot(Request::get(format!("/suppliers/{supplier_id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["name"], "Acme Supplies");

        // Update supplier.
        let resp = app
            .clone()
            .oneshot(
                Request::put(format!("/suppliers/{supplier_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"name": "Acme Global"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["name"], "Acme Global");

        // List suppliers.
        let resp = app
            .clone()
            .oneshot(Request::get("/suppliers").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);

        // Empty PO list.
        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/purchase-orders?supplier_id={supplier_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 0);

        // Invalid status filter yields 400.
        let resp = app
            .clone()
            .oneshot(Request::get("/purchase-orders?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Delete (deactivate) supplier.
        let resp = app
            .oneshot(
                Request::delete(format!("/suppliers/{supplier_id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn list_purchase_orders_has_more_and_next_cursor() {
        let app = app();
        let supplier_id = create_supplier_via_api(&app).await;
        for i in 0..3 {
            let body = serde_json::json!({
                "supplier_id": supplier_id,
                "items": [{
                    "sku": format!("CUR-{i}"),
                    "name": "Cursor Item",
                    "quantity": "1",
                    "unit_cost": "1.00"
                }]
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/purchase-orders")
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
            .oneshot(Request::get("/purchase-orders?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 3);
        assert_eq!(json["purchase_orders"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/purchase-orders?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["purchase_orders"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_purchase_orders_invalid_cursor_returns_400() {
        let app = app();
        let resp = app
            .oneshot(
                Request::get("/purchase-orders?after=!!!invalid!!!").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
