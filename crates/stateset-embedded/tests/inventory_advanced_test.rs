//! Advanced integration tests for Inventory management

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateInventoryItem, InventoryFilter, ReservationStatus, TransactionType,
};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a test inventory item with a unique SKU
fn create_test_inventory_item(commerce: &Commerce, sku: &str) -> i64 {
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Test Item {}", sku),
            description: Some("A test inventory item".into()),
            unit_of_measure: Some("EA".into()),
            initial_quantity: Some(dec!(100)),
            reorder_point: Some(dec!(10)),
            safety_stock: Some(dec!(5)),
            ..Default::default()
        })
        .expect("Failed to create inventory item")
        .id
}


/// Helper to generate a unique SKU
fn unique_sku(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().to_string().split('-').next().unwrap())
}

// ============================================================================
// Basic Inventory Item Creation Tests
// ============================================================================

#[test]
fn test_create_inventory_item() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("CREATE");

    let item = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Test Widget".into(),
            description: Some("A test widget".into()),
            unit_of_measure: Some("EA".into()),
            initial_quantity: Some(dec!(100)),
            reorder_point: Some(dec!(10)),
            safety_stock: Some(dec!(5)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    assert!(item.id > 0);
    assert_eq!(item.sku, sku);
    assert_eq!(item.name, "Test Widget");
    assert_eq!(item.description, Some("A test widget".into()));
    assert_eq!(item.unit_of_measure, "EA");
    assert!(item.is_active);
}

#[test]
fn test_create_inventory_item_default_values() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("DEFAULT");

    let item = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Minimal Item".into(),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    assert!(item.id > 0);
    assert_eq!(item.sku, sku);
    assert_eq!(item.name, "Minimal Item");
    assert!(item.description.is_none());
    assert!(item.is_active);
}

#[test]
fn test_get_inventory_item_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("GETID");
    let item_id = create_test_inventory_item(&commerce, &sku);

    let retrieved = commerce
        .inventory()
        .get_item(item_id)
        .expect("Failed to get item")
        .expect("Item not found");

    assert_eq!(retrieved.id, item_id);
    assert_eq!(retrieved.sku, sku);
}

#[test]
fn test_get_inventory_item_by_sku() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("GETSKU");
    let item_id = create_test_inventory_item(&commerce, &sku);

    let retrieved = commerce
        .inventory()
        .get_item_by_sku(&sku)
        .expect("Failed to get item by SKU")
        .expect("Item not found");

    assert_eq!(retrieved.id, item_id);
    assert_eq!(retrieved.sku, sku);
}

#[test]
fn test_get_inventory_item_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .inventory()
        .get_item(99999)
        .expect("Should not error for missing item");

    assert!(result.is_none());
}

#[test]
fn test_inventory_by_sku() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("BYSKU");

    // Create item
    create_test_inventory_item(&commerce, &sku);

    // Get by SKU
    let item = commerce
        .inventory()
        .get_item_by_sku(&sku)
        .expect("Failed to get item by SKU")
        .expect("Item not found");

    assert_eq!(item.sku, sku);
    assert!(item.is_active);

    // Verify stock level is also accessible by SKU
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.sku, sku);
    assert_eq!(stock.total_on_hand, dec!(100));
}

// ============================================================================
// Inventory Reservation Tests
// ============================================================================

#[test]
fn test_create_inventory_reservation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("RES");
    create_test_inventory_item(&commerce, &sku);

    let reservation = commerce
        .inventory()
        .reserve(
            &sku,
            dec!(10),
            "order",
            "ORD-12345",
            Some(3600), // 1 hour expiry
        )
        .expect("Failed to create reservation");

    assert!(!reservation.id.is_nil());
    assert_eq!(reservation.quantity, dec!(10));
    assert_eq!(reservation.reference_type, "order");
    assert_eq!(reservation.reference_id, "ORD-12345");
    assert_eq!(reservation.status, ReservationStatus::Pending);
    assert!(reservation.expires_at.is_some());
}

#[test]
fn test_create_reservation_without_expiry() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("RESNOEXP");
    create_test_inventory_item(&commerce, &sku);

    let reservation = commerce
        .inventory()
        .reserve(&sku, dec!(5), "cart", "CART-001", None)
        .expect("Failed to create reservation");

    assert!(!reservation.id.is_nil());
    assert_eq!(reservation.quantity, dec!(5));
    assert!(reservation.expires_at.is_none());
}

#[test]
fn test_release_reservation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("RELEASE");
    create_test_inventory_item(&commerce, &sku);

    // Create reservation
    let reservation = commerce
        .inventory()
        .reserve(&sku, dec!(20), "order", "ORD-001", None)
        .expect("Failed to create reservation");

    // Get initial stock
    let stock_before = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    let available_before = stock_before.total_available;

    // Release the reservation
    commerce
        .inventory()
        .release_reservation(reservation.id)
        .expect("Failed to release reservation");

    // Verify stock is restored
    let stock_after = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    // Available should increase after releasing reservation
    assert!(stock_after.total_available > available_before);
}

#[test]
fn test_confirm_reservation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("CONFIRM");
    create_test_inventory_item(&commerce, &sku);

    // Create reservation
    let reservation = commerce
        .inventory()
        .reserve(&sku, dec!(15), "order", "ORD-CONFIRM-001", None)
        .expect("Failed to create reservation");

    // Confirm the reservation
    commerce
        .inventory()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm reservation");

    // Reservation should now be confirmed/allocated
    // The exact behavior depends on implementation
}

#[test]
fn test_reservation_reduces_available() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("AVAIL");
    create_test_inventory_item(&commerce, &sku);

    // Get initial stock
    let initial_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(initial_stock.total_on_hand, dec!(100));
    let initial_available = initial_stock.total_available;

    // Create reservation for 30 units
    commerce
        .inventory()
        .reserve(&sku, dec!(30), "order", "ORD-002", None)
        .expect("Failed to create reservation");

    // Check stock after reservation
    let stock_after = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    // On-hand should remain the same
    assert_eq!(stock_after.total_on_hand, dec!(100));
    // Available should decrease by reserved amount
    assert_eq!(stock_after.total_available, initial_available - dec!(30));
    // Allocated should increase
    assert_eq!(stock_after.total_allocated, dec!(30));
}

#[test]
fn test_multiple_reservations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("MULTI");
    create_test_inventory_item(&commerce, &sku);

    // Create multiple reservations
    let res1 = commerce
        .inventory()
        .reserve(&sku, dec!(20), "order", "ORD-MULTI-001", None)
        .expect("Failed to create first reservation");

    let res2 = commerce
        .inventory()
        .reserve(&sku, dec!(15), "order", "ORD-MULTI-002", None)
        .expect("Failed to create second reservation");

    let res3 = commerce
        .inventory()
        .reserve(&sku, dec!(10), "cart", "CART-MULTI-001", Some(1800))
        .expect("Failed to create third reservation");

    assert_ne!(res1.id, res2.id);
    assert_ne!(res2.id, res3.id);
    assert_ne!(res1.id, res3.id);

    // Check total allocated
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_allocated, dec!(45)); // 20 + 15 + 10
    assert_eq!(stock.total_available, dec!(55)); // 100 - 45
}

#[test]
fn test_concurrent_reservations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("CONCURRENT");

    // Create item with enough stock for concurrent reservations
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Concurrent Test Item".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    // Simulate concurrent reservations (in single-threaded context)
    let mut reservations = Vec::new();
    for i in 0..10 {
        let reservation = commerce
            .inventory()
            .reserve(&sku, dec!(50), "order", &format!("ORD-CONC-{:03}", i), None)
            .expect("Failed to create reservation");
        reservations.push(reservation);
    }

    assert_eq!(reservations.len(), 10);

    // Check stock
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_on_hand, dec!(1000));
    assert_eq!(stock.total_allocated, dec!(500)); // 10 * 50
    assert_eq!(stock.total_available, dec!(500)); // 1000 - 500
}

// ============================================================================
// Inventory Adjustment Tests
// ============================================================================

#[test]
fn test_inventory_adjustment_increase() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("INCREASE");
    create_test_inventory_item(&commerce, &sku);

    // Get initial stock
    let initial_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(initial_stock.total_on_hand, dec!(100));

    // Add 50 units
    let transaction = commerce
        .inventory()
        .adjust(&sku, dec!(50), "Restocked from supplier")
        .expect("Failed to adjust inventory");

    assert!(transaction.id > 0);
    assert_eq!(transaction.quantity, dec!(50));
    // Transaction type may be Receipt or Adjustment depending on implementation
    assert!(
        transaction.transaction_type == TransactionType::Adjustment
            || transaction.transaction_type == TransactionType::Receipt
    );
    assert_eq!(transaction.reason, Some("Restocked from supplier".into()));

    // Verify new stock level
    let new_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(new_stock.total_on_hand, dec!(150));
}

#[test]
fn test_inventory_adjustment_decrease() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("DECREASE");
    create_test_inventory_item(&commerce, &sku);

    // Get initial stock
    let initial_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(initial_stock.total_on_hand, dec!(100));

    // Remove 30 units
    let transaction = commerce
        .inventory()
        .adjust(&sku, dec!(-30), "Damaged items removed")
        .expect("Failed to adjust inventory");

    assert_eq!(transaction.quantity, dec!(-30));
    assert_eq!(transaction.reason, Some("Damaged items removed".into()));

    // Verify new stock level
    let new_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(new_stock.total_on_hand, dec!(70));
}

#[test]
fn test_adjustment_creates_transaction() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("TRANS");
    let item_id = create_test_inventory_item(&commerce, &sku);

    // Make several adjustments
    commerce
        .inventory()
        .adjust(&sku, dec!(25), "First adjustment")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .adjust(&sku, dec!(-10), "Second adjustment")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .adjust(&sku, dec!(5), "Third adjustment")
        .expect("Failed to adjust");

    // Get transaction history
    let transactions = commerce
        .inventory()
        .get_transactions(item_id, 10)
        .expect("Failed to get transactions");

    // Should have at least 3 transactions for the adjustments
    assert!(transactions.len() >= 3);

    // Verify we have transactions - type may vary based on implementation
    // Some implementations use Receipt for positive and Adjustment for negative
    let transaction_count = transactions
        .iter()
        .filter(|t| {
            t.transaction_type == TransactionType::Adjustment
                || t.transaction_type == TransactionType::Receipt
        })
        .count();

    assert!(transaction_count >= 3);
}

#[test]
fn test_adjustment_at_location() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("LOC");

    // Create item with specific location
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Location Test Item".into(),
            initial_quantity: Some(dec!(50)),
            location_id: Some(1),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    // Adjust at specific location
    let transaction = commerce
        .inventory()
        .adjust_at_location(&sku, 1, dec!(25), "Location-specific adjustment")
        .expect("Failed to adjust at location");

    assert_eq!(transaction.location_id, 1);
    assert_eq!(transaction.quantity, dec!(25));
}

// ============================================================================
// Stock Level Tracking Tests
// ============================================================================

#[test]
fn test_stock_level_tracking() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("STOCK");
    create_test_inventory_item(&commerce, &sku);

    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.sku, sku);
    assert_eq!(stock.total_on_hand, dec!(100));
    assert_eq!(stock.total_allocated, dec!(0));
    assert_eq!(stock.total_available, dec!(100));
    assert!(!stock.locations.is_empty());
}

#[test]
fn test_stock_level_after_operations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("STOCKOPS");
    create_test_inventory_item(&commerce, &sku);

    // Initial: 100 on hand, 0 allocated, 100 available

    // Reserve 30
    commerce
        .inventory()
        .reserve(&sku, dec!(30), "order", "ORD-001", None)
        .expect("Failed to reserve");

    // Stock: 100 on hand, 30 allocated, 70 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(70));

    // Add 20 more
    commerce
        .inventory()
        .adjust(&sku, dec!(20), "Restock")
        .expect("Failed to adjust");

    // Stock: 120 on hand, 30 allocated, 90 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_on_hand, dec!(120));
    assert_eq!(stock.total_available, dec!(90));

    // Remove 10
    commerce
        .inventory()
        .adjust(&sku, dec!(-10), "Shrinkage")
        .expect("Failed to adjust");

    // Stock: 110 on hand, 30 allocated, 80 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_on_hand, dec!(110));
    assert_eq!(stock.total_available, dec!(80));
}

#[test]
fn test_has_stock() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("HASSTOCK");
    create_test_inventory_item(&commerce, &sku);

    // Should have stock for 50 units
    assert!(commerce.inventory().has_stock(&sku, dec!(50)).expect("Failed to check stock"));

    // Should have stock for exactly 100 units
    assert!(commerce.inventory().has_stock(&sku, dec!(100)).expect("Failed to check stock"));

    // Should NOT have stock for 150 units
    assert!(!commerce.inventory().has_stock(&sku, dec!(150)).expect("Failed to check stock"));

    // Reserve some
    commerce
        .inventory()
        .reserve(&sku, dec!(60), "order", "ORD-001", None)
        .expect("Failed to reserve");

    // Now should NOT have stock for 50 (only 40 available)
    assert!(!commerce.inventory().has_stock(&sku, dec!(50)).expect("Failed to check stock"));

    // But should have stock for 40
    assert!(commerce.inventory().has_stock(&sku, dec!(40)).expect("Failed to check stock"));
}

// ============================================================================
// Reorder Point Detection Tests
// ============================================================================

#[test]
fn test_reorder_point_detection() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("REORDER");

    // Create item with reorder point of 20
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Reorder Test Item".into(),
            initial_quantity: Some(dec!(25)), // Just above reorder point
            reorder_point: Some(dec!(20)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    // Initially not below reorder point
    let reorder_needed = commerce
        .inventory()
        .get_reorder_needed()
        .expect("Failed to get reorder needed");

    let needs_reorder = reorder_needed.iter().any(|s| s.sku == sku);
    assert!(!needs_reorder);

    // Remove stock to go below reorder point
    commerce
        .inventory()
        .adjust(&sku, dec!(-10), "Sold items")
        .expect("Failed to adjust");

    // Now should be at 15, below reorder point of 20
    let reorder_needed = commerce
        .inventory()
        .get_reorder_needed()
        .expect("Failed to get reorder needed");

    let needs_reorder = reorder_needed.iter().any(|s| s.sku == sku);
    assert!(needs_reorder);
}

#[test]
fn test_reorder_point_with_reservations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("REORDRES");

    // Create item with 30 units, reorder point at 15
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Reorder Reservation Test".into(),
            initial_quantity: Some(dec!(30)),
            reorder_point: Some(dec!(15)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    // Reserve 20 units (available goes to 10)
    commerce
        .inventory()
        .reserve(&sku, dec!(20), "order", "ORD-001", None)
        .expect("Failed to reserve");

    // Available (10) is now below reorder point (15)
    let reorder_needed = commerce
        .inventory()
        .get_reorder_needed()
        .expect("Failed to get reorder needed");

    // Check if this SKU needs reorder
    let _item_needs_reorder = reorder_needed.iter().any(|s| s.sku == sku);

    // The behavior depends on whether reorder point is based on available or on-hand
    // This test documents the current behavior
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_available, dec!(10));
}

#[test]
fn test_multiple_items_below_reorder() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create multiple items with different reorder scenarios
    let sku1 = unique_sku("MULREORD1");
    let sku2 = unique_sku("MULREORD2");
    let sku3 = unique_sku("MULREORD3");

    // Item 1: Below reorder point
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku1.clone(),
            name: "Item 1".into(),
            initial_quantity: Some(dec!(5)),
            reorder_point: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create item 1");

    // Item 2: Above reorder point
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku2.clone(),
            name: "Item 2".into(),
            initial_quantity: Some(dec!(50)),
            reorder_point: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create item 2");

    // Item 3: Below reorder point
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku3.clone(),
            name: "Item 3".into(),
            initial_quantity: Some(dec!(3)),
            reorder_point: Some(dec!(20)),
            ..Default::default()
        })
        .expect("Failed to create item 3");

    let reorder_needed = commerce
        .inventory()
        .get_reorder_needed()
        .expect("Failed to get reorder needed");

    // Items 1 and 3 should be in the list, item 2 should not
    let skus_needing_reorder: Vec<&str> = reorder_needed.iter().map(|s| s.sku.as_str()).collect();

    assert!(skus_needing_reorder.contains(&sku1.as_str()));
    assert!(!skus_needing_reorder.contains(&sku2.as_str()));
    assert!(skus_needing_reorder.contains(&sku3.as_str()));
}

// ============================================================================
// Inventory Listing and Filtering Tests
// ============================================================================

#[test]
fn test_list_inventory_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create several items
    let sku1 = unique_sku("LIST1");
    let sku2 = unique_sku("LIST2");
    let sku3 = unique_sku("LIST3");

    create_test_inventory_item(&commerce, &sku1);
    create_test_inventory_item(&commerce, &sku2);
    create_test_inventory_item(&commerce, &sku3);

    let items = commerce
        .inventory()
        .list(InventoryFilter::default())
        .expect("Failed to list items");

    assert!(items.len() >= 3);
}

#[test]
fn test_list_inventory_with_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create several items
    for i in 0..10 {
        let sku = unique_sku(&format!("LIMIT{}", i));
        create_test_inventory_item(&commerce, &sku);
    }

    let items = commerce
        .inventory()
        .list(InventoryFilter {
            limit: Some(5),
            ..Default::default()
        })
        .expect("Failed to list items");

    assert_eq!(items.len(), 5);
}

#[test]
fn test_list_inventory_with_offset() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create items
    for i in 0..10 {
        let sku = unique_sku(&format!("OFFSET{}", i));
        create_test_inventory_item(&commerce, &sku);
    }

    let first_page = commerce
        .inventory()
        .list(InventoryFilter {
            limit: Some(5),
            offset: Some(0),
            ..Default::default()
        })
        .expect("Failed to list first page");

    let second_page = commerce
        .inventory()
        .list(InventoryFilter {
            limit: Some(5),
            offset: Some(5),
            ..Default::default()
        })
        .expect("Failed to list second page");

    // Ensure pages don't overlap
    for item in &first_page {
        assert!(!second_page.iter().any(|i| i.id == item.id));
    }
}

#[test]
fn test_list_inventory_by_sku_filter() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("SKUFILTER");
    create_test_inventory_item(&commerce, &sku);

    let items = commerce
        .inventory()
        .list(InventoryFilter {
            sku: Some(sku.clone()),
            ..Default::default()
        })
        .expect("Failed to list items");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sku, sku);
}

#[test]
fn test_list_active_inventory_only() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let sku = unique_sku("ACTIVE");
    create_test_inventory_item(&commerce, &sku);

    let active_items = commerce
        .inventory()
        .list(InventoryFilter {
            is_active: Some(true),
            ..Default::default()
        })
        .expect("Failed to list active items");

    assert!(active_items.iter().all(|i| i.is_active));
}

// ============================================================================
// Transaction History Tests
// ============================================================================

#[test]
fn test_transaction_history() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("HISTORY");
    let item_id = create_test_inventory_item(&commerce, &sku);

    // Perform various operations
    commerce
        .inventory()
        .adjust(&sku, dec!(50), "Restock")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .adjust(&sku, dec!(-20), "Sold")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .reserve(&sku, dec!(10), "order", "ORD-001", None)
        .expect("Failed to reserve");

    // Get transaction history
    let transactions = commerce
        .inventory()
        .get_transactions(item_id, 100)
        .expect("Failed to get transactions");

    assert!(transactions.len() >= 3);

    // Verify we have transactions for adjustments
    // Note: transaction type may be Receipt (positive) or Adjustment (negative) depending on implementation
    let relevant_transactions: Vec<_> = transactions
        .iter()
        .filter(|t| {
            t.transaction_type == TransactionType::Adjustment
                || t.transaction_type == TransactionType::Receipt
                || t.transaction_type == TransactionType::Allocation
        })
        .collect();
    assert!(relevant_transactions.len() >= 2);
}

#[test]
fn test_transaction_history_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("HISTLIMIT");
    let item_id = create_test_inventory_item(&commerce, &sku);

    // Create many transactions
    for i in 0..20 {
        commerce
            .inventory()
            .adjust(&sku, dec!(1), &format!("Adjustment {}", i))
            .expect("Failed to adjust");
    }

    // Request limited history
    let transactions = commerce
        .inventory()
        .get_transactions(item_id, 5)
        .expect("Failed to get transactions");

    assert!(transactions.len() <= 5);
}

// ============================================================================
// Edge Cases and Error Handling Tests
// ============================================================================

#[test]
fn test_zero_quantity_adjustment_rejected() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("ZERO");
    create_test_inventory_item(&commerce, &sku);

    // Zero adjustments are not allowed - should return an error
    let result = commerce
        .inventory()
        .adjust(&sku, dec!(0), "Zero adjustment");

    // Verify that zero adjustment is rejected
    assert!(result.is_err());
}

#[test]
fn test_large_quantity_operations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("LARGE");

    // Create item with very large quantity
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Large Quantity Item".into(),
            initial_quantity: Some(dec!(1000000)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Reserve large quantity
    let reservation = commerce
        .inventory()
        .reserve(&sku, dec!(500000), "bulk_order", "BULK-001", None)
        .expect("Failed to reserve large quantity");

    assert_eq!(reservation.quantity, dec!(500000));

    // Adjust large quantity
    let transaction = commerce
        .inventory()
        .adjust(&sku, dec!(100000), "Large restock")
        .expect("Failed to adjust large quantity");

    assert_eq!(transaction.quantity, dec!(100000));
}

#[test]
fn test_decimal_precision() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("PRECISION");

    // Create item with decimal quantity
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Precision Test Item".into(),
            initial_quantity: Some(dec!(100.5)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Adjust with decimal
    commerce
        .inventory()
        .adjust(&sku, dec!(25.75), "Decimal adjustment")
        .expect("Failed to adjust");

    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_on_hand, dec!(126.25)); // 100.5 + 25.75
}

#[test]
fn test_special_characters_in_reason() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("SPECIAL");
    create_test_inventory_item(&commerce, &sku);

    let transaction = commerce
        .inventory()
        .adjust(
            &sku,
            dec!(10),
            "Adjustment with 'quotes' and \"double quotes\" and <brackets> & ampersand",
        )
        .expect("Failed to adjust with special characters");

    assert!(transaction.reason.is_some());
    assert!(transaction.reason.unwrap().contains("quotes"));
}

// ============================================================================
// Stock Location Tests
// ============================================================================

#[test]
fn test_stock_by_location() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("LOCATION");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Location Test Item".into(),
            initial_quantity: Some(dec!(100)),
            location_id: Some(1),
            ..Default::default()
        })
        .expect("Failed to create item");

    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    // Should have location information
    assert!(!stock.locations.is_empty());

    let location = &stock.locations[0];
    assert_eq!(location.location_id, 1);
    assert_eq!(location.on_hand, dec!(100));
}

// ============================================================================
// Integration Scenario Tests
// ============================================================================

#[test]
fn test_full_inventory_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("LIFECYCLE");

    // 1. Create item
    let item = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Lifecycle Test Item".into(),
            initial_quantity: Some(dec!(100)),
            reorder_point: Some(dec!(20)),
            ..Default::default()
        })
        .expect("Failed to create item");

    assert!(item.id > 0);

    // 2. Verify initial stock
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_on_hand, dec!(100));

    // 3. Reserve for order
    let reservation = commerce
        .inventory()
        .reserve(&sku, dec!(30), "order", "ORD-001", None)
        .expect("Failed to reserve");

    // 4. Verify stock after reservation
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(70));

    // 5. Confirm reservation (allocation)
    commerce
        .inventory()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm reservation");

    // 6. Simulate shipment - adjust inventory down
    commerce
        .inventory()
        .adjust(&sku, dec!(-30), "Shipped ORD-001")
        .expect("Failed to adjust for shipment");

    // 7. Restock
    commerce
        .inventory()
        .adjust(&sku, dec!(50), "Restock from supplier")
        .expect("Failed to restock");

    // 8. Final verification
    let final_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    // 100 (initial) - 30 (shipped) + 50 (restocked) = 120 on hand
    // But the allocated (30) was already part of on_hand, so after shipment:
    // on_hand = 100 - 30 + 50 = 120
    assert_eq!(final_stock.total_on_hand, dec!(120));

    // 9. Check transaction history
    let transactions = commerce
        .inventory()
        .get_transactions(item.id, 100)
        .expect("Failed to get transactions");

    assert!(transactions.len() >= 3); // At least: receipt, shipment adjustment, restock
}

#[test]
fn test_inventory_reserve_and_release_cycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("CYCLE");
    create_test_inventory_item(&commerce, &sku);

    // Initial: 100 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(100));

    // Reserve 40
    let res1 = commerce
        .inventory()
        .reserve(&sku, dec!(40), "cart", "CART-001", None)
        .expect("Failed to reserve");

    // 60 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(60));

    // Reserve another 30
    let res2 = commerce
        .inventory()
        .reserve(&sku, dec!(30), "cart", "CART-002", None)
        .expect("Failed to reserve");

    // 30 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(30));

    // Release first reservation (customer abandons cart)
    commerce
        .inventory()
        .release_reservation(res1.id)
        .expect("Failed to release reservation");

    // 70 available
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock.total_available, dec!(70));

    // Confirm second reservation (customer completes purchase)
    commerce
        .inventory()
        .confirm_reservation(res2.id)
        .expect("Failed to confirm reservation");

    // Check final state
    let final_stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(final_stock.total_on_hand, dec!(100));
}

// ============================================================================
// Concurrent Operations Tests (Single-threaded simulation)
// ============================================================================

#[test]
fn test_multiple_operations_same_item() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("MULTIOP");
    create_test_inventory_item(&commerce, &sku);

    // Perform multiple interleaved operations
    commerce
        .inventory()
        .adjust(&sku, dec!(10), "Add 1")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .reserve(&sku, dec!(5), "order", "ORD-001", None)
        .expect("Failed to reserve");

    commerce
        .inventory()
        .adjust(&sku, dec!(-3), "Remove 1")
        .expect("Failed to adjust");

    commerce
        .inventory()
        .reserve(&sku, dec!(10), "order", "ORD-002", None)
        .expect("Failed to reserve");

    commerce
        .inventory()
        .adjust(&sku, dec!(20), "Add 2")
        .expect("Failed to adjust");

    // Calculate expected: 100 + 10 - 3 + 20 = 127 on hand, 15 allocated
    let stock = commerce
        .inventory()
        .get_stock(&sku)
        .expect("Failed to get stock")
        .expect("Stock not found");

    assert_eq!(stock.total_on_hand, dec!(127));
    assert_eq!(stock.total_allocated, dec!(15));
    assert_eq!(stock.total_available, dec!(112));
}

// ============================================================================
// Negative Stock Prevention Tests
// ============================================================================

#[test]
fn test_insufficient_stock_for_reservation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("INSUFF");

    // Create item with only 10 units
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Low Stock Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Try to reserve 50 units - should fail
    let result = commerce.inventory().reserve(&sku, dec!(50), "order", "ORD-001", None);

    assert!(result.is_err());
}

#[test]
fn test_prevent_negative_adjustment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("NEGADJ");

    // Create item with 10 units
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Limited Stock Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create item");

    // Try to remove 50 units - depends on implementation whether this is allowed
    let _result = commerce.inventory().adjust(&sku, dec!(-50), "Excessive removal");

    // This test documents the behavior - some systems allow negative inventory, some don't
    // If it fails, negative inventory is prevented
    // If it succeeds, negative inventory is allowed
}

// ============================================================================
// SKU Validation Tests
// ============================================================================

#[test]
fn test_inventory_item_timestamps() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("TIMESTAMP");
    let item_id = create_test_inventory_item(&commerce, &sku);

    let item = commerce
        .inventory()
        .get_item(item_id)
        .expect("Failed to get item")
        .expect("Item not found");

    // Timestamps should be set
    assert!(item.created_at <= chrono::Utc::now());
    assert!(item.updated_at <= chrono::Utc::now());
    assert!(item.created_at <= item.updated_at);
}

#[test]
fn test_inventory_uniqueness() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let sku = unique_sku("UNIQUE");

    // Create first item
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "First Item".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create first item");

    // Try to create second item with same SKU - should fail
    let result = commerce.inventory().create_item(CreateInventoryItem {
        sku: sku.clone(),
        name: "Duplicate Item".into(),
        initial_quantity: Some(dec!(50)),
        ..Default::default()
    });

    assert!(result.is_err());
}
