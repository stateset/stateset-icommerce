use axum::body::Body;
use axum::http::{Request, StatusCode};
use criterion::{Criterion, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use serde_json::json;
use stateset_benches::perf_gate::run_gate_if_enabled_with_iterations;
use stateset_embedded::Commerce;
use stateset_http::AppState;
use tower::ServiceExt;

/// Build an [`AppState`] + bare router (no middleware) suitable for benchmarking.
///
/// We skip the full [`ServerBuilder`] middleware stack (CORS, request-ID, etc.)
/// so that the benchmark focuses on route-handler + Commerce round-trip latency.
fn bench_state() -> (axum::Router, AppState) {
    let commerce = Commerce::new(":memory:").expect("in-memory Commerce");
    let state = AppState::new(commerce);
    let router = stateset_http::routes::api_router().with_state(state.clone());
    (router, state)
}

/// Seed a customer and return its ID.
fn seed_customer(state: &AppState) -> String {
    let customer = state
        .commerce()
        .customers()
        .create(stateset_core::CreateCustomer {
            email: format!("bench-http-{}@example.com", uuid::Uuid::new_v4()),
            first_name: "Bench".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("seed customer");
    customer.id.to_string()
}

/// Seed N orders for a given customer and return their IDs.
fn seed_orders(state: &AppState, customer_id: &str, count: usize) -> Vec<String> {
    let cid: stateset_core::CustomerId = customer_id.parse().expect("parse customer id");
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let order = state
            .commerce()
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id: cid,
                items: vec![stateset_core::CreateOrderItem {
                    product_id: stateset_core::ProductId::new(),
                    variant_id: None,
                    sku: format!("HTTP-SKU-{i:06}"),
                    name: format!("Bench Widget {i}"),
                    quantity: 1,
                    unit_price: dec!(19.99),
                    discount: None,
                    tax_amount: None,
                }],
                ..Default::default()
            })
            .expect("seed order");
        ids.push(order.id.to_string());
    }
    ids
}

fn bench_create_order(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    run_gate_if_enabled_with_iterations("http_create_order", 5, || {
        let (router, state) = bench_state();
        let customer_id = seed_customer(&state);
        let body = json!({
            "customer_id": customer_id,
            "items": [{
                "product_id": uuid::Uuid::new_v4().to_string(),
                "sku": "BENCH-001",
                "name": "Widget",
                "quantity": 2,
                "unit_price": "29.99"
            }]
        });
        let _resp = rt.block_on(async {
            router
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/orders")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap()
        });
    });

    // Pre-build the shared state + router outside the measured loop.
    // Each iteration sends one POST /api/v1/orders request.
    let (router, state) = bench_state();
    let customer_id = seed_customer(&state);

    c.bench_function("http_create_order", |bencher| {
        bencher.iter(|| {
            let body = json!({
                "customer_id": customer_id,
                "items": [{
                    "product_id": uuid::Uuid::new_v4().to_string(),
                    "sku": "BENCH-001",
                    "name": "Widget",
                    "quantity": 2,
                    "unit_price": "29.99"
                }]
            });
            rt.block_on(async {
                let resp = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/api/v1/orders")
                            .header("content-type", "application/json")
                            .body(Body::from(serde_json::to_vec(&body).unwrap()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::CREATED);
            });
        });
    });
}

fn bench_get_order(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    // Seed one order and then benchmark GETting it repeatedly.
    let (router, state) = bench_state();
    let customer_id = seed_customer(&state);
    let order_ids = seed_orders(&state, &customer_id, 1);
    let order_id = order_ids[0].clone();

    run_gate_if_enabled_with_iterations("http_get_order", 50, || {
        rt.block_on(async {
            let resp = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/orders/{order_id}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        });
    });

    c.bench_function("http_get_order", |bencher| {
        bencher.iter(|| {
            rt.block_on(async {
                let resp = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/orders/{order_id}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
            });
        });
    });
}

fn bench_list_orders(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let mut group = c.benchmark_group("http_list_orders");

    for count in [10, 50] {
        // Pre-seed data for this size
        let (router, state) = bench_state();
        let customer_id = seed_customer(&state);
        seed_orders(&state, &customer_id, count);

        let gate_name = format!("http_list_orders_{count}");
        run_gate_if_enabled_with_iterations(gate_name.as_str(), 10, || {
            rt.block_on(async {
                let resp = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/v1/orders?limit=20")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
            });
        });

        group.bench_function(format!("{count}_orders_page20"), |bencher| {
            bencher.iter(|| {
                rt.block_on(async {
                    let resp = router
                        .clone()
                        .oneshot(
                            Request::builder()
                                .uri("/api/v1/orders?limit=20")
                                .body(Body::empty())
                                .unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(resp.status(), StatusCode::OK);
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_create_order, bench_get_order, bench_list_orders);
criterion_main!(benches);
