#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use rusqlite::params;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CustomerRepository,
    InventoryRepository, OrderRepository, OrderStatus, PaymentStatus, UpdateOrder,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn create_customer(db: &SqliteDatabase, email: &str) -> stateset_core::Customer {
    db.customers()
        .create(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            ..Default::default()
        })
        .expect("create customer")
}

fn create_order(db: &SqliteDatabase, customer_id: Uuid) -> stateset_core::Order {
    db.orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-TRANSITION".to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
}

fn set_status(db: &SqliteDatabase, order_id: Uuid, status: OrderStatus) {
    db.orders()
        .update(
            order_id,
            UpdateOrder {
                status: Some(status),
                ..Default::default()
            },
        )
        .expect("update status");
}

#[test]
fn sqlite_rejects_invalid_status_transition() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "transition@example.com");
    let order = create_order(&db, customer.id);

    let result = db.orders().update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Delivered),
            ..Default::default()
        },
    );

    assert!(matches!(
        result,
        Err(CommerceError::InvalidOrderStatusTransition { .. })
    ));
}

#[test]
fn sqlite_rejects_cancel_after_shipped() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "cancel@example.com");
    let order = create_order(&db, customer.id);

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);
    set_status(&db, order.id, OrderStatus::Shipped);

    let result = db.orders().update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Cancelled),
            ..Default::default()
        },
    );

    assert!(matches!(result, Err(CommerceError::OrderCannotBeCancelled(_))));
}

#[test]
fn sqlite_requires_payment_for_refund() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "refund@example.com");
    let order = create_order(&db, customer.id);

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);
    set_status(&db, order.id, OrderStatus::Shipped);
    set_status(&db, order.id, OrderStatus::Delivered);

    let result = db.orders().update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Refunded),
            ..Default::default()
        },
    );

    assert!(matches!(result, Err(CommerceError::OrderCannotBeRefunded(_))));
}

#[test]
fn sqlite_allows_refund_with_payment_status() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "refund-paid@example.com");
    let order = create_order(&db, customer.id);

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);
    set_status(&db, order.id, OrderStatus::Shipped);
    set_status(&db, order.id, OrderStatus::Delivered);

    let updated = db
        .orders()
        .update(
            order.id,
            UpdateOrder {
                status: Some(OrderStatus::Refunded),
                payment_status: Some(PaymentStatus::Refunded),
                ..Default::default()
            },
        )
        .expect("refund order");

    assert_eq!(updated.status, OrderStatus::Refunded);
}

#[test]
fn sqlite_ship_fails_when_reservation_expired() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "expired-reservation@example.com");

    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "EXP-SKU-001".to_string(),
            name: "Expirable Item".to_string(),
            initial_quantity: Some(dec!(1)),
            ..Default::default()
        })
        .expect("create inventory item");

    let order = db.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: Uuid::new_v4(),
            sku: "EXP-SKU-001".to_string(),
            name: "Expirable Item".to_string(),
            quantity: 1,
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        ..Default::default()
    }).expect("create order");

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);

    let conn = db.conn().expect("get sqlite connection");
    let reservation_id: String = conn
        .query_row(
            "SELECT id FROM inventory_reservations WHERE reference_type = 'order' AND reference_id = ?",
            params![order.id.to_string()],
            |row| row.get(0),
        )
        .expect("get reservation id");

    conn.execute(
        "UPDATE inventory_reservations SET expires_at = datetime('now', '-1 hour') WHERE id = ?",
        params![reservation_id.clone()],
    )
    .expect("expire reservation");

    let result = db.orders().update(
        order.id,
        UpdateOrder {
            status: Some(OrderStatus::Shipped),
            ..Default::default()
        },
    );

    assert!(matches!(result, Err(CommerceError::ReservationExpired(_))));

    let refreshed = db
        .orders()
        .get(order.id)
        .expect("get order")
        .expect("order exists");
    assert_eq!(refreshed.status, OrderStatus::Processing);

    let status: String = conn
        .query_row(
            "SELECT status FROM inventory_reservations WHERE id = ?",
            params![reservation_id],
            |row| row.get(0),
        )
        .expect("get reservation status");
    assert_eq!(status, "expired");
}
