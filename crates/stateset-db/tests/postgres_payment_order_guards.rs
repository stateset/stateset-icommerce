#![cfg(feature = "postgres")]
//! Postgres mirrors of `sqlite_payment_order_guards.rs`: the order-side payment
//! guards (D1 disputed captures keep their slice of the order total, D2 no
//! captures against a cancelled order, D5 payment currency must match the
//! order currency), `open_captures_for_order`, idempotent handling of a racing
//! duplicate idempotency key, and the batch-update transition guard.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` /
//! `DATABASE_URL`) and are skipped otherwise, so they run only in CI with a
//! provisioned database (the Postgres Parity job).

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    CurrencyCode, CustomerId, OrderStatus, Payment, PaymentFilter, PaymentMethodType,
    PaymentTransactionStatus, ProductId, UpdateOrder, UpdatePayment,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
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

async fn customer(db: &PostgresDatabase) -> CustomerId {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("order-guards-{}@example.com", Uuid::new_v4()),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
}

/// A single-unit order whose `total_amount` is exactly `unit_price`, in
/// `currency`, left in `Pending`.
async fn order_totalling(
    db: &PostgresDatabase,
    unit_price: Decimal,
    currency: CurrencyCode,
) -> Uuid {
    let sku = format!("GUARD-{}", Uuid::new_v4());
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer(db).await,
            currency: Some(currency),
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: sku,
                quantity: 1,
                unit_price,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
        .id
        .into_uuid()
}

async fn cancel_order(db: &PostgresDatabase, order_id: Uuid) {
    let order = db
        .orders()
        .update_async(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .await
        .expect("cancel order");
    assert_eq!(order.status, OrderStatus::Cancelled);
}

fn payment_input(order_id: Option<Uuid>, amount: Decimal) -> CreatePayment {
    CreatePayment {
        order_id: order_id.map(Into::into),
        payment_method: PaymentMethodType::CreditCard,
        amount,
        ..Default::default()
    }
}

async fn payment(db: &PostgresDatabase, order_id: Option<Uuid>, amount: Decimal) -> Payment {
    db.payments().create_async(payment_input(order_id, amount)).await.expect("create payment")
}

async fn completed_payment(
    db: &PostgresDatabase,
    order_id: Option<Uuid>,
    amount: Decimal,
) -> Payment {
    let p = payment(db, order_id, amount).await;
    db.payments().mark_completed_async(p.id.into_uuid()).await.expect("mark completed")
}

async fn set_status(
    db: &PostgresDatabase,
    id: Uuid,
    status: PaymentTransactionStatus,
) -> stateset_core::Result<Payment> {
    db.payments()
        .update_async(id, UpdatePayment { status: Some(status), ..Default::default() })
        .await
}

async fn status(db: &PostgresDatabase, id: Uuid) -> PaymentTransactionStatus {
    db.payments().get_async(id).await.expect("get payment").expect("payment exists").status
}

async fn payments_for(db: &PostgresDatabase, order_id: Uuid) -> Vec<Payment> {
    db.payments().for_order_async(order_id).await.expect("list payments for order")
}

async fn open_ids(db: &PostgresDatabase, order_id: Uuid) -> Vec<Uuid> {
    db.payments()
        .open_captures_for_order_async(order_id)
        .await
        .expect("open captures")
        .into_iter()
        .map(|p| p.id.into_uuid())
        .collect()
}

#[track_caller]
fn assert_over_capture(err: &CommerceError) {
    assert!(matches!(err, CommerceError::CaptureExceedsOrderTotal { .. }), "got {err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"));
}

#[track_caller]
fn assert_validation_mentioning(err: &CommerceError, needle: &str) {
    match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(needle), "expected {needle:?} in {msg:?}");
        }
        other => panic!("expected ValidationError mentioning {needle:?}, got {other:?}"),
    }
}

// ============================================================================
// D1 — a disputed payment keeps its slice of the order total
// ============================================================================

#[tokio::test]
async fn postgres_disputed_payment_keeps_its_slice_of_the_order_total() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let first = completed_payment(&db, Some(order_id), dec!(100.00)).await;
    let first_id = first.id.into_uuid();

    let disputed = set_status(&db, first_id, PaymentTransactionStatus::Disputed)
        .await
        .expect("completed -> disputed is a legal edge");
    assert_eq!(disputed.status, PaymentTransactionStatus::Disputed);

    let err = db
        .payments()
        .create_async(payment_input(Some(order_id), dec!(100.00)))
        .await
        .expect_err("a disputed capture still consumes the order total");
    assert_over_capture(&err);

    let resolved = set_status(&db, first_id, PaymentTransactionStatus::Completed)
        .await
        .expect("disputed -> completed via update");
    assert_eq!(resolved.status, PaymentTransactionStatus::Completed);

    let payments = payments_for(&db, order_id).await;
    assert_eq!(payments.len(), 1, "the refused capture must not have written a row");
    assert_eq!(open_ids(&db, order_id).await, vec![first_id]);
}

// ============================================================================
// D2 — captures against a cancelled order are refused
// ============================================================================

#[tokio::test]
async fn postgres_creating_a_payment_against_a_cancelled_order_is_refused() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    cancel_order(&db, order_id).await;

    let err = db
        .payments()
        .create_async(payment_input(Some(order_id), dec!(100.00)))
        .await
        .expect_err("a cancelled order has no money owed on it");
    assert_validation_mentioning(&err, "cancelled");
    assert!(payments_for(&db, order_id).await.is_empty());

    let err = db
        .payments()
        .create_batch_atomic_async(vec![payment_input(Some(order_id), dec!(50.00))])
        .await
        .expect_err("atomic batch create shares the order guards");
    assert_validation_mentioning(&err, "cancelled");
    assert!(payments_for(&db, order_id).await.is_empty());
}

#[tokio::test]
async fn postgres_completing_a_payment_after_its_order_was_cancelled_is_refused() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let pending = payment(&db, Some(order_id), dec!(100.00)).await;
    let pending_id = pending.id.into_uuid();
    // NOTE: once the orders module consults `open_captures_for_order` before
    // cancelling, this cancel will itself be refused (the pending capture is
    // outstanding) and this test should fail the payment first instead.
    cancel_order(&db, order_id).await;

    let err = db
        .payments()
        .mark_completed_async(pending_id)
        .await
        .expect_err("completing a capture against a cancelled order orphans the money");
    assert_validation_mentioning(&err, "cancelled");
    assert_eq!(status(&db, pending_id).await, PaymentTransactionStatus::Pending);
}

// ============================================================================
// D5 — the payment currency must match the order currency
// ============================================================================

#[tokio::test]
async fn postgres_payment_currency_must_match_order_currency() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;

    let err = db
        .payments()
        .create_async(CreatePayment {
            currency: Some(CurrencyCode::JPY),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .await
        .expect_err("JPY 100 is not USD 100");
    assert_validation_mentioning(&err, "JPY");
    assert_validation_mentioning(&err, "USD");
    assert!(payments_for(&db, order_id).await.is_empty());

    let ok = db
        .payments()
        .create_async(CreatePayment {
            currency: Some(CurrencyCode::USD),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .await
        .expect("USD on a USD order");
    assert_eq!(ok.currency, CurrencyCode::USD);

    // `currency: None` defaults to USD, which does not match a EUR order.
    let eur_order = order_totalling(&db, dec!(100.00), CurrencyCode::EUR).await;
    let err = db
        .payments()
        .create_async(payment_input(Some(eur_order), dec!(100.00)))
        .await
        .expect_err("defaulted USD does not match a EUR order");
    assert_validation_mentioning(&err, "EUR");
}

// ============================================================================
// open_captures_for_order
// ============================================================================

#[tokio::test]
async fn postgres_open_captures_for_order_lists_only_outstanding_money() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    assert!(open_ids(&db, order_id).await.is_empty());

    let first = completed_payment(&db, Some(order_id), dec!(60.00)).await;
    assert_eq!(open_ids(&db, order_id).await, vec![first.id.into_uuid()]);

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: first.id,
            amount: None,
            ..Default::default()
        })
        .await
        .expect("create full refund");
    db.payments().complete_refund_async(refund.id).await.expect("complete refund");
    assert!(open_ids(&db, order_id).await.is_empty());

    let second = payment(&db, Some(order_id), dec!(40.00)).await;
    assert_eq!(open_ids(&db, order_id).await, vec![second.id.into_uuid()]);
    db.payments().cancel_async(second.id.into_uuid()).await.expect("cancel pending");
    assert!(open_ids(&db, order_id).await.is_empty());

    let third = completed_payment(&db, Some(order_id), dec!(40.00)).await;
    let partial = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: third.id,
            amount: Some(dec!(15.00)),
            ..Default::default()
        })
        .await
        .expect("partial refund");
    db.payments().complete_refund_async(partial.id).await.expect("complete partial");
    let open = db.payments().open_captures_for_order_async(order_id).await.expect("open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, third.id);
    assert_eq!(open[0].amount - open[0].amount_refunded, dec!(25.00));
}

// ============================================================================
// Idempotency — a racing duplicate key returns the existing payment
// ============================================================================

#[tokio::test]
async fn postgres_concurrent_duplicate_idempotency_key_returns_the_existing_payment() {
    let db = require_db!();
    let key = format!("idem-{}", Uuid::new_v4());
    let input =
        || CreatePayment { idempotency_key: Some(key.clone()), ..payment_input(None, dec!(25.00)) };

    // Both futures pass the pre-transaction lookup before either INSERT lands;
    // the loser trips the UNIQUE index and must resolve to the winner's row.
    let payments = db.payments();
    let (a, b) = tokio::join!(payments.create_async(input()), payments.create_async(input()));
    let a = a.expect("a duplicate idempotency key is idempotent, never a conflict");
    let b = b.expect("a duplicate idempotency key is idempotent, never a conflict");
    assert_eq!(a.id, b.id, "both callers must observe the same payment");

    let by_key = db
        .payments()
        .list_async(PaymentFilter::default())
        .await
        .expect("list")
        .into_iter()
        .filter(|p| p.idempotency_key.as_deref() == Some(key.as_str()))
        .count();
    assert_eq!(by_key, 1, "exactly one payment row for one idempotency key");
}

// ============================================================================
// Batch updates share the transition guard
// ============================================================================

#[tokio::test]
async fn postgres_update_batch_records_an_illegal_transition_as_a_failure() {
    let db = require_db!();
    let settled = completed_payment(&db, None, dec!(10.00)).await;
    let pending = payment(&db, None, dec!(10.00)).await;

    let cancel = || UpdatePayment {
        status: Some(PaymentTransactionStatus::Cancelled),
        ..Default::default()
    };
    let result = db
        .payments()
        .update_batch_async(vec![(settled.id, cancel()), (pending.id, cancel())])
        .await
        .expect("partial-success batch never errors as a whole");

    assert_eq!(result.failure_count, 1, "{result:?}");
    assert_eq!(result.success_count, 1, "{result:?}");
    assert_eq!(result.failed[0].id.as_deref(), Some(settled.id.to_string().as_str()));
    assert_eq!(status(&db, settled.id.into_uuid()).await, PaymentTransactionStatus::Completed);
    assert_eq!(status(&db, pending.id.into_uuid()).await, PaymentTransactionStatus::Cancelled);
}

#[tokio::test]
async fn postgres_update_batch_atomic_aborts_the_whole_batch_on_an_illegal_transition() {
    let db = require_db!();
    let pending = payment(&db, None, dec!(10.00)).await;
    let settled = completed_payment(&db, None, dec!(10.00)).await;

    let cancel = || UpdatePayment {
        status: Some(PaymentTransactionStatus::Cancelled),
        ..Default::default()
    };
    let err = db
        .payments()
        .update_batch_atomic_async(vec![(pending.id, cancel()), (settled.id, cancel())])
        .await
        .expect_err("one illegal write aborts the atomic batch");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    assert_eq!(status(&db, pending.id.into_uuid()).await, PaymentTransactionStatus::Pending);
    assert_eq!(status(&db, settled.id.into_uuid()).await, PaymentTransactionStatus::Completed);
}
