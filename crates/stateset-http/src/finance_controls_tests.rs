//! Finance authorization and identity regressions through the full HTTP stack.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, PermissionLevel, Role, RoleBuilder};
use stateset_core::*;
use stateset_embedded::Commerce;
use tower::ServiceExt;

fn app() -> Router {
    let commerce = Commerce::new(":memory:").unwrap();
    let engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(RoleBuilder::new("writer").default_level(PermissionLevel::Write).build())
        .assign_role("maker", "admin")
        .assign_role("checker", "admin")
        .assign_role("writer", "writer")
        .build();
    crate::ServerBuilder::new(commerce)
        .with_ignore_tenant_header()
        .with_bearer_auth_for_actor("maker-key", "maker")
        .add_bearer_auth_for_actor("checker-key", "checker")
        .add_bearer_auth_for_actor("writer-key", "writer")
        .with_authz_engine(engine)
        .require_idempotency_keys(false)
        .build()
}
async fn request(app: &Router, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-tenant-id", "test")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn finance_permissions_identity_and_maker_checker() {
    let app = app();
    let body = json!({"supplier_id": uuid::Uuid::new_v4(), "due_date":"2026-12-01T00:00:00Z", "created_by":"checker", "items":[{"description":"Stock","quantity":"1","unit_price":"10"}]});
    let (status, bill) = request(&app, "/api/v1/ap/bills", "maker-key", body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{bill}");
    assert_eq!(bill["created_by"], "maker");
    let approve = format!("/api/v1/ap/bills/{}/approve", bill["id"].as_str().unwrap());
    let (status, _) = request(&app, &approve, "maker-key", json!({"approved_by":"checker"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = request(&app, &approve, "writer-key", json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, approved) =
        request(&app, &approve, "checker-key", json!({"approved_by":"maker"})).await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["approved_by"], "checker");
    let (status, _) = request(&app, "/api/v1/ap/bills", "writer-key", body).await;
    assert_eq!(status, StatusCode::CREATED);
    for path in [
        "/api/v1/ap/payments",
        "/api/v1/ap/payment-runs/id/process",
        "/api/v1/ar/credit-memos",
        "/api/v1/ar/payment-applications",
        "/api/v1/ar/write-offs",
        "/api/v1/invoices/id/payments",
        "/api/v1/gl/journal-entries",
        "/api/v1/gl/journal-entries/id/post",
        "/api/v1/gl/journal-entries/id/reverse",
        "/api/v1/gl/close-month",
        "/api/v1/gl/revalue",
        "/api/v1/revenue-obligations/id/recognize",
        "/api/v1/prepayments/id/apply",
        "/api/v1/cost-accounting/items",
        "/api/v1/cost-accounting/adjustments/id/approve",
        "/api/v1/fixed-assets/id/post-depreciation",
    ] {
        let (status, response) = request(&app, path, "writer-key", json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {response}");
    }
}
#[tokio::test]
async fn ledger_audit_identity_cannot_be_spoofed() {
    let commerce = Commerce::new(":memory:").unwrap();
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let cash = gl.get_account_by_number("1010").unwrap().unwrap().id;
    let sales = gl.get_account_by_number("4010").unwrap().unwrap().id;
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "Identity".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();
    let app = crate::ServerBuilder::new(commerce)
        .with_ignore_tenant_header()
        .with_bearer_auth_for_actor("checker-key", "checker")
        .with_authz_engine(
            AuthzEngineBuilder::new()
                .add_role(Role::admin())
                .assign_role("checker", "admin")
                .build(),
        )
        .require_idempotency_keys(false)
        .build();
    let (status, entry) = request(&app, "/api/v1/gl/journal-entries", "checker-key", json!({"entry_date":"2026-01-10","description":"Authenticated posting","auto_post":true,"posted_by":"spoofed","lines":[{"account_id":cash,"debit_amount":"10"},{"account_id":sales,"credit_amount":"10"}]})).await;
    assert_eq!(status, StatusCode::CREATED, "{entry}");
    assert_eq!(entry["posted_by"], "checker");
    let (status, period_response) = request(
        &app,
        &format!("/api/v1/gl/periods/{}/close", period.id),
        "checker-key",
        json!({"closed_by":"spoofed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{period_response}");
    assert_eq!(period_response["closed_by"], "checker");
    let (status, period_response) = request(
        &app,
        &format!("/api/v1/gl/periods/{}/lock", period.id),
        "checker-key",
        json!({"locked_by":"spoofed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{period_response}");
    assert_eq!(period_response["locked_by"], "checker");
}
#[tokio::test]
async fn anonymous_finance_cannot_claim_header_identity() {
    let commerce = Commerce::new(":memory:").unwrap();
    let app = crate::ServerBuilder::new(commerce)
        .with_ignore_tenant_header()
        .without_auth()
        .require_idempotency_keys(false)
        .build();
    let response = app
        .oneshot(
            Request::post("/api/v1/ap/bills")
                .header("x-actor-id", "spoofed")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn finance_retries_preserve_one_settlement_and_one_credit_memo() {
    let commerce = Commerce::new(":memory:").unwrap();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "finance-retry@example.com".into(),
            first_name: "Retry".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            items: vec![CreateInvoiceItem {
                description: "Service".into(),
                quantity: rust_decimal::Decimal::ONE,
                unit_price: rust_decimal::Decimal::from(100),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    commerce.invoices().send(invoice.id.into()).unwrap();
    let invoices = commerce.invoices();
    let ar = commerce.accounts_receivable();
    let app = crate::ServerBuilder::new(commerce)
        .with_ignore_tenant_header()
        .with_bearer_auth_for_actor("checker-key", "checker")
        .with_authz_engine(
            AuthzEngineBuilder::new()
                .add_role(Role::admin())
                .assign_role("checker", "admin")
                .build(),
        )
        .build();
    let invoke = |path: String, key: Option<&str>, body: Value| {
        let mut request = Request::post(path)
            .header("authorization", "Bearer checker-key")
            .header("x-tenant-id", "test")
            .header("content-type", "application/json");
        if let Some(key) = key {
            request = request.header("idempotency-key", key);
        }
        app.clone().oneshot(request.body(Body::from(body.to_string())).unwrap())
    };
    let path = format!("/api/v1/invoices/{}/payments", invoice.id);
    let payment = json!({"amount":"10.00","payment_id":uuid::Uuid::new_v4()});
    assert_eq!(
        invoke(path.clone(), None, payment.clone()).await.unwrap().status(),
        StatusCode::PRECONDITION_REQUIRED
    );
    for replay in [false, true] {
        let response =
            invoke(path.clone(), Some("invoice-payment"), payment.clone()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["idempotency-replayed"], replay.to_string());
    }
    assert_eq!(
        invoices.get(invoice.id.into()).unwrap().unwrap().amount_paid,
        rust_decimal::Decimal::from(10)
    );
    assert_eq!(
        invoke(path, Some("invoice-payment"), json!({"amount":"11.00"})).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let memo = json!({"customer_id":customer.id,"original_invoice_id":invoice.id,"reason":"pricing_error","amount":"5.00"});
    let first =
        invoke("/api/v1/ar/credit-memos".into(), Some("credit-memo"), memo.clone()).await.unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let original = axum::body::to_bytes(first.into_body(), usize::MAX).await.unwrap();
    let second = invoke("/api/v1/ar/credit-memos".into(), Some("credit-memo"), memo).await.unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(second.headers()["idempotency-replayed"], "true");
    assert_eq!(axum::body::to_bytes(second.into_body(), usize::MAX).await.unwrap(), original);
    assert_eq!(ar.list_credit_memos(CreditMemoFilter::default()).unwrap().len(), 1);
}
