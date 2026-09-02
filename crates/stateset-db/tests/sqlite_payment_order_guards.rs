#![cfg(feature = "sqlite")]
//! Regression tests for the order-side payment guards (SQLite backend).
//!
//! Verified defects these cover (payments re-audit, Sep 2026):
//!
//! - **D1** — `Disputed` was not a capturing status, so disputing a completed
//!   payment silently released its slice of the order total: a second
//!   full-amount capture passed the over-capture guard, and resolving the
//!   dispute back to `Completed` through `update` (which never re-checked
//!   capacity) left the order captured twice.
//! - **D2** — the capture guard read only `orders.total_amount`, so a new
//!   payment could be created and completed against a `Cancelled` order.
//! - **D5** — the payment's currency was never compared with the order's, and
//!   the capacity sum added across currencies (JPY 100 on a USD 100 order).
//! - A concurrent duplicate idempotency key surfaced as a raw UNIQUE conflict
//!   instead of returning the existing payment.
//! - Batch updates had no test proving the transition guard applies to them.
//!
//! Every scenario here has a Postgres mirror in
//! `postgres_payment_order_guards.rs`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CreateRefund, CurrencyCode, CustomerId, CustomerRepository, InventoryRepository,
    OrderId, OrderRepository, OrderStatus, Payment, PaymentMethodType, PaymentRepository,
    PaymentTransactionStatus, ProductId, UpdateOrder, UpdatePayment,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

fn customer(db: &SqliteDatabase) -> CustomerId {
    db.customers()
        .create(CreateCustomer {
            email: format!("order-guards-{}@example.com", Uuid::new_v4()),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

/// A single-unit order whose `total_amount` is exactly `unit_price`, in
/// `currency`, left in `Pending`.
fn order_totalling(db: &SqliteDatabase, unit_price: Decimal, currency: CurrencyCode) -> OrderId {
    let sku = format!("GUARD-{}", Uuid::new_v4());
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("create inventory item");
    db.orders()
        .create(CreateOrder {
            customer_id: customer(db),
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
        .expect("create order")
        .id
}

fn cancel_order(db: &SqliteDatabase, order_id: OrderId) {
    let order = db
        .orders()
        .update(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .expect("cancel order");
    assert_eq!(order.status, OrderStatus::Cancelled);
}

fn payment_input(order_id: Option<OrderId>, amount: Decimal) -> CreatePayment {
    CreatePayment {
        order_id,
        payment_method: PaymentMethodType::CreditCard,
        amount,
        ..Default::default()
    }
}

fn payment(db: &SqliteDatabase, order_id: Option<OrderId>, amount: Decimal) -> Payment {
    db.payments().create(payment_input(order_id, amount)).expect("create payment")
}

fn completed_payment(db: &SqliteDatabase, order_id: Option<OrderId>, amount: Decimal) -> Payment {
    let p = payment(db, order_id, amount);
    db.payments().mark_completed(p.id).expect("mark completed")
}

fn set_status(
    db: &SqliteDatabase,
    id: stateset_core::PaymentId,
    status: PaymentTransactionStatus,
) -> stateset_core::Result<Payment> {
    db.payments().update(id, UpdatePayment { status: Some(status), ..Default::default() })
}

fn status(db: &SqliteDatabase, id: stateset_core::PaymentId) -> PaymentTransactionStatus {
    db.payments().get(id).expect("get payment").expect("payment exists").status
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

#[test]
fn disputed_payment_keeps_its_slice_of_the_order_total() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let first = completed_payment(&db, Some(order_id), dec!(100.00));

    let disputed = set_status(&db, first.id, PaymentTransactionStatus::Disputed)
        .expect("completed -> disputed is a legal edge");
    assert_eq!(disputed.status, PaymentTransactionStatus::Disputed);

    // The chargeback is contested money, not a settled loss: the order is
    // still fully captured, so a second capture must be refused.
    let err = db
        .payments()
        .create(payment_input(Some(order_id), dec!(100.00)))
        .expect_err("a disputed capture still consumes the order total");
    assert_over_capture(&err);

    // Resolving the dispute in the merchant's favour goes through `update`
    // (not only `mark_completed`) and must succeed without double-counting.
    let resolved = set_status(&db, first.id, PaymentTransactionStatus::Completed)
        .expect("disputed -> completed via update");
    assert_eq!(resolved.status, PaymentTransactionStatus::Completed);

    let payments = db.payments().for_order(order_id).expect("list");
    assert_eq!(payments.len(), 1, "the refused capture must not have written a row");
    assert_eq!(payments[0].amount, dec!(100.00));
}

#[test]
fn disputed_payment_is_an_open_capture() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let first = completed_payment(&db, Some(order_id), dec!(100.00));
    set_status(&db, first.id, PaymentTransactionStatus::Disputed).expect("dispute");

    let open = db.payments().open_captures_for_order(order_id).expect("open captures");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, first.id);
}

// ============================================================================
// D2 — captures against a cancelled order are refused
// ============================================================================

#[test]
fn creating_a_payment_against_a_cancelled_order_is_refused() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    cancel_order(&db, order_id);

    let err = db
        .payments()
        .create(payment_input(Some(order_id), dec!(100.00)))
        .expect_err("a cancelled order has no money owed on it");
    assert_validation_mentioning(&err, "cancelled");
    assert!(db.payments().for_order(order_id).expect("list").is_empty());
}

#[test]
fn completing_a_payment_after_its_order_was_cancelled_is_refused() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let pending = payment(&db, Some(order_id), dec!(100.00));
    // NOTE: once the orders module consults `open_captures_for_order` before
    // cancelling, this cancel will itself be refused (the pending capture is
    // outstanding) and this test should fail the payment first instead.
    cancel_order(&db, order_id);

    let err = db
        .payments()
        .mark_completed(pending.id)
        .expect_err("completing a capture against a cancelled order orphans the money");
    assert_validation_mentioning(&err, "cancelled");
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Pending);
}

#[test]
fn atomic_batch_create_against_a_cancelled_order_is_refused() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    cancel_order(&db, order_id);

    let err = db
        .payments()
        .create_batch_atomic(vec![payment_input(Some(order_id), dec!(50.00))])
        .expect_err("atomic batch create shares the order guards");
    assert_validation_mentioning(&err, "cancelled");
    assert!(db.payments().for_order(order_id).expect("list").is_empty());
}

// ============================================================================
// D5 — the payment currency must match the order currency
// ============================================================================

#[test]
fn payment_currency_must_match_order_currency() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);

    let err = db
        .payments()
        .create(CreatePayment {
            currency: Some(CurrencyCode::JPY),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .expect_err("JPY 100 is not USD 100");
    assert_validation_mentioning(&err, "JPY");
    assert_validation_mentioning(&err, "USD");
    assert!(db.payments().for_order(order_id).expect("list").is_empty());

    // The matching currency (explicit or defaulted) is accepted.
    let ok = db
        .payments()
        .create(CreatePayment {
            currency: Some(CurrencyCode::USD),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .expect("USD on a USD order");
    assert_eq!(ok.currency, CurrencyCode::USD);
}

#[test]
fn defaulted_payment_currency_is_checked_against_a_non_usd_order() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::EUR);

    // `currency: None` defaults to USD, which does not match a EUR order.
    let err = db
        .payments()
        .create(payment_input(Some(order_id), dec!(100.00)))
        .expect_err("defaulted USD does not match a EUR order");
    assert_validation_mentioning(&err, "EUR");

    let ok = db
        .payments()
        .create(CreatePayment {
            currency: Some(CurrencyCode::EUR),
            ..payment_input(Some(order_id), dec!(100.00))
        })
        .expect("EUR on a EUR order");
    assert_eq!(ok.currency, CurrencyCode::EUR);
}

// ============================================================================
// open_captures_for_order
// ============================================================================

#[test]
fn open_captures_for_order_lists_only_outstanding_money() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    assert!(db.payments().open_captures_for_order(order_id).expect("empty").is_empty());

    // A completed capture is outstanding...
    let first = completed_payment(&db, Some(order_id), dec!(60.00));
    let open = db.payments().open_captures_for_order(order_id).expect("open");
    assert_eq!(open.iter().map(|p| p.id).collect::<Vec<_>>(), vec![first.id]);

    // ...until it is fully refunded.
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: first.id, amount: None, ..Default::default() })
        .expect("create full refund");
    db.payments().complete_refund(refund.id).expect("complete refund");
    assert!(db.payments().open_captures_for_order(order_id).expect("refunded").is_empty());

    // An in-flight (pending) capture counts; a cancelled one does not.
    let second = payment(&db, Some(order_id), dec!(40.00));
    let open = db.payments().open_captures_for_order(order_id).expect("open");
    assert_eq!(open.iter().map(|p| p.id).collect::<Vec<_>>(), vec![second.id]);
    db.payments().cancel(second.id).expect("cancel pending");
    assert!(db.payments().open_captures_for_order(order_id).expect("cancelled").is_empty());

    // A partially refunded capture is still outstanding for the remainder.
    let third = completed_payment(&db, Some(order_id), dec!(40.00));
    let partial = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: third.id,
            amount: Some(dec!(15.00)),
            ..Default::default()
        })
        .expect("partial refund");
    db.payments().complete_refund(partial.id).expect("complete partial");
    let open = db.payments().open_captures_for_order(order_id).expect("open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, third.id);
    assert_eq!(open[0].amount - open[0].amount_refunded, dec!(25.00));
}

// ============================================================================
// Idempotency — a racing duplicate key returns the existing payment
// ============================================================================

#[test]
fn concurrent_duplicate_idempotency_key_returns_the_existing_payment() {
    let db = Arc::new(db());
    let key = format!("idem-{}", Uuid::new_v4());
    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                db.payments().create(CreatePayment {
                    idempotency_key: Some(key),
                    ..payment_input(None, dec!(25.00))
                })
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    let ids: Vec<_> = results
        .into_iter()
        .map(|r| r.expect("a duplicate idempotency key is idempotent, never a conflict").id)
        .collect();
    assert_eq!(ids[0], ids[1], "both callers must observe the same payment");

    let count = db.payments().count(stateset_core::PaymentFilter::default()).expect("count");
    assert_eq!(count, 1, "exactly one payment row for one idempotency key");
}

// ============================================================================
// Batch updates share the transition guard
// ============================================================================

#[test]
fn update_batch_records_an_illegal_transition_as_a_failure() {
    let db = db();
    let settled = completed_payment(&db, None, dec!(10.00));
    let pending = payment(&db, None, dec!(10.00));

    let result = db
        .payments()
        .update_batch(vec![
            (
                settled.id,
                UpdatePayment {
                    status: Some(PaymentTransactionStatus::Cancelled),
                    ..Default::default()
                },
            ),
            (
                pending.id,
                UpdatePayment {
                    status: Some(PaymentTransactionStatus::Cancelled),
                    ..Default::default()
                },
            ),
        ])
        .expect("partial-success batch never errors as a whole");

    assert_eq!(result.failure_count, 1, "{result:?}");
    assert_eq!(result.success_count, 1, "{result:?}");
    assert_eq!(result.failed[0].id.as_deref(), Some(settled.id.to_string().as_str()));
    assert_eq!(status(&db, settled.id), PaymentTransactionStatus::Completed);
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Cancelled);
}

#[test]
fn update_batch_atomic_aborts_the_whole_batch_on_an_illegal_transition() {
    let db = db();
    let pending = payment(&db, None, dec!(10.00));
    let settled = completed_payment(&db, None, dec!(10.00));

    let err = db
        .payments()
        .update_batch_atomic(vec![
            (
                pending.id,
                UpdatePayment {
                    status: Some(PaymentTransactionStatus::Cancelled),
                    ..Default::default()
                },
            ),
            (
                settled.id,
                UpdatePayment {
                    status: Some(PaymentTransactionStatus::Cancelled),
                    ..Default::default()
                },
            ),
        ])
        .expect_err("one illegal write aborts the atomic batch");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    // The legal first write was rolled back with the batch.
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Pending);
    assert_eq!(status(&db, settled.id), PaymentTransactionStatus::Completed);
}
