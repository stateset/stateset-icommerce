#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund, CustomerRepository,
    OrderId, OrderRepository, PaymentId, PaymentRepository, PaymentStatus,
    PaymentTransactionStatus, ProductId, UpdatePayment,
};
use stateset_db::SqliteDatabase;

fn fixture() -> (SqliteDatabase, OrderId) {
    let db = SqliteDatabase::in_memory().unwrap();
    let buyer = db
        .customers()
        .create(CreateCustomer {
            email: "projection@example.test".into(),
            first_name: "Projection".into(),
            last_name: "Buyer".into(),
            ..Default::default()
        })
        .unwrap();
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: buyer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SERVICE".into(),
                name: "Service".into(),
                quantity: 1,
                unit_price: dec!(0.30),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    (db, order.id)
}
fn capture(db: &SqliteDatabase, order: OrderId, amount: Decimal) -> PaymentId {
    db.payments()
        .create(CreatePayment { order_id: Some(order), amount, ..Default::default() })
        .unwrap()
        .id
}
fn status(db: &SqliteDatabase, order: OrderId) -> PaymentStatus {
    db.orders().get(order).unwrap().unwrap().payment_status
}
fn refund(db: &SqliteDatabase, payment: PaymentId, amount: Decimal) -> uuid::Uuid {
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment,
            amount: Some(amount),
            ..Default::default()
        })
        .unwrap();
    db.payments().complete_refund(refund.id).unwrap();
    refund.id
}

#[test]
fn exact_split_payments_project_partial_then_paid_without_pending_captures() {
    let (db, order) = fixture();
    let first = capture(&db, order, dec!(0.10));
    let second = capture(&db, order, dec!(0.20));
    assert_eq!(status(&db, order), PaymentStatus::Pending);
    db.payments().mark_completed(first).unwrap();
    assert_eq!(status(&db, order), PaymentStatus::PartiallyPaid);
    db.payments()
        .update(
            second,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(status(&db, order), PaymentStatus::Paid);
    let version = db.orders().get(order).unwrap().unwrap().version;
    db.payments().mark_completed(second).unwrap();
    assert_eq!(db.orders().get(order).unwrap().unwrap().version, version);
}

#[test]
fn refunds_project_across_multiple_captures_and_duplicate_refunds_do_not_change_version() {
    let (db, order) = fixture();
    let first = capture(&db, order, dec!(0.10));
    let second = capture(&db, order, dec!(0.20));
    db.payments().mark_completed(first).unwrap();
    db.payments().mark_completed(second).unwrap();
    let returned = refund(&db, first, dec!(0.10));
    assert_eq!(status(&db, order), PaymentStatus::PartiallyRefunded);
    let version = db.orders().get(order).unwrap().unwrap().version;
    db.payments().complete_refund(returned).unwrap();
    assert_eq!(db.orders().get(order).unwrap().unwrap().version, version);
    refund(&db, second, dec!(0.20));
    assert_eq!(status(&db, order), PaymentStatus::Refunded);
}

#[test]
fn failed_pending_payment_does_not_erase_settled_partial_payment() {
    let (db, order) = fixture();
    let first = capture(&db, order, dec!(0.10));
    let second = capture(&db, order, dec!(0.20));
    db.payments().mark_completed(first).unwrap();
    db.payments().mark_failed(second, "declined", None).unwrap();
    assert_eq!(status(&db, order), PaymentStatus::PartiallyPaid);
}

#[test]
fn order_projection_failure_rolls_back_payment_and_refund_writes() {
    let (db, order) = fixture();
    let payment = capture(&db, order, dec!(0.30));
    db.conn().unwrap().execute_batch("CREATE TRIGGER reject_payment_projection BEFORE UPDATE OF payment_status ON orders BEGIN SELECT RAISE(ABORT, 'projection unavailable'); END;").unwrap();
    assert!(db.payments().mark_completed(payment).is_err());
    assert_eq!(
        db.payments().get(payment).unwrap().unwrap().status,
        PaymentTransactionStatus::Pending
    );
    assert_eq!(status(&db, order), PaymentStatus::Pending);
    db.conn().unwrap().execute_batch("DROP TRIGGER reject_payment_projection;").unwrap();
    db.payments().mark_completed(payment).unwrap();
    let returned = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .unwrap();
    db.conn().unwrap().execute_batch("CREATE TRIGGER reject_refund_projection BEFORE UPDATE OF payment_status ON orders BEGIN SELECT RAISE(ABORT, 'projection unavailable'); END;").unwrap();
    assert!(db.payments().complete_refund(returned.id).is_err());
    assert_eq!(db.payments().get(payment).unwrap().unwrap().amount_refunded, dec!(0));
    assert_eq!(
        db.payments().get_refund(returned.id).unwrap().unwrap().status.to_string(),
        "pending"
    );
    assert_eq!(status(&db, order), PaymentStatus::Paid);
}

#[test]
fn refused_recapture_does_not_erase_the_refunded_order_status() {
    let (db, order) = fixture();
    let first = capture(&db, order, dec!(0.30));
    db.payments().mark_completed(first).unwrap();
    refund(&db, first, dec!(0.30));
    assert_eq!(status(&db, order), PaymentStatus::Refunded);
    // The engine caps gross captures, so a refund does not authorize a second charge.
    assert!(
        db.payments()
            .create(CreatePayment {
                order_id: Some(order),
                amount: dec!(0.30),
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(status(&db, order), PaymentStatus::Refunded);
}

#[test]
fn concurrent_split_completions_leave_the_order_fully_paid() {
    let (db, order) = fixture();
    let first = capture(&db, order, dec!(0.10));
    let second = capture(&db, order, dec!(0.20));
    let db = std::sync::Arc::new(db);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = [first, second]
        .into_iter()
        .map(|id| {
            let db = db.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                db.payments().mark_completed(id).unwrap();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(status(&db, order), PaymentStatus::Paid);
}
