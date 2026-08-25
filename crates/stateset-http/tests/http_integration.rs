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
use stateset_authz::{AuthzEngineBuilder, PermissionLevel, Role, RoleBuilder};
use stateset_embedded::Commerce;
use stateset_http::{
    AppState, CreateCustomerRequest, CreateOrderItemRequest, CreateOrderRequest,
    MetricsHeaderLimits, PaginationParams, ServerBuilder,
};
use stateset_primitives::{CustomerId, OrderId, PaymentId, ProductId, ReturnId, ShipmentId};
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
    // These fixtures exercise idempotency scoping via `x-tenant-id` without a
    // tenant database directory; opt into the explicit escape hatch now that
    // silent fallthrough is rejected.
    let state = AppState::new(test_commerce()).with_ignore_tenant_header();
    let router = stateset_http::routes::api_router().with_state(state.clone());
    (router, state)
}

fn secure_app() -> (axum::Router, String) {
    let builder = ServerBuilder::new(test_commerce()).with_ignore_tenant_header();
    let token =
        builder.bearer_auth_token().expect("default auth token should be configured").to_string();
    (builder.build(), token)
}

fn secure_authz_app() -> (axum::Router, String) {
    let builder = ServerBuilder::new(test_commerce())
        .with_ignore_tenant_header()
        .with_authz_engine(authz_engine())
        .trust_actor_headers_for_authz();
    let token =
        builder.bearer_auth_token().expect("default auth token should be configured").to_string();
    (builder.build(), token)
}

fn secure_bound_authz_app(actor_id: &str) -> (axum::Router, String) {
    let builder = ServerBuilder::new(test_commerce())
        .with_ignore_tenant_header()
        .with_authz_engine(authz_engine())
        .bind_auth_actor(actor_id);
    let token =
        builder.bearer_auth_token().expect("default auth token should be configured").to_string();
    (builder.build(), token)
}

fn multi_actor_authz_app() -> axum::Router {
    ServerBuilder::new(test_commerce())
        .without_auth()
        .with_ignore_tenant_header()
        .add_bearer_auth_for_actor("viewer-token", "viewer-1")
        .add_bearer_auth_for_actor("admin-token", "admin-1")
        .with_authz_engine(authz_engine())
        .build()
}

fn tenant_app(base_dir: &std::path::Path, token: &str, tenant_id: &str) -> axum::Router {
    ServerBuilder::new(test_commerce())
        .with_bearer_auth_for_tenant(token, tenant_id)
        .with_tenant_db_dir(base_dir.to_path_buf())
        .build()
}

fn test_commerce() -> Commerce {
    Commerce::new(":memory:").expect("in-memory Commerce")
}

fn authz_engine() -> stateset_authz::AuthzEngine {
    AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .add_role(RoleBuilder::new("writer").default_level(PermissionLevel::Write).build())
        .assign_role("admin-1", "admin")
        .assign_role("viewer-1", "viewer")
        .assign_role("writer-1", "writer")
        .build()
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

/// Create an order directly via the Commerce engine.
///
/// Returns `(order_id, first_order_item_id)` so callers that need to create
/// returns (which require at least one real order item) can supply a valid
/// `order_item_id`.
fn seed_order(state: &AppState) -> (OrderId, Uuid) {
    let customer_id = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: format!("order-seed-{}@example.com", Uuid::new_v4()),
            first_name: "Order".into(),
            last_name: "Seed".into(),
            ..Default::default()
        })
        .unwrap()
        .id;
    let product = state
        .commerce()
        .products()
        .create(stateset_core::CreateProduct {
            name: "Seed Widget".into(),
            variants: Some(vec![stateset_core::CreateProductVariant {
                sku: format!("SEED-SKU-{}", Uuid::new_v4()),
                price: dec!(9.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    let product_id = product.id;
    let order = state
        .commerce()
        .orders()
        .create(stateset_core::CreateOrder {
            customer_id,
            items: vec![stateset_core::CreateOrderItem {
                product_id,
                sku: format!("SEED-SKU-{}", Uuid::new_v4()),
                name: "Seed Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let first_item_id = *order.items[0].id.as_uuid();
    (order.id, first_item_id)
}

/// Like [`seed_order`], but the order is shipped: returns can only be requested
/// against shipped goods.
fn seed_shipped_order(state: &AppState) -> (OrderId, Uuid) {
    let (order_id, item_id) = seed_order(state);
    state.commerce().orders().ship(order_id, None).unwrap();
    (order_id, item_id)
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
    assert!(json.get("tenant_cache").is_none());
}

#[tokio::test]
async fn health_ready_returns_200_with_database_connected() {
    let resp =
        app().oneshot(Request::get("/health/ready").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
    assert_eq!(json["database"], "connected");
    assert!(json.get("tenant_cache").is_none());
}

#[tokio::test]
async fn metrics_returns_200_with_prometheus_payload() {
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
    assert!(text.contains("stateset_http_metrics_auth_enabled 1"));
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
async fn api_authz_requires_actor_header_when_enabled() {
    let (router, token) = secure_authz_app();

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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_authz_fails_closed_without_actor_binding_or_trusted_headers() {
    let builder = ServerBuilder::new(test_commerce()).with_authz_engine(authz_engine());
    let token =
        builder.bearer_auth_token().expect("default auth token should be configured").to_string();
    let router = builder.build();

    let resp = router
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("x-actor-id", "viewer-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn viewer_can_read_but_not_create_when_authz_enabled() {
    let (router, token) = secure_authz_app();

    let read_resp = router
        .clone()
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("x-actor-id", "viewer-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read_resp.status(), StatusCode::OK);

    let payload = json!({
        "email": "viewer@example.com",
        "first_name": "Viewer",
        "last_name": "Only"
    });
    let create_resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("x-actor-id", "viewer-1")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_can_create_customer_when_authz_enabled() {
    let (router, token) = secure_authz_app();
    let payload = json!({
        "email": "admin@example.com",
        "first_name": "Admin",
        "last_name": "User"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("x-actor-id", "admin-1")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn bound_actor_token_allows_authorized_create_without_actor_header() {
    let (router, token) = secure_bound_authz_app("admin-1");
    let payload = json!({
        "email": "bound-admin@example.com",
        "first_name": "Bound",
        "last_name": "Admin"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn bound_actor_token_rejects_conflicting_actor_header() {
    let (router, token) = secure_bound_authz_app("viewer-1");

    let resp = router
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "tenant-1")
                .header("x-actor-id", "admin-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn multi_actor_tokens_map_to_distinct_roles() {
    let router = multi_actor_authz_app();

    let viewer_read = router
        .clone()
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", "Bearer viewer-token")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_read.status(), StatusCode::OK);

    let payload = json!({
        "email": "multi-admin@example.com",
        "first_name": "Multi",
        "last_name": "Admin"
    });

    let viewer_create = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", "Bearer viewer-token")
                .header("x-tenant-id", "tenant-1")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_create.status(), StatusCode::FORBIDDEN);

    let admin_create = router
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", "Bearer admin-token")
                .header("x-tenant-id", "tenant-1")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_create.status(), StatusCode::CREATED);
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
async fn tenant_header_without_tenant_routing_returns_400() {
    let router = ServerBuilder::new(test_commerce()).without_auth().build();

    let resp = router
        .oneshot(
            Request::get("/api/v1/customers")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("tenant routing is not enabled on this deployment"),
        "unexpected message: {}",
        json["error"]["message"]
    );
}

#[tokio::test]
async fn ignore_tenant_header_escape_hatch_serves_shared_data() {
    let router =
        ServerBuilder::new(test_commerce()).without_auth().with_ignore_tenant_header().build();

    let resp = router
        .oneshot(
            Request::get("/api/v1/customers")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn tenant_routing_rejects_path_traversal_tenant_ids() {
    let tenant_dir =
        std::env::temp_dir().join(format!("stateset-http-tenant-traversal-{}", Uuid::new_v4()));
    let token = "traversal-token";
    let router = tenant_app(&tenant_dir, token, "tenant-real");

    for tenant in ["..", "../other", "./x", "/etc/passwd", "a/b", "a\\b", "a.b"] {
        let resp = router
            .clone()
            .oneshot(
                Request::get("/api/v1/customers")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-tenant-id", tenant)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Depending on middleware ordering the request is refused either as an
        // invalid tenant id (400) or as a tenant/token mismatch (403); either
        // way it must never reach a database outside the tenant directory.
        assert!(
            resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::FORBIDDEN,
            "tenant id {tenant:?} returned {}",
            resp.status()
        );
    }
    let _ = std::fs::remove_dir_all(&tenant_dir);
}

#[tokio::test]
async fn tenant_db_routing_isolates_data_between_tenants() {
    let tenant_dir =
        std::env::temp_dir().join(format!("stateset-http-tenant-e2e-{}", Uuid::new_v4()));
    let tenant_a_token = "tenant-a-token";
    let tenant_b_token = "tenant-b-token";
    let router_a = tenant_app(&tenant_dir, tenant_a_token, "tenant-a");
    let router_b = tenant_app(&tenant_dir, tenant_b_token, "tenant-b");

    let create_body = json!({
        "email": "tenant-a@example.com",
        "first_name": "Tenant",
        "last_name": "A"
    });

    let create_resp = router_a
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", format!("Bearer {tenant_a_token}"))
                .header("content-type", "application/json")
                .header("x-tenant-id", "tenant-a")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_resp.status();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(create_status, StatusCode::CREATED, "{}", String::from_utf8_lossy(&create_body));

    let tenant_a_list = router_a
        .clone()
        .oneshot(
            Request::get("/api/v1/customers")
                .header("authorization", format!("Bearer {tenant_a_token}"))
                .header("x-tenant-id", "tenant-a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tenant_a_list.status(), StatusCode::OK);
    let tenant_a_json = body_json(tenant_a_list).await;
    assert_eq!(tenant_a_json["total"], 1);

    let tenant_b_list = router_b
        .clone()
        .oneshot(
            Request::get("/api/v1/customers")
                .header("authorization", format!("Bearer {tenant_b_token}"))
                .header("x-tenant-id", "tenant-b")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tenant_b_list.status(), StatusCode::OK);
    let tenant_b_json = body_json(tenant_b_list).await;
    assert_eq!(tenant_b_json["total"], 0);

    let missing_tenant_header = router_a
        .oneshot(
            Request::get("/api/v1/customers")
                .header("authorization", format!("Bearer {tenant_a_token}"))
                .body(Body::empty())
                .unwrap(),
        )
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
async fn oversized_create_customer_payload_returns_413() {
    let router =
        ServerBuilder::new(test_commerce()).without_auth().with_max_request_body_bytes(64).build();
    let payload = json!({
        "email": format!("{}@example.com", "a".repeat(80)),
        "first_name": "Alice",
        "last_name": "Wonderland"
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

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
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
        stock_policy: stateset_core::StockPolicy::RejectIfInsufficient,
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
    assert_eq!(deserialized.stock_policy, stateset_core::StockPolicy::RejectIfInsufficient);
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

// ============================================================================
// 5. Returns
// ============================================================================

#[tokio::test]
async fn list_returns_returns_200_with_empty_list() {
    let resp =
        app().oneshot(Request::get("/api/v1/returns").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["returns"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["has_more"], false);
}

#[tokio::test]
async fn create_return_returns_201_with_correct_fields() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_shipped_order(&state);

    let router = stateset_http::routes::api_router().with_state(state);
    let payload = json!({
        "order_id": order_id,
        "reason": "defective",
        "items": [{"order_item_id": item_id, "quantity": 1}]
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert!(json["id"].as_str().is_some());
    assert_eq!(json["order_id"], order_id.to_string());
    assert_eq!(json["reason"], "defective");
    assert!(json["status"].as_str().is_some());
    assert!(json["created_at"].as_str().is_some());
    assert!(json["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn get_return_by_id_returns_correct_return() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_shipped_order(&state);

    // Create return via API
    let create_router = stateset_http::routes::api_router().with_state(state.clone());
    let payload = json!({
        "order_id": order_id,
        "reason": "wrong_item",
        "reason_details": "Received blue widget, expected red",
        "items": [{"order_item_id": item_id, "quantity": 1}]
    });

    let resp = create_router
        .oneshot(
            Request::post("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let return_id = created["id"].as_str().unwrap();

    // Get return by ID
    let get_router = stateset_http::routes::api_router().with_state(state);
    let resp = get_router
        .oneshot(Request::get(format!("/api/v1/returns/{return_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp).await;
    assert_eq!(fetched["id"], return_id);
    assert_eq!(fetched["order_id"], order_id.to_string());
    assert_eq!(fetched["reason"], "wrong_item");
}

#[tokio::test]
async fn get_return_nonexistent_returns_404() {
    let id = ReturnId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/returns/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn create_return_with_invalid_reason_returns_400() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_shipped_order(&state);

    let router = stateset_http::routes::api_router().with_state(state);
    let payload = json!({
        "order_id": order_id,
        "reason": "unicorn_dust",
        "items": [{"order_item_id": item_id, "quantity": 1}]
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn list_returns_with_custom_pagination() {
    let resp = app()
        .oneshot(Request::get("/api/v1/returns?limit=10&offset=5").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 5);
}

#[tokio::test]
async fn e2e_create_return_then_list_returns() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_shipped_order(&state);

    // Create two returns for the same order (different items via separate orders)
    for reason in ["defective", "changed_mind"] {
        let router = stateset_http::routes::api_router().with_state(state.clone());
        let payload = json!({
            "order_id": order_id,
            "reason": reason,
            "items": [{"order_item_id": item_id, "quantity": 1}]
        });

        let resp = router
            .oneshot(
                Request::post("/api/v1/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List and verify both appear
    let router = stateset_http::routes::api_router().with_state(state);
    let resp =
        router.oneshot(Request::get("/api/v1/returns").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["returns"].as_array().unwrap().len(), 2);
}

// ============================================================================
// 6. Payments
// ============================================================================

#[tokio::test]
async fn list_payments_returns_200_with_empty_list() {
    let resp =
        app().oneshot(Request::get("/api/v1/payments").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["payments"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["has_more"], false);
}

#[tokio::test]
async fn get_payment_nonexistent_returns_404() {
    let id = PaymentId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/payments/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn get_payment_by_id_returns_correct_payment() {
    let state = AppState::new(test_commerce());

    // Seed a payment directly via the Commerce engine
    let payment = state
        .commerce()
        .payments()
        .create(stateset_core::CreatePayment {
            payment_method: stateset_core::PaymentMethodType::CreditCard,
            amount: dec!(49.99),
            ..Default::default()
        })
        .unwrap();
    let payment_id = payment.id.to_string();

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(
            Request::get(format!("/api/v1/payments/{payment_id}")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["id"], payment_id);
    assert!(json["payment_number"].as_str().is_some());
    assert!(json["status"].as_str().is_some());
    assert!(json["amount"].as_str().is_some());
    assert!(json["created_at"].as_str().is_some());
    assert!(json["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn list_payments_with_custom_pagination() {
    let resp = app()
        .oneshot(Request::get("/api/v1/payments?limit=5&offset=10").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 5);
    assert_eq!(json["offset"], 10);
}

#[tokio::test]
async fn e2e_create_payments_then_list_payments() {
    let state = AppState::new(test_commerce());

    // Seed three payments via the Commerce engine
    for amount in [dec!(10.00), dec!(20.00), dec!(30.00)] {
        state
            .commerce()
            .payments()
            .create(stateset_core::CreatePayment {
                payment_method: stateset_core::PaymentMethodType::CreditCard,
                amount,
                ..Default::default()
            })
            .unwrap();
    }

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/payments").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 3);
    assert_eq!(list["payments"].as_array().unwrap().len(), 3);
    assert_eq!(list["has_more"], false);
}

#[tokio::test]
async fn list_payments_invalid_status_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/payments?status=bogus").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn list_payments_invalid_order_id_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/payments?order_id=not-a-uuid").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

// ============================================================================
// 7. Shipments
// ============================================================================

#[tokio::test]
async fn list_shipments_returns_200_with_empty_list() {
    let resp = app()
        .oneshot(Request::get("/api/v1/shipments").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["shipments"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["has_more"], false);
}

#[tokio::test]
async fn get_shipment_nonexistent_returns_404() {
    let id = ShipmentId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/shipments/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn get_shipment_by_id_returns_correct_shipment() {
    let state = AppState::new(test_commerce());
    let (order_id, _) = seed_order(&state);

    // Seed a shipment directly via the Commerce engine
    let shipment = state
        .commerce()
        .shipments()
        .create(stateset_core::CreateShipment {
            order_id,
            recipient_name: "Jane Doe".into(),
            shipping_address: "123 Main St, Springfield, IL 62701".into(),
            tracking_number: Some("1Z999AA1012345678".into()),
            ..Default::default()
        })
        .unwrap();
    let shipment_id = shipment.id.to_string();

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(
            Request::get(format!("/api/v1/shipments/{shipment_id}")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["id"], shipment_id);
    assert_eq!(json["order_id"], order_id.to_string());
    assert_eq!(json["recipient_name"], "Jane Doe");
    assert!(json["shipment_number"].as_str().is_some());
    assert!(json["status"].as_str().is_some());
    assert!(json["created_at"].as_str().is_some());
    assert!(json["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn list_shipments_with_custom_pagination() {
    let resp = app()
        .oneshot(Request::get("/api/v1/shipments?limit=10&offset=5").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 5);
}

#[tokio::test]
async fn e2e_create_shipment_then_list_shipments() {
    let state = AppState::new(test_commerce());
    let (order_id, _) = seed_order(&state);

    // Seed a shipment via the Commerce engine
    let shipment = state
        .commerce()
        .shipments()
        .create(stateset_core::CreateShipment {
            order_id,
            recipient_name: "Bob Smith".into(),
            shipping_address: "456 Oak Ave, Portland, OR 97201".into(),
            ..Default::default()
        })
        .unwrap();
    let shipment_id = shipment.id.to_string();

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/shipments").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 1);
    let shipments = list["shipments"].as_array().unwrap();
    assert_eq!(shipments.len(), 1);
    assert_eq!(shipments[0]["id"], shipment_id);
}

#[tokio::test]
async fn list_shipments_invalid_status_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/shipments?status=bogus").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn list_shipments_invalid_order_id_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/shipments?order_id=not-a-uuid").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

// ============================================================================
// 8. Inventory
// ============================================================================

#[tokio::test]
async fn list_inventory_returns_200_with_empty_list() {
    let resp = app()
        .oneshot(Request::get("/api/v1/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert!(json["items"].as_array().unwrap().is_empty());
    assert_eq!(json["limit"], PaginationParams::DEFAULT_LIMIT);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["has_more"], false);
}

#[tokio::test]
async fn get_stock_nonexistent_returns_404() {
    let resp = app()
        .oneshot(Request::get("/api/v1/inventory/NONEXISTENT-SKU").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
}

#[tokio::test]
async fn get_stock_by_sku_returns_correct_stock_levels() {
    let state = AppState::new(test_commerce());

    state
        .commerce()
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: "WIDGET-INT-001".into(),
            name: "Integration Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .unwrap();

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/inventory/WIDGET-INT-001").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["sku"], "WIDGET-INT-001");
    assert_eq!(json["name"], "Integration Widget");
    assert!(json["total_on_hand"].as_str().is_some());
    assert!(json["total_available"].as_str().is_some());
    assert!(json["total_allocated"].as_str().is_some());
}

#[tokio::test]
async fn adjust_stock_returns_updated_stock_levels() {
    let state = AppState::new(test_commerce());

    state
        .commerce()
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: "ADJ-INT-001".into(),
            name: "Adjustable Widget".into(),
            initial_quantity: Some(dec!(50)),
            ..Default::default()
        })
        .unwrap();

    let router = stateset_http::routes::api_router().with_state(state);
    let payload = json!({
        "quantity": "-10",
        "reason": "Damaged stock removal"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/inventory/ADJ-INT-001/adjust")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["sku"], "ADJ-INT-001");
    assert!(json["total_on_hand"].as_str().is_some());
    assert!(json["total_available"].as_str().is_some());
}

#[tokio::test]
async fn adjust_stock_nonexistent_sku_returns_404() {
    let router = stateset_http::routes::api_router().with_state(AppState::new(test_commerce()));
    let payload = json!({
        "quantity": "5",
        "reason": "Restock"
    });

    let resp = router
        .oneshot(
            Request::post("/api/v1/inventory/NONEXISTENT-SKU/adjust")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_inventory_with_custom_pagination() {
    let resp = app()
        .oneshot(Request::get("/api/v1/inventory?limit=10&offset=5").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 5);
}

#[tokio::test]
async fn e2e_create_inventory_items_then_list_inventory() {
    let state = AppState::new(test_commerce());

    for (sku, name, qty) in
        [("LIST-INT-001", "Alpha Item", dec!(100)), ("LIST-INT-002", "Beta Item", dec!(50))]
    {
        state
            .commerce()
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku: sku.into(),
                name: name.into(),
                initial_quantity: Some(qty),
                ..Default::default()
            })
            .unwrap();
    }

    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(Request::get("/api/v1/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert_eq!(list["has_more"], false);
}

// ============================================================================
// Deep Health Check
// ============================================================================

#[tokio::test]
async fn deep_health_returns_database_latency_and_metrics() {
    let (router, token) = secure_app();
    let resp = router
        .oneshot(
            Request::get("/health/deep")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
    assert!(json["database"]["connected"].as_bool().unwrap());
    assert!(json["database"]["latency_ms"].is_number());
}

#[tokio::test]
async fn deep_health_requires_bearer_auth_by_default() {
    let (router, _token) = secure_app();
    let resp =
        router.oneshot(Request::get("/health/deep").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Customer PATCH / DELETE
// ============================================================================

#[tokio::test]
async fn update_customer_returns_updated_fields() {
    let (router, state) = app_with_state();

    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: "patch-me@test.com".into(),
            first_name: "Old".into(),
            last_name: "Name".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .unwrap();

    let body = json!({ "first_name": "New", "last_name": "Updated" });
    let resp = router
        .oneshot(
            Request::patch(format!("/api/v1/customers/{}", customer.id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["first_name"], "New");
    assert_eq!(json["last_name"], "Updated");
    assert_eq!(json["email"], "patch-me@test.com");
}

#[tokio::test]
async fn delete_customer_returns_204() {
    let (router, state) = app_with_state();

    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: "delete-int@test.com".into(),
            first_name: "Del".into(),
            last_name: "Ete".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .unwrap();

    let resp = router
        .oneshot(
            Request::delete(format!("/api/v1/customers/{}", customer.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ============================================================================
// Product PATCH / DELETE
// ============================================================================

#[tokio::test]
async fn update_product_returns_updated_fields() {
    let (router, state) = app_with_state();

    let product = state
        .commerce()
        .products()
        .create(stateset_core::CreateProduct {
            name: "Widget".into(),
            slug: None,
            description: Some("Old desc".into()),
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .unwrap();

    let body = json!({ "name": "Super Widget", "description": "New desc" });
    let resp = router
        .oneshot(
            Request::patch(format!("/api/v1/products/{}", product.id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["name"], "Super Widget");
}

#[tokio::test]
async fn delete_product_returns_204() {
    let (router, state) = app_with_state();

    let product = state
        .commerce()
        .products()
        .create(stateset_core::CreateProduct {
            name: "Deletable".into(),
            slug: None,
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .unwrap();

    let resp = router
        .oneshot(
            Request::delete(format!("/api/v1/products/{}", product.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ============================================================================
// Shipment Create
// ============================================================================

#[tokio::test]
async fn create_shipment_returns_201() {
    let (router, state) = app_with_state();

    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: "shipper-int@test.com".into(),
            first_name: "Ship".into(),
            last_name: "Per".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .unwrap();
    let order = state
        .commerce()
        .orders()
        .create(stateset_core::CreateOrder {
            customer_id: customer.id,
            items: vec![stateset_core::CreateOrderItem {
                product_id: ProductId::new(),
                variant_id: None,
                sku: "SHIP-INT".into(),
                name: "Shippable".into(),
                quantity: 1,
                unit_price: dec!(15.00),
                discount: None,
                tax_amount: None,
            }],
            currency: None,
            shipping_address: None,
            billing_address: None,
            notes: None,
            payment_method: None,
            shipping_method: None,
            stock_policy: stateset_core::StockPolicy::default(),
        })
        .unwrap();

    let body = json!({
        "order_id": order.id.to_string(),
        "carrier": "ups",
        "tracking_number": "1Z999AA10123456784"
    });
    let resp = router
        .oneshot(
            Request::post("/api/v1/shipments")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert_eq!(json["tracking_number"], "1Z999AA10123456784");
}

// ============================================================================
// Invoice Create
// ============================================================================

#[tokio::test]
async fn create_invoice_returns_201() {
    let (router, state) = app_with_state();

    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: "invoice-int@test.com".into(),
            first_name: "Bill".into(),
            last_name: "Able".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .unwrap();

    let body = json!({
        "customer_id": customer.id.to_string(),
        "payment_terms": "net_30"
    });
    let resp = router
        .oneshot(
            Request::post("/api/v1/invoices")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// ============================================================================
// End-to-End Commerce Flow
// ============================================================================

/// Full commerce lifecycle: customer → product → order → payment → shipment.
/// This is the critical happy-path test for v1.0.0 readiness.
#[tokio::test]
async fn e2e_full_commerce_lifecycle() {
    let (router, _state) = app_with_state();

    // Step 1: Create customer
    let body = json!({
        "email": "e2e-buyer@test.com",
        "first_name": "E2E",
        "last_name": "Buyer"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let customer = body_json(resp).await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    // Step 2: Create product
    let body = json!({
        "name": "E2E Widget",
        "description": "A widget for end-to-end testing"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let product = body_json(resp).await;
    let product_id = product["id"].as_str().unwrap().to_string();

    // Step 3: Create order
    let body = json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "E2E-WIDGET-001",
            "name": "E2E Widget",
            "quantity": 2,
            "unit_price": 49.99
        }]
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let order = body_json(resp).await;
    let order_id = order["id"].as_str().unwrap().to_string();
    assert_eq!(order["status"], "pending");
    assert_eq!(order["items"].as_array().unwrap().len(), 1);

    // Step 4: Create payment for order
    let body = json!({
        "order_id": order_id,
        "amount": 99.98,
        "payment_method": "credit_card"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/payments")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let payment = body_json(resp).await;
    assert!(payment["id"].is_string());

    // Step 5: Create shipment
    let body = json!({
        "order_id": order_id,
        "carrier": "ups",
        "tracking_number": "1Z999AA10123456784"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/shipments")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let shipment = body_json(resp).await;
    assert_eq!(shipment["tracking_number"], "1Z999AA10123456784");

    // Step 6: Verify order is retrievable with all data
    let resp = router
        .clone()
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let final_order = body_json(resp).await;
    assert_eq!(final_order["id"], order_id);
    assert_eq!(final_order["customer_id"], customer_id);

    // Step 7: Verify customer is retrievable
    let resp = router
        .oneshot(
            Request::get(format!("/api/v1/customers/{customer_id}")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let final_customer = body_json(resp).await;
    assert_eq!(final_customer["email"], "e2e-buyer@test.com");
}

// ============================================================================
// 9. E2E Order Lifecycle
// ============================================================================

/// Full order lifecycle: create customer -> create product -> create inventory ->
/// create order -> ship order -> verify status transitions.
#[tokio::test]
async fn e2e_order_lifecycle_create_ship_verify() {
    let (router, state) = app_with_state();

    let body = json!({
        "email": "lifecycle@example.com",
        "first_name": "Life",
        "last_name": "Cycle"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let customer = body_json(resp).await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    let body = json!({ "name": "Lifecycle Widget", "description": "Widget for lifecycle testing" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let product = body_json(resp).await;
    let product_id = product["id"].as_str().unwrap().to_string();

    state
        .commerce()
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: "LIFECYCLE-SKU-001".into(),
            name: "Lifecycle Widget".into(),
            initial_quantity: Some(dec!(50)),
            ..Default::default()
        })
        .unwrap();

    let body = json!({
        "customer_id": customer_id,
        "items": [{ "product_id": product_id, "sku": "LIFECYCLE-SKU-001", "name": "Lifecycle Widget", "quantity": 2, "unit_price": "29.99" }],
        "currency": "USD"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let order = body_json(resp).await;
    let order_id = order["id"].as_str().unwrap().to_string();
    assert_eq!(order["status"], "pending");

    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let shipped = body_json(resp).await;
    assert_eq!(shipped["status"], "shipped");

    let resp = router
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let final_order = body_json(resp).await;
    assert_eq!(final_order["status"], "shipped");
    assert_eq!(final_order["customer_id"], customer_id);
}

/// Order lifecycle with cancellation path.
#[tokio::test]
async fn e2e_order_lifecycle_cancel() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);

    let body = json!({
        "customer_id": customer_id,
        "items": [{ "product_id": product_id, "sku": "CANCEL-SKU-001", "name": "Cancel Widget", "quantity": 1, "unit_price": "15.00" }]
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let order = body_json(resp).await;
    let order_id = order["id"].as_str().unwrap().to_string();
    assert_eq!(order["status"], "pending");

    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cancelled = body_json(resp).await;
    assert_eq!(cancelled["status"], "cancelled");

    let resp = router
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let final_order = body_json(resp).await;
    assert_eq!(final_order["status"], "cancelled");
}

// ============================================================================
// 10. Return Processing E2E
// ============================================================================

/// Return processing E2E: create order -> ship -> create return -> approve.
#[tokio::test]
async fn e2e_return_processing_approve() {
    let (router, state) = app_with_state();
    let (order_id, item_id) = seed_order(&state);

    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = json!({
        "order_id": order_id, "reason": "defective", "reason_details": "Widget arrived cracked",
        "items": [{"order_item_id": item_id, "quantity": 1}]
    });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let ret = body_json(resp).await;
    let return_id = ret["id"].as_str().unwrap().to_string();

    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/returns/{return_id}/approve"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let approved = body_json(resp).await;
    assert_eq!(approved["status"], "approved");

    let resp = router
        .oneshot(Request::get(format!("/api/v1/returns/{return_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp).await;
    assert_eq!(fetched["status"], "approved");
    assert_eq!(fetched["reason"], "defective");
}

/// Return with multiple reasons using fresh state per iteration.
#[tokio::test]
async fn e2e_return_different_reasons() {
    for reason in ["changed_mind", "wrong_item", "not_as_described"] {
        let state = AppState::new(test_commerce());
        let (order_id, item_id) = seed_shipped_order(&state);
        let router = stateset_http::routes::api_router().with_state(state);
        let body = json!({ "order_id": order_id, "reason": reason, "items": [{"order_item_id": item_id, "quantity": 1}] });
        let resp = router
            .oneshot(
                Request::post("/api/v1/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED, "Failed for reason: {reason}");
        let ret = body_json(resp).await;
        assert_eq!(ret["reason"], reason);
    }
}

// ============================================================================
// 11. Inventory Management E2E
// ============================================================================

#[tokio::test]
async fn e2e_inventory_create_adjust_verify() {
    let (router, state) = app_with_state();
    state
        .commerce()
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: "INV-E2E-001".into(),
            name: "E2E Inventory Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .unwrap();

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/inventory/INV-E2E-001").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let stock = body_json(resp).await;
    assert_eq!(stock["sku"], "INV-E2E-001");

    let body = json!({ "quantity": "25", "reason": "Restock shipment" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/inventory/INV-E2E-001/adjust")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = json!({ "quantity": "-10", "reason": "Damaged removal" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/inventory/INV-E2E-001/adjust")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let adjusted = body_json(resp).await;
    assert_eq!(adjusted["sku"], "INV-E2E-001");
    assert!(adjusted["total_on_hand"].as_str().is_some());
}

#[tokio::test]
async fn e2e_inventory_multiple_items_list() {
    let (router, state) = app_with_state();
    for (sku, name, qty) in [
        ("MULTI-INV-A", "Alpha Gizmo", dec!(200)),
        ("MULTI-INV-B", "Beta Gizmo", dec!(75)),
        ("MULTI-INV-C", "Gamma Gizmo", dec!(10)),
    ] {
        state
            .commerce()
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku: sku.into(),
                name: name.into(),
                initial_quantity: Some(qty),
                ..Default::default()
            })
            .unwrap();
    }
    let resp = router
        .oneshot(Request::get("/api/v1/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 3);
    assert_eq!(list["items"].as_array().unwrap().len(), 3);
}

// ============================================================================
// 12. Multi-Entity Filtering & Pagination
// ============================================================================

#[tokio::test]
async fn e2e_order_status_filtering() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let mut order_ids = Vec::new();
    for i in 0..3 {
        let body = json!({ "customer_id": customer_id, "items": [{ "product_id": product_id, "sku": format!("FILTER-SKU-{i:03}"), "name": "Filter Widget", "quantity": 1, "unit_price": "10.00" }] });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let order = body_json(resp).await;
        order_ids.push(order["id"].as_str().unwrap().to_string());
    }
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{}/ship", order_ids[0]))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{}/cancel", order_ids[1]))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?status=pending").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 1);

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?status=shipped").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 1);

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?status=cancelled").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 1);

    let resp =
        router.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 3);
}

#[tokio::test]
async fn e2e_order_pagination_limit_offset() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    for i in 0..5 {
        let body = json!({ "customer_id": customer_id, "items": [{ "product_id": product_id, "sku": format!("PAGE-SKU-{i:03}"), "name": "Paginated Widget", "quantity": 1, "unit_price": "10.00" }] });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?limit=2&offset=0").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let p1 = body_json(resp).await;
    assert_eq!(p1["total"], 5);
    assert_eq!(p1["limit"], 2);
    assert_eq!(p1["orders"].as_array().unwrap().len(), 2);
    assert_eq!(p1["has_more"], true);

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?limit=2&offset=2").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let p2 = body_json(resp).await;
    assert_eq!(p2["orders"].as_array().unwrap().len(), 2);
    assert_eq!(p2["has_more"], true);

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?limit=2&offset=4").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let p3 = body_json(resp).await;
    assert_eq!(p3["orders"].as_array().unwrap().len(), 1);
    assert_eq!(p3["has_more"], false);

    let resp = router
        .oneshot(Request::get("/api/v1/orders?limit=2&offset=10").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["orders"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn e2e_order_cursor_pagination() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    for i in 0..4 {
        let body = json!({ "customer_id": customer_id, "items": [{ "product_id": product_id, "sku": format!("CURSOR-SKU-{i:03}"), "name": "Cursor Widget", "quantity": 1, "unit_price": "5.00" }] });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders?limit=2").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let page1 = body_json(resp).await;
    assert_eq!(page1["orders"].as_array().unwrap().len(), 2);
    assert_eq!(page1["has_more"], true);
    let cursor = page1["next_cursor"].as_str().unwrap();

    let resp = router
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/orders?limit=2&after={cursor}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let page2 = body_json(resp).await;
    assert_eq!(page2["orders"].as_array().unwrap().len(), 2);

    let p1_ids: Vec<&str> =
        page1["orders"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    let p2_ids: Vec<&str> =
        page2["orders"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    for id in &p2_ids {
        assert!(!p1_ids.contains(id), "Cursor pagination returned duplicate ID: {id}");
    }
}

#[tokio::test]
async fn e2e_customer_filtering_by_email() {
    let state = AppState::new(test_commerce());
    for (email, first) in [("alice-filter@test.com", "Alice"), ("bob-filter@test.com", "Bob")] {
        let router = stateset_http::routes::api_router().with_state(state.clone());
        let body = json!({ "email": email, "first_name": first, "last_name": "FilterTest" });
        let resp = router
            .oneshot(
                Request::post("/api/v1/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    let router = stateset_http::routes::api_router().with_state(state);
    let resp = router
        .oneshot(
            Request::get("/api/v1/customers?email=alice-filter@test.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["customers"].as_array().unwrap()[0]["email"], "alice-filter@test.com");
}

// ============================================================================
// 13. Error Handling
// ============================================================================

#[tokio::test]
async fn error_invalid_uuid_format_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/orders/not-a-valid-uuid").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn error_invalid_order_status_filter_returns_400() {
    let resp = app()
        .oneshot(
            Request::get("/api/v1/orders?status=nonexistent_status").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(json["error"]["message"].as_str().unwrap().contains("Invalid status"));
}

#[tokio::test]
async fn error_invalid_payment_status_filter_returns_400() {
    let resp = app()
        .oneshot(Request::get("/api/v1/orders?payment_status=bogus").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn error_invalid_fulfillment_status_filter_returns_400() {
    let resp = app()
        .oneshot(
            Request::get("/api/v1/orders?fulfillment_status=bogus").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn error_create_order_empty_items() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let body = json!({ "customer_id": customer_id, "items": [] });
    let resp = router
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn error_create_customer_missing_all_required() {
    let body = json!({});
    let resp = app()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn error_ship_nonexistent_order_returns_404() {
    let id = OrderId::new();
    let resp = app()
        .oneshot(Request::patch(format!("/api/v1/orders/{id}/ship")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_cancel_nonexistent_order_returns_404() {
    let id = OrderId::new();
    let resp = app()
        .oneshot(Request::patch(format!("/api/v1/orders/{id}/cancel")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_approve_nonexistent_return_returns_404() {
    let id = ReturnId::new();
    let resp = app()
        .oneshot(
            Request::patch(format!("/api/v1/returns/{id}/approve")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_complete_nonexistent_payment_returns_404() {
    let id = PaymentId::new();
    let resp = app()
        .oneshot(
            Request::post(format!("/api/v1/payments/{id}/complete")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_body_shape_is_consistent() {
    let id = OrderId::new();
    let resp = app()
        .oneshot(Request::get(format!("/api/v1/orders/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"));
    let json = body_json(resp).await;
    assert!(json["error"].is_object());
    assert!(json["error"]["code"].is_string());
    assert!(json["error"]["message"].is_string());
}

#[tokio::test]
async fn error_invalid_cursor_returns_400() {
    let resp = app()
        .oneshot(
            Request::get("/api/v1/orders?after=not-a-valid-base64!!!").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

// ============================================================================
// 14. ETag / HTTP Caching
// ============================================================================

#[tokio::test]
async fn etag_present_on_get_response() {
    let resp =
        app().oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let etag = resp.headers().get("etag");
    assert!(etag.is_some(), "GET response must include ETag header");
    let etag_str = etag.unwrap().to_str().unwrap();
    assert!(etag_str.starts_with("W/\""), "ETag should be weak");
    assert!(resp.headers().get("cache-control").is_some(), "Must include Cache-Control");
}

#[tokio::test]
async fn etag_304_not_modified_on_match() {
    let router = app();
    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let etag = resp.headers().get("etag").unwrap().to_str().unwrap().to_string();

    let resp = router
        .oneshot(
            Request::get("/api/v1/orders")
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn etag_200_on_mismatch() {
    let resp = app()
        .oneshot(
            Request::get("/api/v1/orders")
                .header("if-none-match", "W/\"stale-etag\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("etag").is_some());
}

#[tokio::test]
async fn cache_control_differs_by_endpoint_type() {
    let router = ServerBuilder::new(test_commerce()).without_auth().build();
    let body = json!({ "email": "cachetest@test.com", "first_name": "Cache", "last_name": "Test" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let cid = created["id"].as_str().unwrap();

    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list_cc = resp
        .headers()
        .get("cache-control")
        .expect("list cache-control")
        .to_str()
        .unwrap()
        .to_string();

    let resp = router
        .oneshot(Request::get(format!("/api/v1/customers/{cid}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let single_cc = resp
        .headers()
        .get("cache-control")
        .expect("single cache-control")
        .to_str()
        .unwrap()
        .to_string();

    assert_ne!(list_cc, single_cc);
    assert!(list_cc.contains("max-age=10"));
    assert!(single_cc.contains("max-age=60"));
}

// ============================================================================
// 15. Tenant Isolation
// ============================================================================

#[tokio::test]
async fn e2e_tenant_isolation_orders() {
    let tenant_dir =
        std::env::temp_dir().join(format!("stateset-http-tenant-orders-{}", Uuid::new_v4()));
    let tenant_x_token = "tenant-x-token";
    let tenant_y_token = "tenant-y-token";
    let router_x = tenant_app(&tenant_dir, tenant_x_token, "tenant-x");
    let router_y = tenant_app(&tenant_dir, tenant_y_token, "tenant-y");

    let body =
        json!({ "email": "tenant-x@example.com", "first_name": "TenantX", "last_name": "User" });
    let resp = router_x
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("authorization", format!("Bearer {tenant_x_token}"))
                .header("content-type", "application/json")
                .header("x-tenant-id", "tenant-x")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::CREATED, "{}", String::from_utf8_lossy(&body_bytes));
    let cust: Value = serde_json::from_slice(&body_bytes).unwrap();
    let cid = cust["id"].as_str().unwrap();

    let body = json!({ "name": "Tenant X Widget" });
    let resp = router_x
        .clone()
        .oneshot(
            Request::post("/api/v1/products")
                .header("authorization", format!("Bearer {tenant_x_token}"))
                .header("content-type", "application/json")
                .header("x-tenant-id", "tenant-x")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let prod = body_json(resp).await;
    let pid = prod["id"].as_str().unwrap();

    let body = json!({ "customer_id": cid, "items": [{ "product_id": pid, "sku": "TX-001", "name": "Tenant X Widget", "quantity": 1, "unit_price": "25.00" }] });
    let resp = router_x
        .clone()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("authorization", format!("Bearer {tenant_x_token}"))
                .header("content-type", "application/json")
                .header("x-tenant-id", "tenant-x")
                // ServerBuilder-built routers require idempotency keys on
                // money-moving creates by default.
                .header("idempotency-key", "tenant-x-order-1")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = router_x
        .clone()
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {tenant_x_token}"))
                .header("x-tenant-id", "tenant-x")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 1);

    let resp = router_y
        .clone()
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {tenant_y_token}"))
                .header("x-tenant-id", "tenant-y")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 0);

    let resp = router_x
        .oneshot(
            Request::get("/api/v1/orders")
                .header("authorization", format!("Bearer {tenant_x_token}"))
                .header("x-tenant-id", "tenant-y")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let _ = fs::remove_dir_all(tenant_dir);
}

#[tokio::test]
async fn e2e_tenant_isolation_returns() {
    let tenant_dir =
        std::env::temp_dir().join(format!("stateset-http-tenant-returns-{}", Uuid::new_v4()));
    let tenant_a_token = "tenant-a-token";
    let tenant_b_token = "tenant-b-token";
    let router_a = tenant_app(&tenant_dir, tenant_a_token, "tenant-a");
    let router_b = tenant_app(&tenant_dir, tenant_b_token, "tenant-b");

    let resp = router_a
        .clone()
        .oneshot(
            Request::get("/api/v1/returns")
                .header("authorization", format!("Bearer {tenant_a_token}"))
                .header("x-tenant-id", "tenant-a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body_bytes));
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["total"], 0);

    let resp = router_b
        .oneshot(
            Request::get("/api/v1/returns")
                .header("authorization", format!("Bearer {tenant_b_token}"))
                .header("x-tenant-id", "tenant-b")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body_bytes));
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["total"], 0);
    let _ = fs::remove_dir_all(tenant_dir);
}

// ============================================================================
// 16. Payment Lifecycle E2E
// ============================================================================

#[tokio::test]
async fn e2e_payment_lifecycle_complete_and_refund() {
    let (router, state) = app_with_state();
    let (order_id, _) = seed_order(&state);

    let body = json!({ "order_id": order_id, "amount": 19.98, "payment_method": "credit_card" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/payments")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let payment = body_json(resp).await;
    let payment_id = payment["id"].as_str().unwrap().to_string();

    let resp = router
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/payments/{payment_id}/complete"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["status"], "completed");

    let body = json!({ "amount": 9.99, "reason": "Customer dissatisfied" });
    let resp = router
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/payments/{payment_id}/refund"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = router
        .oneshot(
            Request::get(format!("/api/v1/payments/{payment_id}")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["id"], payment_id);
}

// ============================================================================
// 17. Concurrent Operations
// ============================================================================

#[tokio::test]
async fn concurrent_requests_to_different_endpoints() {
    let router = app();
    let (r1, r2, r3) = tokio::join!(
        router.clone().oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()),
        router.clone().oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap()),
        router.clone().oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap()),
    );
    assert_eq!(r1.unwrap().status(), StatusCode::OK);
    assert_eq!(r2.unwrap().status(), StatusCode::OK);
    assert_eq!(r3.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn concurrent_order_creation() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let mut handles = Vec::new();
    for i in 0..5 {
        let r = router.clone();
        let cid = customer_id.clone();
        let pid = product_id.clone();
        handles.push(tokio::spawn(async move {
            let body = json!({ "customer_id": cid, "items": [{ "product_id": pid, "sku": format!("CONC-SKU-{i:03}"), "name": "Concurrent Widget", "quantity": 1, "unit_price": "10.00" }] });
            r.oneshot(Request::post("/api/v1/orders").header("content-type", "application/json").body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap()).await.unwrap()
        }));
    }
    let results: Vec<_> = futures::future::join_all(handles).await;
    for result in &results {
        assert_eq!(result.as_ref().unwrap().status(), StatusCode::CREATED);
    }

    let resp =
        router.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 5);
}

#[tokio::test]
async fn concurrent_customer_creation() {
    let router = app();
    let mut handles = Vec::new();
    for i in 0..5 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let body = json!({ "email": format!("concurrent-{i}@test.com"), "first_name": format!("C{i}"), "last_name": "User" });
            r.oneshot(Request::post("/api/v1/customers").header("content-type", "application/json").body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap()).await.unwrap()
        }));
    }
    let results: Vec<_> = futures::future::join_all(handles).await;
    for result in &results {
        assert_eq!(result.as_ref().unwrap().status(), StatusCode::CREATED);
    }

    let resp = router
        .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["total"], 5);
}

// ============================================================================
// 18. Additional Error & Edge Cases
// ============================================================================

#[tokio::test]
async fn error_return_with_zero_quantity() {
    let (router, state) = app_with_state();
    let (order_id, item_id) = seed_shipped_order(&state);
    let body = json!({ "order_id": order_id, "reason": "defective", "items": [{"order_item_id": item_id, "quantity": 0}] });
    let resp = router
        .oneshot(
            Request::post("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn pagination_limit_zero_clamped_to_one() {
    let resp = app()
        .oneshot(Request::get("/api/v1/orders?limit=0").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["limit"], 1);
}

#[tokio::test]
async fn pagination_limit_above_max_is_clamped() {
    let resp = app()
        .oneshot(Request::get("/api/v1/orders?limit=999").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["limit"], PaginationParams::MAX_LIMIT);
}

// ============================================================================
// 19. Request ID Propagation
// ============================================================================

#[tokio::test]
async fn request_id_propagated_in_response() {
    let router = ServerBuilder::new(test_commerce()).without_auth().with_request_id().build();
    let resp = router.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let rid = resp.headers().get("x-request-id");
    assert!(rid.is_some());
    assert!(Uuid::parse_str(rid.unwrap().to_str().unwrap()).is_ok());
}

#[tokio::test]
async fn request_id_client_supplied_preserved() {
    let router = ServerBuilder::new(test_commerce()).without_auth().with_request_id().build();
    let resp = router
        .oneshot(
            Request::get("/health")
                .header("x-request-id", "my-custom-id-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("x-request-id").unwrap().to_str().unwrap(), "my-custom-id-12345");
}

// ============================================================================
// 20. Rate Limiting
// ============================================================================

#[tokio::test]
async fn rate_limiter_returns_429_on_exhaustion() {
    let router = ServerBuilder::new(test_commerce()).without_auth().with_rate_limit(1, 2).build();
    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = router
        .clone()
        .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp =
        router.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().get("retry-after").is_some());
}

// ============================================================================
// 21. OpenAPI Spec
// ============================================================================

#[tokio::test]
async fn openapi_spec_served_as_json() {
    let resp = app()
        .oneshot(Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("json"));
    let json = body_json(resp).await;
    assert!(json["openapi"].is_string());
    assert!(json["paths"].is_object());
}

// ============================================================================
// 22. Customer & Product CRUD Roundtrip
// ============================================================================

#[tokio::test]
async fn e2e_customer_create_update_delete_roundtrip() {
    let (router, _) = app_with_state();
    let body =
        json!({ "email": "roundtrip-del@test.com", "first_name": "Round", "last_name": "Trip" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/customers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let cid = body_json(resp).await["id"].as_str().unwrap().to_string();

    let body = json!({ "first_name": "Updated", "last_name": "Name" });
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/customers/{cid}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["first_name"], "Updated");

    let resp = router
        .clone()
        .oneshot(Request::delete(format!("/api/v1/customers/{cid}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Soft delete: entity may still be accessible or return 404
    let resp = router
        .oneshot(Request::get(format!("/api/v1/customers/{cid}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn e2e_product_create_update_delete_roundtrip() {
    let (router, _) = app_with_state();
    let body = json!({ "name": "Roundtrip Product", "description": "Will be updated and deleted" });
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let pid = body_json(resp).await["id"].as_str().unwrap().to_string();

    let body = json!({ "name": "Updated Product" });
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/products/{pid}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["name"], "Updated Product");

    let resp = router
        .clone()
        .oneshot(Request::delete(format!("/api/v1/products/{pid}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = router
        .oneshot(Request::get(format!("/api/v1/products/{pid}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND);
}

// ============================================================================
// OpenAPI Endpoint & Route Coverage Tests
// ============================================================================

#[tokio::test]
async fn openapi_json_endpoint_returns_200() {
    let (router, _state) = app_with_state();
    let resp = router
        .oneshot(Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn openapi_json_endpoint_returns_valid_json() {
    let (router, _state) = app_with_state();
    let resp = router
        .oneshot(Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["openapi"].is_string(), "spec should have 'openapi' version field");
    assert!(json["info"].is_object(), "spec should have 'info' block");
    assert!(json["paths"].is_object(), "spec should have 'paths' block");
}

#[tokio::test]
async fn openapi_json_has_json_content_type() {
    let (router, _state) = app_with_state();
    let resp = router
        .oneshot(Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(
        ct.contains("application/json"),
        "openapi.json should return application/json, got: {ct}"
    );
}

#[tokio::test]
async fn docs_ui_endpoint_returns_html() {
    let (router, _state) = app_with_state();
    let resp =
        router.oneshot(Request::get("/api/v1/docs").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(ct.contains("text/html"), "docs should return text/html, got: {ct}");
}

/// Verify that all GET list endpoints respond (not 404) and return JSON.
#[tokio::test]
async fn all_list_endpoints_respond_with_json() {
    let (router, _state) = app_with_state();
    let list_paths = [
        "/api/v1/orders",
        "/api/v1/customers",
        "/api/v1/products",
        "/api/v1/inventory",
        "/api/v1/returns",
        "/api/v1/shipments",
        "/api/v1/payments",
        "/api/v1/invoices",
        "/api/v1/reviews",
        "/api/v1/wishlists",
        "/api/v1/gift-cards",
        "/api/v1/loyalty/programs",
    ];
    for path in list_paths {
        let resp =
            router.clone().oneshot(Request::get(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_ne!(resp.status(), StatusCode::NOT_FOUND, "GET {path} should not be 404");
        let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.contains("application/json"), "GET {path} should return JSON, got: {ct}");
    }
}

/// Verify that invalid HTTP methods return 405 Method Not Allowed.
#[tokio::test]
async fn invalid_method_returns_405_on_get_only_routes() {
    let (router, _state) = app_with_state();
    // These routes only accept GET; POST/PUT/DELETE should be 405
    let get_only_paths = ["/health", "/health/ready"];
    for path in get_only_paths {
        let resp = router
            .clone()
            .oneshot(Request::builder().method("DELETE").uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED, "DELETE {path} should be 405");
    }
}

/// Verify PUT is not allowed on endpoints that only accept POST+GET.
#[tokio::test]
async fn put_not_allowed_on_collection_endpoints() {
    let (router, _state) = app_with_state();
    let collection_paths = [
        "/api/v1/orders",
        "/api/v1/customers",
        "/api/v1/products",
        "/api/v1/returns",
        "/api/v1/shipments",
        "/api/v1/payments",
        "/api/v1/invoices",
    ];
    for path in collection_paths {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED, "PUT {path} should be 405");
    }
}

/// Verify POST on detail routes (get-only) returns 405.
#[tokio::test]
async fn post_not_allowed_on_detail_get_endpoints() {
    let fake_id = Uuid::new_v4();
    let (router, _state) = app_with_state();
    let detail_paths = [
        format!("/api/v1/orders/{fake_id}"),
        format!("/api/v1/customers/{fake_id}"),
        "/api/v1/inventory/FAKE-SKU".to_string(),
    ];
    for path in &detail_paths {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Orders/{id} only has GET, so POST should be 405
        // (unless there's a nested route catching it, in which case it may be 404)
        assert!(
            resp.status() == StatusCode::METHOD_NOT_ALLOWED
                || resp.status() == StatusCode::NOT_FOUND,
            "POST {} should be 405 or 404, got {}",
            path,
            resp.status()
        );
    }
}

/// Health endpoints return correct Content-Type.
#[tokio::test]
async fn health_endpoints_return_json_content_type() {
    let (router, _state) = app_with_state();
    let health_paths = ["/health", "/health/ready"];
    for path in health_paths {
        let resp =
            router.clone().oneshot(Request::get(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.contains("application/json"), "GET {path} should return JSON, got: {ct}");
    }
}

/// Verify nonexistent v1 API routes return 404 Not Found.
#[tokio::test]
async fn nonexistent_v1_route_returns_404() {
    let (router, _state) = app_with_state();
    let resp = router
        .oneshot(Request::get("/api/v1/does-not-exist").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Verify the events stream endpoint is accessible (responds, not 404).
#[tokio::test]
async fn events_stream_endpoint_responds() {
    let (router, _state) = app_with_state();
    // SSE endpoint may return 200 and start streaming; we just check it's not 404/405
    let resp = router
        .oneshot(Request::get("/api/v1/events/stream").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND, "events/stream should not be 404");
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED, "events/stream should accept GET");
}

// ============================================================================
// Idempotency-Key middleware
// ============================================================================

/// Build a minimal order-create payload for the seeded customer/product.
fn order_payload(customer_id: &str, product_id: &str) -> Value {
    json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "INT-SKU-001",
            "name": "Test Widget",
            "quantity": 1,
            "unit_price": "19.99"
        }]
    })
}

fn order_post(payload: &Value, key: &str, tenant: &str) -> Request<Body> {
    Request::post("/api/v1/orders")
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        .header("x-tenant-id", tenant)
        .body(Body::from(serde_json::to_vec(payload).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn idempotent_replay_returns_identical_response_and_no_duplicate() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let payload = order_payload(&customer_id, &product_id);

    let first =
        router.clone().oneshot(order_post(&payload, "order-key-1", "tenant-a")).await.unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(
        first.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
        Some("false")
    );
    let first_body = axum::body::to_bytes(first.into_body(), usize::MAX).await.unwrap();

    let second =
        router.clone().oneshot(order_post(&payload, "order-key-1", "tenant-a")).await.unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(
        second.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
        Some("true")
    );
    let second_body = axum::body::to_bytes(second.into_body(), usize::MAX).await.unwrap();

    // The replayed response is byte-identical to the original.
    assert_eq!(first_body, second_body);

    // Exactly one order was created despite two requests with the same key.
    let orders = state.commerce().orders().list(Default::default()).unwrap();
    assert_eq!(orders.len(), 1, "replay must not create a duplicate order");
}

#[tokio::test]
async fn idempotent_conflicting_body_is_rejected() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let payload = order_payload(&customer_id, &product_id);

    let first =
        router.clone().oneshot(order_post(&payload, "order-key-2", "tenant-a")).await.unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    // Same key, different body (extra quantity) → 422 with the error envelope.
    let mut conflicting = payload.clone();
    conflicting["items"][0]["quantity"] = json!(99);
    let conflict =
        router.clone().oneshot(order_post(&conflicting, "order-key-2", "tenant-a")).await.unwrap();
    assert_eq!(conflict.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(conflict).await;
    assert_eq!(json["error"]["code"], "validation_error");

    // The conflicting request did not create a second order.
    let orders = state.commerce().orders().list(Default::default()).unwrap();
    assert_eq!(orders.len(), 1);
}

#[tokio::test]
async fn idempotency_keys_are_isolated_per_tenant() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let payload = order_payload(&customer_id, &product_id);

    // Same key + identical body, but different tenants → two fresh writes.
    for tenant in ["tenant-a", "tenant-b"] {
        let resp =
            router.clone().oneshot(order_post(&payload, "shared-key", tenant)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(
            resp.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
            Some("false"),
            "each tenant's first use of the key must be a fresh write, not a replay"
        );
    }

    // Both tenants resolved to the same in-memory engine here, so two orders
    // exist — proving the idempotency cache is keyed per tenant, not global.
    let orders = state.commerce().orders().list(Default::default()).unwrap();
    assert_eq!(orders.len(), 2);
}

#[tokio::test]
async fn requests_without_idempotency_key_are_not_deduplicated() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    let payload = order_payload(&customer_id, &product_id);

    for _ in 0..2 {
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("content-type", "application/json")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(resp.headers().get("idempotency-replayed").is_none());
    }

    let orders = state.commerce().orders().list(Default::default()).unwrap();
    assert_eq!(orders.len(), 2, "without a key, each request creates a new order");
}

/// POST /gl/close-month runs the month-end close orchestration: dry run first
/// (nothing written), then a real close that posts the closing entry and
/// closes the period.
#[tokio::test]
async fn gl_close_month_dry_run_then_close() {
    let (router, state) = app_with_state();
    let gl = state.commerce().general_ledger();
    gl.initialize_chart_of_accounts().unwrap();

    // Wide open period covering today so posted entries land inside it.
    let period = gl
        .create_period(stateset_core::CreateGlPeriod {
            period_name: "FY-wide".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            end_date: chrono::NaiveDate::from_ymd_opt(2030, 12, 31).unwrap(),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();

    // Seed P&L activity: debit cash, credit sales revenue.
    let cash = gl.get_account_by_number("1010").unwrap().unwrap();
    let revenue = gl.get_account_by_number("4010").unwrap().unwrap();
    gl.create_journal_entry(stateset_core::CreateJournalEntry {
        entry_date: chrono::Utc::now().date_naive(),
        entry_type: None,
        description: "Cash sale".into(),
        lines: vec![
            stateset_core::CreateJournalEntryLine::debit(cash.id, dec!(500), None),
            stateset_core::CreateJournalEntryLine::credit(revenue.id, dec!(500), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(true),
    })
    .unwrap();

    // Dry run: 200, per-step dry_run statuses, period stays open.
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/gl/close-month")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "period_id": period.id.to_string(), "dry_run": true })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["period_close"]["status"], "dry_run");
    assert_eq!(json["fx_revaluation"]["status"], "skipped");
    assert_eq!(json["period_status"], "open");
    assert!(json["closing_entry"].is_null());

    // Real close: closing entry posted, period closed.
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/gl/close-month")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "period_id": period.id.to_string(),
                        "closed_by": "integration-test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["period_close"]["status"], "executed");
    assert_eq!(json["period_close"]["entry_count"], 1);
    assert_eq!(json["period_status"], "closed");
    assert_eq!(json["closing_entry"]["entry_type"], "closing");
    assert_eq!(json["closing_entry"]["is_balanced"], true);
    let closed = state.commerce().general_ledger().get_period(period.id).unwrap().unwrap();
    assert_eq!(closed.status, stateset_core::PeriodStatus::Closed);
    assert_eq!(closed.closed_by.as_deref(), Some("integration-test"));
}

/// POST /gl/close-month with an invalid period id is a 400; unknown period is 404.
#[tokio::test]
async fn gl_close_month_bad_period_ids() {
    let (router, _state) = app_with_state();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/gl/close-month")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({ "period_id": "not-a-uuid" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/gl/close-month")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "period_id": Uuid::new_v4().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_order_stock_policy_reject_if_insufficient_returns_400_and_persists_nothing() {
    let (router, state) = app_with_state();
    let customer_id = seed_customer(&state);
    let product_id = seed_product(&state);
    state
        .commerce()
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: "POLICY-HTTP-SKU".into(),
            name: "Scarce Widget".into(),
            initial_quantity: Some(dec!(1)),
            ..Default::default()
        })
        .unwrap();

    let payload = json!({
        "customer_id": customer_id,
        "items": [{
            "product_id": product_id,
            "sku": "POLICY-HTTP-SKU",
            "name": "Scarce Widget",
            "quantity": 3,
            "unit_price": "19.99"
        }],
        "stock_policy": "reject_if_insufficient"
    });

    let resp = router
        .clone()
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(json["error"]["message"].as_str().unwrap().contains("Insufficient stock"), "{json}");

    let orders = state.commerce().orders().list(stateset_core::OrderFilter::default()).unwrap();
    assert!(orders.is_empty(), "rejected order must not be persisted");
    let stock = state.commerce().inventory().get_stock("POLICY-HTTP-SKU").unwrap().unwrap();
    assert_eq!(stock.total_available, dec!(1));

    // The default policy (field omitted) still creates the order and backorders the rest.
    let mut default_payload = payload.clone();
    default_payload.as_object_mut().unwrap().remove("stock_policy");
    let resp = router
        .oneshot(
            Request::post("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&default_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn ship_order_with_lines_returns_partially_shipped_and_shipped_quantity() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_order(&state); // one line, quantity 2
    let router = stateset_http::routes::api_router().with_state(state);

    let body = json!({
        "tracking_number": "TRK-PARTIAL",
        "lines": [{"order_item_id": item_id, "quantity": 1}]
    });
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "partially_shipped");
    assert_eq!(json["items"][0]["quantity"], 2);
    assert_eq!(json["items"][0]["shipped_quantity"], 1);

    // Ship the remainder with no body: legacy "ship everything" completes it.
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "shipped");
    assert_eq!(json["items"][0]["shipped_quantity"], 2);
}

#[tokio::test]
async fn ship_order_overship_returns_400_and_persists_nothing() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_order(&state);
    let router = stateset_http::routes::api_router().with_state(state);

    let body = json!({"lines": [{"order_item_id": item_id, "quantity": 3}]});
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert!(
        json["error"]["message"].as_str().unwrap().contains("requested 3, remaining 2"),
        "{json}"
    );

    let resp = router
        .oneshot(Request::get(format!("/api/v1/orders/{order_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_ne!(json["status"], "partially_shipped");
    assert_eq!(json["items"][0]["shipped_quantity"], 0);
}

#[tokio::test]
async fn return_after_partial_ship_is_capped_at_shipped_quantity() {
    let state = AppState::new(test_commerce());
    let (order_id, item_id) = seed_order(&state);
    let router = stateset_http::routes::api_router().with_state(state);

    let body = json!({"lines": [{"order_item_id": item_id, "quantity": 1}]});
    let resp = router
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/orders/{order_id}/ship"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2 ordered, 1 shipped: returning 2 is rejected, returning 1 is accepted.
    for (qty, expected) in [(2, StatusCode::UNPROCESSABLE_ENTITY), (1, StatusCode::CREATED)] {
        let body = json!({
            "order_id": order_id, "reason": "defective",
            "items": [{"order_item_id": item_id, "quantity": qty}]
        });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/v1/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), expected, "return qty {qty}");
    }
}
