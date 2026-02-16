//! Benchmarks for stateset-db
//!
//! Run with: cargo bench --package stateset-db

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;
use uuid::Uuid;

use stateset_core::{
    CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct,
    CustomerFilter, CustomerRepository, FulfillmentStatus, InventoryRepository, OrderFilter,
    OrderRepository, OrderStatus, PaymentStatus, ProductFilter, ProductRepository,
    ReserveInventory, UpdateOrder,
};
use stateset_db::{DatabaseConfig, SqliteDatabase};

fn setup_database() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("Failed to create in-memory database")
}

struct BenchDb {
    _dir: TempDir,
    db: Arc<SqliteDatabase>,
}

fn setup_database_with_max_connections(max_connections: u32) -> BenchDb {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let path = dir.path().join("bench.db");
    let mut config = DatabaseConfig::sqlite(path.to_str().expect("Temp path is not valid UTF-8"));
    config.max_connections = max_connections;
    let db = SqliteDatabase::new(&config).expect("Failed to create benchmark database");
    BenchDb { _dir: dir, db: Arc::new(db) }
}

fn create_test_customer(idx: usize) -> CreateCustomer {
    CreateCustomer {
        email: format!("customer{}@example.com", idx),
        first_name: format!("First{}", idx),
        last_name: format!("Last{}", idx),
        phone: Some(format!("+1-555-{:04}", idx % 10000)),
        accepts_marketing: Some(true),
        tags: None,
        metadata: None,
    }
}

fn create_test_product(idx: usize) -> CreateProduct {
    CreateProduct {
        name: format!("Product {}", idx),
        slug: Some(format!("product-{}", idx)),
        description: Some(format!("Description for product {}", idx)),
        product_type: Some(stateset_core::ProductType::Simple),
        attributes: None,
        seo: None,
        variants: Some(vec![stateset_core::CreateProductVariant {
            sku: format!("SKU-{:06}", idx),
            name: Some(format!("Variant {}", idx)),
            price: dec!(29.99),
            compare_at_price: Some(dec!(39.99)),
            cost: Some(dec!(15.00)),
            barcode: Some(format!("BARCODE{:06}", idx)),
            weight: Some(dec!(0.5)),
            weight_unit: Some("kg".to_string()),
            options: None,
            is_default: Some(true),
        }]),
    }
}

fn create_test_inventory_item(idx: usize) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: format!("INV-{:06}", idx),
        name: format!("Inventory Item {}", idx),
        description: Some(format!("Description for item {}", idx)),
        unit_of_measure: Some("each".to_string()),
        initial_quantity: Some(dec!(100)),
        location_id: None,
        reorder_point: Some(dec!(20)),
        safety_stock: Some(dec!(10)),
    }
}

fn create_test_order_item(idx: usize) -> CreateOrderItem {
    CreateOrderItem {
        product_id: Uuid::new_v4(),
        variant_id: None,
        sku: format!("SKU-{:06}", idx),
        name: format!("Product {}", idx),
        quantity: 2,
        unit_price: dec!(29.99),
        discount: Some(dec!(0.00)),
        tax_amount: Some(dec!(2.40)),
    }
}

fn create_test_order(customer_id: Uuid, item_count: usize) -> CreateOrder {
    CreateOrder {
        customer_id,
        currency: Some("USD".to_string()),
        shipping_address: None,
        billing_address: None,
        payment_method: Some("credit_card".to_string()),
        shipping_method: Some("standard".to_string()),
        notes: None,
        items: (0..item_count).map(create_test_order_item).collect(),
    }
}

// Customer CRUD Benchmarks
fn benchmark_customer_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("customer_crud");

    group.bench_function("create_single", |b| {
        let db = setup_database();
        let customers = db.customers();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            customers.create(black_box(create_test_customer(idx))).unwrap()
        });
    });

    group.finish();
}

fn benchmark_customer_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("customer_crud");

    // Setup: create customers first
    let db = setup_database();
    let customers = db.customers();
    let mut customer_ids = Vec::new();
    for i in 0..100 {
        let customer = customers.create(create_test_customer(i)).unwrap();
        customer_ids.push(customer.id);
    }

    group.bench_function("get_by_id", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % customer_ids.len();
            customers.get(black_box(customer_ids[idx])).unwrap()
        });
    });

    group.bench_function("list_all", |b| {
        b.iter(|| customers.list(black_box(CustomerFilter::default())).unwrap());
    });

    group.finish();
}

fn benchmark_customer_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("customer_batch");

    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_batch", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let db = setup_database();
                    let customers = db.customers();
                    for i in 0..size {
                        customers.create(create_test_customer(i)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// Product CRUD Benchmarks
fn benchmark_product_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_crud");

    group.bench_function("create_single", |b| {
        let db = setup_database();
        let products = db.products();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            products.create(black_box(create_test_product(idx))).unwrap()
        });
    });

    group.finish();
}

fn benchmark_product_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_crud");

    // Setup: create products first
    let db = setup_database();
    let products = db.products();
    let mut product_ids = Vec::new();
    for i in 0..100 {
        let product = products.create(create_test_product(i)).unwrap();
        product_ids.push(product.id);
    }

    group.bench_function("get_by_id", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % product_ids.len();
            products.get(black_box(product_ids[idx])).unwrap()
        });
    });

    group.bench_function("get_variant_by_sku", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 100;
            let sku = format!("SKU-{:06}", idx);
            products.get_variant_by_sku(black_box(&sku)).unwrap()
        });
    });

    group.bench_function("list_all", |b| {
        b.iter(|| products.list(black_box(ProductFilter::default())).unwrap());
    });

    group.finish();
}

// Inventory Benchmarks
fn benchmark_inventory_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_crud");

    group.bench_function("create_item", |b| {
        let db = setup_database();
        let inventory = db.inventory();
        let mut idx = 0;
        b.iter(|| {
            idx += 1;
            inventory.create_item(black_box(create_test_inventory_item(idx))).unwrap()
        });
    });

    group.finish();
}

fn benchmark_inventory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_operations");

    // Setup: create inventory items
    let db = setup_database();
    let inventory = db.inventory();
    for i in 0..50 {
        inventory.create_item(create_test_inventory_item(i)).unwrap();
    }

    group.bench_function("get_stock", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 50;
            let sku = format!("INV-{:06}", idx);
            inventory.get_stock(black_box(&sku)).unwrap()
        });
    });

    group.bench_function("adjust_quantity", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 50;
            inventory
                .adjust(stateset_core::AdjustInventory {
                    sku: format!("INV-{:06}", idx),
                    location_id: None,
                    quantity: dec!(1),
                    reason: "Benchmark adjustment".to_string(),
                    reference_type: None,
                    reference_id: None,
                })
                .unwrap()
        });
    });

    group.finish();
}

fn benchmark_inventory_reservation(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_reservation");

    // Setup: create inventory items with stock
    let db = setup_database();
    let inventory = db.inventory();
    for i in 0..10 {
        inventory.create_item(create_test_inventory_item(i)).unwrap();
    }

    group.bench_function("reserve_and_release", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % 10;
            let sku = format!("INV-{:06}", idx);

            // Reserve
            let reservation = inventory
                .reserve(stateset_core::ReserveInventory {
                    sku: sku.clone(),
                    location_id: None,
                    quantity: dec!(1),
                    reference_type: "benchmark".to_string(),
                    reference_id: format!("bench-{}", idx),
                    expires_in_seconds: Some(3600),
                })
                .unwrap();

            // Release
            inventory.release_reservation(reservation.id).unwrap();
        });
    });

    group.finish();
}

fn benchmark_inventory_concurrent_reservations(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_reservation_concurrent");
    let reservations_per_thread = 25;

    for thread_count in [2, 4, 8].iter() {
        let bench_db = setup_database_with_max_connections(*thread_count as u32);
        let inventory = bench_db.db.inventory();
        let item_count = thread_count * 2;

        for i in 0..item_count {
            let mut item = create_test_inventory_item(i);
            item.initial_quantity = Some(dec!(1000));
            inventory.create_item(item).unwrap();
        }

        let skus: Arc<Vec<String>> =
            Arc::new((0..item_count).map(|i| format!("INV-{:06}", i)).collect());
        let db = bench_db.db.clone();

        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = Vec::with_capacity(threads);
                    for t in 0..threads {
                        let db = db.clone();
                        let skus = skus.clone();
                        handles.push(thread::spawn(move || {
                            let inventory = db.inventory();
                            for i in 0..reservations_per_thread {
                                let sku = &skus[(t + i) % skus.len()];
                                let reservation = inventory
                                    .reserve(ReserveInventory {
                                        sku: sku.clone(),
                                        location_id: None,
                                        quantity: dec!(1),
                                        reference_type: "benchmark".to_string(),
                                        reference_id: format!("bench-{}-{}", t, i),
                                        expires_in_seconds: Some(3600),
                                    })
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

// Order Benchmarks
fn benchmark_order_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_crud");

    // Setup: create a customer
    let db = setup_database();
    let customers = db.customers();
    let customer = customers.create(create_test_customer(0)).unwrap();

    for item_count in [1, 5, 10, 25].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_with_items", item_count),
            item_count,
            |b, &count| {
                let orders = db.orders();
                b.iter(|| orders.create(black_box(create_test_order(customer.id, count))).unwrap());
            },
        );
    }

    group.finish();
}

fn benchmark_order_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_crud");

    // Setup: create customer and orders
    let db = setup_database();
    let customers = db.customers();
    let customer = customers.create(create_test_customer(0)).unwrap();
    let orders = db.orders();
    let mut order_ids = Vec::new();
    for _ in 0..50 {
        let order = orders.create(create_test_order(customer.id, 3)).unwrap();
        order_ids.push(order.id);
    }

    group.bench_function("get_by_id", |b| {
        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % order_ids.len();
            orders.get(black_box(order_ids[idx])).unwrap()
        });
    });

    group.bench_function("list_all", |b| {
        b.iter(|| orders.list(black_box(OrderFilter::default())).unwrap());
    });

    group.bench_function("list_for_customer", |b| {
        b.iter(|| {
            orders
                .list(black_box(OrderFilter {
                    customer_id: Some(customer.id),
                    ..Default::default()
                }))
                .unwrap()
        });
    });

    group.finish();
}

fn benchmark_order_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_lifecycle");

    let db = setup_database();
    let customer = db.customers().create(create_test_customer(0)).unwrap();
    let orders = db.orders();

    group.bench_function("create_update_statuses", |b| {
        b.iter(|| {
            let order = orders.create(create_test_order(customer.id, 3)).unwrap();

            orders
                .update(
                    order.id,
                    UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
                )
                .unwrap();

            orders
                .update(
                    order.id,
                    UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
                )
                .unwrap();

            orders
                .update(
                    order.id,
                    UpdateOrder {
                        fulfillment_status: Some(FulfillmentStatus::Shipped),
                        tracking_number: Some("TRACK123".to_string()),
                        ..Default::default()
                    },
                )
                .unwrap();
        });
    });

    group.finish();
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    for batch_size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("customers_partial", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let db = setup_database();
                    let customers = db.customers();
                    let inputs: Vec<CreateCustomer> = (0..size).map(create_test_customer).collect();
                    customers.create_batch(inputs).unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("customers_atomic", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let db = setup_database();
                    let customers = db.customers();
                    let inputs: Vec<CreateCustomer> = (0..size).map(create_test_customer).collect();
                    customers.create_batch_atomic(inputs).unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("inventory_items_atomic", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let db = setup_database();
                    let inventory = db.inventory();
                    let inputs: Vec<CreateInventoryItem> =
                        (0..size).map(create_test_inventory_item).collect();
                    inventory.create_item_batch_atomic(inputs).unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("orders_atomic", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let db = setup_database();
                    let customer = db.customers().create(create_test_customer(0)).unwrap();
                    let orders = db.orders();
                    let inputs: Vec<CreateOrder> =
                        (0..size).map(|_| create_test_order(customer.id, 3)).collect();
                    orders.create_batch_atomic(inputs).unwrap()
                });
            },
        );
    }

    group.finish();
}

// Mixed workload benchmark
fn benchmark_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    group.bench_function("typical_commerce_session", |b| {
        b.iter(|| {
            let db = setup_database();

            // Create customer
            let customer = db.customers().create(create_test_customer(0)).unwrap();

            // Create products
            for i in 0..5 {
                db.products().create(create_test_product(i)).unwrap();
            }

            // Create inventory
            for i in 0..5 {
                db.inventory().create_item(create_test_inventory_item(i)).unwrap();
            }

            // Create order
            let order = db.orders().create(create_test_order(customer.id, 3)).unwrap();

            // Read operations
            db.customers().get(customer.id).unwrap();
            db.orders().get(order.id).unwrap();
            db.products().list(ProductFilter::default()).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_customer_create,
    benchmark_customer_read,
    benchmark_customer_batch,
    benchmark_product_create,
    benchmark_product_read,
    benchmark_inventory_create,
    benchmark_inventory_operations,
    benchmark_inventory_reservation,
    benchmark_inventory_concurrent_reservations,
    benchmark_order_create,
    benchmark_order_read,
    benchmark_order_lifecycle,
    benchmark_batch_operations,
    benchmark_mixed_workload,
);

criterion_main!(benches);
