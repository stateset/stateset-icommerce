#![cfg(feature = "sqlite")]
//! Partial capture, pending-refund settlement, and partial lost chargebacks
//! (SQLite). Mirrored in `postgres_capture_refund_chargeback.rs`.
//!
//! 1. A payment may capture LESS than it authorized: the captured amount is
//!    recorded exactly, bounds every refund, and is the only slice of the
//!    order total the payment holds.
//! 2. A refund the processor left pending carries the processor's id and can
//!    be found again; completing it counts toward the refunded totals and
//!    failing it releases its reservation — each exactly once.
//! 3. A dispute lost for PART of a payment writes a `chargeback_lost` refund
//!    row for the disputed amount and leaves the payment partially refunded;
//!    a disputed payment can never be cancelled.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    CustomerRepository, LOST_CHARGEBACK_REFUND_REASON, OrderId, OrderRepository, Payment,
    PaymentId, PaymentMethodType, PaymentRepository, PaymentStatus, PaymentTransactionStatus,
    ProductId, RefundStatus, UpdatePayment,
};
use stateset_db::{DatabaseConfig, SqliteDatabase};
use std::sync::Arc;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

fn order_totalling(db: &SqliteDatabase, total: Decimal) -> OrderId {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("capture-{}@example.com", Uuid::new_v4()),
            first_name: "Partial".into(),
            last_name: "Capture".into(),
            ..Default::default()
        })
        .expect("create customer");
    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PC-{}", Uuid::new_v4()),
                name: "Widget".into(),
                quantity: 1,
                unit_price: total,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
        .id
}

fn authorized(db: &SqliteDatabase, order_id: OrderId, amount: Decimal) -> Payment {
    db.payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .expect("create payment")
}

fn order_payment_status(db: &SqliteDatabase, order_id: OrderId) -> PaymentStatus {
    db.orders().get(order_id).expect("get order").expect("order").payment_status
}

fn set_status(
    db: &SqliteDatabase,
    id: PaymentId,
    status: PaymentTransactionStatus,
) -> stateset_core::Result<Payment> {
    db.payments().update(id, UpdatePayment { status: Some(status), ..Default::default() })
}

fn events(db: &SqliteDatabase, event_type: &str, aggregate_id: &str) -> Vec<serde_json::Value> {
    let conn = db.conn().expect("conn");
    let mut stmt = conn
        .prepare("SELECT payload FROM kernel_outbox WHERE event_type = ? AND aggregate_id = ?")
        .expect("prepare");
    stmt.query_map([event_type, aggregate_id], |row| row.get::<_, String>(0))
        .expect("query")
        .map(|raw| serde_json::from_str(&raw.expect("row")).expect("json"))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Partial capture
// ---------------------------------------------------------------------------

#[test]
fn full_capture_records_the_whole_amount_as_captured() {
    let db = db();
    let order_id = order_totalling(&db, dec!(80.00));
    let payment = authorized(&db, order_id, dec!(80.00));
    assert_eq!(payment.captured_amount, None, "nothing is captured before capture");
    let done = db.payments().mark_completed(payment.id).expect("complete");
    assert_eq!(done.captured_amount, Some(dec!(80.00)));
    assert_eq!(done.captured(), dec!(80.00));
}

#[test]
fn partial_capture_is_recorded_exactly_and_bounds_refunds() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = authorized(&db, order_id, dec!(100.00));

    let captured = db.payments().mark_captured(payment.id, dec!(60.25)).expect("partial capture");
    assert_eq!(captured.status, PaymentTransactionStatus::Completed);
    assert_eq!(captured.amount, dec!(100.00), "the authorized amount is kept");
    assert_eq!(captured.captured_amount, Some(dec!(60.25)));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::PartiallyPaid);
    let facts = events(&db, "payments.captured.v1", &payment.id.to_string());
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["captured_amount"], "60.25");
    assert_eq!(facts[0]["partial"], true);

    // Only what moved can be refunded.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.26)),
            ..Default::default()
        })
        .expect_err("refund above the capture");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");

    let full = db
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, ..Default::default() })
        .expect("full refund of the capture");
    assert_eq!(full.amount, dec!(60.25));
    db.payments().complete_refund(full.id).expect("complete");
    let after = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(after.status, PaymentTransactionStatus::Refunded);
    assert_eq!(after.amount_refunded, dec!(60.25));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
    assert!(db.payments().open_captures_for_order(order_id).expect("open").is_empty());
}

#[test]
fn partial_capture_releases_the_uncaptured_slice_of_the_order_total() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let first = authorized(&db, order_id, dec!(100.00));
    // While in flight the whole authorization is reserved.
    assert!(matches!(
        db.payments().create(CreatePayment {
            order_id: Some(order_id),
            amount: dec!(0.01),
            ..Default::default()
        }),
        Err(CommerceError::CaptureExceedsOrderTotal { .. })
    ));
    db.payments().mark_captured(first.id, dec!(70.00)).expect("capture 70");

    let top_up = authorized(&db, order_id, dec!(30.00));
    db.payments().mark_completed(top_up.id).expect("capture the rest");
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);
}

#[test]
fn partial_capture_amount_is_validated() {
    let db = db();
    let order_id = order_totalling(&db, dec!(50.00));
    let payment = authorized(&db, order_id, dec!(50.00));
    for bad in [dec!(0), dec!(-1), dec!(50.01), dec!(10.001)] {
        assert!(db.payments().mark_captured(payment.id, bad).is_err(), "{bad} must be refused");
    }
    let unchanged = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(unchanged.status, PaymentTransactionStatus::Pending);
    assert_eq!(unchanged.captured_amount, None);

    db.payments().mark_captured(payment.id, dec!(20.00)).expect("capture");
    // Re-recording the same capture is idempotent; a different one conflicts.
    let again = db.payments().mark_captured(payment.id, dec!(20.00)).expect("same capture");
    assert_eq!(again.captured_amount, Some(dec!(20.00)));
    assert!(matches!(
        db.payments().mark_captured(payment.id, dec!(25.00)),
        Err(CommerceError::Conflict(_))
    ));
    // A plain `mark_completed` retry keeps the partial capture.
    let retried = db.payments().mark_completed(payment.id).expect("retry");
    assert_eq!(retried.captured_amount, Some(dec!(20.00)));
    assert_eq!(events(&db, "payments.captured.v1", &payment.id.to_string()).len(), 1);
}

#[test]
fn captured_amount_migration_backfills_settled_payments_and_reruns() {
    let db = db();
    let order_id = order_totalling(&db, dec!(90.00));
    let settled = authorized(&db, order_id, dec!(40.00));
    db.payments().mark_completed(settled.id).expect("complete");
    let in_flight = authorized(&db, order_id, dec!(50.00));
    {
        let mut conn = db.conn().expect("conn");
        conn.execute_batch(
            "ALTER TABLE payments DROP COLUMN captured_amount;
             DELETE FROM _migrations WHERE name = '101_payment_captured_amount';",
        )
        .expect("rewind to the legacy schema");
        stateset_db::migrations::run_migrations(&mut conn).expect("migrate");
        stateset_db::migrations::run_migrations(&mut conn).expect("rerun is a no-op");
    }
    let settled = db.payments().get(settled.id).expect("get").expect("payment");
    assert_eq!(settled.captured_amount, Some(dec!(40.00)));
    let in_flight = db.payments().get(in_flight.id).expect("get").expect("payment");
    assert_eq!(in_flight.captured_amount, None);
}

// ---------------------------------------------------------------------------
// 2. Pending refunds: processor id, settlement, release
// ---------------------------------------------------------------------------

fn captured_payment(db: &SqliteDatabase, total: Decimal) -> (OrderId, Payment) {
    let order_id = order_totalling(db, total);
    let payment = authorized(db, order_id, total);
    (order_id, db.payments().mark_completed(payment.id).expect("complete"))
}

#[test]
fn in_flight_refunds_with_a_processor_id_are_listed_for_reconciliation() {
    let db = db();
    let (_, payment) = captured_payment(&db, dec!(100.00));
    let tracked = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect("refund");
    let untracked = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(5.00)),
            ..Default::default()
        })
        .expect("refund");
    assert!(db.payments().list_in_flight_refunds(10).expect("list").is_empty());

    let with_id = db.payments().set_refund_external_id(tracked.id, "re_123").expect("set id");
    assert_eq!(with_id.external_id.as_deref(), Some("re_123"));
    // Idempotent for the same id, refused for a different one.
    db.payments().set_refund_external_id(tracked.id, "re_123").expect("same id");
    assert!(matches!(
        db.payments().set_refund_external_id(tracked.id, "re_999"),
        Err(CommerceError::Conflict(_))
    ));
    assert_eq!(
        events(&db, "payments.refund_external_id_recorded.v1", &tracked.id.to_string()).len(),
        1
    );

    let listed = db.payments().list_in_flight_refunds(10).expect("list");
    assert_eq!(listed.iter().map(|r| r.id).collect::<Vec<_>>(), vec![tracked.id]);
    let _ = untracked;

    db.payments().complete_refund(tracked.id).expect("settle");
    assert!(db.payments().list_in_flight_refunds(10).expect("list").is_empty());
}

#[test]
fn settling_a_pending_refund_counts_once_and_failing_releases_the_reservation() {
    let db = db();
    let (order_id, payment) = captured_payment(&db, dec!(100.00));
    let pending = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(70.00)),
            ..Default::default()
        })
        .expect("refund");
    // The reservation holds while pending: only 30 is left.
    assert!(
        db.payments()
            .create_refund(CreateRefund {
                payment_id: payment.id,
                amount: Some(dec!(30.01)),
                ..Default::default()
            })
            .is_err()
    );

    let failed = db.payments().fail_refund(pending.id, "processor: expired card").expect("fail");
    assert_eq!(failed.status, RefundStatus::Failed);
    assert_eq!(events(&db, "payments.refund_failed.v1", &pending.id.to_string()).len(), 1);
    // Failing again is a no-op and emits nothing more.
    db.payments().fail_refund(pending.id, "again").expect("idempotent fail");
    assert_eq!(events(&db, "payments.refund_failed.v1", &pending.id.to_string()).len(), 1);
    // Released: the whole capture is refundable again.
    let second = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(100.00)),
            ..Default::default()
        })
        .expect("full refund after the release");
    db.payments().complete_refund(second.id).expect("complete");
    db.payments().complete_refund(second.id).expect("idempotent complete");
    let after = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(after.amount_refunded, dec!(100.00));
    assert_eq!(events(&db, "payments.refund_completed.v1", &second.id.to_string()).len(), 1);
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn concurrent_settlements_of_one_refund_fold_it_in_exactly_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("concurrent.db");
    let db = Arc::new(
        SqliteDatabase::new(&DatabaseConfig {
            url: path.to_str().expect("utf8").to_string(),
            max_connections: 8,
        })
        .expect("file db"),
    );
    let (_, payment) = captured_payment(&db, dec!(100.00));
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .expect("refund");

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let db = Arc::clone(&db);
            std::thread::spawn(move || {
                if i % 2 == 0 {
                    db.payments().complete_refund(refund.id).map(|r| r.status)
                } else {
                    db.payments().fail_refund(refund.id, "racing sweep").map(|r| r.status)
                }
            })
        })
        .collect();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    let stored = db.payments().get_refund(refund.id).expect("get").expect("refund");
    // Exactly one terminal outcome won; every success agrees with it.
    for outcome in outcomes.iter().flatten() {
        assert_eq!(*outcome, stored.status, "{outcomes:?}");
    }
    let after = db.payments().get(payment.id).expect("get").expect("payment");
    match stored.status {
        RefundStatus::Completed => assert_eq!(after.amount_refunded, dec!(40.00)),
        RefundStatus::Failed => assert_eq!(after.amount_refunded, Decimal::ZERO),
        other => panic!("refund left {other}"),
    }
}

// ---------------------------------------------------------------------------
// 3. Chargebacks
// ---------------------------------------------------------------------------

#[test]
fn disputed_payment_cannot_be_cancelled() {
    let db = db();
    let (order_id, payment) = captured_payment(&db, dec!(100.00));
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    let err = db.payments().cancel(payment.id).expect_err("a dispute must be resolved first");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let still = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(still.status, PaymentTransactionStatus::Disputed);
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Paid);
}

#[test]
fn partial_lost_chargeback_writes_a_refund_row_and_keeps_the_rest_paid() {
    let db = db();
    let (order_id, payment) = captured_payment(&db, dec!(100.00));
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    // A bare status flip to partially_refunded is still refused.
    assert!(set_status(&db, payment.id, PaymentTransactionStatus::PartiallyRefunded).is_err());

    let lost = db
        .payments()
        .record_lost_chargeback(payment.id, Some(dec!(35.50)))
        .expect("partial chargeback lost");
    assert_eq!(lost.status, PaymentTransactionStatus::PartiallyRefunded);
    assert_eq!(lost.amount_refunded, dec!(35.50));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::PartiallyRefunded);

    let rows = db.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RefundStatus::Completed);
    assert_eq!(rows[0].amount, dec!(35.50));
    assert_eq!(rows[0].reason.as_deref(), Some(LOST_CHARGEBACK_REFUND_REASON));
    let facts = events(&db, "payments.chargeback_lost.v1", &payment.id.to_string());
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["amount"], "35.50");
    assert_eq!(facts[0]["partial"], true);

    // What the network did not take is still refundable, and no more.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(64.51)),
            ..Default::default()
        })
        .expect_err("over the remainder");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    let rest = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(64.50)),
            ..Default::default()
        })
        .expect("refund the remainder");
    db.payments().complete_refund(rest.id).expect("complete");
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn lost_chargeback_for_the_whole_remainder_is_the_full_path() {
    let db = db();
    let (order_id, payment) = captured_payment(&db, dec!(100.00));
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    let lost =
        db.payments().record_lost_chargeback(payment.id, None).expect("full chargeback lost");
    assert_eq!(lost.status, PaymentTransactionStatus::Refunded);
    assert_eq!(lost.amount_refunded, dec!(100.00));
    assert_eq!(order_payment_status(&db, order_id), PaymentStatus::Refunded);
}

#[test]
fn lost_chargeback_is_bounded_and_requires_a_dispute() {
    let db = db();
    let (_, payment) = captured_payment(&db, dec!(100.00));
    assert!(matches!(
        db.payments().record_lost_chargeback(payment.id, Some(dec!(10))),
        Err(CommerceError::ValidationError(_))
    ));
    // A refund in flight before the dispute keeps its reservation.
    let pending = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(30.00)),
            ..Default::default()
        })
        .expect("refund");
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    for bad in [dec!(0), dec!(70.01), dec!(100.00)] {
        assert!(
            db.payments().record_lost_chargeback(payment.id, Some(bad)).is_err(),
            "{bad} must be refused"
        );
    }
    let still = db.payments().get(payment.id).expect("get").expect("payment");
    assert_eq!(still.status, PaymentTransactionStatus::Disputed);
    assert!(db.payments().get_refunds(payment.id).expect("refunds").len() == 1);
    // The pending refund cannot fold part of itself into the disputed payment.
    assert!(db.payments().complete_refund(pending.id).is_err());
}

#[test]
fn partial_lost_chargeback_on_a_partial_capture_is_bounded_by_the_capture() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = authorized(&db, order_id, dec!(100.00));
    db.payments().mark_captured(payment.id, dec!(50.00)).expect("capture 50");
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    assert!(db.payments().record_lost_chargeback(payment.id, Some(dec!(50.01))).is_err());
    let lost = db.payments().record_lost_chargeback(payment.id, None).expect("full loss");
    assert_eq!(lost.amount_refunded, dec!(50.00));
    assert_eq!(lost.status, PaymentTransactionStatus::Refunded);
}

#[test]
fn full_lost_chargeback_via_status_write_uses_the_captured_amount() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let payment = authorized(&db, order_id, dec!(100.00));
    db.payments().mark_captured(payment.id, dec!(45.00)).expect("capture 45");
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).expect("dispute");
    let lost = set_status(&db, payment.id, PaymentTransactionStatus::Refunded).expect("lost");
    assert_eq!(lost.amount_refunded, dec!(45.00), "only the capture was reversed");
    let rows = db.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(rows[0].amount, dec!(45.00));
}
