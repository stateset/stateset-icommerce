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

async fn force_cancel_order(db: &PostgresDatabase, order_id: Uuid) -> stateset_core::Order {
    let order = db
        .orders()
        .update_async(
            order_id,
            UpdateOrder {
                status: Some(OrderStatus::Cancelled),
                void_payments: true,
                ..Default::default()
            },
        )
        .await
        .expect("forced cancel");
    assert_eq!(order.status, OrderStatus::Cancelled);
    order
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
    // A plain cancel is refused while the pending capture is outstanding; a
    // forced cancel voids it in the same transaction, so a live capture can
    // never complete against a cancelled order.
    let err = db
        .orders()
        .update_async(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .await
        .expect_err("plain cancel is refused while a capture is in flight");
    assert_validation_mentioning(&err, "100.00 USD");
    force_cancel_order(&db, order_id).await;
    assert_eq!(status(&db, pending_id).await, PaymentTransactionStatus::Cancelled, "voided");

    let err = db
        .payments()
        .mark_completed_async(pending_id)
        .await
        .expect_err("a voided capture cannot complete");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(status(&db, pending_id).await, PaymentTransactionStatus::Cancelled);
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

// ============================================================================
// Round 4 mirrors — PG batch-create against a cancelled order, defaulted
// currency, cancel money rule, idempotency fingerprint, refund-by-flip.
// ============================================================================

#[tokio::test]
async fn postgres_atomic_batch_create_against_a_cancelled_order_is_refused() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    cancel_order(&db, order_id).await;

    let err = db
        .payments()
        .create_batch_atomic_async(vec![payment_input(Some(order_id), dec!(50.00))])
        .await
        .expect_err("atomic batch create shares the order guards");
    assert_validation_mentioning(&err, "cancelled");
    assert!(payments_for(&db, order_id).await.is_empty());
}

#[tokio::test]
async fn postgres_defaulted_payment_currency_is_checked_against_a_non_usd_order() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::EUR).await;

    // `currency: None` defaults to USD, which does not match a EUR order.
    let err = db
        .payments()
        .create_async(payment_input(Some(order_id), dec!(100.00)))
        .await
        .expect_err("defaulted USD does not match a EUR order");
    assert_validation_mentioning(&err, "EUR");
    assert!(payments_for(&db, order_id).await.is_empty());

    let ok = db
        .payments()
        .create_async(CreatePayment {
            currency: Some(CurrencyCode::EUR),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .await
        .expect("EUR on a EUR order");
    assert_eq!(ok.currency, CurrencyCode::EUR);
}

#[tokio::test]
async fn postgres_cancelling_an_order_with_open_captures_is_refused_without_void_payments() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let captured = completed_payment(&db, Some(order_id), dec!(60.00)).await;
    let captured_id = captured.id.into_uuid();

    let err = db
        .orders()
        .update_async(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .await
        .expect_err("captured money must not be orphaned by a cancel");
    assert_validation_mentioning(&err, "60.00 USD");
    assert_validation_mentioning(&err, "void_payments");
    let order = db.orders().get_async(order_id).await.expect("get").expect("exists");
    assert_eq!(order.status, OrderStatus::Pending, "cancel rolled back");
    assert_eq!(status(&db, captured_id).await, PaymentTransactionStatus::Completed);

    // Once the money is returned the plain cancel goes through.
    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: captured.id,
            amount: None,
            ..Default::default()
        })
        .await
        .expect("refund");
    db.payments().complete_refund_async(refund.id).await.expect("complete refund");
    cancel_order(&db, order_id).await;
}

#[tokio::test]
async fn postgres_forced_cancel_voids_in_flight_payments_and_leaves_settled_ones_for_refund() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let settled = completed_payment(&db, Some(order_id), dec!(60.00)).await;
    let in_flight = payment(&db, Some(order_id), dec!(30.00)).await;
    let processing = payment(&db, Some(order_id), dec!(10.00)).await;
    db.payments().mark_processing_async(processing.id.into_uuid()).await.expect("processing");

    force_cancel_order(&db, order_id).await;

    assert_eq!(status(&db, in_flight.id.into_uuid()).await, PaymentTransactionStatus::Cancelled);
    assert_eq!(status(&db, processing.id.into_uuid()).await, PaymentTransactionStatus::Cancelled);
    assert_eq!(status(&db, settled.id.into_uuid()).await, PaymentTransactionStatus::Completed);
    assert_eq!(open_ids(&db, order_id).await, vec![settled.id.into_uuid()]);

    let event = db
        .kernel_outbox()
        .pending_async(500)
        .await
        .expect("pending")
        .into_iter()
        .find(|e| {
            e.event_type == "orders.updated.v1"
                && e.aggregate_id == order_id.to_string()
                && e.payload["status_after"] == "cancelled"
        })
        .expect("orders.updated.v1 for the cancel");
    assert_eq!(event.payload["void_payments"], true);
    let voided: Vec<String> = event.payload["voided_payment_ids"]
        .as_array()
        .expect("array")
        .iter()
        .map(|v| v.as_str().expect("uuid").to_string())
        .collect();
    assert_eq!(voided.len(), 2);
    assert!(voided.contains(&in_flight.id.to_string()));
    assert!(voided.contains(&processing.id.to_string()));
    assert_eq!(
        event.payload["outstanding_payment_ids"],
        serde_json::json!([settled.id.to_string()])
    );
    let outstanding: Decimal =
        event.payload["outstanding_captured"].as_str().expect("string money").parse().unwrap();
    assert_eq!(outstanding, dec!(60));

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: settled.id,
            amount: None,
            ..Default::default()
        })
        .await
        .expect("refund after cancel");
    db.payments().complete_refund_async(refund.id).await.expect("complete refund");
    assert!(open_ids(&db, order_id).await.is_empty());
}

#[tokio::test]
async fn postgres_update_batch_atomic_cancel_shares_the_money_rule() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    completed_payment(&db, Some(order_id), dec!(100.00)).await;
    let err = db
        .orders()
        .update_batch_atomic_async(vec![(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )])
        .await
        .expect_err("batch cancel is refused like a single cancel");
    assert_validation_mentioning(&err, "100.00 USD");
    let order = db.orders().get_async(order_id).await.expect("get").expect("exists");
    assert_eq!(order.status, OrderStatus::Pending);
}

#[tokio::test]
async fn postgres_duplicate_idempotency_key_with_different_parameters_is_a_conflict() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let key = format!("idem-{}", Uuid::new_v4());
    let first = db
        .payments()
        .create_async(CreatePayment {
            idempotency_key: Some(key.clone()),
            ..payment_input(Some(order_id), dec!(25.00))
        })
        .await
        .expect("first");
    let replay = db
        .payments()
        .create_async(CreatePayment {
            idempotency_key: Some(key.clone()),
            ..payment_input(Some(order_id), dec!(25.00))
        })
        .await
        .expect("identical replay");
    assert_eq!(replay.id, first.id);

    let other_order = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    for (label, input) in [
        ("amount", payment_input(Some(order_id), dec!(26.00))),
        ("order", payment_input(Some(other_order), dec!(25.00))),
        ("no order", payment_input(None, dec!(25.00))),
        (
            "currency",
            CreatePayment {
                currency: Some(CurrencyCode::EUR),
                ..payment_input(Some(order_id), dec!(25.00))
            },
        ),
        (
            "method",
            CreatePayment {
                payment_method: PaymentMethodType::BankTransfer,
                ..payment_input(Some(order_id), dec!(25.00))
            },
        ),
    ] {
        let err = db
            .payments()
            .create_async(CreatePayment { idempotency_key: Some(key.clone()), ..input })
            .await
            .expect_err("a different request under a used key must conflict");
        assert!(
            matches!(err, CommerceError::Conflict(ref m) if m.contains(&key) && m.contains(&first.id.to_string())),
            "{label}: {err:?}"
        );
    }
    let stored = db.payments().get_async(first.id.into_uuid()).await.expect("get").expect("exists");
    assert_eq!(stored.amount, dec!(25.00));
    assert_eq!(payments_for(&db, order_id).await.len(), 1);
}

#[tokio::test]
async fn postgres_concurrent_duplicate_idempotency_key_with_different_amounts_conflicts() {
    let db = require_db!();
    let key = format!("idem-{}", Uuid::new_v4());
    let input = |amount: Decimal| CreatePayment {
        idempotency_key: Some(key.clone()),
        ..payment_input(None, amount)
    };

    // Both futures pass the pre-transaction lookup before either INSERT lands;
    // the loser trips the UNIQUE index, reads the winner's row and must see
    // that its own request differs.
    let payments = db.payments();
    let (a, b) = tokio::join!(
        payments.create_async(input(dec!(25.00))),
        payments.create_async(input(dec!(26.00)))
    );
    let results = [a, b];
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let conflicts = results
        .iter()
        .filter(|r| matches!(r, Err(CommerceError::Conflict(m)) if m.contains(&key)))
        .count();
    assert_eq!(ok, 1, "exactly one caller wins: {results:?}");
    assert_eq!(conflicts, 1, "the other is told the key was used differently: {results:?}");
    let by_key = db
        .payments()
        .list_async(PaymentFilter::default())
        .await
        .expect("list")
        .into_iter()
        .filter(|p| p.idempotency_key.as_deref() == Some(key.as_str()))
        .count();
    assert_eq!(by_key, 1);
}

#[tokio::test]
async fn postgres_refunded_cannot_be_reached_through_update() {
    let db = require_db!();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD).await;
    let p = completed_payment(&db, Some(order_id), dec!(100.00)).await;
    let pid = p.id.into_uuid();
    set_status(&db, pid, PaymentTransactionStatus::Disputed).await.expect("dispute");

    let err = set_status(&db, pid, PaymentTransactionStatus::Refunded)
        .await
        .expect_err("refund by status flip is refused");
    assert_validation_mentioning(&err, "complete_refund");
    assert_eq!(status(&db, pid).await, PaymentTransactionStatus::Disputed);
    assert_eq!(open_ids(&db, order_id).await, vec![pid]);

    set_status(&db, pid, PaymentTransactionStatus::Completed).await.expect("dispute won");
    for target in [PaymentTransactionStatus::Refunded, PaymentTransactionStatus::PartiallyRefunded]
    {
        let err = set_status(&db, pid, target).await.expect_err("refused");
        assert_validation_mentioning(&err, "complete_refund");
    }
    assert_eq!(status(&db, pid).await, PaymentTransactionStatus::Completed);

    let refund = db
        .payments()
        .create_refund_async(CreateRefund { payment_id: p.id, amount: None, ..Default::default() })
        .await
        .expect("refund");
    db.payments().complete_refund_async(refund.id).await.expect("complete");
    let after = db.payments().get_async(pid).await.unwrap().unwrap();
    assert_eq!(after.status, PaymentTransactionStatus::Refunded);
    assert_eq!(after.amount_refunded, dec!(100.00));
    assert!(open_ids(&db, order_id).await.is_empty());

    // Batch path shares the rule.
    let p2 = completed_payment(&db, None, dec!(10.00)).await;
    let err = db
        .payments()
        .update_batch_atomic_async(vec![(
            p2.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                ..Default::default()
            },
        )])
        .await
        .expect_err("batch refused");
    assert_validation_mentioning(&err, "complete_refund");
    assert_eq!(status(&db, p2.id.into_uuid()).await, PaymentTransactionStatus::Completed);
}
