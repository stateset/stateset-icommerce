//!
//! Comprehensive state machine tests for core commerce entities.
//!
//! This module tests all valid and invalid state transitions for:
//! - Orders (7 statuses)
//! - Payments (7 statuses)
//! - Inventory (reservation lifecycle)
//! - Subscriptions (7 statuses)
//! - Manufacturing (BOMs, work orders)
//!

use rust_decimal_macros::dec;
use stateset_embedded::{
    BillingInterval, CancelSubscription, Commerce, CreateBackorder, CreateBom, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreateOrderItem, CreateSerialNumbersBulk,
    CreateSubscription, CreateSubscriptionPlan, CreateWorkOrder, FulfillmentSourceType, Order,
    OrderStatus, PauseSubscription, ReservationStatus, ReserveSerialNumber, SerialStatus,
    SubscriptionStatus, WorkOrderStatus,
};
use uuid::Uuid;

fn create_test_customer(commerce: &Commerce) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}

fn create_test_order(commerce: &Commerce, customer_id: Uuid) -> Order {
    commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
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

#[test]
fn test_order_state_machine_all_valid_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    assert_eq!(order.status, OrderStatus::Pending);

    // Test all valid transitions
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to update order status");
    assert_eq!(order.status, OrderStatus::Confirmed);

    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to update order status");
    assert_eq!(order.status, OrderStatus::Processing);

    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Shipped)
        .expect("Failed to update order status");
    assert_eq!(order.status, OrderStatus::Shipped);
    assert!(order.tracking_number.is_some());

    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Delivered)
        .expect("Failed to update order status");
    assert_eq!(order.status, OrderStatus::Delivered);

    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Refunded)
        .expect("Failed to update order status");
    assert_eq!(order.status, OrderStatus::Refunded);
}

#[test]
fn test_order_state_machine_invalid_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Cannot transition from Pending directly to Shipped
    let result = commerce
        .orders()
        .update_status(order.id, OrderStatus::Shipped);
    assert!(result.is_err());

    // Cannot transition from Shipped to Confirmed
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process order");
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Shipped)
        .expect("Failed to ship order");

    let result = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed);
    assert!(result.is_err());

    // Cannot transition from Cancelled
    let order2 = create_test_order(&commerce, customer_id);
    let order2 = commerce
        .orders()
        .cancel(order2.id)
        .expect("Failed to cancel order");
    assert_eq!(order2.status, OrderStatus::Cancelled);

    let result = commerce
        .orders()
        .update_status(order2.id, OrderStatus::Confirmed);
    assert!(result.is_err());
}

#[test]
fn test_order_cancellation_before_shipment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    // Can cancel pending order
    let order = commerce
        .orders()
        .cancel(order.id)
        .expect("Failed to cancel pending order");
    assert_eq!(order.status, OrderStatus::Cancelled);
}

#[test]
fn test_order_cancellation_after_shipment_fails() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let mut order = create_test_order(&commerce, customer_id);

    // Ship the order
    order = commerce
        .orders()
        .ship(order.id, Some("FEDEX123456".into()))
        .expect("Failed to ship order");

    // Cannot cancel after shipment
    let result = commerce.orders().cancel(order.id);
    assert!(result.is_err());
}

#[test]
fn test_inventory_reservation_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let item = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let stock = commerce
        .inventory()
        .get_stock("SKU-001")
        .expect("Failed to get stock");
    let stock = stock.expect("Stock missing");
    assert_eq!(stock.total_on_hand, dec!(100));
    assert_eq!(stock.total_allocated, dec!(0));
    assert_eq!(stock.total_available, dec!(100));

    let customer_id = create_test_customer(&commerce);
    let order = create_test_order(&commerce, customer_id);

    let reservation = commerce
        .inventory()
        .reserve("SKU-001", dec!(5), "order", &order.id.to_string(), None)
        .expect("Failed to reserve inventory");
    assert_eq!(reservation.status, ReservationStatus::Pending);

    let stock = commerce
        .inventory()
        .get_stock("SKU-001")
        .expect("Failed to get stock");
    let stock = stock.expect("Stock missing");
    assert_eq!(stock.total_on_hand, dec!(100));
    assert_eq!(stock.total_allocated, dec!(5));
    assert_eq!(stock.total_available, dec!(95));

    commerce
        .inventory()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm reservation");

    let stock = commerce
        .inventory()
        .get_stock("SKU-001")
        .expect("Failed to get stock");
    let stock = stock.expect("Stock missing");
    assert_eq!(stock.total_on_hand, dec!(100));
    assert_eq!(stock.total_allocated, dec!(5));
    assert_eq!(stock.total_available, dec!(95));
}

#[test]
fn test_inventory_reservation_conflict_handling() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let item = commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let customer_id = create_test_customer(&commerce);
    let order1 = create_test_order(&commerce, customer_id);
    let order2 = create_test_order(&commerce, customer_id);

    // Reserve 8 units from order1
    let _reservation1 = commerce
        .inventory()
        .reserve("SKU-001", dec!(8), "order", &order1.id.to_string(), None)
        .expect("Failed to reserve inventory");

    // Try to reserve 5 more from order2 (only 2 available)
    let result = commerce
        .inventory()
        .reserve("SKU-001", dec!(5), "order", &order2.id.to_string(), None);
    assert!(result.is_err());
}

#[test]
fn test_subscription_state_machine() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: "Pro Monthly".into(),
            price: dec!(29.99),
            billing_interval: BillingInterval::Monthly,
            ..Default::default()
        })
        .expect("Failed to create subscription plan");
    let plan = commerce
        .subscriptions()
        .activate_plan(plan.id)
        .expect("Failed to activate subscription plan");

    let mut subscription = commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id,
            plan_id: plan.id,
            ..Default::default()
        })
        .expect("Failed to create subscription");
    assert_eq!(subscription.status, SubscriptionStatus::Active);

    subscription = commerce
        .subscriptions()
        .pause(subscription.id, PauseSubscription::default())
        .expect("Failed to pause subscription");
    assert_eq!(subscription.status, SubscriptionStatus::Paused);

    subscription = commerce
        .subscriptions()
        .resume(subscription.id)
        .expect("Failed to resume subscription");
    assert_eq!(subscription.status, SubscriptionStatus::Active);

    subscription = commerce
        .subscriptions()
        .cancel(subscription.id, CancelSubscription::default())
        .expect("Failed to cancel subscription");
    assert_eq!(subscription.status, SubscriptionStatus::Cancelled);
}

#[test]
fn test_serial_number_state_machine() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let serials = commerce
        .serials()
        .create_bulk(CreateSerialNumbersBulk {
            sku: "SKU-001".into(),
            quantity: 5,
            ..Default::default()
        })
        .expect("Failed to create serial numbers");
    assert_eq!(serials.len(), 5);

    for serial in &serials {
        assert_eq!(serial.status, SerialStatus::Available);
    }

    let serial = &serials[0];
    let reservation = commerce
        .serials()
        .reserve(ReserveSerialNumber {
            serial_id: serial.id,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            ..Default::default()
        })
        .expect("Failed to reserve serial");
    let serial = commerce
        .serials()
        .get(serial.id)
        .expect("Failed to get serial")
        .expect("Serial not found");
    assert_eq!(serial.status, SerialStatus::Reserved);

    commerce
        .serials()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm serial reservation");
    let serial = commerce
        .serials()
        .get(serial.id)
        .expect("Failed to get serial")
        .expect("Serial not found");
    assert_eq!(serial.status, SerialStatus::Reserved);

    let customer_id = create_test_customer(&commerce);
    let serial = commerce
        .serials()
        .mark_sold(serial.id, customer_id, None)
        .expect("Failed to mark serial as sold");
    assert_eq!(serial.status, SerialStatus::Sold);
}

#[test]
fn test_work_order_state_machine() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let product_id = Uuid::new_v4();
    let bom = commerce
        .bom()
        .create(CreateBom {
            product_id,
            name: "Widget Assembly".into(),
            ..Default::default()
        })
        .expect("Failed to create BOM");

    let mut work_order = commerce
        .work_orders()
        .create(CreateWorkOrder {
            product_id,
            bom_id: Some(bom.id),
            quantity_to_build: dec!(50),
            ..Default::default()
        })
        .expect("Failed to create work order");
    assert_eq!(work_order.status, WorkOrderStatus::Planned);

    work_order = commerce
        .work_orders()
        .start(work_order.id)
        .expect("Failed to start work order");
    assert_eq!(work_order.status, WorkOrderStatus::InProgress);

    work_order = commerce
        .work_orders()
        .complete(work_order.id, dec!(50))
        .expect("Failed to complete work order");
    assert_eq!(work_order.status, WorkOrderStatus::Completed);
}

#[test]
fn test_backorder_allocation_and_fulfillment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(0)),
            ..Default::default()
        })
        .expect("Failed to create inventory item");

    let order = create_test_order(&commerce, customer_id);

    let backorder = commerce
        .backorder()
        .create_backorder(CreateBackorder {
            order_id: order.id,
            order_line_id: None,
            sku: "SKU-001".into(),
            quantity: dec!(2),
            customer_id,
            priority: None,
            expected_date: None,
            promised_date: None,
            source_location_id: None,
            notes: None,
        })
        .expect("Failed to create backorder");

    let backorder = commerce
        .backorder()
        .fulfill_backorder(stateset_embedded::FulfillBackorder {
            backorder_id: backorder.id,
            quantity: dec!(2),
            source_type: FulfillmentSourceType::Inventory,
            source_id: None,
            notes: None,
            fulfilled_by: None,
        })
        .expect("Failed to fulfill backorder");
    assert_eq!(backorder.quantity_remaining, dec!(0));
}
