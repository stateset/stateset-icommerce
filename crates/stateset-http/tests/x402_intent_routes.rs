//! `/api/v1/x402/intents` — the payment-intent lifecycle at the API boundary.
//!
//! x402 had no HTTP surface at all, so the two guards that make intents safe
//! were never exercised through the API: reconciliation of the amount against
//! the cart or order an intent claims, and the "at most one claiming intent
//! per cart/order" rule that stops a double charge. These tests drive both
//! through the router.

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

async fn post(router: &axum::Router, path: &str, body: Value) -> axum::response::Response {
    let request = Request::post(path)
        .header("content-type", "application/json")
        .header("x-tenant-id", "tenant-x402")
        .body(Body::from(body.to_string()))
        .unwrap();
    router.clone().oneshot(request).await.unwrap()
}

async fn get(router: &axum::Router, path: &str) -> axum::response::Response {
    let request =
        Request::get(path).header("x-tenant-id", "tenant-x402").body(Body::empty()).unwrap();
    router.clone().oneshot(request).await.unwrap()
}

fn intent_body(payer: &str) -> Value {
    json!({
        "payer_address": payer,
        "payee_address": "0xpayee-http",
        "amount": 1_000_000u64,
        "asset": "usdc",
        "network": "set_chain",
    })
}

#[tokio::test]
async fn intent_lifecycle_is_reachable_and_durable_over_http() {
    let state = state();
    let app = router(&state);

    let created = post(&app, "/api/v1/x402/intents", intent_body("0xpayer-http-1")).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let intent = body_json(created).await;
    let id = intent["id"].as_str().expect("intent id").to_string();
    assert_eq!(intent["status"], "created");
    // `X402Asset` renders through `Display`, which is upper-case.
    assert_eq!(intent["asset"], "USDC");

    // A brand-new router over the same Commerce still sees it.
    let app2 = router(&state);
    let fetched = get(&app2, &format!("/api/v1/x402/intents/{id}")).await;
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(body_json(fetched).await["id"], id);

    let listed = get(&app2, "/api/v1/x402/intents?status=created").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let rows = body_json(listed).await;
    assert!(rows.as_array().expect("array").iter().any(|row| row["id"] == id.as_str()));

    // An unknown status is a 400, not a silent empty list.
    let bad = get(&app2, "/api/v1/x402/intents?status=teleported").await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    // Cancelling is a real state change visible through the API.
    let cancelled = post(&app2, &format!("/api/v1/x402/intents/{id}/cancel"), json!({})).await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(body_json(cancelled).await["status"], "cancelled");

    let missing = get(&app2, &format!("/api/v1/x402/intents/{}", uuid::Uuid::new_v4())).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let bad_asset = post(
        &app2,
        "/api/v1/x402/intents",
        json!({"payer_address": "0xp", "payee_address": "0xq", "amount": 1u64,
               "asset": "dogecoin", "network": "set_chain"}),
    )
    .await;
    assert_eq!(bad_asset.status(), StatusCode::BAD_REQUEST);
}

/// The double-pay guard, driven through the API: a cart may carry at most one
/// intent that still claims it, and releasing the claim frees the cart.
#[tokio::test]
async fn a_cart_accepts_only_one_claiming_intent_over_http() {
    let state = state();
    let app = router(&state);

    let cart = post(
        &app,
        "/api/v1/carts",
        json!({
            "currency": "USD",
            "items": [{"sku": "X402-SKU", "name": "Widget", "quantity": 1, "unit_price": "10.00"}]
        }),
    )
    .await;
    assert_eq!(cart.status(), StatusCode::CREATED);
    let cart = body_json(cart).await;
    let cart_id = cart["id"].as_str().expect("cart id").to_string();
    let grand_total: f64 =
        cart["grand_total"].as_str().expect("grand_total").parse().expect("decimal");
    // USDC carries six decimals, so the smallest-unit amount the
    // reconciliation guard demands is the grand total scaled by 1e6.
    let smallest_unit = (grand_total * 1_000_000.0).round() as u64;
    let claim = |payer: &str| {
        json!({
            "payer_address": payer,
            "payee_address": "0xpayee-http",
            "amount": smallest_unit,
            "asset": "usdc",
            "network": "set_chain",
            "cart_id": cart_id,
        })
    };

    // Reconciliation: the wrong amount is refused before anything is created.
    let wrong = post(
        &app,
        "/api/v1/x402/intents",
        json!({
            "payer_address": "0xp-wrong",
            "payee_address": "0xpayee-http",
            "amount": smallest_unit + 1,
            "asset": "usdc",
            "network": "set_chain",
            "cart_id": cart_id,
        }),
    )
    .await;
    assert!(
        wrong.status().is_client_error(),
        "an amount that does not match the cart must be refused, got {}",
        wrong.status()
    );

    let first = post(&app, "/api/v1/x402/intents", claim("0xp-first")).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_id = body_json(first).await["id"].as_str().expect("id").to_string();

    // The double-pay guard: a second claim on the same cart is a 409 naming
    // the intent that already holds it.
    let second = post(&app, "/api/v1/x402/intents", claim("0xp-second")).await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let conflict = body_json(second).await;
    assert!(
        conflict.to_string().contains(&first_id),
        "the conflict must name the winning intent: {conflict}"
    );

    let listed = get(&app, &format!("/api/v1/x402/carts/{cart_id}/intents")).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(body_json(listed).await.as_array().expect("array").len(), 1);

    // Releasing the claim frees the cart for a replacement intent.
    let cancelled = post(&app, &format!("/api/v1/x402/intents/{first_id}/cancel"), json!({})).await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    let replacement = post(&app, "/api/v1/x402/intents", claim("0xp-third")).await;
    assert_eq!(replacement.status(), StatusCode::CREATED);
}
