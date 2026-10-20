//! Recovery contracts through authenticated public routes and file-backed tenants.
use std::path::Path;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, Role};
use stateset_embedded::{Commerce, maintenance::RestoreOptions};
use stateset_http::ServerBuilder;
use tower::ServiceExt;

fn app(directory: &Path) -> Router {
    let authz = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .assign_role("operator", "admin")
        .assign_role("reader", "viewer")
        .build();
    // The default store is intentionally ephemeral. All durable state must
    // follow the authenticated tenant into its own database.
    ServerBuilder::new(Commerce::in_memory().unwrap())
        .without_auth()
        .with_tenant_db_dir(directory)
        .add_bearer_auth_for_actor_and_tenant("operator-a", "operator", "tenant-a")
        .add_bearer_auth_for_actor_and_tenant("operator-b", "operator", "tenant-b")
        .add_bearer_auth_for_actor_and_tenant("reader-a", "reader", "tenant-a")
        .with_authz_engine(authz)
        .build()
}

async fn post(
    app: &Router,
    path: &str,
    key: &str,
    token: &str,
    payload: &Value,
) -> (StatusCode, Option<String>, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1{path}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", if token == "operator-b" { "tenant-b" } else { "tenant-a" })
                .header("content-type", "application/json")
                .header("idempotency-key", key)
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let replayed = response
        .headers()
        .get("idempotency-replayed")
        .map(|value| value.to_str().unwrap().to_owned());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, replayed, body)
}

async fn create(app: &Router, path: &str, key: &str, payload: &Value) -> Vec<u8> {
    let (status, replayed, body) = post(app, path, key, "operator-a", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{}", String::from_utf8_lossy(&body));
    assert_eq!(replayed.as_deref(), Some("false"));
    body
}

fn customer() -> Value {
    json!({"email": "recovery@example.com", "first_name": "Ada", "last_name": "L"})
}

fn order(customer: &Value) -> Value {
    json!({"customer_id": customer["id"], "items": [{
        "product_id": stateset_primitives::ProductId::new(),
        "sku": "RECOVERY-ITEM", "name": "Recovery item", "quantity": 1,
        "unit_price": "19.99"
    }]})
}

#[tokio::test]
async fn tenant_commerce_replays_after_restart_and_backup_restore_without_bypassing_auth() {
    let directory = tempfile::tempdir().unwrap();
    let router = app(directory.path());
    let customer_payload = customer();
    let customer_body = create(&router, "/customers", "customer", &customer_payload).await;
    let customer: Value = serde_json::from_slice(&customer_body).unwrap();
    let order_payload = order(&customer);
    let order_body = create(&router, "/orders", "order", &order_payload).await;
    let order: Value = serde_json::from_slice(&order_body).unwrap();
    assert_eq!(order["total_amount"], "19.99");
    let payment_payload = json!({"order_id": order["id"], "amount": "19.99", "currency": "USD"});
    let payment_body = create(&router, "/payments", "payment", &payment_payload).await;
    let payment: Value = serde_json::from_slice(&payment_body).unwrap();
    assert_eq!(payment["amount"], "19.99");
    let payment_path = format!("/payments/{}", payment["id"].as_str().unwrap());
    let (status, _, body) =
        post(&router, &format!("{payment_path}/complete"), "complete", "operator-a", &json!({}))
            .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let refund_path = format!("{payment_path}/refund");
    let refund_payload = json!({"amount": "5.01", "reason": "partial return"});
    let refund_body = create(&router, &refund_path, "refund", &refund_payload).await;

    // Identical keys and bodies in a different tenant must execute independently.
    let (status, replayed, other) =
        post(&router, "/customers", "customer", "operator-b", &customer_payload).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(replayed.as_deref(), Some("false"));
    assert_ne!(serde_json::from_slice::<Value>(&other).unwrap()["id"], customer["id"]);

    let records = [
        ("/customers", "customer", customer_payload, customer_body),
        ("/orders", "order", order_payload, order_body),
        ("/payments", "payment", payment_payload, payment_body),
        (refund_path.as_str(), "refund", refund_payload, refund_body),
    ];
    drop(router);
    let restarted = app(directory.path());
    for (path, key, payload, expected) in &records {
        let (status, replayed, actual) = post(&restarted, path, key, "operator-a", payload).await;
        assert_eq!(status, StatusCode::CREATED, "{path}: {}", String::from_utf8_lossy(&actual));
        assert_eq!(replayed.as_deref(), Some("true"), "{path} must replay after restart");
        assert_eq!(&actual, expected);
        // Cached responses must still pass current authentication/authorization.
        for (token, expected_status) in
            [("reader-a", StatusCode::FORBIDDEN), ("invalid", StatusCode::UNAUTHORIZED)]
        {
            let (status, replayed, _) = post(&restarted, path, key, token, payload).await;
            assert_eq!(status, expected_status);
            assert!(replayed.is_none());
        }
    }
    drop(restarted);

    let store_path = directory.path().join("tenant-a.db");
    let commerce = Commerce::new(store_path.to_str().unwrap()).unwrap();
    let backup = directory.path().join("backup.db");
    commerce.maintenance().backup_to(&backup).unwrap();
    let recovered = tempfile::tempdir().unwrap();
    commerce
        .maintenance()
        .restore_from(&backup, recovered.path().join("tenant-a.db"), &RestoreOptions::default())
        .unwrap();
    drop(commerce);
    let restored = app(recovered.path());
    for (path, key, payload, expected) in &records {
        let (status, replayed, actual) = post(&restored, path, key, "operator-a", payload).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(replayed.as_deref(), Some("true"));
        assert_eq!(&actual, expected);
    }
    let connection = rusqlite::Connection::open(recovered.path().join("tenant-a.db")).unwrap();
    for (table, expected) in
        [("orders", 1), ("payments", 1), ("refunds", 1), ("http_idempotency_keys", 4)]
    {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, expected, "{table} must not be duplicated by replay");
    }
    let integrity: String =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    println!(
        "tenant-recovery-proof restart=replayed backup=replayed authorization=enforced orders=1 payments=1 refunds=1 integrity=ok"
    );
}

#[tokio::test]
async fn tenant_payment_receipt_failure_survives_restart_without_a_second_charge() {
    let directory = tempfile::tempdir().unwrap();
    let router = app(directory.path());
    let customer: Value =
        serde_json::from_slice(&create(&router, "/customers", "customer", &customer()).await)
            .unwrap();
    let order: Value =
        serde_json::from_slice(&create(&router, "/orders", "order", &order(&customer)).await)
            .unwrap();
    let connection = rusqlite::Connection::open(directory.path().join("tenant-a.db")).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_payment_receipt BEFORE UPDATE ON http_idempotency_keys
        WHEN NEW.idempotency_key = 'charge-once'
        BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
        )
        .unwrap();
    let payload = json!({"order_id": order["id"], "amount": "19.99", "currency": "USD"});
    let (status, _, _) = post(&router, "/payments", "charge-once", "operator-a", &payload).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    connection.execute_batch("DROP TRIGGER fail_payment_receipt").unwrap();
    drop(router);
    let restarted = app(directory.path());
    let (status, _, body) =
        post(&restarted, "/payments", "charge-once", "operator-a", &payload).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
        "idempotency_in_progress"
    );
    let count: i64 =
        connection.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);
    let status: i64 = connection.query_row("SELECT response_status FROM http_idempotency_keys WHERE idempotency_key = 'charge-once'", [], |row| row.get(0)).unwrap();
    assert_eq!(status, 0);
    println!("tenant-receipt-failure-proof payments=1 reservation=pending");
}
