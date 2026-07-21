//! Cart and checkout endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{CartId, ProductId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response schemas
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CartAddressRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AddCartItemRequest {
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    /// Decimal unit price as a string.
    pub unit_price: String,
    /// Decimal original price as a string.
    pub original_price: Option<String>,
    pub requires_shipping: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateCartRequest {
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    /// ISO 4217 currency code, e.g. `USD`.
    pub currency: Option<String>,
    #[serde(default)]
    pub items: Vec<AddCartItemRequest>,
    pub notes: Option<String>,
    pub expires_in_minutes: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateCartItemRequest {
    pub quantity: Option<i32>,
    /// Decimal unit price as a string.
    pub unit_price: Option<String>,
    /// Decimal discount amount as a string.
    pub discount_amount: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetCartShippingRequest {
    pub shipping_address: CartAddressRequest,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    /// Decimal shipping amount as a string.
    pub shipping_amount: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetCartPaymentRequest {
    pub payment_method: String,
    pub payment_token: Option<String>,
    pub billing_address: Option<CartAddressRequest>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CartFilterParams {
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    /// One of `active`, `ready_for_payment`, `payment_pending`, `completed`,
    /// `abandoned`, `expired`, `cancelled`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CartItemResponse {
    pub id: String,
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: String,
    pub discount_amount: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CartResponse {
    pub id: String,
    pub cart_number: String,
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub status: String,
    pub currency: String,
    pub items: Vec<CartItemResponse>,
    pub subtotal: String,
    pub tax_amount: String,
    pub shipping_amount: String,
    pub discount_amount: String,
    pub grand_total: String,
    pub shipping_method: Option<String>,
    pub payment_method: Option<String>,
    pub order_id: Option<String>,
    pub order_number: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CartListResponse {
    pub carts: Vec<CartResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CheckoutResultResponse {
    pub cart_id: String,
    pub order_id: String,
    pub order_number: String,
    pub payment_id: Option<String>,
    pub total_charged: String,
    pub currency: String,
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

fn parse_opt_decimal(s: Option<&str>, what: &str) -> Result<Option<Decimal>, HttpError> {
    s.map(|v| parse_decimal(v, what)).transpose()
}

fn to_address(a: CartAddressRequest) -> stateset_core::CartAddress {
    stateset_core::CartAddress {
        first_name: a.first_name.unwrap_or_default(),
        last_name: a.last_name.unwrap_or_default(),
        company: a.company,
        line1: a.line1,
        line2: a.line2,
        city: a.city,
        state: a.state,
        postal_code: a.postal_code,
        country: a.country,
        phone: a.phone,
        email: a.email,
    }
}

fn to_add_item(i: AddCartItemRequest) -> Result<stateset_core::AddCartItem, HttpError> {
    Ok(stateset_core::AddCartItem {
        product_id: i
            .product_id
            .as_deref()
            .map(|s| parse_id::<ProductId>(s, "product_id"))
            .transpose()?,
        variant_id: None,
        sku: i.sku,
        name: i.name,
        description: i.description,
        image_url: i.image_url,
        quantity: i.quantity,
        unit_price: parse_decimal(&i.unit_price, "unit_price")?,
        original_price: parse_opt_decimal(i.original_price.as_deref(), "original_price")?,
        weight: None,
        requires_shipping: i.requires_shipping,
        metadata: None,
    })
}

fn item_resp(i: &stateset_core::CartItem) -> CartItemResponse {
    CartItemResponse {
        id: i.id.to_string(),
        product_id: i.product_id.map(|p| p.to_string()),
        sku: i.sku.clone(),
        name: i.name.clone(),
        quantity: i.quantity,
        unit_price: i.unit_price.to_string(),
        discount_amount: i.discount_amount.to_string(),
        total: i.total.to_string(),
    }
}

fn to_resp(c: &stateset_core::Cart) -> CartResponse {
    CartResponse {
        id: c.id.to_string(),
        cart_number: c.cart_number.clone(),
        customer_id: c.customer_id.map(|id| id.to_string()),
        customer_email: c.customer_email.clone(),
        status: c.status.to_string(),
        currency: c.currency.to_string(),
        items: c.items.iter().map(item_resp).collect(),
        subtotal: c.subtotal.to_string(),
        tax_amount: c.tax_amount.to_string(),
        shipping_amount: c.shipping_amount.to_string(),
        discount_amount: c.discount_amount.to_string(),
        grand_total: c.grand_total.to_string(),
        shipping_method: c.shipping_method.clone(),
        payment_method: c.payment_method.clone(),
        order_id: c.order_id.map(|id| id.to_string()),
        order_number: c.order_number.clone(),
        created_at: c.created_at.to_rfc3339(),
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/carts", post(create).get(list))
        .route("/carts/{id}", get(get_one))
        .route("/carts/{id}/items", post(add_item))
        .route("/carts/{id}/items/{item_id}", put(update_item).delete(remove_item))
        .route("/carts/{id}/shipping", post(set_shipping))
        .route("/carts/{id}/payment", post(set_payment))
        .route("/carts/{id}/complete", post(complete))
        .route("/carts/{id}/cancel", post(cancel))
}

#[utoipa::path(post, operation_id = "carts_create", path = "/api/v1/carts", tag = "carts",
    request_body = CreateCartRequest,
    responses((status = 201, body = CartResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCartRequest>,
) -> Result<(StatusCode, Json<CartResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let customer_id = req
        .customer_id
        .as_deref()
        .map(|s| parse_id::<Uuid>(s, "customer_id"))
        .transpose()?
        .map(Into::into);
    let currency = req.currency.as_deref().map(|s| parse_id(s, "currency")).transpose()?;
    let items = if req.items.is_empty() {
        None
    } else {
        Some(req.items.into_iter().map(to_add_item).collect::<Result<Vec<_>, _>>()?)
    };
    let input = stateset_core::CreateCart {
        customer_id,
        customer_email: req.customer_email,
        customer_name: req.customer_name,
        currency,
        items,
        shipping_address: None,
        billing_address: None,
        notes: req.notes,
        metadata: None,
        expires_in_minutes: req.expires_in_minutes,
    };
    let cart = c.carts().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&cart))))
}

#[utoipa::path(get, operation_id = "carts_list", path = "/api/v1/carts", tag = "carts",
    params(CartFilterParams),
    responses((status = 200, body = CartListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CartFilterParams>,
) -> Result<Json<CartListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let customer_id = params
        .customer_id
        .as_deref()
        .map(|s| parse_id::<Uuid>(s, "customer_id"))
        .transpose()?
        .map(Into::into);
    let status = params.status.as_deref().map(|s| parse_id(s, "status")).transpose()?;
    let base = stateset_core::CartFilter {
        customer_id,
        customer_email: params.customer_email.clone(),
        status,
        ..Default::default()
    };
    let total = usize::try_from(c.carts().count(base.clone())?).unwrap_or(usize::MAX);
    let filter = stateset_core::CartFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let carts = c.carts().list(filter)?;
    Ok(Json(CartListResponse { carts: carts.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "carts_get_one", path = "/api/v1/carts/{id}", tag = "carts",
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 200, body = CartResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
) -> Result<Json<CartResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let cart =
        c.carts().get(id)?.ok_or_else(|| HttpError::NotFound(format!("Cart {id} not found")))?;
    Ok(Json(to_resp(&cart)))
}

#[utoipa::path(post, operation_id = "carts_add_item", path = "/api/v1/carts/{id}/items", tag = "carts",
    request_body = AddCartItemRequest,
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 201, body = CartItemResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
    Json(req): Json<AddCartItemRequest>,
) -> Result<(StatusCode, Json<CartItemResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let item = c.carts().add_item(id, to_add_item(req)?)?;
    Ok((StatusCode::CREATED, Json(item_resp(&item))))
}

#[utoipa::path(put, operation_id = "carts_update_item", path = "/api/v1/carts/{id}/items/{item_id}", tag = "carts",
    request_body = UpdateCartItemRequest,
    params(("id" = String, Path, description = "Cart ID"),
        ("item_id" = String, Path, description = "Cart item ID")),
    responses((status = 200, body = CartItemResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((_id, item_id)): Path<(CartId, Uuid)>,
    Json(req): Json<UpdateCartItemRequest>,
) -> Result<Json<CartItemResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::UpdateCartItem {
        quantity: req.quantity,
        unit_price: parse_opt_decimal(req.unit_price.as_deref(), "unit_price")?,
        discount_amount: parse_opt_decimal(req.discount_amount.as_deref(), "discount_amount")?,
        metadata: None,
    };
    Ok(Json(item_resp(&c.carts().update_item(item_id, input)?)))
}

#[utoipa::path(delete, operation_id = "carts_remove_item", path = "/api/v1/carts/{id}/items/{item_id}", tag = "carts",
    params(("id" = String, Path, description = "Cart ID"),
        ("item_id" = String, Path, description = "Cart item ID")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn remove_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((_id, item_id)): Path<(CartId, Uuid)>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.carts().remove_item(item_id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "carts_set_shipping", path = "/api/v1/carts/{id}/shipping", tag = "carts",
    request_body = SetCartShippingRequest,
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 200, body = CartResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_shipping(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
    Json(req): Json<SetCartShippingRequest>,
) -> Result<Json<CartResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::SetCartShipping {
        shipping_address: to_address(req.shipping_address),
        shipping_method: req.shipping_method,
        shipping_carrier: req.shipping_carrier,
        shipping_amount: parse_opt_decimal(req.shipping_amount.as_deref(), "shipping_amount")?,
    };
    Ok(Json(to_resp(&c.carts().set_shipping(id, input)?)))
}

#[utoipa::path(post, operation_id = "carts_set_payment", path = "/api/v1/carts/{id}/payment", tag = "carts",
    request_body = SetCartPaymentRequest,
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 200, body = CartResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
    Json(req): Json<SetCartPaymentRequest>,
) -> Result<Json<CartResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::SetCartPayment {
        payment_method: req.payment_method,
        payment_token: req.payment_token,
        billing_address: req.billing_address.map(to_address),
    };
    Ok(Json(to_resp(&c.carts().set_payment(id, input)?)))
}

#[utoipa::path(post, operation_id = "carts_complete", path = "/api/v1/carts/{id}/complete", tag = "carts",
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 200, body = CheckoutResultResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
) -> Result<Json<CheckoutResultResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let result = c.carts().complete(id)?;
    Ok(Json(CheckoutResultResponse {
        cart_id: result.cart_id.to_string(),
        order_id: result.order_id.to_string(),
        order_number: result.order_number,
        payment_id: result.payment_id.map(|p| p.to_string()),
        total_charged: result.total_charged.to_string(),
        currency: result.currency.to_string(),
    }))
}

#[utoipa::path(post, operation_id = "carts_cancel", path = "/api/v1/carts/{id}/cancel", tag = "carts",
    params(("id" = String, Path, description = "Cart ID")),
    responses((status = 200, body = CartResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CartId>,
) -> Result<Json<CartResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.carts().cancel(id)?)))
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
    async fn create_add_item_ship_pay_complete_flow() {
        let app = app();
        let body = serde_json::json!({
            "customer_email": "buyer@example.com",
            "customer_name": "Buyer Example",
            "items": [{ "sku": "SKU-1", "name": "Widget", "quantity": 2, "unit_price": "10.00" }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/carts")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let cart = json_body(resp).await;
        assert_eq!(cart["status"], "active");
        assert_eq!(cart["subtotal"], "20.00");
        let id = cart["id"].as_str().unwrap().to_string();

        // Add another item.
        let item = serde_json::json!({
            "sku": "SKU-2", "name": "Gadget", "quantity": 1, "unit_price": "5.50"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/carts/{id}/items"))
                    .header("content-type", "application/json")
                    .body(Body::from(item.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Set shipping address.
        let shipping = serde_json::json!({
            "shipping_address": {
                "first_name": "Buyer", "last_name": "Example",
                "line1": "123 Main St", "city": "Anytown",
                "postal_code": "12345", "country": "US"
            },
            "shipping_method": "ground"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/carts/{id}/shipping"))
                    .header("content-type", "application/json")
                    .body(Body::from(shipping.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Set payment.
        let payment =
            serde_json::json!({ "payment_method": "credit_card", "payment_token": "tok" });
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/carts/{id}/payment"))
                    .header("content-type", "application/json")
                    .body(Body::from(payment.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Complete checkout.
        let resp = app
            .oneshot(Request::post(format!("/carts/{id}/complete")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = json_body(resp).await;
        assert_eq!(result["cart_id"].as_str().unwrap(), id);
        assert_eq!(result["total_charged"], "25.50");
        assert!(result["order_number"].as_str().is_some());
    }

    #[tokio::test]
    async fn list_and_get_and_cancel() {
        let app = app();
        let body = serde_json::json!({ "customer_email": "list@example.com" });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/carts")
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
            .oneshot(Request::get("/carts?status=active").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_body(resp).await;
        assert_eq!(list["total"], 1);

        let resp = app
            .clone()
            .oneshot(Request::get(format!("/carts/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(Request::post(format!("/carts/{id}/cancel")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "cancelled");
    }

    #[tokio::test]
    async fn create_rejects_invalid_price() {
        let app = app();
        let body = serde_json::json!({
            "items": [{ "sku": "SKU-X", "name": "Bad", "quantity": 1, "unit_price": "abc" }]
        });
        let resp = app
            .oneshot(
                Request::post("/carts")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
