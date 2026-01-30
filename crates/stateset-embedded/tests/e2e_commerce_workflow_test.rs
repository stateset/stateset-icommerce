//! End-to-end commerce workflow test
//!
//! Tests the full lifecycle: Customer → Product → Inventory → Order →
//! Payment → Shipment → Delivery → Return → Refund
//!
//! This is the single most important integration test — it validates that
//! all commerce domains work together correctly.

use rust_decimal_macros::dec;
use stateset_embedded::{
    Address, Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CreateProduct, CreateRefund, CreateReturn, CreateReturnItem, CreateShipment,
    ItemCondition, OrderStatus, PaymentMethodType, PaymentTransactionStatus, RefundStatus,
    ReturnReason, ReturnStatus, ShipmentStatus, ShippingCarrier, ShippingMethod, UpdateReturn,
};
use uuid::Uuid;

// ============================================================================
// Full Commerce Lifecycle
// ============================================================================

#[test]
fn test_full_commerce_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to initialize commerce engine");

    // ========================================================================
    // 1. Create customer
    // ========================================================================
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("e2e-{}@example.com", Uuid::new_v4()),
            first_name: "Alice".into(),
            last_name: "E2E".into(),
            ..Default::default()
        })
        .expect("Failed to create customer");
    assert!(!customer.id.is_nil());

    // ========================================================================
    // 2. Create products
    // ========================================================================
    let product = commerce
        .products()
        .create(CreateProduct {
            name: "Premium Widget".into(),
            description: Some("High-quality widget for e2e testing".into()),
            sku: Some(format!("E2E-{}", Uuid::new_v4().simple())),
            price: Some(dec!(49.99)),
            ..Default::default()
        })
        .expect("Failed to create product");
    let product_sku = product.sku.clone().unwrap_or_else(|| "E2E-SKU".into());

    // ========================================================================
    // 3. Create inventory
    // ========================================================================
    let _inventory = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: product_sku.clone(),
            name: "Premium Widget".into(),
            initial_quantity: Some(dec!(100)),
            reorder_point: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let stock_before = commerce
        .inventory()
        .get_stock(&product_sku)
        .expect("Failed to get stock")
        .expect("Stock not found");
    assert_eq!(stock_before.total_on_hand, dec!(100));

    // ========================================================================
    // 4. Create order with 2 units
    // ========================================================================
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: product_sku.clone(),
                name: "Premium Widget".into(),
                quantity: 2,
                unit_price: dec!(49.99),
                ..Default::default()
            }],
            shipping_address: Some(Address {
                line1: "123 Main St".into(),
                line2: None,
                city: "San Francisco".into(),
                state: Some("CA".into()),
                postal_code: "94102".into(),
                country: "US".into(),
            }),
            ..Default::default()
        })
        .expect("Failed to create order");
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(order.items.len(), 1);

    // ========================================================================
    // 5. Process payment
    // ========================================================================
    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(customer.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(99.98), // 2 x 49.99
            currency: Some("USD".into()),
            card_brand: Some(stateset_embedded::CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");
    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    let payment = commerce
        .payments()
        .mark_completed(payment.id)
        .expect("Failed to complete payment");
    assert_eq!(payment.status, PaymentTransactionStatus::Completed);

    // ========================================================================
    // 6. Confirm order
    // ========================================================================
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");

    // ========================================================================
    // 7. Reserve inventory
    // ========================================================================
    let reservation = commerce
        .inventory()
        .reserve(
            &product_sku,
            dec!(2),
            "order",
            &order.id.to_string(),
            None,
        )
        .expect("Failed to reserve inventory");

    let stock_reserved = commerce
        .inventory()
        .get_stock(&product_sku)
        .expect("get stock")
        .expect("stock not found");
    assert_eq!(stock_reserved.total_allocated, dec!(2));
    assert_eq!(stock_reserved.total_available, dec!(98));

    // ========================================================================
    // 8. Confirm reservation (deduct stock)
    // ========================================================================
    commerce
        .inventory()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm reservation");

    let stock_after_confirm = commerce
        .inventory()
        .get_stock(&product_sku)
        .expect("get stock")
        .expect("stock not found");
    assert_eq!(stock_after_confirm.total_on_hand, dec!(98));
    assert_eq!(stock_after_confirm.total_allocated, dec!(0));

    // ========================================================================
    // 9. Create shipment
    // ========================================================================
    let shipment = commerce
        .shipments()
        .create(CreateShipment {
            order_id: order.id,
            carrier: Some(ShippingCarrier::FedEx),
            shipping_method: Some(ShippingMethod::Express),
            tracking_number: Some("FEDEX-E2E-123456".into()),
            recipient_name: "Alice E2E".into(),
            shipping_address: "123 Main St, San Francisco, CA 94102".into(),
            ..Default::default()
        })
        .expect("Failed to create shipment");
    assert_eq!(shipment.order_id, order.id);

    // ========================================================================
    // 10. Ship and deliver
    // ========================================================================
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("set processing");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Shipped)
        .expect("set shipped");

    let shipment = commerce
        .shipments()
        .ship(shipment.id, None)
        .expect("Failed to ship");
    assert_eq!(shipment.status, ShipmentStatus::Shipped);

    let shipment = commerce
        .shipments()
        .mark_delivered(shipment.id)
        .expect("Failed to deliver");
    assert_eq!(shipment.status, ShipmentStatus::Delivered);

    commerce
        .orders()
        .update_status(order.id, OrderStatus::Delivered)
        .expect("set delivered");

    // ========================================================================
    // 11. Verify order is delivered
    // ========================================================================
    let delivered_order = commerce
        .orders()
        .get(order.id)
        .expect("get order")
        .expect("order not found");
    assert_eq!(delivered_order.status, OrderStatus::Delivered);

    // ========================================================================
    // 12. Create return for 1 item
    // ========================================================================
    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            reason_details: Some("Widget stopped working after 1 week".into()),
            items: vec![CreateReturnItem {
                order_item_id: delivered_order.items[0].id,
                quantity: 1,
                condition: Some(ItemCondition::Defective),
            }],
            ..Default::default()
        })
        .expect("Failed to create return");
    assert_eq!(ret.status, ReturnStatus::Requested);

    // ========================================================================
    // 13. Approve and complete return
    // ========================================================================
    let ret = commerce.returns().approve(ret.id).expect("approve return");
    assert_eq!(ret.status, ReturnStatus::Approved);

    let ret = commerce
        .returns()
        .add_tracking(ret.id, "RMA-E2E-789")
        .expect("add tracking");
    assert_eq!(ret.status, ReturnStatus::InTransit);

    let ret = commerce
        .returns()
        .mark_received(ret.id)
        .expect("mark received");
    assert_eq!(ret.status, ReturnStatus::Received);

    let ret = commerce
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                refund_amount: Some(dec!(49.99)),
                refund_method: Some("original_payment".into()),
                ..Default::default()
            },
        )
        .expect("complete return");
    assert_eq!(ret.status, ReturnStatus::Completed);

    // ========================================================================
    // 14. Process refund
    // ========================================================================
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(49.99)),
            reason: Some("Defective item returned".into()),
            ..Default::default()
        })
        .expect("Failed to create refund");
    assert!(!refund.id.is_nil());
    assert_eq!(refund.amount, dec!(49.99));

    // ========================================================================
    // 15. Verify final state
    // ========================================================================

    // Order still delivered (returns don't change order status)
    let final_order = commerce
        .orders()
        .get(order.id)
        .expect("get order")
        .expect("order not found");
    assert_eq!(final_order.status, OrderStatus::Delivered);

    // Return completed
    let final_return = commerce
        .returns()
        .get(ret.id)
        .expect("get return")
        .expect("return not found");
    assert_eq!(final_return.status, ReturnStatus::Completed);

    // Inventory: still 98 on hand (restocking is a separate operation)
    let final_stock = commerce
        .inventory()
        .get_stock(&product_sku)
        .expect("get stock")
        .expect("stock not found");
    assert_eq!(final_stock.total_on_hand, dec!(98));

    // Payment has partial refund
    let final_payment = commerce
        .payments()
        .get(payment.id)
        .expect("get payment")
        .expect("payment not found");
    assert_eq!(final_payment.amount_refunded, dec!(49.99));
}

// ============================================================================
// Variant: Order with multiple products
// ============================================================================

#[test]
fn test_multi_product_order_partial_return() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("multi-{}@test.com", Uuid::new_v4()),
            first_name: "Bob".into(),
            last_name: "Multi".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id;

    // Create order with 2 different items
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: "MULTI-A".into(),
                    name: "Item A".into(),
                    quantity: 1,
                    unit_price: dec!(25.00),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: "MULTI-B".into(),
                    name: "Item B".into(),
                    quantity: 1,
                    unit_price: dec!(75.00),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("create order");

    assert_eq!(order.items.len(), 2);

    // Move to delivered
    commerce.orders().update_status(order.id, OrderStatus::Confirmed).expect("confirm");
    commerce.orders().update_status(order.id, OrderStatus::Processing).expect("process");
    commerce.orders().update_status(order.id, OrderStatus::Shipped).expect("ship");
    commerce.orders().update_status(order.id, OrderStatus::Delivered).expect("deliver");

    // Return only Item A
    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::WrongItem,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                condition: Some(ItemCondition::New),
            }],
            ..Default::default()
        })
        .expect("create return");

    assert_eq!(ret.items.len(), 1, "Only one item should be returned");
    assert_eq!(ret.reason, ReturnReason::WrongItem);

    // Complete return
    commerce.returns().approve(ret.id).expect("approve");
    commerce
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                refund_amount: Some(dec!(25.00)),
                ..Default::default()
            },
        )
        .expect("complete");

    // Verify only partial refund
    let completed = commerce.returns().get(ret.id).expect("get").expect("found");
    assert_eq!(completed.refund_amount, Some(dec!(25.00)));
}
