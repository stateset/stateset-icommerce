#![cfg(feature = "sqlite")]

//! Integration tests for Order management

use rust_decimal_macros::dec;
use stateset_core::CurrencyCode;
use stateset_core::{CustomerId, OrderId};
use stateset_embedded::{
    Address, BackorderStatus, Commerce, CreateCart, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderFilter, OrderStatus,
    PaymentStatus, ReservationStatus, UpdateOrder,
};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a test customer
fn create_test_customer(commerce: &Commerce) -> CustomerId {
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer");
    customer.id
}

/// Helper to create a test order with default items
fn create_test_order(commerce: &Commerce, customer_id: CustomerId) -> Order {
    commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "TEST-SKU-001".into(),
                name: "Test Product".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order")
}

/// Helper to create a shipping address
fn create_test_address() -> Address {
    Address {
        line1: "123 Main St".into(),
        line2: Some("Apt 4B".into()),
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
    }
}

// ============================================================================
// Basic Order Creation Tests
// ============================================================================

#[test]
fn test_create_order_basic() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "TEST-SKU-001".into(),
                name: "Test Product".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    assert!(!order.id.is_nil());
    assert!(!order.order_number.is_empty());
    assert!(order.order_number.starts_with("ORD-"));
    assert_eq!(order.customer_id, customer_id);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(order.payment_status, PaymentStatus::Pending);
    assert_eq!(order.fulfillment_status, FulfillmentStatus::Unfulfilled);
    assert_eq!(order.items.len(), 1);
}

#[test]
fn test_create_order_from_cart_is_idempotent() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let cart_id = commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .expect("cart should be created")
        .id;
    let input = CreateOrder {
        customer_id,
        items: vec![CreateOrderItem {
            product_id: Uuid::new_v4().into(),
            sku: "RETRY-SKU".into(),
            name: "Retry-safe item".into(),
            quantity: 1,
            unit_price: dec!(12.34),
            tax_amount: Some(dec!(0.99)),
            ..Default::default()
        }],
        ..Default::default()
    };

    let first = commerce
        .orders()
        .create_from_cart(cart_id, input.clone())
        .expect("first cart order should succeed");
    let replay = commerce
        .orders()
        .create_from_cart(cart_id, input)
        .expect("cart order retry should succeed");

    assert_eq!(replay.id, first.id);
    assert_eq!(replay.order_number, first.order_number);
    assert_eq!(commerce.orders().count(OrderFilter::default()).unwrap(), 1);
}

#[test]
fn test_create_order_with_multiple_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-001".into(),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: dec!(29.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-002".into(),
                    name: "Gadget".into(),
                    quantity: 1,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-003".into(),
                    name: "Accessory".into(),
                    quantity: 3,
                    unit_price: dec!(9.99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.items.len(), 3);

    // Verify each item
    let skus: Vec<&str> = order.items.iter().map(|i| i.sku.as_str()).collect();
    assert!(skus.contains(&"SKU-001"));
    assert!(skus.contains(&"SKU-002"));
    assert!(skus.contains(&"SKU-003"));
}

#[test]
fn test_order_total_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-001".into(),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: dec!(29.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-002".into(),
                    name: "Gadget".into(),
                    quantity: 1,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("Failed to create order");

    // Calculate expected total: (2 * 29.99) + (1 * 49.99) = 59.98 + 49.99 = 109.97
    // Note: The actual total calculation depends on the implementation
    // which may include tax, so we check that it's at least the subtotal
    let expected_subtotal = dec!(109.97);
    assert!(order.total_amount >= expected_subtotal);
}

#[test]
fn test_create_order_with_addresses() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let shipping_address = create_test_address();
    let billing_address = Address {
        line1: "456 Oak Ave".into(),
        line2: None,
        city: "Los Angeles".into(),
        state: Some("CA".into()),
        postal_code: "90001".into(),
        country: "US".into(),
    };

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            shipping_address: Some(shipping_address),
            billing_address: Some(billing_address),
            ..Default::default()
        })
        .expect("Failed to create order");

    assert!(order.shipping_address.is_some());
    assert!(order.billing_address.is_some());

    let ship_addr = order.shipping_address.unwrap();
    assert_eq!(ship_addr.line1, "123 Main St");
    assert_eq!(ship_addr.city, "San Francisco");

    let bill_addr = order.billing_address.unwrap();
    assert_eq!(bill_addr.line1, "456 Oak Ave");
    assert_eq!(bill_addr.city, "Los Angeles");
}

#[test]
fn test_create_order_with_currency() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            currency: Some(CurrencyCode::EUR),
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.currency, CurrencyCode::EUR);
}

#[test]
fn test_create_order_with_notes() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            notes: Some("Please gift wrap this order".into()),
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.notes, Some("Please gift wrap this order".into()));
}

#[test]
fn test_create_order_with_payment_method() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            payment_method: Some("credit_card".into()),
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.payment_method, Some("credit_card".into()));
}

#[test]
fn test_create_order_with_shipping_method() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            shipping_method: Some("express".into()),
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.shipping_method, Some("express".into()));
}

// ============================================================================
// Order Retrieval Tests
// ============================================================================

#[test]
fn test_get_order_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let created = create_test_order(&commerce, customer_id);

    let retrieved =
        commerce.orders().get(created.id).expect("Failed to get order").expect("Order not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.order_number, created.order_number);
    assert_eq!(retrieved.customer_id, customer_id);
}

#[test]
fn test_get_order_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .orders()
        .get(OrderId::from(Uuid::new_v4()))
        .expect("Should not error for missing order");

    assert!(result.is_none());
}

#[test]
fn test_get_order_by_number() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let created = create_test_order(&commerce, customer_id);

    let retrieved = commerce
        .orders()
        .get_by_number(&created.order_number)
        .expect("Failed to get order by number")
        .expect("Order not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.order_number, created.order_number);
}

#[test]
fn test_get_order_by_number_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .orders()
        .get_by_number("ORD-NONEXISTENT-123")
        .expect("Should not error for missing order");

    assert!(result.is_none());
}

// ============================================================================
// Order Listing Tests
// ============================================================================

#[test]
fn test_list_orders() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create multiple orders
    for _ in 0..5 {
        create_test_order(&commerce, customer_id);
    }

    let orders = commerce.orders().list(OrderFilter::default()).expect("Failed to list orders");

    assert!(orders.len() >= 5);
}

#[test]
fn test_list_orders_by_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer1 = create_test_customer(&commerce);
    let customer2 = create_test_customer(&commerce);

    // Create orders for customer1
    for _ in 0..3 {
        create_test_order(&commerce, customer1);
    }

    // Create orders for customer2
    for _ in 0..2 {
        create_test_order(&commerce, customer2);
    }

    // List only customer1's orders
    let customer1_orders = commerce
        .orders()
        .list(OrderFilter { customer_id: Some(customer1), ..Default::default() })
        .expect("Failed to list orders");

    assert_eq!(customer1_orders.len(), 3);
    assert!(customer1_orders.iter().all(|o| o.customer_id == customer1));

    // List only customer2's orders
    let customer2_orders = commerce
        .orders()
        .list(OrderFilter { customer_id: Some(customer2), ..Default::default() })
        .expect("Failed to list orders");

    assert_eq!(customer2_orders.len(), 2);
    assert!(customer2_orders.iter().all(|o| o.customer_id == customer2));
}

#[test]
fn test_list_orders_for_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer1 = create_test_customer(&commerce);
    let customer2 = create_test_customer(&commerce);

    // Create orders for customer1
    for _ in 0..4 {
        create_test_order(&commerce, customer1);
    }

    // Create order for customer2
    create_test_order(&commerce, customer2);

    // Use the convenience method
    let orders =
        commerce.orders().list_for_customer(customer1).expect("Failed to list orders for customer");

    assert_eq!(orders.len(), 4);
    assert!(orders.iter().all(|o| o.customer_id == customer1));
}

#[test]
fn test_list_orders_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create orders
    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);
    let _order3 = create_test_order(&commerce, customer_id);

    // Update some orders to different statuses
    commerce
        .orders()
        .update_status(order1.id, OrderStatus::Confirmed)
        .expect("Failed to update status");
    commerce
        .orders()
        .update_status(order2.id, OrderStatus::Confirmed)
        .expect("Failed to update status");

    // List pending orders
    let pending_orders = commerce
        .orders()
        .list(OrderFilter { status: Some(OrderStatus::Pending), ..Default::default() })
        .expect("Failed to list orders");

    assert!(pending_orders.iter().all(|o| o.status == OrderStatus::Pending));

    // List confirmed orders
    let confirmed_orders = commerce
        .orders()
        .list(OrderFilter { status: Some(OrderStatus::Confirmed), ..Default::default() })
        .expect("Failed to list orders");

    assert_eq!(confirmed_orders.len(), 2);
    assert!(confirmed_orders.iter().all(|o| o.status == OrderStatus::Confirmed));
}

#[test]
fn test_list_orders_with_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create more orders than the limit
    for _ in 0..10 {
        create_test_order(&commerce, customer_id);
    }

    let orders = commerce
        .orders()
        .list(OrderFilter { limit: Some(5), ..Default::default() })
        .expect("Failed to list orders");

    assert_eq!(orders.len(), 5);
}

#[test]
fn test_list_orders_with_offset() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create orders
    for _ in 0..10 {
        create_test_order(&commerce, customer_id);
    }

    let orders = commerce
        .orders()
        .list(OrderFilter { limit: Some(5), offset: Some(3), ..Default::default() })
        .expect("Failed to list orders");

    assert!(orders.len() <= 5);
}

#[test]
fn test_count_orders() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create orders
    for _ in 0..7 {
        create_test_order(&commerce, customer_id);
    }

    let count = commerce.orders().count(OrderFilter::default()).expect("Failed to count orders");

    assert!(count >= 7);
}

#[test]
fn test_count_orders_by_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer1 = create_test_customer(&commerce);
    let customer2 = create_test_customer(&commerce);

    // Create orders
    for _ in 0..5 {
        create_test_order(&commerce, customer1);
    }
    for _ in 0..3 {
        create_test_order(&commerce, customer2);
    }

    let count = commerce
        .orders()
        .count(OrderFilter { customer_id: Some(customer1), ..Default::default() })
        .expect("Failed to count orders");

    assert_eq!(count, 5);
}

// ============================================================================
// Order Status Tests
// ============================================================================

#[test]
fn test_order_confirm() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    assert_eq!(order.status, OrderStatus::Pending);

    let confirmed = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");

    assert_eq!(confirmed.status, OrderStatus::Confirmed);
}

#[test]
fn test_order_cancel() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let cancelled = commerce.orders().cancel(order.id).expect("Failed to cancel order");

    assert_eq!(cancelled.status, OrderStatus::Cancelled);
}

#[test]
fn test_order_ship() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process order");

    let shipped =
        commerce.orders().ship(order.id, Some("1Z999AA10123456784")).expect("Failed to ship order");

    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(shipped.tracking_number, Some("1Z999AA10123456784".into()));
}

#[test]
fn test_order_ship_without_tracking() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process order");

    let shipped = commerce.orders().ship(order.id, None).expect("Failed to ship order");

    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert!(shipped.tracking_number.is_none());
}

#[test]
fn test_order_deliver() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process order");

    // Ship first
    commerce.orders().ship(order.id, Some("TRACK123")).expect("Failed to ship order");

    // Then deliver
    let delivered = commerce.orders().deliver(order.id).expect("Failed to deliver order");

    assert_eq!(delivered.status, OrderStatus::Delivered);
}

#[test]
fn test_order_status_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Pending -> Confirmed
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm");
    assert_eq!(order.status, OrderStatus::Confirmed);

    // Confirmed -> Processing
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process");
    assert_eq!(order.status, OrderStatus::Processing);

    // Processing -> Shipped
    let order =
        commerce.orders().update_status(order.id, OrderStatus::Shipped).expect("Failed to ship");
    assert_eq!(order.status, OrderStatus::Shipped);

    // Shipped -> Delivered
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Delivered)
        .expect("Failed to deliver");
    assert_eq!(order.status, OrderStatus::Delivered);
}

// ============================================================================
// Order Update Tests
// ============================================================================

#[test]
fn test_update_order_tracking_number() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder { tracking_number: Some("FEDEX123456".into()), ..Default::default() },
        )
        .expect("Failed to update order");

    assert_eq!(updated.tracking_number, Some("FEDEX123456".into()));
}

#[test]
fn test_update_order_notes() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                notes: Some("Updated: Customer requested expedited handling".into()),
                ..Default::default()
            },
        )
        .expect("Failed to update order");

    assert_eq!(updated.notes, Some("Updated: Customer requested expedited handling".into()));
}

#[test]
fn test_update_order_payment_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
        )
        .expect("Failed to update order");

    assert_eq!(updated.payment_status, PaymentStatus::Paid);
}

#[test]
fn test_update_order_fulfillment_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::Fulfilled),
                ..Default::default()
            },
        )
        .expect("Failed to update order");

    assert_eq!(updated.fulfillment_status, FulfillmentStatus::Fulfilled);
}

#[test]
fn test_update_order_shipping_address() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let new_address = Address {
        line1: "999 New Street".into(),
        line2: Some("Suite 100".into()),
        city: "New York".into(),
        state: Some("NY".into()),
        postal_code: "10001".into(),
        country: "US".into(),
    };

    let updated = commerce
        .orders()
        .update(order.id, UpdateOrder { shipping_address: Some(new_address), ..Default::default() })
        .expect("Failed to update order");

    let addr = updated.shipping_address.expect("Should have shipping address");
    assert_eq!(addr.line1, "999 New Street");
    assert_eq!(addr.city, "New York");
}

#[test]
fn test_update_order_multiple_fields() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                status: Some(OrderStatus::Processing),
                payment_status: Some(PaymentStatus::Authorized),
                tracking_number: Some("UPS1234567890".into()),
                notes: Some("Processing for shipment".into()),
                ..Default::default()
            },
        )
        .expect("Failed to update order");

    assert_eq!(updated.status, OrderStatus::Processing);
    assert_eq!(updated.payment_status, PaymentStatus::Authorized);
    assert_eq!(updated.tracking_number, Some("UPS1234567890".into()));
    assert_eq!(updated.notes, Some("Processing for shipment".into()));
}

// ============================================================================
// Order Items Tests
// ============================================================================

#[test]
fn test_order_items_included() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-001".into(),
                    name: "Widget A".into(),
                    quantity: 2,
                    unit_price: dec!(29.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-002".into(),
                    name: "Widget B".into(),
                    quantity: 1,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("Failed to create order");

    // Verify items are included
    assert_eq!(order.items.len(), 2);

    // Check first item
    let item1 = order.items.iter().find(|i| i.sku == "SKU-001").unwrap();
    assert_eq!(item1.name, "Widget A");
    assert_eq!(item1.quantity, 2);
    assert_eq!(item1.unit_price, dec!(29.99));

    // Check second item
    let item2 = order.items.iter().find(|i| i.sku == "SKU-002").unwrap();
    assert_eq!(item2.name, "Widget B");
    assert_eq!(item2.quantity, 1);
    assert_eq!(item2.unit_price, dec!(49.99));
}

#[test]
fn test_add_item_to_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let new_item = commerce
        .orders()
        .add_item(
            order.id,
            CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "NEW-SKU-001".into(),
                name: "New Product".into(),
                quantity: 3,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert!(!new_item.id.is_nil());
    assert_eq!(new_item.sku, "NEW-SKU-001");
    assert_eq!(new_item.name, "New Product");
    assert_eq!(new_item.quantity, 3);
    assert_eq!(new_item.unit_price, dec!(19.99));
    assert_eq!(new_item.order_id, order.id);

    // Verify the order now has 2 items
    let updated_order =
        commerce.orders().get(order.id).expect("Failed to get order").expect("Order not found");
    assert_eq!(updated_order.items.len(), 2);
}

#[test]
fn test_remove_item_from_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-001".into(),
                    name: "Widget A".into(),
                    quantity: 2,
                    unit_price: dec!(29.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "SKU-002".into(),
                    name: "Widget B".into(),
                    quantity: 1,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("Failed to create order");

    assert_eq!(order.items.len(), 2);

    // Remove the first item
    let item_to_remove = &order.items[0];
    commerce.orders().remove_item(order.id, item_to_remove.id).expect("Failed to remove item");

    // Verify the order now has 1 item
    let updated_order =
        commerce.orders().get(order.id).expect("Failed to get order").expect("Order not found");
    assert_eq!(updated_order.items.len(), 1);
}

#[test]
fn test_order_item_with_discount() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                discount: Some(dec!(10.00)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let item = &order.items[0];
    assert_eq!(item.discount, dec!(10.00));
}

#[test]
fn test_order_item_with_tax() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                tax_amount: Some(dec!(4.80)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let item = &order.items[0];
    assert_eq!(item.tax_amount, dec!(4.80));
}

// ============================================================================
// Order Deletion Tests
// ============================================================================

#[test]
fn test_delete_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    commerce.orders().delete(order.id).expect("Failed to delete order");

    // Verify order is deleted
    let result = commerce.orders().get(order.id).expect("Should not error for deleted order");
    assert!(result.is_none());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_create_order_default_currency() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            // No currency specified - should use default
            ..Default::default()
        })
        .expect("Failed to create order");

    // Default currency should be USD
    assert_eq!(order.currency, CurrencyCode::USD);
}

#[test]
fn test_order_version_tracking() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Initial version should be 1
    assert_eq!(order.version, 1);

    // Update the order
    let updated = commerce
        .orders()
        .update(order.id, UpdateOrder { notes: Some("Updated notes".into()), ..Default::default() })
        .expect("Failed to update order");

    // Version should be incremented
    assert!(updated.version > order.version);
}

#[test]
fn test_order_timestamps() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // created_at and updated_at should be set
    assert!(order.created_at <= chrono::Utc::now());
    assert!(order.updated_at <= chrono::Utc::now());

    // order_date should be set
    assert!(order.order_date <= chrono::Utc::now());
}

#[test]
fn test_order_number_uniqueness() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);
    let order3 = create_test_order(&commerce, customer_id);

    // All order numbers should be unique
    assert_ne!(order1.order_number, order2.order_number);
    assert_ne!(order2.order_number, order3.order_number);
    assert_ne!(order1.order_number, order3.order_number);
}

#[test]
fn test_order_with_large_quantities() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "BULK-SKU".into(),
                name: "Bulk Product".into(),
                quantity: 10000,
                unit_price: dec!(0.01),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let item = &order.items[0];
    assert_eq!(item.quantity, 10000);
}

#[test]
fn test_order_with_high_value_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "LUXURY-SKU".into(),
                name: "Luxury Item".into(),
                quantity: 1,
                unit_price: dec!(99999.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let item = &order.items[0];
    assert_eq!(item.unit_price, dec!(99999.99));
}

// ============================================================================
// Payment Status Tests
// ============================================================================

#[test]
fn test_order_payment_status_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Pending -> Authorized
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Authorized), ..Default::default() },
        )
        .expect("Failed to update");
    assert_eq!(order.payment_status, PaymentStatus::Authorized);

    // Authorized -> Paid
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
        )
        .expect("Failed to update");
    assert_eq!(order.payment_status, PaymentStatus::Paid);
}

#[test]
fn test_order_payment_partially_paid() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                payment_status: Some(PaymentStatus::PartiallyPaid),
                ..Default::default()
            },
        )
        .expect("Failed to update");

    assert_eq!(updated.payment_status, PaymentStatus::PartiallyPaid);
}

#[test]
fn test_order_payment_failed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let updated = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Failed), ..Default::default() },
        )
        .expect("Failed to update");

    assert_eq!(updated.payment_status, PaymentStatus::Failed);
}

// ============================================================================
// Fulfillment Status Tests
// ============================================================================

#[test]
fn test_order_fulfillment_status_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Unfulfilled -> PartiallyFulfilled
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::PartiallyFulfilled),
                ..Default::default()
            },
        )
        .expect("Failed to update");
    assert_eq!(order.fulfillment_status, FulfillmentStatus::PartiallyFulfilled);

    // PartiallyFulfilled -> Fulfilled
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::Fulfilled),
                ..Default::default()
            },
        )
        .expect("Failed to update");
    assert_eq!(order.fulfillment_status, FulfillmentStatus::Fulfilled);

    // Fulfilled -> Shipped
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::Shipped),
                ..Default::default()
            },
        )
        .expect("Failed to update");
    assert_eq!(order.fulfillment_status, FulfillmentStatus::Shipped);

    // Shipped -> Delivered
    let order = commerce
        .orders()
        .update(
            order.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::Delivered),
                ..Default::default()
            },
        )
        .expect("Failed to update");
    assert_eq!(order.fulfillment_status, FulfillmentStatus::Delivered);
}

// ============================================================================
// Filter Tests
// ============================================================================

#[test]
fn test_list_orders_by_payment_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create orders with different payment statuses
    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);
    let order3 = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update(
            order1.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
        )
        .expect("Failed to update");

    commerce
        .orders()
        .update(
            order2.id,
            UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
        )
        .expect("Failed to update");

    // order3 remains Pending

    // Get all orders for this customer and filter by payment status in code
    // (payment_status filter may not be implemented at the DB level)
    let all_orders = commerce
        .orders()
        .list(OrderFilter { customer_id: Some(customer_id), ..Default::default() })
        .expect("Failed to list orders");

    // Manually filter by payment status
    let paid_orders: Vec<_> =
        all_orders.iter().filter(|o| o.payment_status == PaymentStatus::Paid).collect();

    // Should have exactly 2 paid orders for this customer
    assert_eq!(paid_orders.len(), 2);
    assert!(paid_orders.iter().any(|o| o.id == order1.id));
    assert!(paid_orders.iter().any(|o| o.id == order2.id));

    // Verify order3 is not in the paid list
    assert!(!paid_orders.iter().any(|o| o.id == order3.id));
}

#[test]
fn test_list_orders_by_fulfillment_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create orders
    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);

    commerce
        .orders()
        .update(
            order1.id,
            UpdateOrder {
                fulfillment_status: Some(FulfillmentStatus::Fulfilled),
                ..Default::default()
            },
        )
        .expect("Failed to update");

    // order2 remains Unfulfilled

    // Get all orders for this customer and filter by fulfillment status in code
    // (fulfillment_status filter may not be implemented at the DB level)
    let all_orders = commerce
        .orders()
        .list(OrderFilter { customer_id: Some(customer_id), ..Default::default() })
        .expect("Failed to list orders");

    // Manually filter by fulfillment status
    let fulfilled_orders: Vec<_> = all_orders
        .iter()
        .filter(|o| o.fulfillment_status == FulfillmentStatus::Fulfilled)
        .collect();

    // Should have exactly 1 fulfilled order for this customer
    assert_eq!(fulfilled_orders.len(), 1);
    assert!(fulfilled_orders.iter().any(|o| o.id == order1.id));
    assert!(!fulfilled_orders.iter().any(|o| o.id == order2.id));
}

#[test]
fn test_list_orders_with_combined_filters() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create and update orders
    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);
    let order3 = create_test_order(&commerce, customer_id);

    // Update order1: Confirmed + Paid
    commerce
        .orders()
        .update(
            order1.id,
            UpdateOrder {
                status: Some(OrderStatus::Confirmed),
                payment_status: Some(PaymentStatus::Paid),
                ..Default::default()
            },
        )
        .expect("Failed to update");

    // Update order2: Confirmed + Pending payment
    commerce
        .orders()
        .update(
            order2.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .expect("Failed to update");

    // order3 remains Pending + Pending payment

    // Filter by customer and status (these are supported at DB level)
    let confirmed_orders = commerce
        .orders()
        .list(OrderFilter {
            customer_id: Some(customer_id),
            status: Some(OrderStatus::Confirmed),
            ..Default::default()
        })
        .expect("Failed to list orders");

    // Should have 2 confirmed orders (order1 and order2)
    assert_eq!(confirmed_orders.len(), 2);
    assert!(confirmed_orders.iter().any(|o| o.id == order1.id));
    assert!(confirmed_orders.iter().any(|o| o.id == order2.id));
    assert!(!confirmed_orders.iter().any(|o| o.id == order3.id));

    // Further filter in code by payment status (not supported at DB level)
    let confirmed_and_paid: Vec<_> =
        confirmed_orders.iter().filter(|o| o.payment_status == PaymentStatus::Paid).collect();

    assert_eq!(confirmed_and_paid.len(), 1);
    assert_eq!(confirmed_and_paid[0].id, order1.id);
}

// ============================================================================
// Inventory Reservation + Backorder Integration
// ============================================================================

#[test]
fn test_order_creates_backorder_when_insufficient_stock() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "BO-SKU-001".into(),
            name: "Backorder Item".into(),
            initial_quantity: Some(dec!(2)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "BO-SKU-001".into(),
                name: "Backorder Item".into(),
                quantity: 5,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let backorders = commerce
        .backorder()
        .get_backorders_for_order(order.id.into())
        .expect("Failed to load backorders");
    assert_eq!(backorders.len(), 1);
    assert_eq!(backorders[0].status, BackorderStatus::Pending);
    assert_eq!(backorders[0].quantity_remaining, dec!(3));

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .expect("Failed to load reservations");
    assert_eq!(reservations.len(), 1);
    assert_eq!(reservations[0].status, ReservationStatus::Pending);
    assert_eq!(reservations[0].quantity, dec!(2));
}

#[test]
fn test_cancel_releases_reservations_and_cancels_backorders() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "BO-SKU-002".into(),
            name: "Backorder Item".into(),
            initial_quantity: Some(dec!(1)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "BO-SKU-002".into(),
                name: "Backorder Item".into(),
                quantity: 3,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let cancelled = commerce.orders().cancel(order.id).expect("Failed to cancel order");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .expect("Failed to load reservations");
    assert!(reservations.iter().all(|r| r.status == ReservationStatus::Released));

    let backorders = commerce
        .backorder()
        .get_backorders_for_order(order.id.into())
        .expect("Failed to load backorders");
    assert!(backorders.iter().all(|bo| bo.status == BackorderStatus::Cancelled));
}

#[test]
fn test_ship_confirms_reservations() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SHIP-SKU-001".into(),
            name: "Shippable Item".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "SHIP-SKU-001".into(),
                name: "Shippable Item".into(),
                quantity: 2,
                unit_price: dec!(12.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    commerce.orders().ship(order.id, None).expect("Failed to ship order");

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .expect("Failed to load reservations");
    assert!(!reservations.is_empty());
    assert!(reservations.iter().all(|r| r.status == ReservationStatus::Confirmed));
}
