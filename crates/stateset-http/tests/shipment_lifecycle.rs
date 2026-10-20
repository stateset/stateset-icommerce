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
    request_with_key(app, method, path, token, body, None).await
}

async fn request_with_key(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/v1{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("x-tenant-id", if token == "other-token" { "tenant-b" } else { "tenant-a" });
    if let Some(key) = key {
        builder = builder.header("Idempotency-Key", key);
    }
    let response =
        app.clone().oneshot(builder.body(Body::from(body.to_string())).unwrap()).await.unwrap();
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
        ("POST", "/items", json!({"sku":"W-1", "name":"Widget", "quantity":1})),
        ("DELETE", "/items/00000000-0000-4000-8000-000000000001", json!({})),
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

#[tokio::test]
async fn shipment_creation_returns_normalized_items_and_replays_without_reallocation() {
    let (app, order) = fixture();
    let body = json!({"order_id":order, "recipient_name":"Ada", "shipping_address":"1 Main",
        "items":[{"sku":"W-1", "name":"Widget", "quantity":1}]});
    let (status, created) = request_with_key(
        &app,
        "POST",
        "/shipments",
        "writer-token",
        body.clone(),
        Some("manifest-create"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["items"][0]["quantity"], 1);
    assert!(created["items"][0]["order_item_id"].is_string());
    assert!(created["items"][0]["product_id"].is_string());
    assert_eq!(created["items"][0]["shipment_id"], created["id"]);
    let replay = request_with_key(
        &app,
        "POST",
        "/shipments",
        "writer-token",
        body.clone(),
        Some("manifest-create"),
    )
    .await;
    assert_eq!(replay, (StatusCode::CREATED, created.clone()));
    let mut changed = body.clone();
    changed["items"][0]["quantity"] = json!(2);
    assert_eq!(
        request_with_key(
            &app,
            "POST",
            "/shipments",
            "writer-token",
            changed,
            Some("manifest-create")
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, rejected) = request(&app, "POST", "/shipments", "writer-token", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected}");
    let (_, listed) = request(&app, "GET", "/shipments", "viewer-token", json!({})).await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["shipments"][0]["items"], created["items"]);
}

#[tokio::test]
async fn manifest_item_endpoints_enforce_permissions_parent_scope_and_packing_lifecycle() {
    let (app, order) = fixture();
    let first = create(&app, &order).await;
    let second = create(&app, &order).await;
    let path = format!("/shipments/{}", first["id"].as_str().unwrap());
    let item_path = format!("{path}/items");
    let item = json!({"sku":"W-1", "name":"Widget", "quantity":1});
    assert_eq!(
        request(&app, "POST", &item_path, "viewer-token", item.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "POST", &item_path, "unknown-token", item.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, added) = request_with_key(
        &app,
        "POST",
        &item_path,
        "writer-token",
        item.clone(),
        Some("manifest-add"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    assert_eq!(
        request_with_key(
            &app,
            "POST",
            &item_path,
            "writer-token",
            item.clone(),
            Some("manifest-add")
        )
        .await,
        (StatusCode::CREATED, added.clone())
    );
    assert_eq!(
        request_with_key(
            &app,
            "POST",
            &item_path,
            "viewer-token",
            item.clone(),
            Some("manifest-add")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, packed) = request(&app, "GET", &path, "viewer-token", json!({})).await;
    assert_eq!(packed["version"], 2);
    assert_eq!(packed["items"], json!([added.clone()]));
    let delete_path = format!("{item_path}/{}", added["id"].as_str().unwrap());
    assert_eq!(
        request(&app, "DELETE", &delete_path, "writer-token", json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let wrong_parent = format!(
        "/shipments/{}/items/{}",
        second["id"].as_str().unwrap(),
        added["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&app, "DELETE", &wrong_parent, "admin-token", json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(request(&app, "GET", &path, "viewer-token", json!({})).await.1, packed);
    assert_eq!(
        request_with_key(
            &app,
            "DELETE",
            &delete_path,
            "admin-token",
            json!({}),
            Some("manifest-remove")
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request_with_key(
            &app,
            "DELETE",
            &delete_path,
            "admin-token",
            json!({}),
            Some("manifest-remove")
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, empty) = request(&app, "GET", &path, "viewer-token", json!({})).await;
    assert_eq!(
        request_with_key(
            &app,
            "DELETE",
            &delete_path,
            "writer-token",
            json!({}),
            Some("manifest-remove")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(empty["version"], 3);
    assert_eq!(empty["items"], json!([]));
    let (status, added) = request(&app, "POST", &item_path, "writer-token", item.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    for status in ["processing", "ready_to_ship"] {
        assert_eq!(
            request(&app, "PATCH", &path, "writer-token", json!({"status":status})).await.0,
            StatusCode::OK
        );
    }
    // Removal must refuse even though this caller has delete permission.
    let delete_path = format!("{item_path}/{}", added["id"].as_str().unwrap());
    assert_eq!(
        request(&app, "DELETE", &delete_path, "admin-token", json!({})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(&app, "POST", &item_path, "writer-token", item).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, ready) = request(&app, "GET", &path, "viewer-token", json!({})).await;
    assert_eq!(ready["version"], 6);
    assert_eq!(ready["items"], json!([added]));
}

#[tokio::test]
async fn invalid_manifest_inputs_do_not_create_partial_shipments() {
    let (app, order) = fixture();
    for invalid in [
        json!({"sku":"W-1", "name":"Widget", "quantity":0}),
        json!({"sku":"W-1", "name":"Widget", "quantity":1.5}),
        json!({"sku":"W-1", "name":"Widget", "quantity":2147483648_u64}),
        json!({"sku":"W-1", "name":"Widget", "quantity":2}),
        json!({"sku":"wrong", "name":"Widget", "quantity":1}),
        json!({"sku":"W-1", "name":"Widget", "quantity":1, "order_item_id":uuid::Uuid::new_v4()}),
        json!({"sku":"W-1", "name":"Widget", "quantity":1, "product_id":uuid::Uuid::new_v4()}),
        json!({"sku":"W-1", "name":"Widget", "quantity":1, "unexpected":true}),
    ] {
        let body = json!({"order_id":order, "items":[invalid]});
        let (status, response) = request(&app, "POST", "/shipments", "writer-token", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
    }
    let (_, list) = request(&app, "GET", "/shipments", "viewer-token", json!({})).await;
    assert_eq!(list["total"], 0);
    let body = json!({"order_id":order, "items":[{"sku":"W-1", "name":"Widget", "quantity":1}]});
    assert_eq!(
        request(&app, "POST", "/shipments", "writer-token", body).await.0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn manifest_http_writes_roll_back_when_outbox_persistence_fails() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("store.db");
    let commerce = Commerce::new(path.to_str().unwrap()).unwrap();
    let order = seed_order(&commerce);
    let app = ServerBuilder::new(commerce).without_auth().with_ignore_tenant_header().build();
    let db = rusqlite::Connection::open(path).unwrap();
    let initial = create(&app, &order).await;
    let shipment_path = format!("/shipments/{}", initial["id"].as_str().unwrap());
    let items_path = format!("{shipment_path}/items");
    let trigger = "CREATE TRIGGER refuse_manifest_fact BEFORE INSERT ON kernel_outbox WHEN NEW.aggregate_type = 'shipment' BEGIN INSERT INTO unavailable_manifest_sink (id) VALUES (NEW.id); END";
    let item = json!({"sku":"W-1", "name":"Widget", "quantity":1});
    let fact_count = || -> i64 {
        db.query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_type = 'shipment'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    let before_facts = fact_count();
    db.execute_batch(trigger).unwrap();
    let create_body = json!({"order_id":order, "items":[item.clone()]});
    assert_eq!(
        request(&app, "POST", "/shipments", "writer-token", create_body).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        request(&app, "POST", &items_path, "writer-token", item.clone()).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(request(&app, "GET", &shipment_path, "writer-token", json!({})).await.1, initial);
    assert_eq!(request(&app, "GET", "/shipments", "writer-token", json!({})).await.1["total"], 1);
    assert_eq!(fact_count(), before_facts);
    db.execute_batch("DROP TRIGGER refuse_manifest_fact").unwrap();
    let (status, added) = request(&app, "POST", &items_path, "writer-token", item).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    let packed = request(&app, "GET", &shipment_path, "writer-token", json!({})).await.1;
    db.execute_batch(trigger).unwrap();
    let delete_path = format!("{items_path}/{}", added["id"].as_str().unwrap());
    assert_eq!(
        request(&app, "DELETE", &delete_path, "admin-token", json!({})).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(request(&app, "GET", &shipment_path, "writer-token", json!({})).await.1, packed);
    assert_eq!(fact_count(), before_facts + 1);
}

#[tokio::test]
async fn packing_preconditions_reject_stale_edits_and_bind_idempotent_replays() {
    let (app, order) = fixture();
    let shipment = create(&app, &order).await;
    let path = format!("/shipments/{}", shipment["id"].as_str().unwrap());
    let add_path = format!("{path}/items?expected_version=1");
    let body = json!({"sku":"W-1", "name":"Widget", "quantity":1});
    let added = request_with_key(
        &app,
        "POST",
        &add_path,
        "writer-token",
        body.clone(),
        Some("versioned-add"),
    )
    .await;
    assert_eq!(added.0, StatusCode::CREATED, "{:?}", added.1);
    assert_eq!(
        request_with_key(
            &app,
            "POST",
            &add_path,
            "writer-token",
            body.clone(),
            Some("versioned-add")
        )
        .await,
        added
    );
    // Same contents and key but a different precondition must not replay success.
    for suffix in ["?expected_version=2", ""] {
        assert_eq!(
            request_with_key(
                &app,
                "POST",
                &format!("{path}/items{suffix}"),
                "writer-token",
                body.clone(),
                Some("versioned-add")
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(&app, "POST", &add_path, "writer-token", body.clone()).await.0,
        StatusCode::CONFLICT
    );
    let delete_path = format!("{path}/items/{}", added.1["id"].as_str().unwrap());
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("{delete_path}?expected_version=1"),
            "admin-token",
            json!(null)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let current = request(&app, "GET", &path, "viewer-token", json!(null)).await.1;
    assert_eq!(current["version"], 2);
    assert_eq!(current["items"].as_array().unwrap().len(), 1);
    let versioned_delete = format!("{delete_path}?expected_version=2");
    for _ in 0..2 {
        assert_eq!(
            request_with_key(
                &app,
                "DELETE",
                &versioned_delete,
                "admin-token",
                json!(null),
                Some("versioned-delete")
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        request_with_key(
            &app,
            "DELETE",
            &format!("{delete_path}?expected_version=3"),
            "admin-token",
            json!(null),
            Some("versioned-delete")
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let current = request(&app, "GET", &path, "viewer-token", json!(null)).await.1;
    assert_eq!(current["version"], 3);
    assert!(current["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn packing_preconditions_reject_malformed_or_misspelled_query_fields() {
    let (app, order) = fixture();
    let shipment = create(&app, &order).await;
    let path = format!("/shipments/{}", shipment["id"].as_str().unwrap());
    let body = json!({"sku":"W-1", "name":"Widget", "quantity":1});
    let added =
        request(&app, "POST", &format!("{path}/items"), "writer-token", body.clone()).await.1;
    for query in [
        "expected_version=1.5",
        "expected_version=2147483648",
        "expected_version=",
        "expectedVersion=2",
        "expected_version=2&expected_version=3",
    ] {
        assert_eq!(
            request(&app, "POST", &format!("{path}/items?{query}"), "writer-token", body.clone())
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
        assert_eq!(
            request(
                &app,
                "DELETE",
                &format!("{path}/items/{}?{query}", added["id"].as_str().unwrap()),
                "admin-token",
                json!(null)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
    let current = request(&app, "GET", &path, "viewer-token", json!(null)).await.1;
    assert_eq!(current["version"], 2);
    assert_eq!(current["items"].as_array().unwrap().len(), 1);
}
