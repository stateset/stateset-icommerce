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
    // The orders module now consults `open_captures_for_order` before
    // cancelling: a plain cancel is refused while the pending capture is
    // outstanding, and a forced cancel voids it in the same transaction — so
    // there is no window in which a live capture can complete against a
    // cancelled order.
    let err = db
        .orders()
        .update(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .expect_err("plain cancel is refused while a capture is in flight");
    assert_validation_mentioning(&err, "100.00 USD");
    db.orders()
        .update(
            order_id,
            UpdateOrder {
                status: Some(OrderStatus::Cancelled),
                void_payments: true,
                ..Default::default()
            },
        )
        .expect("forced cancel");
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Cancelled, "voided");

    let err =
        db.payments().mark_completed(pending.id).expect_err("a voided capture cannot complete");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Cancelled);
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

// ============================================================================
// Round 4 — order cancel cannot orphan money
// ============================================================================

#[test]
fn cancelling_an_order_with_open_captures_is_refused_without_void_payments() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let captured = completed_payment(&db, Some(order_id), dec!(60.00));

    let err = db
        .orders()
        .update(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .expect_err("captured money must not be orphaned by a cancel");
    assert_validation_mentioning(&err, "60.00 USD");
    assert_validation_mentioning(&err, "void_payments");

    let order = db.orders().get(order_id).expect("get").expect("exists");
    assert_eq!(order.status, OrderStatus::Pending, "cancel rolled back");
    assert_eq!(status(&db, captured.id), PaymentTransactionStatus::Completed);

    // Once the money is returned the plain cancel goes through.
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: captured.id, amount: None, ..Default::default() })
        .expect("refund");
    db.payments().complete_refund(refund.id).expect("complete refund");
    cancel_order(&db, order_id);
}

#[test]
fn an_in_flight_payment_also_blocks_a_plain_cancel() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let pending = payment(&db, Some(order_id), dec!(100.00));

    let err = db
        .orders()
        .update(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .expect_err("a pending capture still holds the order total");
    assert_validation_mentioning(&err, "100.00 USD");
    assert_eq!(status(&db, pending.id), PaymentTransactionStatus::Pending);
}

#[test]
fn forced_cancel_voids_in_flight_payments_and_leaves_settled_ones_for_refund() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let settled = completed_payment(&db, Some(order_id), dec!(60.00));
    let in_flight = payment(&db, Some(order_id), dec!(30.00));
    let processing = payment(&db, Some(order_id), dec!(10.00));
    db.payments().mark_processing(processing.id).expect("processing");

    let order = db
        .orders()
        .update(
            order_id,
            UpdateOrder {
                status: Some(OrderStatus::Cancelled),
                void_payments: true,
                ..Default::default()
            },
        )
        .expect("forced cancel");
    assert_eq!(order.status, OrderStatus::Cancelled);

    assert_eq!(status(&db, in_flight.id), PaymentTransactionStatus::Cancelled, "voided");
    assert_eq!(status(&db, processing.id), PaymentTransactionStatus::Cancelled, "voided");
    assert_eq!(status(&db, settled.id), PaymentTransactionStatus::Completed, "left for refund");
    let open = db.payments().open_captures_for_order(order_id).expect("open");
    assert_eq!(open.iter().map(|p| p.id).collect::<Vec<_>>(), vec![settled.id]);

    // The outbox event records what happened to the money.
    let event = db
        .kernel_outbox()
        .pending(100)
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
    assert_eq!(event.payload["outstanding_captured"], "60.00");

    // The settled money can still be refunded after the cancel.
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: settled.id, amount: None, ..Default::default() })
        .expect("refund after cancel");
    db.payments().complete_refund(refund.id).expect("complete refund");
    assert!(db.payments().open_captures_for_order(order_id).expect("open").is_empty());
}

#[test]
fn plain_cancel_event_reports_no_money_movement() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    cancel_order(&db, order_id);
    let event = db
        .kernel_outbox()
        .pending(100)
        .expect("pending")
        .into_iter()
        .find(|e| e.event_type == "orders.updated.v1" && e.aggregate_id == order_id.to_string())
        .expect("event");
    assert_eq!(event.payload["void_payments"], false);
    assert_eq!(event.payload["voided_payment_ids"], serde_json::json!([]));
    assert_eq!(event.payload["outstanding_captured"], "0");
}

#[test]
fn update_batch_atomic_cancel_shares_the_money_rule() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    completed_payment(&db, Some(order_id), dec!(100.00));
    let err = db
        .orders()
        .update_batch_atomic(vec![(
            order_id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )])
        .expect_err("batch cancel is refused like a single cancel");
    assert_validation_mentioning(&err, "100.00 USD");
    assert_eq!(db.orders().get(order_id).unwrap().unwrap().status, OrderStatus::Pending);
}

// ============================================================================
// Round 4 — idempotency keys fingerprint the request
// ============================================================================

#[test]
fn duplicate_idempotency_key_with_different_parameters_is_a_conflict() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let key = format!("idem-{}", Uuid::new_v4());
    let first = db
        .payments()
        .create(CreatePayment {
            idempotency_key: Some(key.clone()),
            ..payment_input(Some(order_id), dec!(25.00))
        })
        .expect("first");

    // Same request → same payment, no new row.
    let replay = db
        .payments()
        .create(CreatePayment {
            idempotency_key: Some(key.clone()),
            ..payment_input(Some(order_id), dec!(25.00))
        })
        .expect("identical replay");
    assert_eq!(replay.id, first.id);

    // Different amount / order / currency / method → Conflict, and the stored
    // payment is untouched.
    let other_order = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
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
            .create(CreatePayment { idempotency_key: Some(key.clone()), ..input })
            .expect_err("a different request under a used key must conflict");
        assert!(
            matches!(err, CommerceError::Conflict(ref m) if m.contains(&key) && m.contains(&first.id.to_string())),
            "{label}: {err:?}"
        );
    }
    let stored = db.payments().get(first.id).expect("get").expect("exists");
    assert_eq!(stored.amount, dec!(25.00));
    assert_eq!(db.payments().for_order(order_id).expect("list").len(), 1);
}

#[test]
fn concurrent_duplicate_idempotency_key_with_different_amounts_yields_one_payment_and_one_conflict()
{
    let db = Arc::new(db());
    let key = format!("idem-{}", Uuid::new_v4());
    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = [dec!(25.00), dec!(26.00)]
        .into_iter()
        .map(|amount| {
            let db = Arc::clone(&db);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                db.payments().create(CreatePayment {
                    idempotency_key: Some(key),
                    ..payment_input(None, amount)
                })
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();

    let ok: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let conflicts = results
        .iter()
        .filter(|r| matches!(r, Err(CommerceError::Conflict(m)) if m.contains(&key)))
        .count();
    assert_eq!(ok.len(), 1, "exactly one caller wins: {results:?}");
    assert_eq!(conflicts, 1, "the other is told the key was used differently: {results:?}");
    let count = db.payments().count(stateset_core::PaymentFilter::default()).expect("count");
    assert_eq!(count, 1);
}

// ============================================================================
// Round 4 — refund statuses are ledger states, never bare status flips
// ============================================================================

#[test]
fn refunded_cannot_be_reached_through_update() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00), CurrencyCode::USD);
    let p = completed_payment(&db, Some(order_id), dec!(100.00));
    set_status(&db, p.id, PaymentTransactionStatus::Disputed).expect("dispute");

    // Disputed -> Refunded is a legal state-machine edge, but as a status
    // flip it leaves amount_refunded at 0 and the capture "outstanding".
    let err = set_status(&db, p.id, PaymentTransactionStatus::Refunded)
        .expect_err("refund by status flip is refused");
    assert_validation_mentioning(&err, "complete_refund");
    assert_eq!(status(&db, p.id), PaymentTransactionStatus::Disputed);
    assert_eq!(db.payments().open_captures_for_order(order_id).unwrap().len(), 1);

    // Same for Completed -> Refunded / PartiallyRefunded.
    set_status(&db, p.id, PaymentTransactionStatus::Completed).expect("dispute won");
    for target in [PaymentTransactionStatus::Refunded, PaymentTransactionStatus::PartiallyRefunded]
    {
        let err = set_status(&db, p.id, target).expect_err("refused");
        assert_validation_mentioning(&err, "complete_refund");
    }
    assert_eq!(status(&db, p.id), PaymentTransactionStatus::Completed);

    // The real path: create_refund + complete_refund writes amount_refunded.
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: p.id, amount: None, ..Default::default() })
        .expect("refund");
    db.payments().complete_refund(refund.id).expect("complete");
    let after = db.payments().get(p.id).unwrap().unwrap();
    assert_eq!(after.status, PaymentTransactionStatus::Refunded);
    assert_eq!(after.amount_refunded, dec!(100.00));
    assert!(db.payments().open_captures_for_order(order_id).unwrap().is_empty());

    // A same-status write on a refunded payment (metadata patch) still works.
    let patched = db
        .payments()
        .update(
            p.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                metadata: Some("{\"note\":\"ok\"}".into()),
                ..Default::default()
            },
        )
        .expect("no-op status with metadata");
    assert_eq!(patched.metadata.as_deref(), Some("{\"note\":\"ok\"}"));
}

#[test]
fn update_batch_atomic_refuses_refund_by_status_flip() {
    let db = db();
    let p = completed_payment(&db, None, dec!(10.00));
    let err = db
        .payments()
        .update_batch_atomic(vec![(
            p.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                ..Default::default()
            },
        )])
        .expect_err("batch refused");
    assert_validation_mentioning(&err, "complete_refund");
    assert_eq!(status(&db, p.id), PaymentTransactionStatus::Completed);
}
