use criterion::{Criterion, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use stateset_benches::perf_gate::run_gate_if_enabled_with_iterations;
use stateset_benches::{create_temp_commerce, create_test_customers, create_test_orders};
use stateset_core::{CreateInventoryItem, InventoryFilter, OrderFilter};
use uuid::Uuid;

/// Benchmark: reserve + release cycle for inventory.
///
/// Creates an inventory item with stock, then repeatedly reserves and releases.
fn bench_inventory_reserve_release(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("inventory_reserve_release", 10, || {
        let (commerce, _dir) = create_temp_commerce();
        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "RR-GATE-001".into(),
                name: "Reserve Gate Item".into(),
                initial_quantity: Some(dec!(10000)),
                ..Default::default()
            })
            .expect("create item");
        for i in 0..10 {
            let reservation = commerce
                .inventory()
                .reserve("RR-GATE-001", dec!(5), "order", &format!("gate-{i}"), None)
                .expect("reserve");
            commerce.inventory().release_reservation(reservation.id).expect("release");
        }
    });

    c.bench_function("inventory_reserve_release", |bencher| {
        bencher.iter_with_setup(
            || {
                let (commerce, dir) = create_temp_commerce();
                commerce
                    .inventory()
                    .create_item(CreateInventoryItem {
                        sku: "RR-BENCH-001".into(),
                        name: "Reserve Bench Item".into(),
                        initial_quantity: Some(dec!(100_000)),
                        ..Default::default()
                    })
                    .expect("create item");
                (commerce, dir)
            },
            |(commerce, _dir)| {
                for i in 0..20 {
                    let reservation = commerce
                        .inventory()
                        .reserve("RR-BENCH-001", dec!(5), "order", &format!("bench-{i}"), None)
                        .expect("reserve");
                    commerce.inventory().release_reservation(reservation.id).expect("release");
                }
            },
        );
    });
}

/// Benchmark: order creation with 5 items (exercises pricing engine + DB insert).
fn bench_order_creation_5_items(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("order_5_items", 5, || {
        let (commerce, _dir) = create_temp_commerce();
        let customer = commerce
            .customers()
            .create(stateset_core::models::customer::CreateCustomer {
                email: format!("gate-5item-{}@example.com", Uuid::new_v4()),
                first_name: "Gate".into(),
                last_name: "User".into(),
                ..Default::default()
            })
            .expect("customer");
        let items: Vec<_> = (0..5)
            .map(|i| stateset_core::CreateOrderItem {
                product_id: stateset_core::ProductId::new(),
                variant_id: None,
                sku: format!("GATE-5I-{i:03}"),
                name: format!("Item {i}"),
                quantity: (i as i32) + 1,
                unit_price: dec!(9.99) + rust_decimal::Decimal::from(i),
                discount: None,
                tax_amount: None,
            })
            .collect();
        commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id: customer.id,
                items,
                ..Default::default()
            })
            .expect("order");
    });

    c.bench_function("order_5_items", |bencher| {
        bencher.iter_with_setup(
            || {
                let (commerce, dir) = create_temp_commerce();
                let customer = commerce
                    .customers()
                    .create(stateset_core::models::customer::CreateCustomer {
                        email: format!("bench-5item-{}@example.com", Uuid::new_v4()),
                        first_name: "Bench".into(),
                        last_name: "User".into(),
                        ..Default::default()
                    })
                    .expect("customer");
                let items: Vec<_> = (0..5)
                    .map(|i| stateset_core::CreateOrderItem {
                        product_id: stateset_core::ProductId::new(),
                        variant_id: None,
                        sku: format!("BENCH-5I-{i:03}"),
                        name: format!("Item {i}"),
                        quantity: (i as i32) + 1,
                        unit_price: dec!(9.99) + rust_decimal::Decimal::from(i),
                        discount: None,
                        tax_amount: None,
                    })
                    .collect();
                (commerce, dir, customer.id, items)
            },
            |(commerce, _dir, customer_id, items)| {
                commerce
                    .orders()
                    .create(stateset_core::CreateOrder { customer_id, items, ..Default::default() })
                    .expect("order");
            },
        );
    });
}

/// Benchmark: customer lookup by email.
///
/// Seeds 500 customers, then looks up one by email.
fn bench_customer_lookup_by_email(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("customer_lookup_email", 10, || {
        let (commerce, _dir) = create_temp_commerce();
        let customers = create_test_customers(500);
        for cust in &customers {
            commerce.customers().create(cust.clone()).expect("insert");
        }
        let target_email = &customers[250].email;
        let _found = commerce.customers().get_by_email(target_email).expect("lookup");
    });

    c.bench_function("customer_lookup_email", |bencher| {
        bencher.iter_with_setup(
            || {
                let (commerce, dir) = create_temp_commerce();
                let customers = create_test_customers(500);
                for cust in &customers {
                    commerce.customers().create(cust.clone()).expect("insert");
                }
                let target_email = customers[250].email.clone();
                (commerce, dir, target_email)
            },
            |(commerce, _dir, email)| {
                let result = commerce.customers().get_by_email(&email).expect("lookup");
                assert!(result.is_some());
            },
        );
    });
}

/// Benchmark: paginated list with filters.
///
/// Seeds 200 orders, then lists with status filter + pagination (limit 20, offset 40).
fn bench_paginated_order_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_paginated_list");

    for size in [50, 200] {
        let gate_name = format!("paginated_orders_{size}");
        run_gate_if_enabled_with_iterations(gate_name.as_str(), 5, || {
            let (commerce, _dir) = create_temp_commerce();
            let customer = commerce
                .customers()
                .create(stateset_core::models::customer::CreateCustomer {
                    email: format!("gate-page-{}@example.com", Uuid::new_v4()),
                    first_name: "Gate".into(),
                    last_name: "Page".into(),
                    ..Default::default()
                })
                .expect("customer");
            let orders = create_test_orders(size);
            for mut order in orders {
                order.customer_id = customer.id;
                commerce.orders().create(order).expect("insert");
            }
            let _results = commerce
                .orders()
                .list(OrderFilter {
                    customer_id: Some(customer.id),
                    limit: Some(20),
                    offset: Some(10),
                    ..Default::default()
                })
                .expect("list");
        });

        group.bench_function(format!("{size}_orders_page20_offset10"), |bencher| {
            bencher.iter_with_setup(
                || {
                    let (commerce, dir) = create_temp_commerce();
                    let customer = commerce
                        .customers()
                        .create(stateset_core::models::customer::CreateCustomer {
                            email: format!("bench-page-{}@example.com", Uuid::new_v4()),
                            first_name: "Bench".into(),
                            last_name: "Page".into(),
                            ..Default::default()
                        })
                        .expect("customer");
                    let orders = create_test_orders(size);
                    for mut order in orders {
                        order.customer_id = customer.id;
                        commerce.orders().create(order).expect("insert");
                    }
                    (commerce, dir, customer.id)
                },
                |(commerce, _dir, customer_id)| {
                    let results = commerce
                        .orders()
                        .list(OrderFilter {
                            customer_id: Some(customer_id),
                            limit: Some(20),
                            offset: Some(10),
                            ..Default::default()
                        })
                        .expect("list");
                    assert!(!results.is_empty());
                },
            );
        });
    }

    group.finish();
}

/// Benchmark: inventory list with filter for active items only.
fn bench_inventory_list_filtered(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("inventory_list_filtered", 5, || {
        let (commerce, _dir) = create_temp_commerce();
        for i in 0..100 {
            commerce
                .inventory()
                .create_item(CreateInventoryItem {
                    sku: format!("GATE-INV-{i:04}"),
                    name: format!("Gate Item {i}"),
                    initial_quantity: Some(dec!(50)),
                    ..Default::default()
                })
                .expect("create");
        }
        let _results = commerce
            .inventory()
            .list(InventoryFilter { is_active: Some(true), limit: Some(25), ..Default::default() })
            .expect("list");
    });

    c.bench_function("inventory_list_filtered", |bencher| {
        bencher.iter_with_setup(
            || {
                let (commerce, dir) = create_temp_commerce();
                for i in 0..100 {
                    commerce
                        .inventory()
                        .create_item(CreateInventoryItem {
                            sku: format!("BENCH-INV-{i:04}"),
                            name: format!("Bench Item {i}"),
                            initial_quantity: Some(dec!(50)),
                            ..Default::default()
                        })
                        .expect("create");
                }
                (commerce, dir)
            },
            |(commerce, _dir)| {
                let results = commerce
                    .inventory()
                    .list(InventoryFilter {
                        is_active: Some(true),
                        limit: Some(25),
                        ..Default::default()
                    })
                    .expect("list");
                assert!(!results.is_empty());
            },
        );
    });
}

criterion_group!(
    benches,
    bench_inventory_reserve_release,
    bench_order_creation_5_items,
    bench_customer_lookup_by_email,
    bench_paginated_order_list,
    bench_inventory_list_filtered,
);
criterion_main!(benches);
