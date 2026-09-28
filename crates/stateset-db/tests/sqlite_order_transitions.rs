#![cfg(feature = "sqlite")]

use rusqlite::params;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CreateRefund, CustomerId, CustomerRepository, InventoryRepository, OrderId,
    OrderRepository, OrderStatus, PaymentMethodType, PaymentRepository, PaymentStatus, ProductId,
    ReservationStatus, UpdateOrder,
};
use stateset_db::SqliteDatabase;

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

fn create_order(db: &SqliteDatabase, customer_id: CustomerId) -> stateset_core::Order {
    db.orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
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

/// Record a real payment for the order's whole total (create + complete), so
/// the order's derived payment status becomes `paid`.
fn pay_in_full(db: &SqliteDatabase, order_id: OrderId) -> stateset_core::Payment {
    let order = db.orders().get(order_id).expect("get order").expect("order");
    let payment = db
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: order.total_amount,
            currency: Some(order.currency),
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("complete payment")
}

fn set_status(db: &SqliteDatabase, order_id: OrderId, status: OrderStatus) {
    db.orders()
        .update(order_id, UpdateOrder { status: Some(status), ..Default::default() })
        .expect("update status");
}

#[test]
fn sqlite_rejects_invalid_status_transition() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "transition@example.com");
    let order = create_order(&db, customer.id);

    let result = db.orders().update(
        order.id,
        UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
    );

    assert!(matches!(result, Err(CommerceError::InvalidOrderStatusTransition { .. })));
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
        UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
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
        UpdateOrder { status: Some(OrderStatus::Refunded), ..Default::default() },
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
    let payment = pay_in_full(&db, order.id);
    assert_eq!(db.orders().get(order.id).unwrap().unwrap().payment_status, PaymentStatus::Paid);
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, ..Default::default() })
        .expect("create refund");
    db.payments().complete_refund(refund.id).expect("complete refund");

    let updated = db
        .orders()
        .update(order.id, UpdateOrder { status: Some(OrderStatus::Refunded), ..Default::default() })
        .expect("refund order");

    assert_eq!(updated.status, OrderStatus::Refunded);
    assert_eq!(updated.payment_status, PaymentStatus::Refunded, "derived from the refund");
}

/// An order's payment status is derived from its payments and refunds: an
/// update that declares one is refused outright — alone, alongside a refund
/// transition (so an unpaid order cannot be made refundable by declaring it
/// paid), and inside an atomic batch — and nothing is written.
#[test]
fn sqlite_update_refuses_a_declared_payment_status() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "refund-declared@example.com");
    let order = create_order(&db, customer.id);

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);
    set_status(&db, order.id, OrderStatus::Shipped);
    set_status(&db, order.id, OrderStatus::Delivered);
    let before = db.orders().get(order.id).expect("get").expect("order");

    for declared in [PaymentStatus::Paid, PaymentStatus::Refunded] {
        for status in [None, Some(OrderStatus::Refunded)] {
            let result = db.orders().update(
                order.id,
                UpdateOrder { status, payment_status: Some(declared), ..Default::default() },
            );
            assert!(
                matches!(&result, Err(CommerceError::ValidationError(m)) if m.contains("derived")),
                "declaring {declared} must be refused: {result:?}"
            );
        }
    }
    let batch = db.orders().update_batch_atomic(vec![
        (order.id, UpdateOrder { notes: Some("fine".into()), ..Default::default() }),
        (order.id, UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() }),
    ]);
    assert!(matches!(batch, Err(CommerceError::ValidationError(_))), "{batch:?}");

    let stored = db.orders().get(order.id).expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Delivered);
    assert_eq!(stored.payment_status, PaymentStatus::Pending);
    assert_eq!(stored.notes, before.notes, "the atomic batch wrote nothing");
    assert_eq!(stored.version, before.version);

    // The supported path: record the payment, and the order reads `paid`.
    pay_in_full(&db, order.id);
    assert_eq!(db.orders().get(order.id).unwrap().unwrap().payment_status, PaymentStatus::Paid);
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

    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "EXP-SKU-001".to_string(),
                name: "Expirable Item".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);

    let reservation_id: String = {
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
            params![reservation_id],
        )
        .expect("expire reservation");

        reservation_id
    };

    let result = db
        .orders()
        .update(order.id, UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() });

    assert!(matches!(result, Err(CommerceError::ReservationExpired(_))));

    let refreshed = db.orders().get(order.id).expect("get order").expect("order exists");
    assert_eq!(refreshed.status, OrderStatus::Processing);

    let status: String = db
        .conn()
        .expect("get sqlite connection")
        .query_row(
            "SELECT status FROM inventory_reservations WHERE id = ?",
            params![reservation_id],
            |row| row.get(0),
        )
        .expect("get reservation status");
    assert_eq!(status, "expired");
}

#[test]
fn sqlite_ship_does_not_confirm_other_reservations_when_one_expired() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "partial-expire@example.com");

    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "EXP-SKU-A".to_string(),
            name: "Expirable Item A".to_string(),
            initial_quantity: Some(dec!(1)),
            ..Default::default()
        })
        .expect("create inventory item A");
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "EXP-SKU-B".to_string(),
            name: "Expirable Item B".to_string(),
            initial_quantity: Some(dec!(1)),
            ..Default::default()
        })
        .expect("create inventory item B");

    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "EXP-SKU-A".to_string(),
                    name: "Expirable Item A".to_string(),
                    quantity: 1,
                    unit_price: dec!(10.00),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "EXP-SKU-B".to_string(),
                    name: "Expirable Item B".to_string(),
                    quantity: 1,
                    unit_price: dec!(12.00),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("create order");

    set_status(&db, order.id, OrderStatus::Confirmed);
    set_status(&db, order.id, OrderStatus::Processing);

    let reservations = db
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .expect("list reservations");
    assert_eq!(reservations.len(), 2);

    let expire_id = reservations[0].id;
    let keep_id = reservations[1].id;

    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute(
            "UPDATE inventory_reservations SET expires_at = datetime('now', '-1 hour') WHERE id = ?",
            params![expire_id.to_string()],
        )
        .expect("expire reservation");
    }

    let result = db
        .orders()
        .update(order.id, UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() });

    assert!(matches!(
        result,
        Err(CommerceError::ReservationExpired(id)) if id == expire_id
    ));

    let refreshed = db
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .expect("list reservations");

    let status_by_id = refreshed
        .into_iter()
        .map(|r| (r.id, r.status))
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(status_by_id.get(&expire_id), Some(&ReservationStatus::Expired));
    assert_eq!(status_by_id.get(&keep_id), Some(&ReservationStatus::Pending));
}
