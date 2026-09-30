//! Exercise actual public routes and permission boundaries against the native engine.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, PermissionLevel, Role, RoleBuilder};
use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, ProductId};
use stateset_embedded::Commerce;
use stateset_http::{AppState, ServerBuilder};
use tower::ServiceExt;

fn seed_order(commerce: &Commerce) -> String {
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "shipment-http@example.com".into(),
            first_name: "Ada".into(),
            last_name: "L".into(),
            ..Default::default()
        })
        .unwrap();
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "W-1".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: rust_decimal::Decimal::ONE,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    order.id.to_string()
}

fn fixture() -> (Router, String) {
    let commerce = Commerce::new(":memory:").unwrap();
    let order = seed_order(&commerce);
    let auth = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .add_role(RoleBuilder::new("writer").default_level(PermissionLevel::Write).build())
        .assign_role("admin", "admin")
        .assign_role("viewer", "viewer")
        .assign_role("writer", "writer")
        .build();
    let app = ServerBuilder::new(commerce)
        .without_auth()
        .with_ignore_tenant_header()
        .add_bearer_auth_for_actor("admin-token", "admin")
        .add_bearer_auth_for_actor("writer-token", "writer")
        .add_bearer_auth_for_actor("viewer-token", "viewer")
        .with_authz_engine(auth)
        .build();
    (app, order)
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .header("x-tenant-id", if token == "other-token" { "tenant-b" } else { "tenant-a" })
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) })),
    )
}
async fn create(app: &Router, order: &str) -> Value {
    let (status, body) = request(
        app,
        "POST",
        "/shipments",
        "writer-token",
        json!({
            "order_id": order, "recipient_name": "Ada", "shipping_address": "1 Main",
            "shipping_method": "ground", "carrier": "ups", "recipient_email": "ada@example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["shipping_method"], "ground");
    assert_eq!(body["shipping_address"], "1 Main");
    assert_eq!(body["version"], 1);
    body
}

#[tokio::test]
async fn public_shipment_lifecycle_retains_versions_timestamps_and_fields() {
    let (app, order) = fixture();
    let shipment = create(&app, &order).await;
    let path = format!("/shipments/{}", shipment["id"].as_str().unwrap());
    let (status, _) =
        request(&app, "POST", &format!("{path}/deliver"), "writer-token", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    for (index, state) in
        ["processing", "ready_to_ship", "shipped", "in_transit", "out_for_delivery"]
            .iter()
            .enumerate()
    {
        let (status, body) = request(
            &app,
            "PATCH",
            &path,
            "writer-token",
            json!({"status": state, "expected_version": index + 1}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["version"], index + 2);
        assert_eq!(body["recipient_email"], "ada@example.com");
    }
    let (status, delivered) =
        request(&app, "POST", &format!("{path}/deliver"), "writer-token", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{delivered}");
    assert_eq!(delivered["version"], 7);
    assert!(delivered["shipped_at"].is_string());
    assert!(delivered["delivered_at"].is_string());
    let (_, replay) = request(
        &app,
        "PATCH",
        &path,
        "writer-token",
        json!({"status": "delivered", "expected_version": 7}),
    )
    .await;
    assert_eq!(replay, delivered);
    let (status, _) =
        request(&app, "POST", &format!("{path}/cancel"), "admin-token", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn shipment_patch_validates_fields_versions_and_exact_money_atomically() {
    let (app, order) = fixture();
    let shipment = create(&app, &order).await;
    let path = format!("/shipments/{}", shipment["id"].as_str().unwrap());
    for patch in [
        json!({"status": "delivered", "notes": "must roll back"}),
        json!({"shipping_cost": "-1"}),
        json!({"shipping_cost": 1.25}),
        json!({"shipping_cost": "0.00000000000000000000000000001"}),
        json!({"status": "invented"}),
        json!({"ignored_field": "must fail"}),
    ] {
        let (status, _) = request(&app, "PATCH", &path, "writer-token", patch).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (_, stored) = request(&app, "GET", &path, "viewer-token", json!({})).await;
        assert_eq!(stored, shipment);
    }
    let (status, updated) = request(&app, "PATCH", &path, "writer-token", json!({"expected_version": 1, "notes": "packed", "shipping_cost": "9007199254740993.01", "weight_kg": "0.125"})).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["shipping_cost"], "9007199254740993.01");
    assert_eq!(updated["weight_kg"], "0.125");
    let (status, _) = request(
        &app,
        "PATCH",
        &path,
        "writer-token",
        json!({"expected_version": 1, "notes": "stale"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, stored) = request(&app, "GET", &path, "viewer-token", json!({})).await;
    assert_eq!(stored, updated);
    let (status, _) = request(
        &app,
        "PATCH",
        "/shipments/00000000-0000-4000-8000-000000000000",
        "writer-token",
        json!({"notes": "absent"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn shipment_cancellation_cannot_bypass_delete_permission() {
    let (app, order) = fixture();
    let shipment = create(&app, &order).await;
    let path = format!("/shipments/{}", shipment["id"].as_str().unwrap());
    for status in ["cancelled", "canceled", "CANCELLED"] {
        let (code, _) =
            request(&app, "PATCH", &path, "writer-token", json!({"status": status})).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (code, _) =
        request(&app, "PATCH", &path, "viewer-token", json!({"status": "processing"})).await;
    assert_eq!(code, StatusCode::FORBIDDEN);
    let (code, _) =
        request(&app, "POST", &format!("{path}/cancel"), "writer-token", json!({})).await;
    assert_eq!(code, StatusCode::FORBIDDEN);
    let (code, cancelled) = request(
        &app,
        "POST",
        &format!("{path}/cancel"),
        "admin-token",
        json!({"expected_version": 1}),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["version"], 2);
    let (_, stored) = request(&app, "GET", &path, "viewer-token", json!({})).await;
    assert_eq!(stored, cancelled);
}

#[tokio::test]
async fn shipment_routes_respect_tenant_bound_tokens() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(
        std::env::temp_dir().join(format!("shipment-http-tenants-{}", uuid::Uuid::new_v4())),
    );
    let state = AppState::new(Commerce::new(":memory:").unwrap()).with_tenant_db_dir(&directory.0);
    let tenant_a = state.commerce_for_tenant(Some("tenant-a")).unwrap();
    let order = seed_order(&tenant_a);
    let auth =
        AuthzEngineBuilder::new().add_role(Role::admin()).assign_role("operator", "admin").build();
    let app = ServerBuilder::new(Commerce::new(":memory:").unwrap())
        .without_auth()
        .with_tenant_db_dir(&directory.0)
        .add_bearer_auth_for_actor_and_tenant("writer-token", "operator", "tenant-a")
        .add_bearer_auth_for_actor_and_tenant("other-token", "operator", "tenant-b")
        .with_authz_engine(auth)
        .build();
    let initial = create(&app, &order).await;
    let path = format!("/shipments/{}", initial["id"].as_str().unwrap());
    for (method, suffix, body) in [
        ("GET", "", json!({})),
        ("PATCH", "", json!({"status":"processing"})),
        ("POST", "/cancel", json!({})),
    ] {
        let (status, result) =
            request(&app, method, &format!("{path}{suffix}"), "other-token", body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{result}");
    }
    let (_, stored) = request(&app, "GET", &path, "writer-token", json!({})).await;
    assert_eq!(stored, initial);
    let forged = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1{path}"))
                .header("authorization", "Bearer writer-token")
                .header("x-tenant-id", "tenant-b")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"processing"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let (status, changed) = request(
        &app,
        "PATCH",
        &path,
        "writer-token",
        json!({"status":"processing", "expected_version":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["version"], 2);
}
