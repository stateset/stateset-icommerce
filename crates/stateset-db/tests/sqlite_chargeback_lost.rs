#![cfg(feature = "sqlite")]
//! A lost chargeback moves the order out of `paid` (SQLite).
//!
//! A card payment under dispute is `Disputed`; the dispute resolves either in
//! the merchant's favour (`Disputed -> Completed`, nothing else changes) or
//! against it (`Disputed -> Refunded`, a *lost chargeback*). A lost
//! chargeback means the network already reversed the charge, so the payment's
//! whole remaining balance has left the merchant: it is recorded on the refund
//! ledger (reason `chargeback_lost`), `amount_refunded` becomes the payment
//! amount, and the order's payment status is re-derived — `refunded` when the
//! whole order was lost, `partially_refunded` when only part of it was. The
//! refund guards still hold afterwards. Mirrored in
//! `postgres_chargeback_lost.rs`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    CustomerRepository, LOST_CHARGEBACK_REFUND_REASON, OrderId, OrderRepository, Payment,
    PaymentId, PaymentMethodType, PaymentRepository, PaymentStatus, PaymentTransactionStatus,
    ProductId, RefundStatus, UpdatePayment,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

/// A single-line order whose total is exactly `total`.
fn order_totalling(db: &SqliteDatabase, total: Decimal) -> OrderId {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("chargeback-{}@example.com", Uuid::new_v4()),
            first_name: "Charge".into(),
            last_name: "Back".into(),
            ..Default::default()
        })
        .expect("create customer");
    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("CB-{}", Uuid::new_v4()),
                name: "Disputed widget".into(),
                quantity: 1,
                unit_price: total,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
        .id
}

fn captured(db: &SqliteDatabase, order_id: OrderId, amount: Decimal) -> Payment {
    let payment = db
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("complete payment")
}

fn set_status(
    db: &SqliteDatabase,
    id: PaymentId,
    status: PaymentTransactionStatus,
) -> stateset_core::Result<Payment> {
    db.payments().update(id, UpdatePayment { status: Some(status), ..Default::default() })
}

fn order_payment_status(db: &SqliteDatabase, order_id: OrderId) -> PaymentStatus {
    db.orders().get(order_id).expect("get order").expect("order").payment_status
}

fn refund(db: &SqliteDatabase, payment_id: PaymentId, amount: Decimal) -> stateset_core::Refund {
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id, amount: Some(amount), ..Default::default() })
        .expect("create refund");
    db.payments().complete_refund(refund.id).expect("complete refund")
}

fn chargeback_events(db: &SqliteDatabase, payment_id: PaymentId) -> Vec<serde_json::Value> {
    let conn = db.conn().expect("conn");
    let mut stmt = conn
        .prepare(
            "SELECT payload FROM kernel_outbox
             WHERE event_type = 'payments.chargeback_lost.v1' AND aggregate_id = ?",
        )
        .expect("prepare");
    stmt.query_map([payment_id.to_string()], |row| row.get::<_, String>(0))
        .expect("query")
        .map(|raw| serde_json::from_str(&raw.expect("row")).expect("json"))
        .collect()
}

#[test]
fn lost_chargeback_on_the_whole_order_marks_it_refunded() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = captured(&db, order_id, dec!(100.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);

    // Under dispute the money is contested, not lost: the order still reads paid.
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);

    let lost = set_status(&db, payment.id, PaymentTransactionStatus::Refunded)
        .expect("a lost chargeback is recorded, not refused");
    assert_eq!(lost.status, PaymentTransactionStatus::Refunded);
    assert_eq!(lost.amount_refunded, dec!(100.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);

    // The reversal is on the refund ledger, marked as a chargeback.
    let rows = db.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RefundStatus::Completed);
    assert_eq!(rows[0].amount, dec!(100.00));
    assert_eq!(rows[0].reason.as_deref(), Some(LOST_CHARGEBACK_REFUND_REASON));
    assert!(rows[0].refunded_at.is_some());
    let events = chargeback_events(&db, payment.id);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["amount"], "100.00");

    // Nothing is left to refund, and no money is held against the order.
    let err = db
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, ..Default::default() })
        .expect_err("a charged-back payment cannot be refunded again");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert!(db.payments().open_captures_for_order(order_id).expect("open").is_empty());
}

#[test]
fn lost_chargeback_on_part_of_the_order_marks_it_partially_refunded() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let kept = captured(&db, order_id, dec!(60.00));
    let disputed = captured(&db, order_id, dec!(40.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);

    set_status(&db, disputed.id, PaymentTransactionStatus::Disputed).expect("dispute");
    set_status(&db, disputed.id, PaymentTransactionStatus::Refunded).expect("chargeback lost");
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::PartiallyRefunded);

    // The over-refund caps still hold on what remains: 60.00 on the kept
    // payment, nothing on the charged-back one.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: kept.id,
            amount: Some(dec!(60.01)),
            ..Default::default()
        })
        .expect_err("cannot refund more than remains");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    assert!(
        db.payments()
            .create_refund(CreateRefund {
                payment_id: disputed.id,
                amount: Some(dec!(0.01)),
                ..Default::default()
            })
            .is_err()
    );
    refund(&db, kept.id, dec!(60.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn lost_chargeback_after_a_partial_refund_reverses_only_the_remainder() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = captured(&db, order_id, dec!(100.00));
    refund(&db, payment.id, dec!(30.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::PartiallyRefunded);

    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    let lost =
        set_status(&db, payment.id, PaymentTransactionStatus::Refunded).expect("chargeback lost");
    assert_eq!(lost.amount_refunded, dec!(100.00));
    let reversal = db
        .payments()
        .get_refunds(payment.id)
        .expect("refunds")
        .into_iter()
        .find(|r| r.reason.as_deref() == Some(LOST_CHARGEBACK_REFUND_REASON))
        .expect("chargeback row");
    assert_eq!(reversal.amount, dec!(70.00), "only the unrefunded remainder was reversed");
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn won_chargeback_leaves_the_order_paid() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = captured(&db, order_id, dec!(100.00));
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");

    let won = set_status(&db, payment.id, PaymentTransactionStatus::Completed).expect("won");
    assert_eq!(won.status, PaymentTransactionStatus::Completed);
    assert_eq!(won.amount_refunded, Decimal::ZERO);
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);
    assert!(db.payments().get_refunds(payment.id).expect("refunds").is_empty());
    assert!(chargeback_events(&db, payment.id).is_empty());
    // The payment is still refundable in full.
    refund(&db, payment.id, dec!(100.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn lost_chargeback_through_the_atomic_batch_is_recorded_the_same_way() {
    let db = db();
    let order_id = order_totalling(&db, dec!(50.00));
    let payment = captured(&db, order_id, dec!(50.00));
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");

    db.payments()
        .update_batch_atomic(vec![(
            payment.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                ..Default::default()
            },
        )])
        .expect("batch chargeback lost");
    let after = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(after.amount_refunded, dec!(50.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
    assert_eq!(db.payments().get_refunds(payment.id).expect("refunds").len(), 1);
}
