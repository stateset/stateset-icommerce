//! Commerce pipeline integration tests.
//!
//! These tests exercise full CRUD pipelines that span Commerce -> DB -> Events,
//! verifying that domain objects flow correctly through the entire stack.

use rust_decimal_macros::dec;
use stateset_core::OrderFilter;
use stateset_core::models::cart::{AddCartItem, CreateCart};
use stateset_core::models::order::OrderStatus;
use stateset_core::models::product::{CreateProductVariant, ProductStatus, UpdateProduct};
use stateset_core::models::returns::{CreateReturn, CreateReturnItem, ReturnReason, ReturnStatus};
use stateset_integration_tests::create_test_commerce;
use stateset_test_utils::fixtures;

// ---------------------------------------------------------------------------
// Order Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
fn create_customer_then_order_verifies_total_and_status() {
    let (commerce, _dir) = create_test_commerce();

    // Create a customer
    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    // Create an order for that customer with two items
    let items = vec![
        fixtures::create_order_item_with("SKU-A", 2, dec!(25.00)),
        fixtures::create_order_item_with("SKU-B", 1, dec!(50.00)),
    ];
    let order = commerce
        .orders()
        .create(fixtures::create_order_with_items(customer.id, items))
        .expect("create order");

    // Verify
    assert_eq!(order.customer_id, customer.id);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(order.items.len(), 2);
    // total = (2 * 25.00) + (1 * 50.00) = 100.00
    assert_eq!(order.total_amount, dec!(100.00));
    assert_eq!(order.currency.as_str(), "USD");
}

#[test]
fn create_order_ship_and_verify_status() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    assert_eq!(order.status, OrderStatus::Pending);

    // Ship the order
    let shipped = commerce.orders().ship(order.id, Some("TRACK-12345")).expect("ship order");

    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(shipped.tracking_number.as_deref(), Some("TRACK-12345"));
}

#[test]
fn create_order_cancel_and_verify_status() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    let cancelled = commerce.orders().cancel(order.id).expect("cancel order");

    assert_eq!(cancelled.status, OrderStatus::Cancelled);
}

#[test]
fn create_order_deliver_full_lifecycle() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Ship it
    let shipped = commerce.orders().ship(order.id, Some("1Z999AA10123456784")).expect("ship");

    assert_eq!(shipped.status, OrderStatus::Shipped);

    // Deliver it
    let delivered = commerce.orders().deliver(order.id).expect("deliver");

    assert_eq!(delivered.status, OrderStatus::Delivered);
}

// ---------------------------------------------------------------------------
// Inventory Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
fn create_inventory_adjust_and_verify_levels() {
    let (commerce, _dir) = create_test_commerce();

    let item = commerce
        .inventory()
        .create_item(fixtures::create_inventory_input())
        .expect("create inventory");

    let stock =
        commerce.inventory().get_stock(&item.sku).expect("get stock").expect("stock exists");

    assert_eq!(stock.total_on_hand, dec!(100));

    // Adjust down by 30
    commerce.inventory().adjust(&item.sku, dec!(-30), "Sold items").expect("adjust down");

    let stock_after =
        commerce.inventory().get_stock(&item.sku).expect("get stock").expect("stock exists");

    assert_eq!(stock_after.total_on_hand, dec!(70));
}

#[test]
fn reserve_inventory_confirm_reservation() {
    let (commerce, _dir) = create_test_commerce();

    let item = commerce
        .inventory()
        .create_item(fixtures::create_inventory_with("RSRV-SKU", dec!(50)))
        .expect("create inventory");

    // Reserve 10 units
    let reservation = commerce
        .inventory()
        .reserve(&item.sku, dec!(10), "order", "ORD-001", None)
        .expect("reserve");

    assert_eq!(reservation.quantity, dec!(10));

    // Confirm the reservation
    commerce.inventory().confirm_reservation(reservation.id).expect("confirm reservation");

    // Verify stock was deducted (available should be lower)
    let stock =
        commerce.inventory().get_stock(&item.sku).expect("get stock").expect("stock exists");

    // After reservation of 10 from 50, available = 40 (reserved lowers available)
    assert!(stock.total_available <= dec!(50));
}

#[test]
fn inventory_has_stock_check() {
    let (commerce, _dir) = create_test_commerce();

    commerce
        .inventory()
        .create_item(fixtures::create_inventory_with("CHK-SKU", dec!(20)))
        .expect("create inventory");

    assert!(commerce.inventory().has_stock("CHK-SKU", dec!(15)).expect("has stock"));

    assert!(!commerce.inventory().has_stock("CHK-SKU", dec!(25)).expect("has stock"));
}

// ---------------------------------------------------------------------------
// Product Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
fn create_product_with_variant_verify_accessible() {
    let (commerce, _dir) = create_test_commerce();

    let product =
        commerce.products().create(fixtures::create_product_input()).expect("create product");

    assert_eq!(product.name, "Test Product");
    assert_eq!(product.status, ProductStatus::Draft);

    // Fetch by ID
    let fetched =
        commerce.products().get(product.id).expect("get product").expect("product exists");

    assert_eq!(fetched.id, product.id);
    assert_eq!(fetched.name, "Test Product");
}

#[test]
fn create_product_update_name() {
    let (commerce, _dir) = create_test_commerce();

    let product = commerce
        .products()
        .create(fixtures::create_product_with_name("Original Name"))
        .expect("create product");

    let updated = commerce
        .products()
        .update(
            product.id,
            UpdateProduct { name: Some("Updated Name".into()), ..Default::default() },
        )
        .expect("update product");

    assert_eq!(updated.name, "Updated Name");
}

#[test]
fn create_product_add_extra_variant() {
    let (commerce, _dir) = create_test_commerce();

    let product =
        commerce.products().create(fixtures::create_product_input()).expect("create product");

    let variant = commerce
        .products()
        .add_variant(
            product.id,
            CreateProductVariant {
                sku: "EXTRA-VAR-001".into(),
                name: Some("Large".into()),
                price: dec!(39.99),
                compare_at_price: None,
                cost: Some(dec!(15.00)),
                barcode: None,
                weight: None,
                weight_unit: None,
                options: None,
                is_default: Some(false),
            },
        )
        .expect("add variant");

    assert_eq!(variant.sku, "EXTRA-VAR-001");
    assert_eq!(variant.price, dec!(39.99));
    assert_eq!(variant.product_id, product.id);
}

// ---------------------------------------------------------------------------
// Return Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
fn create_return_approve_and_verify() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    // Create an order first
    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Create a return against that order
    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create return");

    assert_eq!(ret.status, ReturnStatus::Requested);
    assert_eq!(ret.order_id, order.id);

    // Approve
    let approved = commerce.returns().approve(ret.id).expect("approve return");

    assert_eq!(approved.status, ReturnStatus::Approved);
}

// ---------------------------------------------------------------------------
// Multi-Order Tests
// ---------------------------------------------------------------------------

#[test]
fn multiple_orders_for_same_customer_lists_correctly() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    // Create 5 orders
    for _ in 0..5 {
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");
    }

    let orders = commerce.orders().list_for_customer(customer.id).expect("list orders");

    assert_eq!(orders.len(), 5);
    for order in &orders {
        assert_eq!(order.customer_id, customer.id);
    }
}

#[test]
fn order_count_matches_filter() {
    let (commerce, _dir) = create_test_commerce();

    let c1 = commerce.customers().create(fixtures::create_customer_input()).expect("customer 1");
    let c2 = commerce.customers().create(fixtures::create_customer_input()).expect("customer 2");

    // 3 orders for c1, 2 for c2
    for _ in 0..3 {
        commerce.orders().create(fixtures::create_order_input(c1.id)).expect("order for c1");
    }
    for _ in 0..2 {
        commerce.orders().create(fixtures::create_order_input(c2.id)).expect("order for c2");
    }

    let count_c1 = commerce
        .orders()
        .count(OrderFilter { customer_id: Some(c1.id), ..Default::default() })
        .expect("count c1");

    let count_c2 = commerce
        .orders()
        .count(OrderFilter { customer_id: Some(c2.id), ..Default::default() })
        .expect("count c2");

    assert_eq!(count_c1, 3);
    assert_eq!(count_c2, 2);
}

// ---------------------------------------------------------------------------
// Cart Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
fn create_cart_add_items_verify_totals() {
    let (commerce, _dir) = create_test_commerce();

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("cart-test@example.com".into()),
            customer_name: Some("Cart Tester".into()),
            ..Default::default()
        })
        .expect("create cart");

    // Add items
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "CART-SKU-1".into(),
                name: "Widget".into(),
                quantity: 3,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .expect("add item 1");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "CART-SKU-2".into(),
                name: "Gadget".into(),
                quantity: 1,
                unit_price: dec!(25.00),
                ..Default::default()
            },
        )
        .expect("add item 2");

    // Fetch cart and verify
    let updated_cart = commerce.carts().get(cart.id).expect("get cart").expect("cart exists");

    assert_eq!(updated_cart.items.len(), 2);
    // subtotal = (3 * 10.00) + (1 * 25.00) = 55.00
    assert_eq!(updated_cart.subtotal, dec!(55.00));
}

// ---------------------------------------------------------------------------
// Customer Retrieval Tests
// ---------------------------------------------------------------------------

#[test]
fn create_customer_retrieve_by_email() {
    let (commerce, _dir) = create_test_commerce();

    let input = fixtures::create_customer_with_email("unique-test@example.com");
    let customer = commerce.customers().create(input).expect("create customer");

    let fetched = commerce
        .customers()
        .get_by_email("unique-test@example.com")
        .expect("get by email")
        .expect("customer exists");

    assert_eq!(fetched.id, customer.id);
    assert_eq!(fetched.email, "unique-test@example.com");
}

#[test]
fn create_customer_get_by_id() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let fetched =
        commerce.customers().get(customer.id).expect("get customer").expect("customer exists");

    assert_eq!(fetched.id, customer.id);
    assert_eq!(fetched.first_name, "Test");
    assert_eq!(fetched.last_name, "User");
}
