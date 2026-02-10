#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    AdjustInventory, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder,
    CreateOrderItem, CustomerRepository, InventoryRepository, OrderRepository, OrderStatus,
    PaymentStatus, UpdateCustomer, UpdateOrder,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn setup_db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("failed to create in-memory db")
}

fn create_test_customer(db: &SqliteDatabase) -> Uuid {
    db.customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("failed to create customer")
        .id
}

fn create_test_order(db: &SqliteDatabase, customer_id: Uuid) -> stateset_core::Order {
    db.orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "TEST-SKU-001".into(),
                name: "Test Product".into(),
                quantity: 1,
                unit_price: dec!(9.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("failed to create order")
}

#[test]
fn order_update_batch_atomic_rejects_invalid_transition() {
    let db = setup_db();
    let customer_id = create_test_customer(&db);
    let order = create_test_order(&db, customer_id);

    let result = db.orders().update_batch_atomic(vec![(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Delivered),
            ..Default::default()
        },
    )]);

    match result {
        Err(CommerceError::InvalidOrderStatusTransition { .. }) => {}
        other => panic!("expected InvalidOrderStatusTransition, got {other:?}"),
    }
}

#[test]
fn order_update_batch_atomic_rejects_refund_without_payment() {
    let db = setup_db();
    let customer_id = create_test_customer(&db);
    let order = create_test_order(&db, customer_id);

    // Move order to Delivered so Refund is a valid status transition.
    let repo = db.orders();
    repo.update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Confirmed),
            ..Default::default()
        },
    )
    .expect("failed to update order to confirmed");
    repo.update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Processing),
            ..Default::default()
        },
    )
    .expect("failed to update order to processing");
    repo.update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Shipped),
            ..Default::default()
        },
    )
    .expect("failed to update order to shipped");
    repo.update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Delivered),
            ..Default::default()
        },
    )
    .expect("failed to update order to delivered");

    let result = db.orders().update_batch_atomic(vec![(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Refunded),
            payment_status: Some(PaymentStatus::Pending),
            ..Default::default()
        },
    )]);

    match result {
        Err(CommerceError::OrderCannotBeRefunded(_)) => {}
        other => panic!("expected OrderCannotBeRefunded, got {other:?}"),
    }
}

#[test]
fn customer_update_batch_atomic_validates_email() {
    let db = setup_db();
    let customer_id = create_test_customer(&db);

    let result = db.customers().update_batch_atomic(vec![(
        customer_id,
        UpdateCustomer {
            email: Some("invalid".into()),
            ..Default::default()
        },
    )]);

    match result {
        Err(CommerceError::ValidationError(_)) => {}
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn inventory_adjust_batch_atomic_rejects_zero_quantity() {
    let db = setup_db();
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "INV-001".into(),
            name: "Inventory Item".into(),
            description: Some("Test item".into()),
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .expect("failed to create inventory item");

    let result = db.inventory().adjust_batch_atomic(vec![AdjustInventory {
        sku: "INV-001".into(),
        location_id: None,
        quantity: dec!(0),
        reason: "no-op".into(),
        reference_type: None,
        reference_id: None,
    }]);

    match result {
        Err(CommerceError::ValidationError(_)) => {}
        other => panic!("expected ValidationError, got {other:?}"),
    }
}
