//! Benchmarks for stateset-embedded high-level API
//!
//! Run with: cargo bench --package stateset-embedded

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rust_decimal_macros::dec;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;
use uuid::Uuid;

use stateset_embedded::{
    Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct,
    CreateTaxJurisdiction, CreateTaxRate, JurisdictionLevel, ProductStatus, ProductType,
    TaxAddress, TaxCalculationRequest, TaxLineItem, TaxType,
};

fn setup_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory commerce instance")
}

struct BenchCommerce {
    _dir: TempDir,
    commerce: Arc<Commerce>,
}

fn setup_commerce_with_max_connections(max_connections: u32) -> BenchCommerce {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let path = dir.path().join("bench.db");
    let commerce = Commerce::builder()
        .database(
            path.to_str().expect("Temp path is not valid UTF-8"),
        )
        .max_connections(max_connections)
        .build()
        .expect("Failed to create benchmark commerce instance");

    BenchCommerce {
        _dir: dir,
        commerce: Arc::new(commerce),
    }
}

fn create_test_customer(idx: usize) -> CreateCustomer {
    CreateCustomer {
        email: format!("customer{}@example.com", idx),
        first_name: format!("First{}", idx),
        last_name: format!("Last{}", idx),
        phone: Some(format!("+1-555-{:04}", idx % 10000)),
        company: None,
        tax_exempt: false,
        notes: None,
        tags: vec![],
        metadata: None,
    }
}

fn create_test_product(idx: usize) -> CreateProduct {
    CreateProduct {
        sku: format!("SKU-{:06}", idx),
        name: format!("Product {}", idx),
        description: Some(format!("Description for product {}", idx)),
        category: Some("Test Category".to_string()),
        brand: Some("Test Brand".to_string()),
        price: dec!(29.99),
        cost: Some(dec!(15.00)),
        compare_at_price: Some(dec!(39.99)),
        weight: Some(dec!(0.5)),
        weight_unit: Some("kg".to_string()),
        barcode: Some(format!("BARCODE{:06}", idx)),
        status: ProductStatus::Active,
        product_type: ProductType::Physical,
        taxable: true,
        tax_code: None,
        requires_shipping: true,
        track_inventory: true,
        vendor: None,
        seo: None,
        tags: vec![],
        attributes: vec![],
        variants: vec![],
        metadata: None,
    }
}

fn create_test_inventory_item(idx: usize) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: format!("INV-{:06}", idx),
        name: format!("Inventory Item {}", idx),
        description: Some(format!("Description for item {}", idx)),
        product_id: None,
        variant_id: None,
        uom: Some("each".to_string()),
        barcode: None,
        category: Some("Test Category".to_string()),
        location_id: None,
        initial_quantity: Some(dec!(100)),
        unit_cost: Some(dec!(10.00)),
        reorder_point: Some(dec!(20)),
        reorder_quantity: Some(dec!(100)),
        max_quantity: Some(dec!(1000)),
        min_quantity: Some(dec!(10)),
        lead_time_days: Some(7),
        is_active: true,
        metadata: None,
    }
}

fn create_test_order_item(idx: usize) -> CreateOrderItem {
    CreateOrderItem {
        product_id: Some(Uuid::new_v4()),
        variant_id: None,
        sku: format!("SKU-{:06}", idx),
        name: format!("Product {}", idx),
        quantity: 2,
        unit_price: dec!(29.99),
        discount: dec!(0.00),
        tax_amount: dec!(2.40),
    }
}

fn create_test_order(customer_id: Uuid, item_count: usize) -> CreateOrder {
    CreateOrder {
        customer_id,
        currency: "USD".to_string(),
        shipping_address: None,
        billing_address: None,
        payment_method: Some("credit_card".to_string()),
        shipping_method: Some("standard".to_string()),
        notes: None,
        items: (0..item_count).map(create_test_order_item).collect(),
    }
}

// High-level API Benchmarks
fn benchmark_commerce_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("commerce_init");

    group.bench_function("new_in_memory", |b| {
        b.iter(|| Commerce::new(black_box(":memory:")).unwrap());
    });

    group.finish();
}

fn benchmark_customer_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("customer_api");

    group.bench_function("create", |b| {
        let commerce = setup_commerce();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            commerce.customers().create(black_box(create_test_customer(idx))).unwrap()
        });
    });

    // Setup for read benchmarks
    let commerce = setup_commerce();
    let mut customer_ids = Vec::new();
    for i in 0..100 {
        let customer = commerce.customers().create(create_test_customer(i)).unwrap();
        customer_ids.push(customer.id);
    }

    group.bench_function("get", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % customer_ids.len();
            commerce.customers().get(black_box(customer_ids[idx])).unwrap()
        });
    });

    group.bench_function("list", |b| {
        b.iter(|| commerce.customers().list(black_box(Default::default())).unwrap());
    });

    group.finish();
}

fn benchmark_product_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_api");

    group.bench_function("create", |b| {
        let commerce = setup_commerce();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            commerce.products().create(black_box(create_test_product(idx))).unwrap()
        });
    });

    // Setup for read benchmarks
    let commerce = setup_commerce();
    let mut product_ids = Vec::new();
    for i in 0..100 {
        let product = commerce.products().create(create_test_product(i)).unwrap();
        product_ids.push(product.id);
    }

    group.bench_function("get", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % product_ids.len();
            commerce.products().get(black_box(product_ids[idx])).unwrap()
        });
    });

    group.bench_function("get_by_sku", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 100;
            let sku = format!("SKU-{:06}", idx);
            commerce.products().get_by_sku(black_box(&sku)).unwrap()
        });
    });

    group.finish();
}

fn benchmark_inventory_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_api");

    group.bench_function("create_item", |b| {
        let commerce = setup_commerce();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            commerce.inventory().create_item(black_box(create_test_inventory_item(idx))).unwrap()
        });
    });

    // Setup for operations benchmarks
    let commerce = setup_commerce();
    for i in 0..50 {
        commerce.inventory().create_item(create_test_inventory_item(i)).unwrap();
    }

    group.bench_function("get_stock", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 50;
            let sku = format!("INV-{:06}", idx);
            commerce.inventory().get_stock(black_box(&sku)).unwrap()
        });
    });

    group.bench_function("adjust", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 50;
            let sku = format!("INV-{:06}", idx);
            commerce.inventory().adjust(black_box(&sku), dec!(1), "Benchmark").unwrap()
        });
    });

    group.bench_function("has_stock", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 50;
            let sku = format!("INV-{:06}", idx);
            commerce.inventory().has_stock(black_box(&sku), dec!(10)).unwrap()
        });
    });

    group.finish();
}

fn benchmark_inventory_reservation_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_reservation_api");

    // Setup
    let commerce = setup_commerce();
    for i in 0..10 {
        commerce.inventory().create_item(create_test_inventory_item(i)).unwrap();
    }

    group.bench_function("reserve_release_cycle", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 10;
            let sku = format!("INV-{:06}", idx);

            let reservation = commerce
                .inventory()
                .reserve(&sku, dec!(1), "benchmark", &format!("ref-{}", idx), Some(3600))
                .unwrap();

            commerce.inventory().release_reservation(reservation.id).unwrap();
        });
    });

    group.finish();
}

fn benchmark_inventory_concurrent_reservations_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_reservation_concurrent");
    let reservations_per_thread = 25;

    for thread_count in [2, 4, 8].iter() {
        let bench = setup_commerce_with_max_connections(*thread_count as u32);
        let commerce = bench.commerce.clone();
        let item_count = thread_count * 2;

        for i in 0..item_count {
            let mut item = create_test_inventory_item(i);
            item.initial_quantity = Some(dec!(1000));
            commerce.inventory().create_item(item).unwrap();
        }

        let skus: Arc<Vec<String>> =
            Arc::new((0..item_count).map(|i| format!("INV-{:06}", i)).collect());
        let commerce = commerce.clone();

        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = Vec::with_capacity(threads);
                    for t in 0..threads {
                        let commerce = commerce.clone();
                        let skus = skus.clone();
                        handles.push(thread::spawn(move || {
                            let inventory = commerce.inventory();
                            for i in 0..reservations_per_thread {
                                let sku = &skus[(t + i) % skus.len()];
                                let reservation = inventory
                                    .reserve(
                                        sku,
                                        dec!(1),
                                        "benchmark",
                                        &format!("bench-{}-{}", t, i),
                                        Some(3600),
                                    )
                                    .unwrap();
                                inventory.release_reservation(reservation.id).unwrap();
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_order_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_api");

    // Setup
    let commerce = setup_commerce();
    let customer = commerce.customers().create(create_test_customer(0)).unwrap();

    for item_count in [1, 5, 10, 25].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_with_items", item_count),
            item_count,
            |b, &count| {
                b.iter(|| {
                    commerce
                        .orders()
                        .create(black_box(create_test_order(customer.id, count)))
                        .unwrap()
                });
            },
        );
    }

    // Create some orders for read benchmarks
    let mut order_ids = Vec::new();
    for _ in 0..50 {
        let order = commerce.orders().create(create_test_order(customer.id, 3)).unwrap();
        order_ids.push(order.id);
    }

    group.bench_function("get", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % order_ids.len();
            commerce.orders().get(black_box(order_ids[idx])).unwrap()
        });
    });

    group.bench_function("list_for_customer", |b| {
        b.iter(|| commerce.orders().list_for_customer(black_box(customer.id)).unwrap());
    });

    group.finish();
}

fn benchmark_order_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_lifecycle");

    let commerce = setup_commerce();
    let customer = commerce.customers().create(create_test_customer(0)).unwrap();

    group.bench_function("create_ship_deliver", |b| {
        b.iter(|| {
            // Create order
            let order = commerce
                .orders()
                .create(create_test_order(customer.id, 3))
                .unwrap();

            // Ship order
            let order = commerce.orders().ship(order.id, Some("TRACK123")).unwrap();

            // Deliver order
            commerce.orders().deliver(order.id).unwrap()
        });
    });

    group.finish();
}

fn benchmark_tax_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("tax_calculation");

    // Setup tax configuration
    let commerce = setup_commerce();

    // Create jurisdiction
    commerce.tax().create_jurisdiction(CreateTaxJurisdiction {
        code: "US-CA".to_string(),
        name: "California".to_string(),
        country: "US".to_string(),
        state: Some("CA".to_string()),
        county: None,
        city: None,
        postal_code_pattern: None,
        level: JurisdictionLevel::State,
        is_active: true,
    }).unwrap();

    // Create tax rate
    commerce.tax().create_rate(CreateTaxRate {
        jurisdiction_id: 1,
        tax_type: TaxType::Sales,
        rate: dec!(0.0875),
        name: "CA Sales Tax".to_string(),
        description: None,
        product_category: None,
        is_compound: false,
        priority: 1,
        is_active: true,
        effective_from: None,
        effective_to: None,
    }).unwrap();

    for line_count in [1, 5, 10, 25].iter() {
        let lines: Vec<TaxLineItem> = (0..*line_count)
            .map(|i| TaxLineItem {
                id: format!("item-{}", i),
                product_category: None,
                amount: dec!(29.99),
                quantity: 2,
                is_taxable: true,
                tax_override: None,
            })
            .collect();

        let request = TaxCalculationRequest {
            ship_from: None,
            ship_to: TaxAddress {
                line1: Some("123 Main St".to_string()),
                line2: None,
                city: "San Francisco".to_string(),
                state: Some("CA".to_string()),
                postal_code: "94102".to_string(),
                country: "US".to_string(),
            },
            line_items: lines.clone(),
            shipping_amount: Some(dec!(9.99)),
            customer_exemptions: vec![],
            currency: "USD".to_string(),
            transaction_date: None,
        };

        group.bench_with_input(
            BenchmarkId::new("calculate_tax", line_count),
            &request,
            |b, req| {
                b.iter(|| commerce.tax().calculate(black_box(req.clone())).unwrap());
            },
        );
    }

    group.finish();
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_customers", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let commerce = setup_commerce();
                    for i in 0..size {
                        commerce.customers().create(create_test_customer(i)).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("create_products", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let commerce = setup_commerce();
                    for i in 0..size {
                        commerce.products().create(create_test_product(i)).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("create_inventory_items", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let commerce = setup_commerce();
                    for i in 0..size {
                        commerce.inventory().create_item(create_test_inventory_item(i)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_full_commerce_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_workflow");

    group.bench_function("complete_order_flow", |b| {
        b.iter(|| {
            let commerce = setup_commerce();

            // 1. Create customer
            let customer = commerce.customers().create(create_test_customer(0)).unwrap();

            // 2. Create products
            for i in 0..3 {
                commerce.products().create(create_test_product(i)).unwrap();
            }

            // 3. Create inventory
            for i in 0..3 {
                commerce.inventory().create_item(create_test_inventory_item(i)).unwrap();
            }

            // 4. Create order
            let order = commerce
                .orders()
                .create(create_test_order(customer.id, 3))
                .unwrap();

            // 5. Reserve inventory
            for i in 0..3 {
                let sku = format!("INV-{:06}", i);
                commerce
                    .inventory()
                    .reserve(&sku, dec!(2), "order", &order.id.to_string(), Some(3600))
                    .unwrap();
            }

            // 6. Adjust inventory (fulfill)
            for i in 0..3 {
                let sku = format!("INV-{:06}", i);
                commerce.inventory().adjust(&sku, dec!(-2), "Order fulfillment").unwrap();
            }

            // 7. Ship order
            let order = commerce.orders().ship(order.id, Some("TRACK123")).unwrap();

            // 8. Deliver order
            commerce.orders().deliver(order.id).unwrap()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_commerce_initialization,
    benchmark_customer_api,
    benchmark_product_api,
    benchmark_inventory_api,
    benchmark_inventory_reservation_api,
    benchmark_inventory_concurrent_reservations_api,
    benchmark_order_api,
    benchmark_order_lifecycle,
    benchmark_tax_calculation,
    benchmark_batch_operations,
    benchmark_full_commerce_workflow,
);

criterion_main!(benches);
