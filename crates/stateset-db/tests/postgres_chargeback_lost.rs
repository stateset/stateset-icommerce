#![cfg(feature = "postgres")]
//! PostgreSQL twin of `sqlite_chargeback_lost.rs`: a lost chargeback
//! (`Disputed -> Refunded`) is recorded on the refund ledger and moves the
//! order out of `paid`; a won one (`Disputed -> Completed`) changes nothing,
//! and the refund guards still hold afterwards.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    LOST_CHARGEBACK_REFUND_REASON, OrderId, Payment, PaymentId, PaymentMethodType, PaymentStatus,
    PaymentTransactionStatus, ProductId, RefundStatus, UpdatePayment,
};
use stateset_db::PostgresDatabase;
use std::env;
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
            email: format!("pg-chargeback-{}@example.com", Uuid::new_v4()),
            first_name: "Charge".into(),
            last_name: "Back".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PG-CB-{}", Uuid::new_v4()),
                name: "Disputed widget".into(),
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

async fn captured(db: &PostgresDatabase, order_id: OrderId, amount: Decimal) -> Payment {
    let payment = db
        .payments()
        .create_async(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("complete payment")
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

async fn order_payment_status(db: &PostgresDatabase, order_id: OrderId) -> PaymentStatus {
    db.orders().get_async(order_id.into_uuid()).await.expect("get").expect("order").payment_status
}

async fn refund(db: &PostgresDatabase, payment_id: PaymentId, amount: Decimal) {
    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id,
            amount: Some(amount),
            ..Default::default()
        })
        .await
        .expect("create refund");
    db.payments().complete_refund_async(refund.id).await.expect("complete refund");
}

async fn chargeback_events(db: &PostgresDatabase, payment_id: PaymentId) -> Vec<serde_json::Value> {
    sqlx::query_scalar(
        "SELECT payload FROM kernel_outbox
         WHERE event_type = 'payments.chargeback_lost.v1' AND aggregate_id = $1",
    )
    .bind(payment_id.to_string())
    .fetch_all(db.pool())
    .await
    .expect("chargeback events")
}

#[tokio::test]
async fn postgres_lost_chargeback_on_the_whole_order_marks_it_refunded() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let payment = captured(&db, order_id, dec!(100.00)).await;
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);

    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);

    let lost = set_status(&db, payment.id, PaymentTransactionStatus::Refunded)
        .await
        .expect("a lost chargeback is recorded, not refused");
    assert_eq!(lost.status, PaymentTransactionStatus::Refunded);
    assert_eq!(lost.amount_refunded, dec!(100.00));
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);

    let rows = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RefundStatus::Completed);
    assert_eq!(rows[0].amount, dec!(100.00));
    assert_eq!(rows[0].reason.as_deref(), Some(LOST_CHARGEBACK_REFUND_REASON));
    assert!(rows[0].refunded_at.is_some());
    let events = chargeback_events(&db, payment.id).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["amount"], "100.00");

    let err = db
        .payments()
        .create_refund_async(CreateRefund { payment_id: payment.id, ..Default::default() })
        .await
        .expect_err("a charged-back payment cannot be refunded again");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert!(
        db.payments()
            .open_captures_for_order_async(order_id.into_uuid())
            .await
            .expect("open")
            .is_empty()
    );
}

#[tokio::test]
async fn postgres_lost_chargeback_on_part_of_the_order_marks_it_partially_refunded() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let kept = captured(&db, order_id, dec!(60.00)).await;
    let disputed = captured(&db, order_id, dec!(40.00)).await;
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);

    set_status(&db, disputed.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    set_status(&db, disputed.id, PaymentTransactionStatus::Refunded).await.expect("lost");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::PartiallyRefunded);

    let err = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: kept.id,
            amount: Some(dec!(60.01)),
            ..Default::default()
        })
        .await
        .expect_err("cannot refund more than remains");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    assert!(
        db.payments()
            .create_refund_async(CreateRefund {
                payment_id: disputed.id,
                amount: Some(dec!(0.01)),
                ..Default::default()
            })
            .await
            .is_err()
    );
    refund(&db, kept.id, dec!(60.00)).await;
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test]
async fn postgres_lost_chargeback_after_a_partial_refund_reverses_only_the_remainder() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let payment = captured(&db, order_id, dec!(100.00)).await;
    refund(&db, payment.id, dec!(30.00)).await;
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::PartiallyRefunded);

    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");
    let lost = set_status(&db, payment.id, PaymentTransactionStatus::Refunded)
        .await
        .expect("chargeback lost");
    assert_eq!(lost.amount_refunded, dec!(100.00));
    let reversal = db
        .payments()
        .get_refunds_async(payment.id.into_uuid())
        .await
        .expect("refunds")
        .into_iter()
        .find(|r| r.reason.as_deref() == Some(LOST_CHARGEBACK_REFUND_REASON))
        .expect("chargeback row");
    assert_eq!(reversal.amount, dec!(70.00), "only the unrefunded remainder was reversed");
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test]
async fn postgres_won_chargeback_leaves_the_order_paid() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00)).await;
    let payment = captured(&db, order_id, dec!(100.00)).await;
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");

    let won = set_status(&db, payment.id, PaymentTransactionStatus::Completed).await.expect("won");
    assert_eq!(won.status, PaymentTransactionStatus::Completed);
    assert_eq!(won.amount_refunded, Decimal::ZERO);
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Paid);
    assert!(
        db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds").is_empty()
    );
    assert!(chargeback_events(&db, payment.id).await.is_empty());
    refund(&db, payment.id, dec!(100.00)).await;
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
}

#[tokio::test]
async fn postgres_lost_chargeback_through_the_atomic_batch_is_recorded_the_same_way() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(50.00)).await;
    let payment = captured(&db, order_id, dec!(50.00)).await;
    set_status(&db, payment.id, PaymentTransactionStatus::Disputed).await.expect("dispute");

    db.payments()
        .update_batch_atomic_async(vec![(
            payment.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                ..Default::default()
            },
        )])
        .await
        .expect("batch chargeback lost");
    let after = db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("p");
    assert_eq!(after.amount_refunded, dec!(50.00));
    assert_eq!(order_payment_status(&db, order_id).await, PaymentStatus::Refunded);
    assert_eq!(
        db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds").len(),
        1
    );
}
