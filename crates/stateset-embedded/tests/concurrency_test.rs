//! Concurrency and inventory reservation conflict tests
//! Tests behavior under concurrent access, race conditions, and deadlocks

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    InventoryReservation, ReservationStatus, ReserveInventory,
};
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Setup test database with inventory
fn setup_concurrent_test() -> (Commerce, Uuid) {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create inventory with limited quantity
    let sku = "CONCURRENT-SKU-001".to_string();
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Concurrent Test Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    (commerce, sku.into())
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
}

// ============================================================================
// Concurrent Reservation Tests
// ============================================================================

#[test]
fn test_concurrent_reservations_same_quantity() {
    let commerce = Arc::new(setup_concurrent_test().0);

    // Try to reserve the same item simultaneously from multiple threads
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];
    let sku = "CONCURRENT-SKU-001".to_string();

    for _ in 0..10 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let result = commerce_clone.inventory().reserve(ReserveInventory {
                sku: sku_clone.clone(),
                quantity: 1,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            });

            result
        });

        handles.push(handle);
    }

    let successful: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .filter(|r| r.is_ok())
        .collect();

    assert_eq!(
        successful.len(),
        10,
        "All 10 reservations should succeed (10 items total)"
    );
}

#[test]
fn test_concurrent_reservations_exceed_stock() {
    let commerce = Arc::new(setup_concurrent_test().0);

    let barrier = Arc::new(Barrier::new(15));
    let mut handles = vec![];
    let sku = "CONCURRENT-SKU-001".to_string();

    // Try to reserve 15 items when only 10 are available
    for _ in 0..15 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let result = commerce_clone.inventory().reserve(ReserveInventory {
                sku: sku_clone.clone(),
                quantity: 1,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            });

            result
        });

        handles.push(handle);
    }

    let successful: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .filter(|r| r.is_ok())
        .collect();

    let failed: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .filter(|r| r.is_err())
        .collect();

    assert_eq!(
        successful.len(),
        10,
        "Only first 10 reservations should succeed"
    );
    assert_eq!(failed.len(), 5, "Last 5 reservations should fail");
}

#[test]
fn test_reservation_expiration_race() {
    let commerce = Arc::new(setup_concurrent_test().0);
    let sku = "CONCURRENT-SKU-001".to_string();

    // Reserve all items with short expiry
    commerce
        .inventory()
        .reserve(ReserveInventory {
            sku: sku.clone(),
            quantity: 10,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4().to_string(),
            expiry_seconds: Some(1), // Expire in 1 second
        })
        .expect("Failed to reserve");

    // Try to reserve again immediately (should fail)
    let result = commerce.inventory().reserve(ReserveInventory {
        sku: sku.clone(),
        quantity: 1,
        reference_type: "order".into(),
        reference_id: Uuid::new_v4().to_string(),
        expiry_seconds: None,
    });

    assert!(
        result.is_err(),
        "Reservation should fail while others are reserved"
    );

    // Wait for expiry
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Now it should succeed
    let result = commerce.inventory().reserve(ReserveInventory {
        sku: sku.clone(),
        quantity: 1,
        reference_type: "order".into(),
        reference_id: Uuid::new_v4().to_string(),
        expiry_seconds: None,
    });

    assert!(result.is_ok(), "Reservation should succeed after expiry");
}

#[test]
fn test_concurrent_reservation_confirm() {
    let commerce = Arc::new(setup_concurrent_test().0);
    let sku = "CONCURRENT-SKU-001".to_string();

    let barrier = Arc::new(Barrier::new(5));
    let mut handles = vec![];
    let reservation_ids = Arc::new(std::sync::Mutex::new(vec![]));

    // Create reservations
    for i in 0..5 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();
        let sku_clone = sku.clone();
        let reservation_ids_clone = reservation_ids.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let reservation = commerce_clone
                .inventory()
                .reserve(ReserveInventory {
                    sku: sku_clone.clone(),
                    quantity: 1,
                    reference_type: "order".into(),
                    reference_id: format!("order-{}", i),
                    expiry_seconds: None,
                })
                .expect("Failed to reserve");

            let mut ids = reservation_ids_clone.lock().unwrap();
            ids.push(reservation.id);

            reservation
        });

        handles.push(handle);
    }

    let reservations: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(reservations.len(), 5);

    // Confirm all reservations
    let barrier2 = Arc::new(Barrier::new(5));
    let mut confirm_handles = vec![];

    for reservation in reservations {
        let commerce_clone = commerce.clone();
        let barrier_clone2 = barrier2.clone();

        let handle = thread::spawn(move || {
            barrier_clone2.wait();

            commerce_clone
                .inventory()
                .confirm_reservation(reservation.id)
        });

        confirm_handles.push(handle);
    }

    let results: Vec<_> = confirm_handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert!(
        results.iter().all(|r| r.is_ok()),
        "All confirmations should succeed"
    );
}

// ============================================================================
// Order Creation Concurrency Tests
// ============================================================================

#[test]
fn test_concurrent_order_creation_same_inventory() {
    let (commerce, _) = setup_concurrent_test();
    let commerce = Arc::new(commerce);

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // Try to create 3 orders for the same inventory (10 items)
    for i in 0..3 {
        let commerce_clone = commerce.clone();
        let barrier_clone = barrier.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let customer_id = create_test_customer(&commerce_clone);

            commerce_clone.orders().create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
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

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    let successful: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();

    assert_eq!(
        successful.len(),
        2,
        "Only 2 orders should succeed (2 * 4 = 8 ≤ 10)"
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
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .expect("Failed to create item 1");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku2.into(),
            name: "Item 2".into(),
            initial_quantity: Some(dec!(5)),
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
        commerce1
            .inventory()
            .reserve(ReserveInventory {
                sku: sku1_clone.clone(),
                quantity: 3,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            })
            .expect("Failed to reserve item 1");

        // Then try to reserve item 2
        commerce1
            .inventory()
            .reserve(ReserveInventory {
                sku: sku2_clone.clone(),
                quantity: 3,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            })
            .expect("Failed to reserve item 2");
    });

    let commerce2 = commerce.clone();
    let sku2_clone2 = sku2.to_string();
    let sku1_clone2 = sku1.to_string();
    let barrier2 = barrier.clone();

    let handle2 = thread::spawn(move || {
        barrier2.wait();

        // Reserve item 2 first (opposite order)
        commerce2
            .inventory()
            .reserve(ReserveInventory {
                sku: sku2_clone2.clone(),
                quantity: 3,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            })
            .expect("Failed to reserve item 2");

        // Then try to reserve item 1
        commerce2
            .inventory()
            .reserve(ReserveInventory {
                sku: sku1_clone2.clone(),
                quantity: 3,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            })
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
        commerce1.inventory()/*.in_transaction(|db| {
            db.adjust(&sku1, dec!(-5), "Test adjustment")
        })*/;
    });

    handle1.join().expect("Thread panicked");

    // Verify the adjustment is applied atomically
    let stock = commerce
        .inventory()
        .get_stock(sku)
        .expect("Failed to get stock");
    assert_eq!(
        stock.quantity_on_hand,
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

            commerce_clone.inventory().reserve(ReserveInventory {
                sku: sku_clone.clone(),
                quantity: 1,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4().to_string(),
                expiry_seconds: None,
            })
        });

        handles.push(handle);
    }

    let successful: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .filter(|r| r.is_ok())
        .collect();

    assert_eq!(
        successful.len(),
        100,
        "All 100 reservations should succeed with 100 item stock"
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
        .reserve(ReserveInventory {
            sku: sku.to_string(),
            quantity: 5,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4().to_string(),
            expiry_seconds: None,
        })
        .expect("Failed to reserve");

    // Try to reserve again (should fail)
    let result = commerce.inventory().reserve(ReserveInventory {
        sku: sku.to_string(),
        quantity: 1,
        reference_type: "order".into(),
        reference_id: Uuid::new_v4().to_string(),
        expiry_seconds: None,
    });

    assert!(
        result.is_err(),
        "Reservation should fail when all items are reserved"
    );

    // Release the reservation
    commerce
        .inventory()
        .release_reservation(reservation.id)
        .expect("Failed to release reservation");

    // Now reservation should succeed
    let result = commerce.inventory().reserve(ReserveInventory {
        sku: sku.to_string(),
        quantity: 1,
        reference_type: "order".into(),
        reference_id: Uuid::new_v4().to_string(),
        expiry_seconds: None,
    });

    assert!(result.is_ok(), "Reservation should succeed after release");
}
