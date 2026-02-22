//! Batch operation integration tests.
//!
//! These tests verify that creating and managing many domain objects at once
//! works correctly through the full Commerce stack.

use rust_decimal_macros::dec;
use stateset_core::models::order::OrderStatus;
use stateset_core::models::product::UpdateProduct;
use stateset_core::ProductFilter;
use stateset_integration_tests::create_test_commerce;
use stateset_test_utils::fixtures;

#[test]
fn batch_create_20_customers() {
    let (commerce, _dir) = create_test_commerce();

    let mut customer_ids = Vec::new();

    for _ in 0..20 {
        let customer = commerce
            .customers()
            .create(fixtures::create_customer_input())
            .expect("create customer");
        customer_ids.push(customer.id);
    }

    // All IDs should be unique
    let unique_count = {
        let mut sorted = customer_ids.clone();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    };
    assert_eq!(unique_count, 20, "All 20 customer IDs should be unique");

    // Verify each customer can be retrieved
    for id in &customer_ids {
        let customer = commerce
            .customers()
            .get(*id)
            .expect("get customer")
            .expect("customer exists");
        assert_eq!(customer.first_name, "Test");
    }
}

#[test]
fn batch_create_10_orders() {
    let (commerce, _dir) = create_test_commerce();

    let customer = commerce
        .customers()
        .create(fixtures::create_customer_input())
        .expect("create customer");

    let mut order_ids = Vec::new();

    for _ in 0..10 {
        let order = commerce
            .orders()
            .create(fixtures::create_order_input(customer.id))
            .expect("create order");
        order_ids.push(order.id);
    }

    // All IDs should be unique
    let unique_count = {
        let mut sorted = order_ids.clone();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    };
    assert_eq!(unique_count, 10, "All 10 order IDs should be unique");

    // List and verify count
    let orders = commerce
        .orders()
        .list_for_customer(customer.id)
        .expect("list orders");
    assert_eq!(orders.len(), 10);
}

#[test]
fn create_order_with_multiple_items_verify_item_count() {
    let (commerce, _dir) = create_test_commerce();

    let customer = commerce
        .customers()
        .create(fixtures::create_customer_input())
        .expect("create customer");

    let items = vec![
        fixtures::create_order_item_with("BATCH-A", 1, dec!(10.00)),
        fixtures::create_order_item_with("BATCH-B", 2, dec!(20.00)),
        fixtures::create_order_item_with("BATCH-C", 3, dec!(30.00)),
        fixtures::create_order_item_with("BATCH-D", 1, dec!(5.00)),
    ];

    let order = commerce
        .orders()
        .create(fixtures::create_order_with_items(customer.id, items))
        .expect("create order");

    assert_eq!(order.items.len(), 4);
    // total = (1*10) + (2*20) + (3*30) + (1*5) = 10 + 40 + 90 + 5 = 145
    assert_eq!(order.total_amount, dec!(145.00));
}

#[test]
fn create_product_update_product_verify_update_applied() {
    let (commerce, _dir) = create_test_commerce();

    let product = commerce
        .products()
        .create(fixtures::create_product_with_name("Before Update"))
        .expect("create product");

    assert_eq!(product.name, "Before Update");

    let updated = commerce
        .products()
        .update(
            product.id,
            UpdateProduct {
                name: Some("After Update".into()),
                description: Some("New description".into()),
                ..Default::default()
            },
        )
        .expect("update product");

    assert_eq!(updated.name, "After Update");
    assert_eq!(updated.description, "New description");
    assert_eq!(updated.id, product.id);
}

#[test]
fn batch_create_products_list_all() {
    let (commerce, _dir) = create_test_commerce();

    for i in 0..8 {
        commerce
            .products()
            .create(fixtures::create_product_with_name(&format!("Product {i}")))
            .expect("create product");
    }

    let products = commerce
        .products()
        .list(ProductFilter::default())
        .expect("list products");

    assert_eq!(products.len(), 8);
}

#[test]
fn batch_create_inventory_items() {
    let (commerce, _dir) = create_test_commerce();

    for i in 0..15 {
        commerce
            .inventory()
            .create_item(fixtures::create_inventory_with(
                &format!("BATCH-INV-{i:03}"),
                dec!(100) + rust_decimal::Decimal::from(i),
            ))
            .expect("create inventory item");
    }

    // Verify each item
    for i in 0..15 {
        let sku = format!("BATCH-INV-{i:03}");
        let stock = commerce
            .inventory()
            .get_stock(&sku)
            .expect("get stock")
            .expect("stock exists");
        assert_eq!(stock.total_on_hand, dec!(100) + rust_decimal::Decimal::from(i));
    }
}

#[test]
fn batch_cancel_orders() {
    let (commerce, _dir) = create_test_commerce();

    let customer = commerce
        .customers()
        .create(fixtures::create_customer_input())
        .expect("create customer");

    let mut order_ids = Vec::new();
    for _ in 0..5 {
        let order = commerce
            .orders()
            .create(fixtures::create_order_input(customer.id))
            .expect("create order");
        order_ids.push(order.id);
    }

    // Cancel all orders
    for id in &order_ids {
        commerce.orders().cancel(*id).expect("cancel order");
    }

    // Verify all cancelled
    for id in &order_ids {
        let order = commerce
            .orders()
            .get(*id)
            .expect("get order")
            .expect("order exists");
        assert_eq!(order.status, OrderStatus::Cancelled);
    }
}

#[test]
fn batch_inventory_adjustments() {
    let (commerce, _dir) = create_test_commerce();

    let item = commerce
        .inventory()
        .create_item(fixtures::create_inventory_with("ADJ-BATCH", dec!(1000)))
        .expect("create inventory");

    // Perform 10 adjustments of -5 each
    for i in 0..10 {
        commerce
            .inventory()
            .adjust(&item.sku, dec!(-5), &format!("Adjustment {i}"))
            .expect("adjust");
    }

    let stock = commerce
        .inventory()
        .get_stock(&item.sku)
        .expect("get stock")
        .expect("stock exists");

    // 1000 - (10 * 5) = 950
    assert_eq!(stock.total_on_hand, dec!(950));

    // Verify transaction history
    let transactions = commerce
        .inventory()
        .get_transactions(item.id, 20)
        .expect("get transactions");

    // Should have 10 adjustment transactions (plus initial receipt)
    assert!(transactions.len() >= 10, "Expected at least 10 transactions, got {}", transactions.len());
}
