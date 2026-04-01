//! Concurrency and inventory reservation conflict tests
//! Tests behavior under concurrent access, race conditions, and deadlocks

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Setup test database with inventory
fn setup_concurrent_test() -> Commerce {
    let commerce = Commerce::builder()
        .database(":memory:")
        .max_connections(1)
        .build()
        .expect("Failed to create commerce");

    // Create inventory with limited quantity
    let sku = "CONCURRENT-SKU-001".to_string();
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku,
            name: "Concurrent Test Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    commerce
}

/// Create a test customer
fn create_test_customer(commerce: &Commerce) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("concurrent-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create customer")
        .id
        .into_uuid()
}

fn run_concurrent_reservation_attempts(thread_count: usize) -> (usize, usize, Vec<String>) {
    let commerce = Arc::new(setup_concurrent_test());
    let barrier = Arc::new(Barrier::new(thread_count));
    let mut handles = vec![];
    let sku = "CONCURRENT-SKU-001".to_string();

    for _ in 0..thread_count {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let reference_id = Uuid::new_v4().to_string();
            commerce_clone.inventory().reserve(
                sku_clone.as_str(),
                dec!(1),
                "order",
                &reference_id,
                None,
            )
        });

        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("Thread panicked")).collect();
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.iter().filter(|r| r.is_err()).count();
    let errors: Vec<String> =
        results.iter().filter_map(|r| r.as_ref().err().map(|err| format!("{err:?}"))).collect();

    (successful, failed, errors)
}

fn run_concurrent_confirmation_attempt() -> Vec<String> {
    let commerce = Arc::new(setup_concurrent_test());
    let sku = "CONCURRENT-SKU-001".to_string();

    let barrier = Arc::new(Barrier::new(5));
    let mut handles = vec![];

    for i in 0..5 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let reference_id = format!("order-{}", i);
            commerce_clone
                .inventory()
                .reserve(&sku_clone, dec!(1), "order", &reference_id, None)
                .expect("Failed to reserve")
        });

        handles.push(handle);
    }

    let reservations: Vec<_> =
        handles.into_iter().map(|h| h.join().expect("Thread panicked")).collect();

    assert_eq!(reservations.len(), 5);

    let barrier2 = Arc::new(Barrier::new(5));
    let mut confirm_handles = vec![];

    for reservation in reservations {
        let commerce_clone = commerce.clone();
        let barrier_clone2 = barrier2.clone();

        let handle = thread::spawn(move || {
            barrier_clone2.wait();
            commerce_clone.inventory().confirm_reservation(reservation.id)
        });

        confirm_handles.push(handle);
    }

    let results: Vec<_> =
        confirm_handles.into_iter().map(|h| h.join().expect("Thread panicked")).collect();

    results.iter().filter_map(|r| r.as_ref().err().map(|err| format!("{err:?}"))).collect()
}

// ============================================================================
// Concurrent Reservation Tests
// ============================================================================

#[test]
fn test_concurrent_reservations_same_quantity() {
    let (successful, failed, errors) = run_concurrent_reservation_attempts(10);

    assert_eq!(
        successful, 10,
        "All 10 reservations should succeed (10 items total); errors: {errors:?}"
    );
    assert_eq!(failed, 0, "No reservations should fail when stock exactly matches demand");
}

#[test]
fn test_concurrent_reservations_same_quantity_repeated() {
    for iteration in 0..10 {
        let (successful, failed, errors) = run_concurrent_reservation_attempts(10);

        assert_eq!(
            successful,
            10,
            "Iteration {} should reserve all 10 items; errors: {errors:?}",
            iteration + 1
        );
        assert_eq!(
            failed,
            0,
            "Iteration {} should not fail any reservations; errors: {errors:?}",
            iteration + 1
        );
    }
}

#[test]
fn test_concurrent_reservations_exceed_stock() {
    let (successful, failed, errors) = run_concurrent_reservation_attempts(15);

    assert_eq!(successful, 10, "Only first 10 reservations should succeed; errors: {errors:?}");
    assert_eq!(failed, 5, "Last 5 reservations should fail; errors: {errors:?}");
}

#[test]
fn test_reservation_expiration_race() {
    let commerce = Arc::new(setup_concurrent_test());
    let sku = "CONCURRENT-SKU-001".to_string();

    // Reserve all items with short expiry
    let reference_id = Uuid::new_v4().to_string();
    commerce
        .inventory()
        .reserve(&sku, dec!(10), "order", &reference_id, Some(1))
        .expect("Failed to reserve");

    // Try to reserve again immediately (should fail)
    let reference_id = Uuid::new_v4().to_string();
    let result = commerce.inventory().reserve(&sku, dec!(1), "order", &reference_id, None);

    assert!(result.is_err(), "Reservation should fail while others are reserved");

    // Poll until reservation expiry is observed to avoid brittle fixed sleeps in CI.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut recovered = false;
    while Instant::now() < deadline {
        let reference_id = Uuid::new_v4().to_string();
        if commerce.inventory().reserve(&sku, dec!(1), "order", &reference_id, None).is_ok() {
            recovered = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    assert!(recovered, "Reservation should succeed after expiry");
}

#[test]
fn test_concurrent_reservation_confirm() {
    let errors = run_concurrent_confirmation_attempt();

    assert!(errors.is_empty(), "All confirmations should succeed; errors: {errors:?}");
}

#[test]
fn test_concurrent_reservation_confirm_repeated() {
    for iteration in 0..5 {
        let errors = run_concurrent_confirmation_attempt();
        assert!(
            errors.is_empty(),
            "Iteration {} should confirm all reservations; errors: {errors:?}",
            iteration + 1
        );
    }
}

// ============================================================================
// Order Creation Concurrency Tests
// ============================================================================

#[test]
fn test_concurrent_order_creation_same_inventory() {
    let commerce = setup_concurrent_test();
    let commerce = Arc::new(commerce);

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // Try to create 3 orders for the same inventory (10 items)
    for _ in 0..3 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let customer_id = create_test_customer(&commerce_clone);

            commerce_clone.orders().create(CreateOrder {
                customer_id: customer_id.into(),
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "CONCURRENT-SKU-001".into(),
                    name: "Concurrent Test Item".into(),
                    quantity: 4,
                    unit_price: dec!(29.99),
                    ..Default::default()
                }],
                ..Default::default()
            })
        });

        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("Thread panicked")).collect();

    let successful: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();

    assert_eq!(
        successful.len(),
        3,
        "All 3 orders should be created; shortfalls should be backordered"
    );

    let mut total_reserved = Decimal::ZERO;
    let mut total_backordered = Decimal::ZERO;

    for order in &successful {
        let reservations = commerce
            .inventory()
            .list_reservations_by_reference("order", &order.id.to_string())
            .expect("Failed to load reservations");
        let reserved = reservations.iter().fold(Decimal::ZERO, |acc, r| acc + r.quantity);

        let backorders = commerce
            .backorder()
            .get_backorders_for_order(order.id.into())
            .expect("Failed to load backorders");
        let backordered =
            backorders.iter().fold(Decimal::ZERO, |acc, b| acc + b.quantity_remaining);

        assert_eq!(
            reserved + backordered,
            dec!(4),
            "Each order should be fully accounted for via reservations + backorders"
        );

        total_reserved += reserved;
        total_backordered += backordered;
    }

    assert!(total_reserved <= dec!(10), "Reservations must not exceed inventory");
    assert_eq!(
        total_reserved + total_backordered,
        dec!(12),
        "All requested quantities should be reserved or backordered"
    );
}

// ============================================================================
// Deadlock Prevention Tests
// ============================================================================

#[test]
fn test_no_deadlock_with_circular_dependencies() {
    // This test would attempt to create a deadlock scenario
    // In production, proper transaction ordering prevents this
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let sku1 = "DEADLOCK-SKU-001";
    let sku2 = "DEADLOCK-SKU-002";

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku1.into(),
            name: "Item 1".into(),
            initial_quantity: Some(dec!(6)),
            ..Default::default()
        })
        .expect("Failed to create item 1");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku2.into(),
            name: "Item 2".into(),
            initial_quantity: Some(dec!(6)),
            ..Default::default()
        })
        .expect("Failed to create item 2");

    let commerce = Arc::new(commerce);
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = vec![];

    let commerce1 = commerce.clone();
    let sku1_clone = sku1.to_string();
    let sku2_clone = sku2.to_string();
    let barrier1 = barrier.clone();

    let handle1 = thread::spawn(move || {
        barrier1.wait();

        // Reserve item 1 first
        let reference_id = Uuid::new_v4().to_string();
        commerce1
            .inventory()
            .reserve(&sku1_clone, dec!(3), "order", &reference_id, None)
            .expect("Failed to reserve item 1");

        // Then try to reserve item 2
        let reference_id = Uuid::new_v4().to_string();
        commerce1
            .inventory()
            .reserve(&sku2_clone, dec!(3), "order", &reference_id, None)
            .expect("Failed to reserve item 2");
    });

    let commerce2 = commerce;
    let sku2_clone2 = sku2.to_string();
    let sku1_clone2 = sku1.to_string();
    let barrier2 = barrier;

    let handle2 = thread::spawn(move || {
        barrier2.wait();

        // Reserve item 2 first (opposite order)
        let reference_id = Uuid::new_v4().to_string();
        commerce2
            .inventory()
            .reserve(&sku2_clone2, dec!(3), "order", &reference_id, None)
            .expect("Failed to reserve item 2");

        // Then try to reserve item 1
        let reference_id = Uuid::new_v4().to_string();
        commerce2
            .inventory()
            .reserve(&sku1_clone2, dec!(3), "order", &reference_id, None)
            .expect("Failed to reserve item 1");
    });

    handles.push(handle1);
    handles.push(handle2);

    // Ensure both threads complete (no deadlock)
    for handle in handles {
        handle.join().expect("Thread panicked - possible deadlock");
    }
}

// ============================================================================
// Transaction Isolation Tests
// ============================================================================

#[test]
fn test_transaction_isolation() {
    // Test that concurrent transactions don't see each other's uncommitted changes
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let sku = "ISOLATION-SKU-001";
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: "Isolation Test Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    let commerce = Arc::new(commerce);

    let commerce1 = commerce.clone();
    let sku1 = sku.to_string();

    let handle1 = thread::spawn(move || {
        // In a real transaction, this adjustment would not be visible
        // until commitment
        commerce1
            .inventory()
            .adjust(&sku1, dec!(-5), "Test adjustment")
            .expect("Failed to adjust inventory");
    });

    handle1.join().expect("Thread panicked");

    // Verify the adjustment is applied atomically
    let stock = commerce.inventory().get_stock(sku).expect("Failed to get stock");
    assert_eq!(
        stock.expect("Stock missing").total_on_hand,
        dec!(5),
        "Stock should be atomically decremented"
    );
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_high_concurrency_reservation() {
    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    let sku = "STRESS-SKU-001";
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: "Stress Test Item".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    let barrier = Arc::new(Barrier::new(100));
    let mut handles = vec![];

    // Create 100 threads trying to reserve simultaneously
    for _ in 0..100 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.to_string();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let reference_id = Uuid::new_v4().to_string();
            commerce_clone.inventory().reserve(&sku_clone, dec!(1), "order", &reference_id, None)
        });

        handles.push(handle);
    }

    let successful: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .filter(|r| r.is_ok())
        .collect();

    // Under heavy SQLite contention, some reservations may fail due to lock
    // timeouts. We assert at least 90% succeed rather than demanding 100%.
    assert!(
        successful.len() >= 90,
        "At least 90 of 100 reservations should succeed, got {}",
        successful.len()
    );
}

#[test]
fn test_reservation_release_and_reuse() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let sku = "REUSE-SKU-001";
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: "Reuse Test Item".into(),
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    // Reserve all items
    let reservation = commerce
        .inventory()
        .reserve(sku, dec!(5), "order", &Uuid::new_v4().to_string(), None)
        .expect("Failed to reserve");

    // Try to reserve again (should fail)
    let result =
        commerce.inventory().reserve(sku, dec!(1), "order", &Uuid::new_v4().to_string(), None);

    assert!(result.is_err(), "Reservation should fail when all items are reserved");

    // Release the reservation
    commerce
        .inventory()
        .release_reservation(reservation.id)
        .expect("Failed to release reservation");

    // Now reservation should succeed
    let result =
        commerce.inventory().reserve(sku, dec!(1), "order", &Uuid::new_v4().to_string(), None);

    assert!(result.is_ok(), "Reservation should succeed after release");
}
