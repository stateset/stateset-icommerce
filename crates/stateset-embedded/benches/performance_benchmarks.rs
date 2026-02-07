//!
//! Performance benchmark suite for critical operations.
//!
//! Baseline targets (median, 95th percentile):
//! - Customer creation: <10ms
//! - Product creation: <10ms
//! - Order creation: <15ms (with inventory reservation)
//! - Inventory query: <5ms
//! - Payment recording: <10ms
//! - Analytics query (sales summary): <50ms
//!
//! Run with: cargo bench -p stateset-embedded

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rust_decimal_macros::dec;
use stateset_embedded::{
    AnalyticsQuery, Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CustomerFilter, OrderFilter, TimePeriod,
};
use tempfile::NamedTempFile;
use uuid::Uuid;

fn create_test_commerce() -> (Commerce, NamedTempFile) {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_file.path().to_str().expect("Invalid path");
    let commerce = Commerce::new(db_path).expect("Failed to create commerce");
    (commerce, temp_file)
}

fn create_test_customer(commerce: &Commerce, i: usize) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", i),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create customer")
        .id
}

fn setup_test_data(commerce: &Commerce) {
    // Create inventory
    for i in 1..=10 {
        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: format!("SKU-{:03}", i),
                name: format!("Product {}", i),
                initial_quantity: Some(dec!(1000)),
                ..Default::default()
            })
            .expect("Failed to create inventory item");
    }

    // Create customers
    for i in 1..=100 {
        create_test_customer(commerce, i);
    }

    // Create products (simulated via orders)
    for i in 1..=100 {
        let customer_id = commerce
            .customers()
            .list(CustomerFilter {
                limit: Some(1),
                offset: Some(0),
                ..Default::default()
            })
            .expect("Failed to list customers")
            .into_iter()
            .next()
            .expect("No customers found")
            .id;

        commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: format!("SKU-{:03}", (i % 10) + 1),
                    name: format!("Product {}", (i % 10) + 1),
                    quantity: (i % 5) + 1,
                    unit_price: dec!(29.99),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("Failed to create order");
    }
}

fn bench_customer_creation(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    let mut group = c.benchmark_group("customer_creation");
    group.bench_function("single_customer", |b| {
        b.iter(|| {
            commerce
                .customers()
                .create(CreateCustomer {
                    email: format!("test-{}@example.com", Uuid::new_v4()),
                    first_name: "Test".into(),
                    last_name: "User".into(),
                    ..Default::default()
                })
                .expect("Failed to create customer");
        });
    });
    group.finish();
}

fn bench_inventory_operations(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    let mut group = c.benchmark_group("inventory_operations");

    // Setup
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    group.bench_function("get_stock", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .inventory()
                    .get_stock("SKU-001")
                    .expect("Failed to get stock"),
            );
        });
    });

    group.bench_function("adjust_inventory", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .inventory()
                    .adjust("SKU-001", dec!(10), "Benchmark adjustment")
                    .expect("Failed to adjust inventory"),
            );
        });
    });

    group.bench_function("reserve_inventory", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .inventory()
                    .reserve(
                        "SKU-001",
                        dec!(5),
                        "order",
                        &Uuid::new_v4().to_string(),
                        None,
                    )
                    .ok(),
            );
        });
    });
    group.finish();
}

fn bench_order_creation(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    // Setup
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let customer_id = create_test_customer(&commerce, 1);
    let product_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_creation");

    group.bench_function("single_item_order", |b| {
        b.iter(|| {
            commerce
                .orders()
                .create(CreateOrder {
                    customer_id,
                    items: vec![CreateOrderItem {
                        product_id,
                        sku: "SKU-001".into(),
                        name: "Widget".into(),
                        quantity: 1,
                        unit_price: dec!(29.99),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .expect("Failed to create order");
        });
    });

    group.bench_function("multi_item_order", |b| {
        b.iter(|| {
            commerce
                .orders()
                .create(CreateOrder {
                    customer_id,
                    items: vec![
                        CreateOrderItem {
                            product_id,
                            sku: "SKU-001".into(),
                            name: "Widget".into(),
                            quantity: 2,
                            unit_price: dec!(29.99),
                            ..Default::default()
                        },
                        CreateOrderItem {
                            product_id,
                            sku: "SKU-002".into(),
                            name: "Gadget".into(),
                            quantity: 1,
                            unit_price: dec!(19.99),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                })
                .expect("Failed to create order");
        });
    });
    group.finish();
}

fn bench_order_status_transitions(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();

    // Setup
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let customer_id = create_test_customer(&commerce, 1);

    let mut group = c.benchmark_group("order_transitions");

    group.bench_function("confirm_order", |b| {
        b.iter(|| {
            let order = commerce
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

            black_box(
                commerce
                    .orders()
                    .update_status(order.id, stateset_embedded::OrderStatus::Confirmed)
                    .expect("Failed to confirm order"),
            );
        });
    });

    group.bench_function("ship_order", |b| {
        b.iter(|| {
            let mut order = commerce
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

            order = commerce
                .orders()
                .update_status(order.id, stateset_embedded::OrderStatus::Confirmed)
                .expect("Failed to confirm order");

            let tracking = format!("TRACKING-{}", Uuid::new_v4());
            black_box(
                commerce
                    .orders()
                    .ship(order.id, Some(tracking.as_str()))
                    .expect("Failed to ship order"),
            );
        });
    });
    group.finish();
}

fn bench_query_performance(c: &mut Criterion) {
    let (commerce, _temp) = create_test_commerce();
    setup_test_data(&commerce);

    let mut group = c.benchmark_group("query_performance");

    group.bench_function("list_customers", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .customers()
                    .list(CustomerFilter {
                        limit: Some(20),
                        offset: Some(0),
                        ..Default::default()
                    })
                    .expect("Failed to list customers"),
            );
        });
    });

    group.bench_function("list_orders", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .orders()
                    .list(OrderFilter {
                        limit: Some(20),
                        offset: Some(0),
                        ..Default::default()
                    })
                    .expect("Failed to list orders"),
            );
        });
    });

    group.bench_function("get_sales_summary", |b| {
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
        });
    });

    group.bench_function("get_stock", |b| {
        b.iter(|| {
            black_box(
                commerce
                    .inventory()
                    .get_stock("SKU-001")
                    .expect("Failed to get stock"),
            );
        });
    });
    group.finish();
}

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    for size in [10usize, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("list_customers", size),
            size,
            |b, &size| {
                let (commerce, _temp) = create_test_commerce();

                // Create customers
                for i in 0..size {
                    create_test_customer(&commerce, i);
                }

                b.iter(|| {
                    black_box(
                        commerce
                            .customers()
                            .list(CustomerFilter {
                                limit: Some(size as u32),
                                offset: Some(0),
                                ..Default::default()
                            })
                            .expect("Failed to list customers"),
                    );
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_customer_creation,
    bench_inventory_operations,
    bench_order_creation,
    bench_order_status_transitions,
    bench_query_performance,
    bench_scalability,
);
criterion_main!(benches);
