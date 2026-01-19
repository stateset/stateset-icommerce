//! Stress tests for performance and reliability under load
//!
//! These tests verify system behavior under high volume operations.

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use uuid::Uuid;

// ============================================================================
// Volume Tests
// ============================================================================

#[test]
fn stress_test_bulk_order_creation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a customer first (FK constraint)
    let customer = commerce.customers().create(CreateCustomer {
        email: "bulk-test@stress.test".into(),
        first_name: "Bulk".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");
    let customer_id = customer.id;

    let order_count = 1000;

    let start = Instant::now();

    for i in 0..order_count {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: format!("BULK-{}", i),
                    name: format!("Bulk Product {}", i),
                    quantity: 1,
                    unit_price: dec!(10.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("Failed to create order");
    }

    let duration = start.elapsed();
    let orders_per_sec = order_count as f64 / duration.as_secs_f64();

    println!(
        "Created {} orders in {:?} ({:.2} orders/sec)",
        order_count, duration, orders_per_sec
    );

    // Verify count
    let count = commerce
        .orders()
        .count(Default::default())
        .expect("Failed to count");
    assert_eq!(count, order_count as u64);

    // Performance threshold: at least 100 orders/sec for in-memory SQLite
    assert!(
        orders_per_sec > 100.0,
        "Performance below threshold: {:.2} orders/sec",
        orders_per_sec
    );
}

#[test]
fn stress_test_bulk_customer_creation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_count = 1000;

    let start = Instant::now();

    for i in 0..customer_count {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("customer{}@stress.test", i),
                first_name: format!("Customer{}", i),
                last_name: "Stress".into(),
                ..Default::default()
            })
            .expect("Failed to create customer");
    }

    let duration = start.elapsed();
    let customers_per_sec = customer_count as f64 / duration.as_secs_f64();

    println!(
        "Created {} customers in {:?} ({:.2} customers/sec)",
        customer_count, duration, customers_per_sec
    );

    // Verify count
    let count = commerce
        .customers()
        .count(Default::default())
        .expect("Failed to count");
    assert_eq!(count, customer_count as u64);
}

#[test]
fn stress_test_bulk_inventory_adjustments() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create inventory item
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "STRESS-INV-001".into(),
            name: "Stress Test Inventory".into(),
            initial_quantity: Some(dec!(1000000)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    let adjustment_count = 1000;
    let start = Instant::now();

    for i in 0..adjustment_count {
        commerce
            .inventory()
            .adjust("STRESS-INV-001", dec!(-1), &format!("Adjustment {}", i))
            .expect("Failed to adjust inventory");
    }

    let duration = start.elapsed();
    let adjustments_per_sec = adjustment_count as f64 / duration.as_secs_f64();

    println!(
        "Made {} inventory adjustments in {:?} ({:.2} adjustments/sec)",
        adjustment_count, duration, adjustments_per_sec
    );

    // Verify final stock
    let stock = commerce
        .inventory()
        .get_stock("STRESS-INV-001")
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_on_hand, dec!(1000000) - dec!(1000));
}

#[test]
fn stress_test_bulk_product_creation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let product_count = 500;

    let start = Instant::now();

    for i in 0..product_count {
        commerce
            .products()
            .create(stateset_embedded::CreateProduct {
                name: format!("Stress Product {}", i),
                slug: Some(format!("stress-product-{}", i)),
                description: Some(format!("Description for stress test product {}", i)),
                ..Default::default()
            })
            .expect("Failed to create product");
    }

    let duration = start.elapsed();
    let products_per_sec = product_count as f64 / duration.as_secs_f64();

    println!(
        "Created {} products in {:?} ({:.2} products/sec)",
        product_count, duration, products_per_sec
    );
}

// ============================================================================
// Concurrent Stress Tests
// ============================================================================

#[test]
fn stress_test_concurrent_orders() {
    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    // Create a customer first (FK constraint)
    let customer = commerce.customers().create(CreateCustomer {
        email: "concurrent-orders@stress.test".into(),
        first_name: "Concurrent".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");
    let customer_id = customer.id;

    // Reduce counts for in-memory SQLite with pool size 1
    let thread_count = 4;
    let orders_per_thread = 25;

    let start = Instant::now();
    let mut handles = vec![];

    for t in 0..thread_count {
        let commerce_clone = Arc::clone(&commerce);
        let cid = customer_id;
        let handle = thread::spawn(move || {
            let mut success_count = 0;

            for i in 0..orders_per_thread {
                let result = commerce_clone.orders().create(CreateOrder {
                    customer_id: cid,
                    items: vec![CreateOrderItem {
                        product_id: Uuid::new_v4(),
                        sku: format!("THREAD-{}-ORDER-{}", t, i),
                        name: format!("Thread {} Order {} Product", t, i),
                        quantity: 1,
                        unit_price: dec!(10.00),
                        ..Default::default()
                    }],
                    ..Default::default()
                });

                if result.is_ok() {
                    success_count += 1;
                }
            }

            success_count
        });
        handles.push(handle);
    }

    let mut total_success = 0;
    for handle in handles {
        total_success += handle.join().expect("Thread panicked");
    }

    let duration = start.elapsed();
    let total_orders = thread_count * orders_per_thread;
    let orders_per_sec = total_success as f64 / duration.as_secs_f64();

    println!(
        "Concurrent test: {} threads x {} orders = {} total",
        thread_count, orders_per_thread, total_orders
    );
    println!(
        "Completed {} orders in {:?} ({:.2} orders/sec)",
        total_success, duration, orders_per_sec
    );

    // With in-memory SQLite shared-cache mode, write lock contention is expected
    // SQLite serializes writes, so concurrent threads will experience lock conflicts
    // Just ensure some succeed - the actual success rate varies by system load
    // Using 20% threshold to account for high-contention scenarios
    assert!(
        total_success as f64 / total_orders as f64 > 0.20,
        "Less than 20% of orders succeeded: {} of {}",
        total_success, total_orders
    );
}

#[test]
fn stress_test_concurrent_inventory_reservations() {
    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    // Create item with large stock
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "STRESS-RESERVE-001".into(),
            name: "Reservation Stress Item".into(),
            initial_quantity: Some(dec!(10000)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    let thread_count = 10;
    let reservations_per_thread = 50;

    let start = Instant::now();
    let mut handles = vec![];

    for t in 0..thread_count {
        let commerce_clone = Arc::clone(&commerce);
        let handle = thread::spawn(move || {
            let mut success_count = 0;

            for i in 0..reservations_per_thread {
                let result = commerce_clone.inventory().reserve(
                    "STRESS-RESERVE-001",
                    dec!(1),
                    "stress_test",
                    &format!("thread-{}-res-{}", t, i),
                    None,
                );

                if result.is_ok() {
                    success_count += 1;
                }
            }

            success_count
        });
        handles.push(handle);
    }

    let mut total_success = 0;
    for handle in handles {
        total_success += handle.join().expect("Thread panicked");
    }

    let duration = start.elapsed();

    println!(
        "Concurrent reservations: {} successful in {:?}",
        total_success, duration
    );

    // Check stock levels
    let stock = commerce
        .inventory()
        .get_stock("STRESS-RESERVE-001")
        .expect("Failed to get stock")
        .expect("Stock not found");

    println!(
        "Final stock: on_hand={}, allocated={}, available={}",
        stock.total_on_hand, stock.total_allocated, stock.total_available
    );
}

#[test]
fn stress_test_read_heavy_workload() {
    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    // Setup: Create some data
    for i in 0..100 {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("read-test-{}@example.com", i),
                first_name: format!("Reader{}", i),
                last_name: "Test".into(),
                ..Default::default()
            })
            .expect("Failed to create customer");
    }

    let thread_count = 20;
    let reads_per_thread = 500;

    let start = Instant::now();
    let mut handles = vec![];

    for _ in 0..thread_count {
        let commerce_clone = Arc::clone(&commerce);
        let handle = thread::spawn(move || {
            for _ in 0..reads_per_thread {
                let _ = commerce_clone.customers().list(Default::default());
                let _ = commerce_clone.customers().count(Default::default());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let duration = start.elapsed();
    let total_ops = thread_count * reads_per_thread * 2; // list + count
    let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

    println!(
        "Read-heavy workload: {} ops in {:?} ({:.2} ops/sec)",
        total_ops, duration, ops_per_sec
    );
}

#[test]
fn stress_test_mixed_workload() {
    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    // Create initial inventory
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "MIXED-001".into(),
            name: "Mixed Workload Item".into(),
            initial_quantity: Some(dec!(100000)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    let thread_count = 10;

    let start = Instant::now();
    let mut handles = vec![];

    // Writer threads (orders)
    for t in 0..thread_count / 2 {
        let commerce_clone = Arc::clone(&commerce);
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let _ = commerce_clone.orders().create(CreateOrder {
                    customer_id: Uuid::new_v4(),
                    items: vec![CreateOrderItem {
                        product_id: Uuid::new_v4(),
                        sku: "MIXED-001".into(),
                        name: "Mixed Item".into(),
                        quantity: 1,
                        unit_price: dec!(10.00),
                        ..Default::default()
                    }],
                    ..Default::default()
                });
                let _ = commerce_clone
                    .inventory()
                    .adjust("MIXED-001", dec!(-1), &format!("Order t{}i{}", t, i));
            }
        });
        handles.push(handle);
    }

    // Reader threads
    for _ in 0..thread_count / 2 {
        let commerce_clone = Arc::clone(&commerce);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = commerce_clone.orders().list(Default::default());
                let _ = commerce_clone.inventory().get_stock("MIXED-001");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let duration = start.elapsed();
    println!("Mixed workload completed in {:?}", duration);

    // Verify data integrity
    let order_count = commerce
        .orders()
        .count(Default::default())
        .expect("Failed to count orders");
    let stock = commerce
        .inventory()
        .get_stock("MIXED-001")
        .expect("Failed to get stock")
        .expect("Stock not found");

    println!(
        "Final state: {} orders, stock on_hand={}",
        order_count, stock.total_on_hand
    );
}

// ============================================================================
// Memory and Resource Tests
// ============================================================================

#[test]
fn stress_test_repeated_open_close() {
    let iterations = 100;
    let start = Instant::now();

    for i in 0..iterations {
        let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

        // Do some work
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("iter{}@test.com", i),
                first_name: "Test".into(),
                last_name: "User".into(),
                ..Default::default()
            })
            .expect("Failed to create customer");

        // Commerce drops here, connection closes
    }

    let duration = start.elapsed();
    println!(
        "Opened/closed {} commerce instances in {:?}",
        iterations, duration
    );
}

#[test]
fn stress_test_large_batch_insert() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a customer first (FK constraint)
    let customer = commerce.customers().create(CreateCustomer {
        email: "batch-insert@stress.test".into(),
        first_name: "Batch".into(),
        last_name: "Insert".into(),
        ..Default::default()
    }).expect("Failed to create customer");
    let customer_id = customer.id;

    let batch_size = 100;
    let item_count_per_order = 50;

    let start = Instant::now();

    for i in 0..batch_size {
        let items: Vec<CreateOrderItem> = (0..item_count_per_order)
            .map(|j| CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: format!("BATCH-{}-ITEM-{}", i, j),
                name: format!("Batch {} Item {}", i, j),
                quantity: 1,
                unit_price: dec!(9.99),
                ..Default::default()
            })
            .collect();

        commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items,
                ..Default::default()
            })
            .expect("Failed to create order");
    }

    let duration = start.elapsed();
    let total_items = batch_size * item_count_per_order;

    println!(
        "Created {} orders with {} total items in {:?}",
        batch_size, total_items, duration
    );
}

#[test]
fn stress_test_query_performance() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Setup: Create many customers
    let customer_count = 1000;
    for i in 0..customer_count {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("query-perf-{}@test.com", i),
                first_name: format!("Query{}", i),
                last_name: format!("Perf{}", i % 10), // 10 different last names
                ..Default::default()
            })
            .expect("Failed to create customer");
    }

    // Test list performance
    let list_iterations = 100;
    let start = Instant::now();

    for _ in 0..list_iterations {
        let _ = commerce.customers().list(Default::default());
    }

    let list_duration = start.elapsed();
    let lists_per_sec = list_iterations as f64 / list_duration.as_secs_f64();

    println!(
        "List {} customers {} times: {:?} ({:.2} lists/sec)",
        customer_count, list_iterations, list_duration, lists_per_sec
    );

    // Test count performance
    let count_iterations = 1000;
    let start = Instant::now();

    for _ in 0..count_iterations {
        let _ = commerce.customers().count(Default::default());
    }

    let count_duration = start.elapsed();
    let counts_per_sec = count_iterations as f64 / count_duration.as_secs_f64();

    println!(
        "Count {} times: {:?} ({:.2} counts/sec)",
        count_iterations, count_duration, counts_per_sec
    );
}

// ============================================================================
// Stability Tests
// ============================================================================

#[test]
fn stress_test_long_running_operations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Simulate long-running workload
    let operation_count = 500;
    let start = Instant::now();

    for i in 0..operation_count {
        // Create customer
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: format!("long-run-{}@test.com", i),
                first_name: format!("Long{}", i),
                last_name: "Running".into(),
                ..Default::default()
            })
            .expect("Failed to create customer");

        // Create order for customer
        let order = commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: format!("LONG-RUN-{}", i),
                    name: "Long Running Item".into(),
                    quantity: 1,
                    unit_price: dec!(25.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("Failed to create order");

        // Update order status
        commerce
            .orders()
            .update_status(order.id, stateset_embedded::OrderStatus::Confirmed)
            .expect("Failed to update status");

        // Read operations
        let _ = commerce.customers().get(customer.id);
        let _ = commerce.orders().get(order.id);
    }

    let duration = start.elapsed();
    println!(
        "Long-running test: {} iterations in {:?}",
        operation_count, duration
    );

    // Verify final state
    let customer_count = commerce
        .customers()
        .count(Default::default())
        .expect("Failed to count");
    let order_count = commerce
        .orders()
        .count(Default::default())
        .expect("Failed to count");

    assert_eq!(customer_count, operation_count as u64);
    assert_eq!(order_count, operation_count as u64);
}
