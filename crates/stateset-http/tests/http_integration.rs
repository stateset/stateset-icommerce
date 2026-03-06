//! Integration tests for the stateset-http crate.
//!
//! These tests exercise the full HTTP stack through the [`ServerBuilder`] router,
//! sending real HTTP requests via `tower::ServiceExt::oneshot` against an
//! in-memory [`Commerce`] instance.

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use stateset_embedded::Commerce;
use stateset_http::{
    AppState, CreateCustomerRequest, CreateOrderItemRequest, CreateOrderRequest,
    MetricsHeaderLimits, PaginationParams, ServerBuilder,
};
use stateset_primitives::{CustomerId, OrderId, ProductId};
use std::fs;
use tower::ServiceExt;
use uuid::Uuid;

// ============================================================================
// Helpers
// ============================================================================

/// Build a full application router backed by an in-memory database.
fn app() -> axum::Router {
    ServerBuilder::new(test_commerce()).without_auth().build()
}

/// Build a router and return the shared state for direct Commerce access.
fn app_with_state() -> (axum::Router, AppState) {
    let state = AppState::new(test_commerce());
    let router = stateset_http::routes::api_router().with_state(state.clone());
    (router, state)
}

fn secure_app() -> (axum::Router, String) {
    let builder = ServerBuilder::new(test_commerce());
    let token =
        builder.bearer_auth_token().expect("default auth token should be configured").to_string();
    (builder.build(), token)
}

fn test_commerce() -> Commerce {
    Commerce::new(":memory:").expect("in-memory Commerce")
}

/// Read a response body into a [`serde_json::Value`].
async fn body_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// Create a customer directly via the Commerce engine and return its ID string.
fn seed_customer(state: &AppState) -> String {
    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: "integration@example.com".into(),
            first_name: "Integration".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    customer.id.to_string()
}

/// Create a product with a variant directly via the Commerce engine.
fn seed_product(state: &AppState) -> String {
    let product = state
        .commerce()
        .products()
        .create(stateset_core::CreateProduct {
            name: "Test Widget".into(),
            variants: Some(vec![stateset_core::CreateProductVariant {
                sku: "INT-SKU-001".into(),
                price: dec!(19.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    product.id.to_string()
}

// ============================================================================
// 1. Route Handler Response Shapes
// ============================================================================

#[tokio::test]
async fn health_returns_200_with_status_ok() {
    let resp = app().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn health_ready_returns_200_with_database_connected() {
    let resp =
        app().oneshot(Request::get("/health/ready").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
    assert_eq!(json["database"], "connected");
}

#[tokio::test]
async fn metrics_returns_200_with_prometheus_payload() {
    let resp = app().oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/plain; version=0.0.4; charset=utf-8"
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("stateset_http_tenant_cache_enabled 0"));
    assert!(text.contains("stateset_http_metrics_scrape_requests_total 1"));
    assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
    assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 1"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 0"));
    assert!(text.contains(
        "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 0"
    ));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
    assert!(
        text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0")
    );
    assert!(
        text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
    );
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 0"));
    assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 0"));
    assert!(text.contains("stateset_http_metrics_auth_enabled 0"));
    assert!(text.contains("stateset_http_metrics_ip_allowlist_entries 0"));
    assert!(text.contains("stateset_http_metrics_trusted_proxy_cidr_entries 0"));
    assert!(text.contains("stateset_http_metrics_forwarded_header_limit_bytes 2048"));
    assert!(text.contains("stateset_http_metrics_authorization_header_limit_bytes 2048"));
    assert!(text.contains("stateset_engine_orders_created_total"));
}

#[tokio::test]
async fn api_requires_bearer_auth_by_default() {
    let (router, _token) = secure_app();

    let resp =
        router.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_accepts_valid_bearer_auth() {
    let (router, token) = secure_app();

    let resp = router
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_requires_bearer_auth_by_default() {
    let (router, _token) = secure_app();

    let resp = router.oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn metrics_accepts_valid_bearer_auth_by_default() {
    let (router, token) = secure_app();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_allowlist_rejects_disallowed_ip() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .build();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", "Bearer metrics-token")
                .header("x-forwarded-for", "203.0.113.10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_allowlist_accepts_allowed_x_real_ip_with_port() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .build();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", "Bearer metrics-token")
                .header("x-real-ip", "127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_allowlist_requires_client_ip_header_when_configured() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .build();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", "Bearer metrics-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_allowlist_rejects_oversized_x_forwarded_for_header() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .build();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", "Bearer metrics-token")
                .header("x-forwarded-for", format!("127.0.0.1,{}", "a".repeat(4096)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_allowlist_rejects_header_over_custom_x_forwarded_for_limit() {
    let limits = MetricsHeaderLimits::new(2048, 15, 512).unwrap();
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .with_metrics_header_limits(limits)
        .build();

    let resp = router
        .oneshot(
            Request::get("/metrics")
                .header("authorization", "Bearer metrics-token")
                .header("x-forwarded-for", "127.0.0.1,10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_cidr_allowlist_accepts_matching_peer_ip() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_cidr_allowlist(["10.0.0.0/8".parse().unwrap()])
        .build();

    let mut request = Request::get("/metrics")
        .header("authorization", "Bearer metrics-token")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("10.9.8.7:8080".parse::<std::net::SocketAddr>().unwrap()));

    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_cidr_allowlist_rejects_non_matching_peer_ip() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_cidr_allowlist(["10.0.0.0/8".parse().unwrap()])
        .build();

    let mut request = Request::get("/metrics")
        .header("authorization", "Bearer metrics-token")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.10:8080".parse::<std::net::SocketAddr>().unwrap()));

    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_trusted_proxy_uses_forwarded_ip_when_peer_is_trusted() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
        .build();

    let mut request = Request::get("/metrics")
        .header("authorization", "Bearer metrics-token")
        .header("x-forwarded-for", "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("10.5.4.3:8080".parse::<std::net::SocketAddr>().unwrap()));

    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_trusted_proxy_ignores_forwarded_ip_when_peer_is_untrusted() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
        .build();

    let mut request = Request::get("/metrics")
        .header("authorization", "Bearer metrics-token")
        .header("x-forwarded-for", "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.10:8080".parse::<std::net::SocketAddr>().unwrap()));

    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_trusted_proxy_uses_standard_forwarded_header_when_peer_is_trusted() {
    let router = ServerBuilder::new(test_commerce())
        .with_metrics_bearer_auth("metrics-token")
        .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
        .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
        .build();

    let mut request = Request::get("/metrics")
        .header("authorization", "Bearer metrics-token")
        .header("forwarded", "for=127.0.0.1;proto=https")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("10.10.10.10:8080".parse::<std::net::SocketAddr>().unwrap()));

    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn tenant_db_routing_isolates_data_between_tenants() {
    let tenant_dir =
        std::env::temp_dir().join(format!("stateset-http-tenant-e2e-{}", Uuid::new_v4()));
    let router = ServerBuilder::new(test_commerce())
        .without_auth()
        .with_tenant_db_dir(tenant_dir.clone())
        .build();

    let create_body = json!({
        "email": "tenant-a@example.com",
        "first_name": "Tenant",
        "last_name": "A"
    });

    let create_resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .header("x-tenant-id", "tenant-a")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let tenant_a_list = router
        .clone()
        .oneshot(
            Request::get("/api/v1/customers")
                .header("x-tenant-id", "tenant-a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tenant_a_list.status(), StatusCode::OK);
    let tenant_a_json = body_json(tenant_a_list).await;
    assert_eq!(tenant_a_json["total"], 1);

    let tenant_b_list = router
        .clone()
        .oneshot(
            Request::get("/api/v1/customers")
                .header("x-tenant-id", "tenant-b")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tenant_b_list.status(), StatusCode::OK);
    let tenant_b_json = body_json(tenant_b_list).await;
    assert_eq!(tenant_b_json["total"], 0);

    let missing_tenant_header = router
        .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(missing_tenant_header.status(), StatusCode::BAD_REQUEST);

    let _ = fs::remove_dir_all(tenant_dir);
}

#[tokio::test]
async fn list_orders_returns_200_with_empty_paginated_list() {
    let resp =
        app().oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["orders"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
}

#[tokio::test]
async fn list_customers_returns_200_with_empty_paginated_list() {
    let resp = app()
        .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["customers"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
}

#[tokio::test]
async fn list_products_returns_200_with_empty_paginated_list() {
    let resp =
        app().oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["products"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn create_customer_returns_201_with_correct_fields() {
    let (router, _state) = app_with_state();

    let payload = json!({
        "email": "alice@example.com",
        "first_name": "Alice",
        "last_name": "Wonderland",
        "phone": "+1-555-0100",
        "accepts_marketing": true
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert_eq!(json["email"], "alice@example.com");
    assert_eq!(json["first_name"], "Alice");
    assert_eq!(json["last_name"], "Wonderland");
    assert!(json["id"].as_str().is_some());
    assert!(json["created_at"].as_str().is_some());
    assert!(json["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn create_order_returns_201() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);

    let payload = json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "INT-SKU-001",
            "name": "Test Widget",
            "quantity": 3,
            "unit_price": "19.99"
        }]
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert_eq!(json["customer_id"], customer_id);
    assert!(json["id"].as_str().is_some());
    assert!(json["order_number"].as_str().is_some());
    assert!(json["items"].as_array().unwrap().len() == 1);
}

#[tokio::test]
async fn get_order_by_id_returns_correct_order() {
    let state = AppState::new(test_commerce());
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);

    // Create order via API
    let create_router = stateset_http::routes::api_router().with_state(state.clone());
    let payload = json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "INT-SKU-001",
            "name": "Test Widget",
            "quantity": 1,
            "unit_price": "19.99"
        }]
    });

    let resp = create_router
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let order_id = created["id"].as_str().unwrap();

    // Get order by ID
    let get_router = stateset_http::routes::api_router().with_state(state);
    let resp = get_router
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp).await;
    assert_eq!(fetched["id"], order_id);
    assert_eq!(fetched["customer_id"], customer_id);
}

#[tokio::test]
async fn get_customer_by_id_returns_correct_customer() {
    let state = AppState::new(test_commerce());

    // Create customer via API
    let create_router = stateset_http::routes::api_router().with_state(state.clone());
    let payload = json!({
        "email": "lookup@example.com",
        "first_name": "Lookup",
        "last_name": "User"
    });

    let resp = create_router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let customer_id = created["id"].as_str().unwrap();

    // Get customer by ID
    let get_router = stateset_http::routes::api_router().with_state(state);
    let resp = get_router
        .oneshot(
            Request::get(format!("/api/v1/customers/{customer_id}")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp).await;
    assert_eq!(fetched["id"], customer_id);
    assert_eq!(fetched["email"], "lookup@example.com");
    assert_eq!(fetched["first_name"], "Lookup");
    assert_eq!(fetched["last_name"], "User");
}

// ============================================================================
// 2. Error Response Formatting
// ============================================================================

#[tokio::test]
async fn get_order_nonexistent_returns_404_with_error_body() {
    let id = OrderId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/orders/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn get_customer_nonexistent_returns_404() {
    let id = CustomerId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/customers/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
}

#[tokio::test]
async fn get_product_nonexistent_returns_404() {
    let id = ProductId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/products/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_order_with_invalid_json_returns_client_error() {
    let resp = app()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(b"{ this is not valid json".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();

    // axum returns 400 or 422 for malformed JSON
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn post_customer_with_missing_required_fields_returns_client_error() {
    // Missing first_name and last_name
    let payload = json!({
        "email": "incomplete@example.com"
    });

    let resp = app()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn post_order_with_empty_body_returns_client_error() {
    let resp = app()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(b"{}".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn nonexistent_route_returns_404() {
    let resp = app()
        .oneshot(Request::get("/api/v1/does-not-exist").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nonexistent_top_level_route_returns_404() {
    let resp = app()
        .oneshot(Request::get("/completely/unknown/path").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// 3. DTO Serialization Round-trips
// ============================================================================

#[test]
fn create_order_request_serialization_roundtrip() {
    let req = CreateOrderRequest {
        customer_id: CustomerId::new(),
        items: vec![CreateOrderItemRequest {
            product_id: ProductId::new(),
            variant_id: None,
            sku: "ROUND-TRIP-SKU".into(),
            name: "Roundtrip Widget".into(),
            quantity: 5,
            unit_price: dec!(49.95),
            discount: Some(dec!(5.00)),
            tax_amount: Some(dec!(4.50)),
        }],
        currency: Some("EUR".into()),
        shipping_address: None,
        billing_address: None,
        notes: Some("Test notes".into()),
        payment_method: Some("card".into()),
        shipping_method: Some("express".into()),
    };

    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: CreateOrderRequest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.items.len(), 1);
    assert_eq!(deserialized.items[0].sku, "ROUND-TRIP-SKU");
    assert_eq!(deserialized.items[0].quantity, 5);
    assert_eq!(deserialized.items[0].unit_price, dec!(49.95));
    assert_eq!(deserialized.items[0].discount, Some(dec!(5.00)));
    assert_eq!(deserialized.items[0].tax_amount, Some(dec!(4.50)));
    assert_eq!(deserialized.currency, Some("EUR".into()));
    assert_eq!(deserialized.notes, Some("Test notes".into()));
    assert_eq!(deserialized.payment_method, Some("card".into()));
    assert_eq!(deserialized.shipping_method, Some("express".into()));
}

#[test]
fn create_customer_request_serialization_roundtrip() {
    let req = CreateCustomerRequest {
        email: "roundtrip@example.com".into(),
        first_name: "Round".into(),
        last_name: "Trip".into(),
        phone: Some("+1-555-0199".into()),
        accepts_marketing: Some(false),
        tags: Some(vec!["vip".into(), "wholesale".into()]),
        metadata: Some(json!({"source": "integration_test"})),
    };

    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: CreateCustomerRequest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.email, "roundtrip@example.com");
    assert_eq!(deserialized.first_name, "Round");
    assert_eq!(deserialized.last_name, "Trip");
    assert_eq!(deserialized.phone, Some("+1-555-0199".into()));
    assert_eq!(deserialized.accepts_marketing, Some(false));
    assert_eq!(deserialized.tags, Some(vec!["vip".into(), "wholesale".into()]));
    assert_eq!(deserialized.metadata.unwrap()["source"], "integration_test");
}

#[test]
fn pagination_params_defaults() {
    let params = PaginationParams::default();
    assert_eq!(params.resolved_limit(), PaginationParams::DEFAULT_LIMIT);
    assert_eq!(params.resolved_limit(), 50);
    assert_eq!(params.resolved_offset(), 0);
    assert!(params.limit.is_none());
    assert!(params.offset.is_none());
}

#[test]
fn pagination_params_custom_values() {
    let params = PaginationParams { limit: Some(25), offset: Some(100) };
    assert_eq!(params.resolved_limit(), 25);
    assert_eq!(params.resolved_offset(), 100);
}

#[test]
fn pagination_params_clamps_above_max() {
    let params = PaginationParams { limit: Some(500), offset: None };
    assert_eq!(params.resolved_limit(), PaginationParams::MAX_LIMIT);
    assert_eq!(PaginationParams::MAX_LIMIT, 200);
}

#[test]
fn pagination_params_from_query_string() {
    let json_str = r#"{"limit": 10, "offset": 20}"#;
    let params: PaginationParams = serde_json::from_str(json_str).unwrap();
    assert_eq!(params.resolved_limit(), 10);
    assert_eq!(params.resolved_offset(), 20);
}

// ============================================================================
// 4. End-to-End Workflows
// ============================================================================

#[tokio::test]
async fn e2e_create_customer_create_order_list_orders_verify() {
    let state = AppState::new(test_commerce());

    // Step 1: Create a customer via HTTP
    let router = stateset_http::routes::api_router().with_state(state.clone());
    let customer_payload = json!({
        "email": "workflow@example.com",
        "first_name": "Workflow",
        "last_name": "User"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&customer_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let customer = body_json(resp).await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    // Step 2: Create a product (via Commerce engine, as product creation is prerequisite)
    let product_id = seed_product(&state);

    // Step 3: Create an order for that customer via HTTP
    let router = stateset_http::routes::api_router().with_state(state.clone());
    let order_payload = json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "INT-SKU-001",
            "name": "Test Widget",
            "quantity": 2,
            "unit_price": "19.99"
        }],
        "currency": "USD",
        "notes": "E2E test order"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&order_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let order = body_json(resp).await;
    let order_id = order["id"].as_str().unwrap().to_string();
    assert_eq!(order["customer_id"], customer_id);

    // Step 4: List orders and verify the created order appears
    let router = stateset_http::routes::api_router().with_state(state.clone());
    let resp =
        router.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 1);
    let orders = list["orders"].as_array().unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["id"], order_id);
    assert_eq!(orders[0]["customer_id"], customer_id);

    // Step 5: Get the specific order by ID and verify
    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched_order = body_json(resp).await;
    assert_eq!(fetched_order["id"], order_id);
    assert_eq!(fetched_order["currency"], "USD");
}

#[tokio::test]
async fn e2e_create_customer_then_get_verify_fields() {
    let state = AppState::new(test_commerce());

    // Create customer
    let router = stateset_http::routes::api_router().with_state(state.clone());
    let payload = json!({
        "email": "verify@example.com",
        "first_name": "Verify",
        "last_name": "Fields",
        "phone": "+1-555-0200",
        "accepts_marketing": true
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let id = created["id"].as_str().unwrap();

    // Get by ID and verify all fields
    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get(format!("/api/v1/customers/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp).await;
    assert_eq!(fetched["id"], id);
    assert_eq!(fetched["email"], "verify@example.com");
    assert_eq!(fetched["first_name"], "Verify");
    assert_eq!(fetched["last_name"], "Fields");
    assert_eq!(fetched["accepts_marketing"], true);
    assert!(fetched["status"].as_str().is_some());
    assert!(fetched["created_at"].as_str().is_some());
    assert!(fetched["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn e2e_create_product_then_list_products() {
    let state = AppState::new(test_commerce());

    // Create product via API
    let router = stateset_http::routes::api_router().with_state(state.clone());
    let payload = json!({
        "name": "Integration Widget",
        "description": "A widget for integration testing",
        "slug": "integration-widget"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    assert_eq!(created["name"], "Integration Widget");

    // List products and verify
    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert!(list["total"].as_u64().unwrap() >= 1);
    let products = list["products"].as_array().unwrap();
    let found = products.iter().any(|p| p["name"] == "Integration Widget");
    assert!(found, "Created product should appear in the list");
}

#[tokio::test]
async fn list_orders_with_custom_pagination_params() {
    let resp = app()
        .oneshot(Request::get("/api/v1/orders?limit=10&offset=5").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 5);
}

#[tokio::test]
async fn list_customers_with_custom_pagination_params() {
    let resp = app()
        .oneshot(Request::get("/api/v1/customers?limit=5&offset=10").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 5);
    assert_eq!(json["offset"], 10);
}

#[tokio::test]
async fn error_response_has_json_content_type() {
    let id = OrderId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/orders/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let content_type =
        resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(
        content_type.contains("application/json"),
        "Error responses should have JSON content type, got: {content_type}"
    );
}

#[tokio::test]
async fn create_multiple_customers_then_list_all() {
    let state = AppState::new(test_commerce());

    // Create three customers
    for (i, name) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
        let router = stateset_http::routes::api_router().with_state(state.clone());
        let payload = json!({
            "email": format!("{name}@example.com").to_lowercase(),
            "first_name": name,
            "last_name": "Tester"
        });

        let resp = router
            .oneshot(
                Request::post("/api/v1/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED, "Customer {i} creation failed");
    }

    // List all customers
    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 3);
    assert_eq!(list["customers"].as_array().unwrap().len(), 3);
}
