//! End-to-end tests for customer-scoped principals (object-level
//! authorization). The exhaustive per-route matrix lives in the crate's
//! `authz_matrix_tests`; these cover the behaviours a deployment relies on.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, Role};
use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct};
use stateset_embedded::Commerce;
use stateset_http::ServerBuilder;
use tower::ServiceExt;
use uuid::Uuid;

struct World {
    commerce: Commerce,
    a: Uuid,
    b: Uuid,
    a_orders: Vec<Uuid>,
    b_orders: Vec<Uuid>,
    product: Uuid,
}

fn world() -> World {
    let commerce = Commerce::new(":memory:").unwrap();
    let product = commerce
        .products()
        .create(CreateProduct { name: "Widget".into(), ..Default::default() })
        .unwrap()
        .id;
    let customer = |label: &str| {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("{label}@example.com"),
                first_name: label.into(),
                last_name: "Test".into(),
                ..Default::default()
            })
            .unwrap()
            .id
    };
    let a = customer("alice");
    let b = customer("bob");
    let order = |customer_id| {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: product,
                    sku: format!("SKU-{}", Uuid::new_v4().simple()),
                    name: "Widget".into(),
                    quantity: 1,
                    unit_price: dec!(10),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap()
            .id
            .into_uuid()
    };
    let a_orders = vec![order(a), order(a)];
    let b_orders = vec![order(b), order(b), order(b)];
    World {
        a: a.into_uuid(),
        b: b.into_uuid(),
        a_orders,
        b_orders,
        product: product.into_uuid(),
        commerce,
    }
}

fn engine() -> stateset_authz::AuthzEngine {
    AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::operator())
        .assign_role("op-1", "admin")
        .assign_role("cust-a", "operator")
        .assign_role("cust-b", "operator")
        .build()
}

fn app(world: World) -> (Router, Uuid, Uuid, Vec<Uuid>, Vec<Uuid>, Uuid) {
    let World { commerce, a, b, a_orders, b_orders, product } = world;
    let app = ServerBuilder::new(commerce)
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .add_bearer_auth_for_actor("op-token", "op-1")
        .add_bearer_auth_for_customer("a-token", "cust-a", a.to_string())
        .add_bearer_auth_for_customer("b-token", "cust-b", b.to_string())
        .with_authz_engine(engine())
        .build();
    (app, a, b, a_orders, b_orders, product)
}

async fn call(
    app: &Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Option<Value>,
    extra: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-tenant-id", "default")
        .header("idempotency-key", Uuid::new_v4().to_string());
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let body = match body {
        Some(value) => {
            request = request.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn customer_reads_own_order_but_gets_404_for_anothers() {
    let (app, _a, _b, a_orders, b_orders, _) = app(world());
    let (status, body) =
        call(&app, "a-token", "GET", &format!("/api/v1/orders/{}", a_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, foreign) =
        call(&app, "a-token", "GET", &format!("/api/v1/orders/{}", b_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let missing_id = Uuid::new_v4();
    let (status, missing) =
        call(&app, "op-token", "GET", &format!("/api/v1/orders/{missing_id}"), None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Same shape and wording as the handler's own "missing" response.
    assert_eq!(foreign["error"]["code"], missing["error"]["code"]);
    assert_eq!(
        foreign["error"]["message"].as_str().unwrap().replace(&b_orders[0].to_string(), "ID"),
        missing["error"]["message"].as_str().unwrap().replace(&missing_id.to_string(), "ID"),
    );

    // The operator sees every customer's order.
    let (status, _) =
        call(&app, "op-token", "GET", &format!("/api/v1/orders/{}", b_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn customer_cannot_mutate_anothers_order_but_can_cancel_their_own() {
    let (app, _a, _b, a_orders, b_orders, _) = app(world());
    let (status, _) = call(
        &app,
        "a-token",
        "PATCH",
        &format!("/api/v1/orders/{}/cancel", b_orders[0]),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) =
        call(&app, "op-token", "GET", &format!("/api/v1/orders/{}", b_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(body["status"], "cancelled", "B's order must be untouched");

    let (status, body) = call(
        &app,
        "a-token",
        "PATCH",
        &format!("/api/v1/orders/{}/cancel", a_orders[0]),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Shipping is an operator action even on one's own order.
    let (status, _) = call(
        &app,
        "a-token",
        "PATCH",
        &format!("/api/v1/orders/{}/ship", a_orders[1]),
        Some(json!({})),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn customer_lists_are_filtered_before_pagination() {
    let (app, a, b, a_orders, _, _) = app(world());
    // Ask for B's orders, one per page: the forced filter wins, and the
    // total/has_more reflect only A's two orders.
    let (status, body) =
        call(&app, "a-token", "GET", &format!("/api/v1/orders?customer_id={b}&limit=1"), None, &[])
            .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let text = body.to_string();
    assert!(!text.contains(&b.to_string()), "{text}");
    assert!(text.contains(&a.to_string()), "{text}");
    assert_eq!(body["total"], json!(a_orders.len()), "{body}");

    let (status, body) = call(&app, "op-token", "GET", "/api/v1/orders?limit=100", None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], json!(5), "operator sees every order: {body}");
}

#[tokio::test]
async fn customer_create_is_bound_to_the_principal() {
    let (app, a, b, _, b_orders, product) = app(world());
    // customer_id omitted: forced to A.
    let (status, cart) = call(&app, "a-token", "POST", "/api/v1/carts", Some(json!({})), &[]).await;
    assert_eq!(status, StatusCode::CREATED, "{cart}");
    assert_eq!(cart["customer_id"], json!(a.to_string()));

    // Another customer's id is refused.
    let (status, _) = call(
        &app,
        "a-token",
        "POST",
        "/api/v1/orders",
        Some(json!({
            "customer_id": b.to_string(),
            "items": [{ "product_id": product.to_string(), "sku": "S", "name": "W",
                        "quantity": 1, "unit_price": "10" }]
        })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A return against B's order looks like a missing order.
    let (status, _) = call(
        &app,
        "a-token",
        "POST",
        "/api/v1/returns",
        Some(json!({ "order_id": b_orders[0].to_string(), "reason": "defective", "items": [] })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn customer_principals_are_refused_operator_and_destructive_routes() {
    let (app, _, _, _, _, _) = app(world());
    for (method, uri) in [
        ("GET", "/api/v1/customers"),
        ("GET", "/api/v1/gl/accounts"),
        ("POST", "/api/v1/inventory/SKU-1/adjust"),
        ("POST", "/api/v1/promotions"),
        ("GET", "/api/v1/shipments"),
        ("POST", "/api/v1/gl/close-month"),
    ] {
        let (status, body) = call(&app, "a-token", method, uri, Some(json!({})), &[]).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {body}");
    }
    let refund = format!("/api/v1/payments/{}/refund", Uuid::new_v4());
    let (status, _) =
        call(&app, "a-token", "POST", &refund, Some(json!({ "amount": "1" })), &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Catalog reads stay open.
    let (status, _) = call(&app, "a-token", "GET", "/api/v1/products", None, &[]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_header_cannot_change_the_customer_principal() {
    let (app, _, _, _, b_orders, _) = app(world());
    // Claiming the operator (or customer B) with A's token is rejected outright.
    for spoof in ["op-1", "cust-b"] {
        let (status, _) = call(
            &app,
            "a-token",
            "GET",
            &format!("/api/v1/orders/{}", b_orders[0]),
            None,
            &[("x-actor-id", spoof)],
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "spoofed actor {spoof}");
    }
}

#[tokio::test]
async fn ownership_is_enforced_without_an_authz_engine() {
    let World { commerce, a, b_orders, .. } = world();
    let app = ServerBuilder::new(commerce)
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .add_bearer_auth("op-token")
        .add_bearer_auth_for_customer("a-token", "cust-a", a.to_string())
        .build();
    let (status, _) =
        call(&app, "a-token", "GET", &format!("/api/v1/orders/{}", b_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) =
        call(&app, "op-token", "GET", &format!("/api/v1/orders/{}", b_orders[0]), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn gateway_asserted_actor_is_scoped_by_server_side_binding() {
    let World { commerce, a, a_orders, b_orders, .. } = world();
    let app = ServerBuilder::new(commerce)
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .add_bearer_auth("gateway-token")
        .with_authz_engine(engine())
        .trust_actor_headers_for_authz()
        .with_customer_principal("cust-a", a.to_string())
        .build();
    let get = |id: Uuid| format!("/api/v1/orders/{id}");
    let as_a = [("x-actor-id", "cust-a")];
    let (status, _) = call(&app, "gateway-token", "GET", &get(a_orders[0]), None, &as_a).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "gateway-token", "GET", &get(b_orders[0]), None, &as_a).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let as_op = [("x-actor-id", "op-1")];
    let (status, _) = call(&app, "gateway-token", "GET", &get(b_orders[0]), None, &as_op).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn invalid_customer_binding_fails_closed() {
    let app = ServerBuilder::new(Commerce::new(":memory:").unwrap())
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .add_bearer_auth_for_customer("a-token", "cust-a", "not-a-uuid")
        .build();
    let (status, _) = call(&app, "a-token", "GET", "/api/v1/products", None, &[]).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    // Customer principals with nothing to establish the actor are refused too.
    let app = ServerBuilder::new(Commerce::new(":memory:").unwrap())
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .with_customer_principal("cust-a", Uuid::new_v4().to_string())
        .build();
    let response =
        app.oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn customer_reads_back_a_warranty_created_on_their_own_order() {
    // The route takes only `order_id`; the warranty used to be stored with a
    // nil customer, so its own buyer got 404 on it and an empty list.
    let (app, _a, _b, a_orders, _b_orders, product) = app(world());
    let body = json!({ "order_id": a_orders[0], "product_id": product, "duration_months": 12 });
    let (status, created) =
        call(&app, "a-token", "POST", "/api/v1/warranties", Some(body), &[]).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("warranty id").to_string();

    let (status, read) =
        call(&app, "a-token", "GET", &format!("/api/v1/warranties/{id}"), None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    let (status, listed) = call(&app, "a-token", "GET", "/api/v1/warranties", None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["total"], 1, "{listed}");

    // The other customer still cannot see it.
    let (status, _) =
        call(&app, "b-token", "GET", &format!("/api/v1/warranties/{id}"), None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
