//! `/a2a/credit` and `/a2a/messages` are backed by the tenant database, not
//! process memory: state survives a rebuilt router over the same `Commerce`,
//! is tenant-scoped, and charges are refused past the limit.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use stateset_embedded::Commerce;
use stateset_http::AppState;
use tower::ServiceExt;

fn state() -> AppState {
    AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
        .with_ignore_tenant_header()
}

fn router(state: &AppState) -> axum::Router {
    stateset_http::routes::api_router().with_state(state.clone())
}

async fn body_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

async fn post(
    router: &axum::Router,
    path: &str,
    tenant: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut req = Request::post(path).header("content-type", "application/json");
    if let Some(t) = tenant {
        req = req.header("x-tenant-id", t);
    }
    router.clone().oneshot(req.body(Body::from(body.to_string())).unwrap()).await.unwrap()
}

async fn get(router: &axum::Router, path: &str, tenant: Option<&str>) -> axum::response::Response {
    let mut req = Request::get(path);
    if let Some(t) = tenant {
        req = req.header("x-tenant-id", t);
    }
    router.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
}

#[tokio::test]
async fn credit_terms_are_durable_tenant_scoped_and_limit_enforced() {
    let state = state();
    let app = router(&state);

    let created = post(
        &app,
        "/api/v1/a2a/credit",
        Some("tenant-a"),
        json!({"creditor_agent_id": "seller", "debtor_agent_id": "buyer",
               "credit_limit": "1000.00", "payment_terms": "net_60"}),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let terms = body_json(created).await;
    let id = terms["id"].as_str().unwrap().to_string();
    assert_eq!(terms["tenant_id"], "tenant-a");
    assert_eq!(terms["payment_terms"], "net_60");
    assert_eq!(terms["available_credit"], "1000.00");

    // Invalid payment terms are a 400.
    let bad = post(
        &app,
        "/api/v1/a2a/credit",
        Some("tenant-a"),
        json!({"creditor_agent_id": "s", "debtor_agent_id": "b", "credit_limit": "1", "payment_terms": "net_45"}),
    )
    .await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    // A brand-new router over the same Commerce still sees the terms.
    let app2 = router(&state);
    let fetched = get(&app2, &format!("/api/v1/a2a/credit/{id}"), Some("tenant-a")).await;
    assert_eq!(fetched.status(), StatusCode::OK);

    // Another tenant cannot see or charge it.
    let other = get(&app2, &format!("/api/v1/a2a/credit/{id}"), Some("tenant-b")).await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
    let other = post(
        &app2,
        &format!("/api/v1/a2a/credit/{id}/charge"),
        Some("tenant-b"),
        json!({"amount": "1"}),
    )
    .await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
    let listed = body_json(get(&app2, "/api/v1/a2a/credit", Some("tenant-b")).await).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(0));

    // Charge, refuse past the limit, pay down, journal.
    let charged = post(
        &app2,
        &format!("/api/v1/a2a/credit/{id}/charge"),
        Some("tenant-a"),
        json!({"amount": "600", "reference_id": "po-1"}),
    )
    .await;
    assert_eq!(charged.status(), StatusCode::OK);
    assert_eq!(body_json(charged).await["outstanding_balance"], "600");
    let over = post(
        &app2,
        &format!("/api/v1/a2a/credit/{id}/charge"),
        Some("tenant-a"),
        json!({"amount": "400.01"}),
    )
    .await;
    assert_eq!(over.status(), StatusCode::BAD_REQUEST);
    let paid = post(
        &app2,
        &format!("/api/v1/a2a/credit/{id}/payment"),
        Some("tenant-a"),
        json!({"amount": "100"}),
    )
    .await;
    assert_eq!(paid.status(), StatusCode::OK);
    assert_eq!(body_json(paid).await["available_credit"], "500.00");
    let entries =
        body_json(get(&app2, &format!("/api/v1/a2a/credit/{id}/entries"), Some("tenant-a")).await)
            .await;
    let entries = entries.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["entry_type"], "charge");
    assert_eq!(entries[0]["reference_id"], "po-1");
    assert_eq!(entries[1]["entry_type"], "payment");
    let listed =
        body_json(get(&app2, "/api/v1/a2a/credit?status=active", Some("tenant-a")).await).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn messages_are_durable_sequenced_tenant_scoped_and_acknowledgeable() {
    let state = state();
    let app = router(&state);
    let a = uuid::Uuid::new_v4().to_string();
    let b = uuid::Uuid::new_v4().to_string();

    let sent = post(
        &app,
        "/api/v1/a2a/messages",
        Some("tenant-a"),
        json!({"from_agent_id": a, "to_agent_id": b, "message_type": "quote_request",
               "payload": {"sku": "X", "qty": 2}}),
    )
    .await;
    assert_eq!(sent.status(), StatusCode::CREATED);
    let first = body_json(sent).await;
    let conversation = first["conversation_id"].as_str().unwrap().to_string();
    let first_id = first["id"].as_str().unwrap().to_string();
    assert_eq!(first["sequence_number"], 1);
    assert_eq!(first["status"], "pending");
    assert_eq!(first["payload"]["qty"], 2);

    let bad = post(
        &app,
        "/api/v1/a2a/messages",
        Some("tenant-a"),
        json!({"from_agent_id": "nope", "to_agent_id": b, "message_type": "x", "payload": {}}),
    )
    .await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    let app2 = router(&state);
    let reply = body_json(
        post(
            &app2,
            "/api/v1/a2a/messages",
            Some("tenant-a"),
            json!({"from_agent_id": b, "to_agent_id": a, "message_type": "quote_response",
                   "payload": {"price": "10.00"}, "conversation_id": conversation}),
        )
        .await,
    )
    .await;
    assert_eq!(reply["sequence_number"], 2);
    assert_eq!(reply["conversation_id"], conversation);

    // Pending inbox for b (durable across routers), tenant-scoped.
    let inbox = body_json(
        get(&app2, &format!("/api/v1/a2a/messages?to_agent_id={b}"), Some("tenant-a")).await,
    )
    .await;
    assert_eq!(inbox.as_array().unwrap().len(), 1);
    assert_eq!(inbox[0]["id"], first_id);
    let other = body_json(
        get(&app2, &format!("/api/v1/a2a/messages?to_agent_id={b}"), Some("tenant-b")).await,
    )
    .await;
    assert_eq!(other.as_array().map(Vec::len), Some(0));
    let other = post(
        &app2,
        &format!("/api/v1/a2a/messages/{first_id}/acknowledge"),
        Some("tenant-b"),
        json!({}),
    )
    .await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);

    let acked = post(
        &app2,
        &format!("/api/v1/a2a/messages/{first_id}/acknowledge"),
        Some("tenant-a"),
        json!({}),
    )
    .await;
    assert_eq!(acked.status(), StatusCode::OK);
    assert_eq!(body_json(acked).await["status"], "acknowledged");
    let inbox = body_json(
        get(&app2, &format!("/api/v1/a2a/messages?to_agent_id={b}"), Some("tenant-a")).await,
    )
    .await;
    assert_eq!(inbox.as_array().map(Vec::len), Some(0));
    let all = body_json(
        get(
            &app2,
            &format!("/api/v1/a2a/messages?conversation_id={conversation}&status=all"),
            Some("tenant-a"),
        )
        .await,
    )
    .await;
    assert_eq!(all.as_array().map(Vec::len), Some(2));

    let failed = post(
        &app2,
        &format!("/api/v1/a2a/messages/{}/fail", reply["id"].as_str().unwrap()),
        Some("tenant-a"),
        json!({"error": "timeout"}),
    )
    .await;
    assert_eq!(failed.status(), StatusCode::OK);
    let failed = body_json(failed).await;
    assert_eq!(failed["attempts"], 1);
    assert_eq!(failed["error"], "timeout");
    let fetched =
        body_json(get(&app2, &format!("/api/v1/a2a/messages/{first_id}"), Some("tenant-a")).await)
            .await;
    assert_eq!(fetched["status"], "acknowledged");
}
