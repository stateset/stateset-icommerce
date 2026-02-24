//! Event pipeline integration tests.
//!
//! These tests verify that commerce operations emit the correct domain events
//! through the event bus, exercising the Commerce -> Events boundary.

use stateset_core::CommerceEvent;
use stateset_integration_tests::create_test_commerce;
use stateset_test_utils::fixtures;

#[test]
fn order_create_emits_order_created_event() {
    let (commerce, _dir) = create_test_commerce();

    // Subscribe BEFORE creating the order so we catch the event
    let mut subscription = commerce.subscribe_events();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    // Drain the CustomerCreated event
    let _customer_event = subscription.try_recv();

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // The OrderCreated event should be available
    let event = subscription.try_recv().expect("should have OrderCreated event");

    match event {
        CommerceEvent::OrderCreated { order_id, customer_id, total_amount, item_count, .. } => {
            assert_eq!(order_id, order.id);
            assert_eq!(customer_id, customer.id);
            assert_eq!(total_amount, order.total_amount);
            assert_eq!(item_count, 1);
        }
        other => panic!("Expected OrderCreated, got: {other:?}"),
    }
}

#[test]
fn order_update_status_emits_status_changed_event() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Subscribe after creation to avoid draining create events
    let mut subscription = commerce.subscribe_events();

    // Cancel the order (Pending -> Cancelled)
    commerce.orders().cancel(order.id).expect("cancel order");

    // Should see OrderStatusChanged event
    let event = subscription.try_recv().expect("should have status changed event");

    match event {
        CommerceEvent::OrderStatusChanged { order_id, from_status, to_status, .. } => {
            assert_eq!(order_id, order.id);
            assert_eq!(from_status, stateset_core::OrderStatus::Pending);
            assert_eq!(to_status, stateset_core::OrderStatus::Cancelled);
        }
        other => panic!("Expected OrderStatusChanged, got: {other:?}"),
    }
}

#[test]
fn customer_create_emits_customer_created_event() {
    let (commerce, _dir) = create_test_commerce();

    let mut subscription = commerce.subscribe_events();

    let customer = commerce
        .customers()
        .create(fixtures::create_customer_with_email("events@example.com"))
        .expect("create customer");

    let event = subscription.try_recv().expect("should have CustomerCreated event");

    match event {
        CommerceEvent::CustomerCreated { customer_id, email, .. } => {
            assert_eq!(customer_id, customer.id);
            assert_eq!(email, "events@example.com");
        }
        other => panic!("Expected CustomerCreated, got: {other:?}"),
    }
}

#[test]
fn product_create_emits_product_created_event() {
    let (commerce, _dir) = create_test_commerce();

    let mut subscription = commerce.subscribe_events();

    let product = commerce
        .products()
        .create(fixtures::create_product_with_name("Event Product"))
        .expect("create product");

    let event = subscription.try_recv().expect("should have ProductCreated event");

    match event {
        CommerceEvent::ProductCreated { product_id, name, .. } => {
            assert_eq!(product_id, product.id);
            assert_eq!(name, "Event Product");
        }
        other => panic!("Expected ProductCreated, got: {other:?}"),
    }
}

#[test]
fn inventory_create_emits_inventory_item_created_event() {
    let (commerce, _dir) = create_test_commerce();

    let mut subscription = commerce.subscribe_events();

    let item = commerce
        .inventory()
        .create_item(fixtures::create_inventory_input())
        .expect("create inventory");

    let event = subscription.try_recv().expect("should have InventoryItemCreated event");

    match event {
        CommerceEvent::InventoryItemCreated { item_id, sku, name, .. } => {
            assert_eq!(item_id, item.id);
            assert_eq!(sku, item.sku);
            assert_eq!(name, "Test Inventory Item");
        }
        other => panic!("Expected InventoryItemCreated, got: {other:?}"),
    }
}

#[test]
fn return_create_emits_return_requested_event() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Subscribe after order creation
    let mut subscription = commerce.subscribe_events();

    let ret = commerce
        .returns()
        .create(stateset_core::CreateReturn {
            order_id: order.id,
            reason: stateset_core::ReturnReason::Defective,
            items: vec![stateset_core::CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create return");

    let event = subscription.try_recv().expect("should have ReturnRequested event");

    match event {
        CommerceEvent::ReturnRequested {
            return_id,
            order_id,
            customer_id,
            reason,
            item_count,
            ..
        } => {
            assert_eq!(return_id, ret.id);
            assert_eq!(order_id, order.id);
            assert_eq!(customer_id, customer.id);
            assert_eq!(reason, stateset_core::ReturnReason::Defective);
            assert_eq!(item_count, 1);
        }
        other => panic!("Expected ReturnRequested, got: {other:?}"),
    }
}

#[test]
fn order_ship_emits_multiple_status_events() {
    let (commerce, _dir) = create_test_commerce();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    let order =
        commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Subscribe after creation
    let mut subscription = commerce.subscribe_events();

    // Shipping from Pending triggers: Pending->Confirmed, Confirmed->Processing, Processing->Shipped
    commerce.orders().ship(order.id, Some("TRACK-XYZ")).expect("ship order");

    // Collect all events
    let mut events = Vec::new();
    while let Some(event) = subscription.try_recv() {
        events.push(event);
    }

    // We should see at least one OrderStatusChanged event with to_status = Shipped
    let has_shipped_event = events.iter().any(|e| {
        matches!(
            e,
            CommerceEvent::OrderStatusChanged {
                to_status: stateset_core::OrderStatus::Shipped,
                ..
            }
        )
    });

    assert!(has_shipped_event, "Expected at least one Shipped status event, got: {events:?}");
    assert!(events.len() >= 2, "Expected multiple events for ship lifecycle, got {}", events.len());
}

#[test]
fn event_store_persists_events() {
    let (commerce, _dir) = create_test_commerce();

    // Subscribe (not strictly needed but let's verify store works)
    let _subscription = commerce.subscribe_events();

    let customer =
        commerce.customers().create(fixtures::create_customer_input()).expect("create customer");

    commerce.orders().create(fixtures::create_order_input(customer.id)).expect("create order");

    // Check that the event store has recorded events
    if let Some(store) = commerce.events().event_store() {
        let events = store.get_events_since(0, 50).expect("get events from store");
        assert!(!events.is_empty(), "Event store should have events");
    }
}
