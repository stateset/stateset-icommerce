//! Order endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};

use crate::dto::{
    CreateOrderItemRequest, CreateOrderRequest, OrderListResponse, OrderResponse, PaginationParams,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{Address, CreateOrder, CreateOrderItem, OrderFilter, OrderId};

/// Build the orders sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orders", post(create_order).get(list_orders))
        .route("/orders/{id}", get(get_order))
        .route("/orders/{id}/cancel", patch(cancel_order))
        .route("/orders/{id}/ship", patch(ship_order))
}

/// `POST /api/v1/orders`
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    tag = "orders",
    request_body = CreateOrderRequest,
    responses(
        (status = 201, description = "Order created", body = OrderResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateOrderRequest>,
) -> Result<(axum::http::StatusCode, Json<OrderResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let input = CreateOrder {
        customer_id: req.customer_id,
        items: req.items.into_iter().map(into_core_order_item).collect(),
        currency: req.currency,
        shipping_address: req.shipping_address.map(Address::from),
        billing_address: req.billing_address.map(Address::from),
        notes: req.notes,
        payment_method: req.payment_method,
        shipping_method: req.shipping_method,
    };
    let order = commerce.orders().create(input)?;
    Ok((axum::http::StatusCode::CREATED, Json(OrderResponse::from(order))))
}

/// `GET /api/v1/orders/:id`
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    tag = "orders",
    params(("id" = String, Path, description = "Order ID (UUID)")),
    responses(
        (status = 200, description = "Order details", body = OrderResponse),
        (status = 404, description = "Order not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<OrderId>,
) -> Result<Json<OrderResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let order = commerce
        .orders()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Order {id} not found")))?;
    Ok(Json(OrderResponse::from(order)))
}

/// `GET /api/v1/orders`
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    tag = "orders",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of orders", body = OrderListResponse),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Json<OrderListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let total = commerce.orders().list(OrderFilter::default())?.len();
    let filter = OrderFilter {
        limit: Some(params.resolved_limit()),
        offset: Some(params.resolved_offset()),
        ..Default::default()
    };
    let orders = commerce.orders().list(filter)?;
    Ok(Json(OrderListResponse {
        orders: orders.into_iter().map(OrderResponse::from).collect(),
        total,
        limit: params.resolved_limit(),
        offset: params.resolved_offset(),
    }))
}

/// `PATCH /api/v1/orders/:id/cancel`
#[utoipa::path(
    patch,
    path = "/api/v1/orders/{id}/cancel",
    tag = "orders",
    params(("id" = String, Path, description = "Order ID (UUID)")),
    responses(
        (status = 200, description = "Order cancelled", body = OrderResponse),
        (status = 400, description = "Order cannot be cancelled", body = ErrorBody),
        (status = 404, description = "Order not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<OrderId>,
) -> Result<Json<OrderResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let order = commerce.orders().cancel(id)?;
    Ok(Json(OrderResponse::from(order)))
}

/// `PATCH /api/v1/orders/:id/ship`
#[utoipa::path(
    patch,
    path = "/api/v1/orders/{id}/ship",
    tag = "orders",
    params(("id" = String, Path, description = "Order ID (UUID)")),
    responses(
        (status = 200, description = "Order shipped", body = OrderResponse),
        (status = 400, description = "Order cannot be shipped", body = ErrorBody),
        (status = 404, description = "Order not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn ship_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<OrderId>,
) -> Result<Json<OrderResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let order = commerce.orders().ship(id, None)?;
    Ok(Json(OrderResponse::from(order)))
}

fn into_core_order_item(item: CreateOrderItemRequest) -> CreateOrderItem {
    CreateOrderItem {
        product_id: item.product_id,
        variant_id: item.variant_id,
        sku: item.sku,
        name: item.name,
        quantity: item.quantity,
        unit_price: item.unit_price,
        discount: item.discount,
        tax_amount: item.tax_amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use rust_decimal_macros::dec;
    use stateset_embedded::Commerce;
    use stateset_primitives::ProductId;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    #[tokio::test]
    async fn create_order_returns_201() {
        let (app, state) = app_with_state();

        // Create a customer first
        let customer = state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "test@example.com".into(),
                first_name: "Test".into(),
                last_name: "User".into(),
                ..Default::default()
            })
            .unwrap();

        // Create a product and variant
        let product = state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Widget".into(),
                variants: Some(vec![stateset_core::CreateProductVariant {
                    sku: "SKU-001".into(),
                    price: dec!(29.99),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();

        let body = serde_json::json!({
            "customer_id": customer.id,
            "items": [{
                "product_id": product.id,
                "sku": "SKU-001",
                "name": "Widget",
                "quantity": 2,
                "unit_price": "29.99"
            }]
        });

        let resp = app
            .oneshot(
                Request::post("/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn list_orders_reports_total_before_pagination() {
        let (app, state) = app_with_state();

        let customer = state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "paging-orders@example.com".into(),
                first_name: "Paging".into(),
                last_name: "Orders".into(),
                ..Default::default()
            })
            .unwrap();

        let product = state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Paging Widget".into(),
                variants: Some(vec![stateset_core::CreateProductVariant {
                    sku: "PAGE-ORD-001".into(),
                    price: dec!(9.99),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();

        for _ in 0..2 {
            state
                .commerce()
                .orders()
                .create(stateset_core::CreateOrder {
                    customer_id: customer.id,
                    items: vec![stateset_core::CreateOrderItem {
                        product_id: product.id,
                        variant_id: None,
                        sku: "PAGE-ORD-001".into(),
                        name: "Paging Widget".into(),
                        quantity: 1,
                        unit_price: dec!(9.99),
                        discount: None,
                        tax_amount: None,
                    }],
                    ..Default::default()
                })
                .unwrap();
        }

        let resp = app
            .oneshot(Request::get("/orders?limit=1&offset=0").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["orders"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_order_not_found() {
        let id = OrderId::new();
        let resp = app()
            .oneshot(Request::get(format!("/orders/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_orders_empty() {
        let resp =
            app().oneshot(Request::get("/orders").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["orders"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_orders_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/orders?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }

    #[tokio::test]
    async fn cancel_nonexistent_order_fails() {
        let id = OrderId::new();
        let resp = app()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/orders/{id}/cancel"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should be 404 or 400 depending on error mapping
        assert!(resp.status().is_client_error());
    }

    #[test]
    fn into_core_order_item_converts() {
        let req = CreateOrderItemRequest {
            product_id: ProductId::new(),
            variant_id: None,
            sku: "SKU".into(),
            name: "Name".into(),
            quantity: 1,
            unit_price: dec!(10),
            discount: Some(dec!(1)),
            tax_amount: None,
        };
        let core = into_core_order_item(req);
        assert_eq!(core.sku, "SKU");
        assert_eq!(core.discount, Some(dec!(1)));
    }
}
