#![cfg(feature = "postgres")]
//! PostgreSQL twin of `sqlite_capture_refund_chargeback.rs`: partial capture
//! is recorded exactly and bounds refunds, pending refunds carry the
//! processor's id and settle exactly once, and a dispute lost for part of a
//! payment writes a `chargeback_lost` refund row; a disputed payment can never
//! be cancelled.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    LOST_CHARGEBACK_REFUND_REASON, OrderId, Payment, PaymentId, PaymentMethodType, PaymentStatus,
    PaymentTransactionStatus, ProductId, RefundStatus, UpdatePayment,
};
use stateset_db::PostgresDatabase;
use std::env;
use std::sync::Arc;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

macro_rules! require_db {
    () => {
        match connect().await {
            Some(db) => db,
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

async fn order_totalling(db: &PostgresDatabase, total: Decimal) -> OrderId {
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("pg-capture-{}@example.com", Uuid::new_v4()),
            first_name: "Partial".into(),
            last_name: "Capture".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PG-PC-{}", Uuid::new_v4()),
                name: "Widget".into(),
                quantity: 1,
                unit_price: total,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
        .id
}

async fn authorized(db: &PostgresDatabase, order_id: OrderId, amount: Decimal) -> Payment {
    db.payments()
        .create_async(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .await
        .expect("create payment")
}

async fn captured_payment(db: &PostgresDatabase, total: Decimal) -> (OrderId, Payment) {
    let order_id = order_totalling(db, total).await;
    let payment = authorized(db, order_id, total).await;
    let done = db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("complete");
    (order_id, done)
}

async fn order_payment_status(db: &PostgresDatabase, order_id: OrderId) -> PaymentStatus {
    db.orders().get_async(order_id.into_uuid()).await.expect("get").expect("order").payment_status
}

async fn set_status(
    db: &PostgresDatabase,
    id: PaymentId,
    status: PaymentTransactionStatus,
) -> stateset_core::Result<Payment> {
    db.payments()
        .update_async(id.into_uuid(), UpdatePayment { status: Some(status), ..Default::default() })
        .await
}

async fn refund(
    db: &PostgresDatabase,
    payment_id: PaymentId,
    amount: Option<Decimal>,
) -> stateset_core::Result<stateset_core::Refund> {
    db.payments()
        .create_refund_async(CreateRefund { payment_id, amount, ..Default::default() })
        .await
}

async fn events(
    db: &PostgresDatabase,
    event_type: &str,
    aggregate_id: &str,
) -> Vec<serde_json::Value> {
    sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT payload FROM kernel_outbox WHERE event_type = $1 AND aggregate_id = $2",
    )
    .bind(event_type)
    .bind(aggregate_id)
    .fetch_all(db.pool())
    .await
    .expect("outbox")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_full_capture_records_the_whole_amount() {
    let db = require_db!();
    let (_, payment) = captured_payment(&db, dec!(80.00)).await;
    assert_eq!(payment.captured_amount, Some(dec!(80.00)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_partial_capture_is_recorded_exactly_and_bounds_refunds() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let payment = authorized(&db, order_id, dec!(100.00)).await;
    let captured = db
        .payments()
        .mark_captured_async(payment.id.into_uuid(), dec!(60.25))
        .await
        .expect("partial capture");
    assert_eq!(captured.status, PaymentTransactionStatus::Completed);
    assert_eq!(captured.amount, dec!(100.00));
    assert_eq!(captured.captured_amount, Some(dec!(60.25)));
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::PartiallyPaid);
    let facts = events(&db, "payments.captured.v1", &payment.id.to_string()).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["captured_amount"], "60.25");

    let err = refund(&db, payment.id, Some(dec!(60.26))).await.expect_err("over the capture");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    let full = refund(&db, payment.id, None).await.expect("refund the capture");
    assert_eq!(full.amount, dec!(60.25));
    db.payments().complete_refund_async(full.id).await.expect("complete");
    let after = db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("p");
    assert_eq!(after.status, PaymentTransactionStatus::Refunded);
    assert_eq!(after.amount_refunded, dec!(60.25));
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_partial_capture_releases_the_uncaptured_slice() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let first = authorized(&db, order_id, dec!(100.00)).await;
    db.payments().mark_captured_async(first.id.into_uuid(), dec!(70.00)).await.expect("capture");
    let top_up = authorized(&db, order_id, dec!(30.00)).await;
    db.payments().mark_completed_async(top_up.id.into_uuid()).await.expect("capture the rest");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_partial_capture_amount_is_validated() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(50.00)).await;
    let payment = authorized(&db, order_id, dec!(50.00)).await;
    let id = payment.id.into_uuid();
    for bad in [dec!(0), dec!(-1), dec!(50.01), dec!(10.001)] {
        assert!(db.payments().mark_captured_async(id, bad).await.is_err(), "{bad}");
    }
    db.payments().mark_captured_async(id, dec!(20.00)).await.expect("capture");
    db.payments().mark_captured_async(id, dec!(20.00)).await.expect("same capture");
    assert!(matches!(
        db.payments().mark_captured_async(id, dec!(25.00)).await,
        Err(CommerceError::Conflict(_))
    ));
    let retried = db.payments().mark_completed_async(id).await.expect("retry");
    assert_eq!(retried.captured_amount, Some(dec!(20.00)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_in_flight_refunds_with_a_processor_id_are_listed_and_settle_once() {
    let db = require_db!();
    let (order_id, payment) = captured_payment(&db, dec!(100.00)).await;
    let tracked = refund(&db, payment.id, Some(dec!(70.00))).await.expect("refund");
    let marker = format!("re_{}", Uuid::new_v4().simple());
    let with_id =
        db.payments().set_refund_external_id_async(tracked.id, &marker).await.expect("set id");
    assert_eq!(with_id.external_id.as_deref(), Some(marker.as_str()));
    db.payments().set_refund_external_id_async(tracked.id, &marker).await.expect("same id");
    assert!(matches!(
        db.payments().set_refund_external_id_async(tracked.id, "re_other").await,
        Err(CommerceError::Conflict(_))
    ));
    let listed = db.payments().list_in_flight_refunds_async(10_000).await.expect("list");
    assert!(listed.iter().any(|r| r.id == tracked.id));

    // Failing releases the reservation, once.
    db.payments().fail_refund_async(tracked.id, "expired card").await.expect("fail");
    db.payments().fail_refund_async(tracked.id, "again").await.expect("idempotent");
    assert_eq!(events(&db, "payments.refund_failed.v1", &tracked.id.to_string()).await.len(), 1);
    let listed = db.payments().list_in_flight_refunds_async(10_000).await.expect("list");
    assert!(!listed.iter().any(|r| r.id == tracked.id));

    let second = refund(&db, payment.id, Some(dec!(100.00))).await.expect("released");
    db.payments().complete_refund_async(second.id).await.expect("complete");
    db.payments().complete_refund_async(second.id).await.expect("idempotent");
    assert_eq!(events(&db, "payments.refund_completed.v1", &second.id.to_string()).await.len(), 1);
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_concurrent_settlements_of_one_refund_fold_it_in_once() {
    let db = Arc::new(require_db!());
    let (_, payment) = captured_payment(&db, dec!(100.00)).await;
    let pending = refund(&db, payment.id, Some(dec!(40.00))).await.expect("refund");
    let mut handles = Vec::new();
    for i in 0..8 {
        let db = Arc::clone(&db);
        handles.push(tokio::spawn(async move {
            if i % 2 == 0 {
                db.payments().complete_refund_async(pending.id).await.map(|r| r.status)
            } else {
                db.payments().fail_refund_async(pending.id, "racing sweep").await.map(|r| r.status)
            }
        }));
    }
    let mut outcomes = Vec::new();
    for handle in handles {
        outcomes.push(handle.await.expect("join"));
    }
    let stored = db.payments().get_refund_async(pending.id).await.expect("get").expect("refund");
    for outcome in outcomes.iter().flatten() {
        assert_eq!(*outcome, stored.status, "{outcomes:?}");
    }
    let after = db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("p");
    match stored.status {
        RefundStatus::Completed => assert_eq!(after.amount_refunded, dec!(40.00)),
        RefundStatus::Failed => assert_eq!(after.amount_refunded, Decimal::ZERO),
        other => panic!("refund left {other}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_disputed_payment_cannot_be_cancelled() {
    let db = require_db!();
    let (order_id, payment) = captured_payment(&db, dec!(100.00)).await;
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    let err = db
        .payments()
        .cancel_async(payment.id.into_uuid())
        .await
        .expect_err("a dispute must be resolved first");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_partial_lost_chargeback_writes_a_refund_row_and_keeps_the_rest() {
    let db = require_db!();
    let (order_id, payment) = captured_payment(&db, dec!(100.00)).await;
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    assert!(
        set_status(&db, payment.id, PaymentTransactionStatus::PartiallyRefunded).await.is_err()
    );

    let lost = db
        .payments()
        .record_lost_chargeback_async(payment.id.into_uuid(), Some(dec!(35.50)))
        .await
        .expect("partial loss");
    assert_eq!(lost.status, PaymentTransactionStatus::PartiallyRefunded);
    assert_eq!(lost.amount_refunded, dec!(35.50));
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::PartiallyRefunded);
    let rows = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RefundStatus::Completed);
    assert_eq!(rows[0].amount, dec!(35.50));
    assert_eq!(rows[0].reason.as_deref(), Some(LOST_CHARGEBACK_REFUND_REASON));
    let facts = events(&db, "payments.chargeback_lost.v1", &payment.id.to_string()).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["partial"], true);

    let err = refund(&db, payment.id, Some(dec!(64.51))).await.expect_err("over the remainder");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    let rest = refund(&db, payment.id, Some(dec!(64.50))).await.expect("the remainder");
    db.payments().complete_refund_async(rest.id).await.expect("complete");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_lost_chargeback_is_bounded_and_requires_a_dispute() {
    let db = require_db!();
    let (_, payment) = captured_payment(&db, dec!(100.00)).await;
    let id = payment.id.into_uuid();
    assert!(matches!(
        db.payments().record_lost_chargeback_async(id, Some(dec!(10))).await,
        Err(CommerceError::ValidationError(_))
    ));
    let pending = refund(&db, payment.id, Some(dec!(30.00))).await.expect("refund");
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    for bad in [dec!(0), dec!(70.01), dec!(100.00)] {
        assert!(db.payments().record_lost_chargeback_async(id, Some(bad)).await.is_err(), "{bad}");
    }
    assert!(db.payments().complete_refund_async(pending.id).await.is_err());
    let lost = db.payments().record_lost_chargeback_async(id, None).await;
    // The whole remainder minus the in-flight reservation is not "the whole
    // remainder", so None (= remaining 100) is refused while 30 is reserved.
    assert!(lost.is_err());
    let partial =
        db.payments().record_lost_chargeback_async(id, Some(dec!(70.00))).await.expect("70");
    assert_eq!(partial.status, PaymentTransactionStatus::PartiallyRefunded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pg_full_lost_chargeback_via_status_write_uses_the_captured_amount() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let payment = authorized(&db, order_id, dec!(100.00)).await;
    db.payments().mark_captured_async(payment.id.into_uuid(), dec!(45.00)).await.expect("45");
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    let lost = set_status(&db, payment.id, PaymentTransactionStatus::Refunded).await.expect("lost");
    assert_eq!(lost.amount_refunded, dec!(45.00));
}
