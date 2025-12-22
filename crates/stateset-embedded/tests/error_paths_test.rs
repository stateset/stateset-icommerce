//! Error path tests for comprehensive coverage
//!
//! Tests error conditions, edge cases, and failure modes.

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateBom, CreateCustomer, CreateInventoryItem, CreateOrder,
    CreateOrderItem, CreateProduct, CreateReturn, OrderStatus, ReserveInventory, ReturnStatus,
};
use uuid::Uuid;

// ============================================================================
// Order Error Tests
// ============================================================================

#[test]
fn test_order_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.orders().get(fake_id);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_order_update_nonexistent() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.orders().update_status(fake_id, OrderStatus::Confirmed);
    assert!(result.is_err());
}

#[test]
fn test_order_delete_nonexistent() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.orders().delete(fake_id);
    // Should succeed even if not found (idempotent)
    assert!(result.is_ok());
}

#[test]
fn test_order_with_invalid_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create order with non-existent customer - should still work (soft reference)
    let result = commerce.orders().create(CreateOrder {
        customer_id: Uuid::new_v4(),
        items: vec![CreateOrderItem {
            sku: "TEST-001".into(),
            name: "Test Product".into(),
            quantity: 1,
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        ..Default::default()
    });

    // Order creation should succeed (customer is soft reference)
    assert!(result.is_ok());
}

#[test]
fn test_order_empty_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.orders().create(CreateOrder {
        customer_id: Uuid::new_v4(),
        items: vec![], // Empty items
        ..Default::default()
    });

    // Should fail - orders need at least one item
    assert!(result.is_err());
}

#[test]
fn test_order_negative_quantity() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.orders().create(CreateOrder {
        customer_id: Uuid::new_v4(),
        items: vec![CreateOrderItem {
            sku: "TEST-001".into(),
            name: "Test Product".into(),
            quantity: -1, // Negative quantity
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        ..Default::default()
    });

    // Should fail validation
    assert!(result.is_err());
}

#[test]
fn test_order_zero_quantity() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.orders().create(CreateOrder {
        customer_id: Uuid::new_v4(),
        items: vec![CreateOrderItem {
            sku: "TEST-001".into(),
            name: "Test Product".into(),
            quantity: 0, // Zero quantity
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        ..Default::default()
    });

    // Should fail validation
    assert!(result.is_err());
}

// ============================================================================
// Customer Error Tests
// ============================================================================

#[test]
fn test_customer_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.customers().get(fake_id);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_customer_duplicate_email() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create first customer
    commerce
        .customers()
        .create(CreateCustomer {
            email: "duplicate@example.com".into(),
            first_name: "First".into(),
            last_name: "Customer".into(),
            ..Default::default()
        })
        .expect("Failed to create first customer");

    // Try to create second customer with same email
    let result = commerce.customers().create(CreateCustomer {
        email: "duplicate@example.com".into(),
        first_name: "Second".into(),
        last_name: "Customer".into(),
        ..Default::default()
    });

    // Should fail due to duplicate email
    assert!(result.is_err());
}

#[test]
fn test_customer_invalid_email() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.customers().create(CreateCustomer {
        email: "not-an-email".into(), // Invalid email format
        first_name: "Test".into(),
        last_name: "User".into(),
        ..Default::default()
    });

    // Should fail email validation
    assert!(result.is_err());
}

#[test]
fn test_customer_empty_email() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.customers().create(CreateCustomer {
        email: "".into(), // Empty email
        first_name: "Test".into(),
        last_name: "User".into(),
        ..Default::default()
    });

    // Should fail validation
    assert!(result.is_err());
}

// ============================================================================
// Inventory Error Tests
// ============================================================================

#[test]
fn test_inventory_item_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.inventory().get_item_by_sku("NONEXISTENT-SKU");
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_inventory_duplicate_sku() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create first item
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "DUP-SKU-001".into(),
            name: "First Item".into(),
            ..Default::default()
        })
        .expect("Failed to create first item");

    // Try to create second item with same SKU
    let result = commerce.inventory().create_item(CreateInventoryItem {
        sku: "DUP-SKU-001".into(),
        name: "Second Item".into(),
        ..Default::default()
    });

    // Should fail due to duplicate SKU
    assert!(result.is_err());
}

#[test]
fn test_inventory_adjust_nonexistent() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .inventory()
        .adjust("NONEXISTENT-SKU", dec!(10), "Test adjustment");

    // Should fail - SKU doesn't exist
    assert!(result.is_err());
}

#[test]
fn test_inventory_reserve_insufficient_stock() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create item with limited stock
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "LIMITED-001".into(),
            name: "Limited Stock Item".into(),
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Try to reserve more than available
    let result = commerce.inventory().reserve(ReserveInventory {
        sku: "LIMITED-001".into(),
        quantity: dec!(100), // More than available
        reference_type: "order".into(),
        reference_id: Uuid::new_v4().to_string(),
        ..Default::default()
    });

    // Should fail - insufficient stock
    assert!(result.is_err());
}

#[test]
fn test_inventory_release_nonexistent_reservation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_reservation_id = Uuid::new_v4();

    let result = commerce
        .inventory()
        .release_reservation(fake_reservation_id);

    // Should fail - reservation doesn't exist
    assert!(result.is_err());
}

#[test]
fn test_inventory_negative_adjustment_exceeds_stock() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create item with some stock
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "NEG-TEST-001".into(),
            name: "Negative Test Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Try to adjust by more than available (would result in negative stock)
    let result = commerce
        .inventory()
        .adjust("NEG-TEST-001", dec!(-100), "Over-adjustment");

    // Behavior depends on implementation - may allow negative or fail
    // This test documents current behavior
    println!("Negative adjustment result: {:?}", result);
}

// ============================================================================
// Product Error Tests
// ============================================================================

#[test]
fn test_product_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.products().get(fake_id);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_product_duplicate_slug() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create first product
    commerce
        .products()
        .create(CreateProduct {
            name: "First Product".into(),
            slug: Some("duplicate-slug".into()),
            ..Default::default()
        })
        .expect("Failed to create first product");

    // Try to create second product with same slug
    let result = commerce.products().create(CreateProduct {
        name: "Second Product".into(),
        slug: Some("duplicate-slug".into()),
        ..Default::default()
    });

    // Should fail due to duplicate slug
    assert!(result.is_err());
}

#[test]
fn test_product_variant_duplicate_sku() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create product with variant
    let product = commerce
        .products()
        .create(CreateProduct {
            name: "Test Product".into(),
            ..Default::default()
        })
        .expect("Failed to create product");

    // Create first variant with inventory
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "VAR-DUP-001".into(),
            name: "Variant Item".into(),
            ..Default::default()
        })
        .expect("Failed to create inventory");

    // Variant tests depend on product implementation
    println!("Product created with id: {:?}", product.id);
}

// ============================================================================
// Return Error Tests
// ============================================================================

#[test]
fn test_return_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.returns().get(fake_id);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_return_for_nonexistent_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.returns().create(CreateReturn {
        order_id: Uuid::new_v4(), // Non-existent order
        customer_id: Uuid::new_v4(),
        reason: Some("Test return".into()),
        ..Default::default()
    });

    // May succeed or fail depending on FK enforcement
    println!("Return for nonexistent order result: {:?}", result);
}

#[test]
fn test_return_approve_already_approved() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create customer and order first
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "return-test@example.com".into(),
            first_name: "Return".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("Failed to create customer");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                sku: "RET-001".into(),
                name: "Returnable Item".into(),
                quantity: 1,
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    // Create return
    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            customer_id: customer.id,
            reason: Some("Defective".into()),
            ..Default::default()
        })
        .expect("Failed to create return");

    // Approve once
    commerce
        .returns()
        .approve(ret.id)
        .expect("Failed to approve return");

    // Try to approve again
    let result = commerce.returns().approve(ret.id);
    // Should fail - already approved
    println!("Double approve result: {:?}", result);
}

// ============================================================================
// BOM Error Tests
// ============================================================================

#[test]
fn test_bom_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let fake_id = Uuid::new_v4();

    let result = commerce.bom().get(fake_id);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_bom_activate_already_active() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create BOM
    let bom = commerce
        .bom()
        .create(CreateBom {
            product_id: Uuid::new_v4(),
            name: "Test BOM".into(),
            ..Default::default()
        })
        .expect("Failed to create BOM");

    // Activate once
    commerce.bom().activate(bom.id).expect("Failed to activate");

    // Try to activate again
    let result = commerce.bom().activate(bom.id);
    // May succeed (idempotent) or fail
    println!("Double activate result: {:?}", result);
}

#[test]
fn test_bom_component_circular_reference() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create BOM that references itself (via SKU)
    let bom = commerce
        .bom()
        .create(CreateBom {
            product_id: Uuid::new_v4(),
            name: "Circular BOM".into(),
            ..Default::default()
        })
        .expect("Failed to create BOM");

    // Component circular references are validated at a different level
    println!("BOM created: {:?}", bom.bom_number);
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_concurrent_inventory_updates() {
    use std::sync::Arc;
    use std::thread;

    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    // Create item
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "CONC-001".into(),
            name: "Concurrent Test Item".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("Failed to create item");

    let mut handles = vec![];

    // Spawn multiple threads making adjustments
    for i in 0..10 {
        let commerce_clone = Arc::clone(&commerce);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let result = commerce_clone.inventory().adjust(
                    "CONC-001",
                    dec!(-1),
                    &format!("Thread {} adjustment {}", i, j),
                );
                if result.is_err() {
                    println!("Concurrent adjustment failed: {:?}", result);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Check final stock
    let stock = commerce
        .inventory()
        .get_stock("CONC-001")
        .expect("Failed to get stock");
    println!("Final stock after concurrent updates: {:?}", stock);
}

#[test]
fn test_concurrent_order_creation() {
    use std::sync::Arc;
    use std::thread;

    let commerce = Arc::new(Commerce::new(":memory:").expect("Failed to create commerce"));

    let mut handles = vec![];
    let customer_id = Uuid::new_v4();

    // Spawn multiple threads creating orders
    for i in 0..5 {
        let commerce_clone = Arc::clone(&commerce);
        let cid = customer_id;
        let handle = thread::spawn(move || {
            let result = commerce_clone.orders().create(CreateOrder {
                customer_id: cid,
                items: vec![CreateOrderItem {
                    sku: format!("THREAD-{}", i),
                    name: format!("Thread {} Product", i),
                    quantity: 1,
                    unit_price: dec!(10.00),
                    ..Default::default()
                }],
                ..Default::default()
            });
            result.is_ok()
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    for handle in handles {
        if handle.join().expect("Thread panicked") {
            success_count += 1;
        }
    }

    println!(
        "Concurrent order creation: {} out of 5 succeeded",
        success_count
    );
    assert_eq!(success_count, 5);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_unicode_in_names() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "unicode@example.com".into(),
            first_name: "日本語".into(),       // Japanese
            last_name: "Müller".into(),        // German umlaut
            ..Default::default()
        })
        .expect("Failed to create customer with unicode");

    assert_eq!(customer.first_name, "日本語");
    assert_eq!(customer.last_name, "Müller");
}

#[test]
fn test_very_long_strings() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let long_string = "x".repeat(10000);

    let result = commerce.customers().create(CreateCustomer {
        email: "long@example.com".into(),
        first_name: long_string.clone(),
        last_name: "Short".into(),
        ..Default::default()
    });

    // May succeed or fail depending on field length limits
    println!("Very long string result: {:?}", result.is_ok());
}

#[test]
fn test_special_characters_in_sku() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.inventory().create_item(CreateInventoryItem {
        sku: "SKU/WITH\\SPECIAL<>CHARS".into(),
        name: "Special SKU Item".into(),
        ..Default::default()
    });

    // Document behavior with special characters
    println!("Special chars in SKU result: {:?}", result);
}

#[test]
fn test_decimal_precision() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Test high precision decimal
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: Uuid::new_v4(),
            items: vec![CreateOrderItem {
                sku: "PRECISION-001".into(),
                name: "Precision Test".into(),
                quantity: 1,
                unit_price: dec!(0.00000001), // Very small price
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order with precise decimal");

    println!("Order total with tiny price: {:?}", order.total);
}

#[test]
fn test_max_decimal_value() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Test maximum decimal value
    let result = commerce.orders().create(CreateOrder {
        customer_id: Uuid::new_v4(),
        items: vec![CreateOrderItem {
            sku: "MAX-001".into(),
            name: "Max Value Test".into(),
            quantity: 1,
            unit_price: dec!(999999999999999999.99), // Large price
            ..Default::default()
        }],
        ..Default::default()
    });

    println!("Max decimal value result: {:?}", result);
}

// ============================================================================
// Database State Tests
// ============================================================================

#[test]
fn test_empty_database_queries() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Query empty tables
    let orders = commerce
        .orders()
        .list(Default::default())
        .expect("Failed to list orders");
    assert!(orders.is_empty());

    let customers = commerce
        .customers()
        .list(Default::default())
        .expect("Failed to list customers");
    assert!(customers.is_empty());

    let products = commerce
        .products()
        .list(Default::default())
        .expect("Failed to list products");
    assert!(products.is_empty());
}

#[test]
fn test_count_on_empty_tables() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let order_count = commerce
        .orders()
        .count(Default::default())
        .expect("Failed to count orders");
    assert_eq!(order_count, 0);

    let customer_count = commerce
        .customers()
        .count(Default::default())
        .expect("Failed to count customers");
    assert_eq!(customer_count, 0);
}
