//! API-level benchmark suite for `stateset-embedded`.
//!
//! Run with: `cargo bench -p stateset-embedded --bench api_benchmarks`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_decimal_macros::dec;
use stateset_embedded::{
    AnalyticsQuery, Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    TimePeriod,
};
use tempfile::NamedTempFile;
use uuid::Uuid;

fn create_test_commerce() -> (Commerce, NamedTempFile) {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_file.path().to_str().expect("Invalid path");
    let commerce = Commerce::new(db_path).expect("Failed to create commerce");
    (commerce, temp_file)
}

fn bench_create_customer(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    c.bench_function("api/create_customer", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .customers()
                    .create(CreateCustomer {
                        email: format!("bench-{}@example.com", Uuid::new_v4()),
                        first_name: "Bench".into(),
                        last_name: "User".into(),
                        accepts_marketing: Some(false),
                        ..Default::default()
                    })
                    .expect("Failed to create customer"),
            );
        })
    });
}

fn bench_inventory_get_stock(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    c.bench_function("api/inventory_get_stock", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .inventory()
                    .get_stock("SKU-001")
                    .expect("Failed to get stock"),
            );
        })
    });
}

fn bench_create_order(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("bench-{}@example.com", Uuid::new_v4()),
            first_name: "Bench".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create customer")
        .id;

    c.bench_function("api/create_order_single_item", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .orders()
                    .create(CreateOrder {
                        customer_id,
                        items: vec![CreateOrderItem {
                            product_id: Uuid::new_v4(),
                            sku: "SKU-001".into(),
                            name: "Widget".into(),
                            quantity: 1,
                            unit_price: dec!(29.99),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })
                    .expect("Failed to create order"),
            );
        })
    });
}

fn bench_sales_summary(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(10000)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("bench-{}@example.com", Uuid::new_v4()),
            first_name: "Bench".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create customer")
        .id;

    // Seed some orders so the query has work to do.
    for _ in 0..50 {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: "SKU-001".into(),
                    name: "Widget".into(),
                    quantity: 1,
                    unit_price: dec!(29.99),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("Failed to create order");
    }

    c.bench_function("api/analytics_sales_summary", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .analytics()
                    .sales_summary(AnalyticsQuery {
                        period: Some(TimePeriod::Last30Days),
                        ..Default::default()
                    })
                    .expect("Failed to get sales summary"),
            );
        })
    });
}

criterion_group!(
    benches,
    bench_create_customer,
    bench_inventory_get_stock,
    bench_create_order,
    bench_sales_summary,
);
criterion_main!(benches);
