#![cfg(feature = "sqlite")]

//! Regression tests for the payment status-transition guards (SQLite backend,
//! sync `Commerce` engine).
//!
//! The defect these cover: `update`/`cancel`/`mark_failed` used to write the
//! payment status unconditionally, bypassing
//! [`PaymentTransactionStatus::can_transition_to`] entirely. Because `cancelled`
//! and `failed` are NOT in the backend's `capturing_statuses` set, cancelling (or
//! failing) an already-`Completed` payment RELEASED the slice of the order total
//! that the settled money was consuming — a fresh full-amount capture then sailed
//! past the over-capture guard and the order was captured twice.
//!
//! Covers:
//! - cancelling / failing a completed (or partially refunded) payment is refused;
//! - the double-capture scenario end to end (complete, cancel, re-capture);
//! - every legal transition still succeeds, including the single-shot
//!   `pending -> completed` capture;
//! - refunds against a cancelled payment are refused, and a refunded payment
//!   cannot be cancelled out from under its refund ledger;
//! - two OS threads racing `mark_completed` against `cancel` cannot both win.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment,
    CreateRefund, CustomerId, OrderId, Payment, PaymentMethodType, PaymentTransactionStatus,
    UpdatePayment,
};
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("payment-guards-{}@example.com", Uuid::new_v4()),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

/// An order whose `total_amount` is exactly `unit_price` (single unit, no tax or
/// shipping), so the over-capture guard's arithmetic is easy to reason about.
fn order_totalling(commerce: &Commerce, unit_price: Decimal) -> OrderId {
    let customer_id = customer(commerce);
    commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "GUARD-SKU-001".into(),
                name: "Guarded Product".into(),
                quantity: 1,
                unit_price,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
        .id
}

fn payment(commerce: &Commerce, order_id: Option<OrderId>, amount: Decimal) -> Payment {
    commerce
        .payments()
        .create(CreatePayment {
            order_id,
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .expect("create payment")
}

fn completed_payment(commerce: &Commerce, order_id: Option<OrderId>, amount: Decimal) -> Payment {
    let p = payment(commerce, order_id, amount);
    commerce.payments().mark_completed(p.id).expect("mark completed")
}

fn status(commerce: &Commerce, id: stateset_embedded::PaymentId) -> PaymentTransactionStatus {
    commerce.payments().get(id).expect("get payment").expect("payment exists").status
}

/// Σ of the amounts of every payment for `order_id` that is holding captured
/// money (completed or refunded-in-part; a cancelled/failed one holds nothing).
fn captured_total(commerce: &Commerce, order_id: OrderId) -> Decimal {
    commerce
        .payments()
        .for_order(order_id)
        .expect("list payments for order")
        .iter()
        .filter(|p| p.status.is_successful() || p.status == PaymentTransactionStatus::Refunded)
        .map(|p| p.amount)
        .sum()
}

fn assert_conflict(err: &CommerceError, context: &str) {
    assert!(
        matches!(err, CommerceError::Conflict(_)),
        "{context}: expected CommerceError::Conflict, got {err:?}"
    );
}

// ============================================================================
// Illegal transitions out of a settled payment
// ============================================================================

#[test]
fn cancel_rejects_completed_payment() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(100.00));

    let err =
        commerce.payments().cancel(paid.id).expect_err("cancelling settled money must be refused");
    assert_conflict(&err, "cancel(completed)");

    assert_eq!(
        status(&commerce, paid.id),
        PaymentTransactionStatus::Completed,
        "a refused cancel must leave the status untouched"
    );
}

#[test]
fn mark_failed_rejects_completed_payment() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(100.00));

    let err = commerce
        .payments()
        .mark_failed(paid.id, "processor said so", Some("bogus"))
        .expect_err("failing settled money must be refused");
    assert_conflict(&err, "mark_failed(completed)");

    let after = commerce.payments().get(paid.id).expect("get").expect("exists");
    assert_eq!(after.status, PaymentTransactionStatus::Completed);
    assert_eq!(after.failure_reason, None, "a refused failure must not write a reason");
    assert_eq!(after.failure_code, None, "a refused failure must not write a code");
}

#[test]
fn cancel_rejects_partially_refunded_payment() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(100.00));
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: paid.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .expect("create refund");
    commerce.payments().complete_refund(refund.id).expect("complete refund");
    assert_eq!(status(&commerce, paid.id), PaymentTransactionStatus::PartiallyRefunded);

    let err = commerce
        .payments()
        .cancel(paid.id)
        .expect_err("a payment with a refund ledger cannot be cancelled");
    assert_conflict(&err, "cancel(partially_refunded)");

    let after = commerce.payments().get(paid.id).expect("get").expect("exists");
    assert_eq!(after.status, PaymentTransactionStatus::PartiallyRefunded);
    assert_eq!(after.amount_refunded, dec!(40.00), "the refund ledger must be intact");
}

#[test]
fn mark_failed_rejects_refunded_payment() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(100.00));
    let refund = commerce
        .payments()
        .create_refund(CreateRefund { payment_id: paid.id, amount: None, ..Default::default() })
        .expect("create refund");
    commerce.payments().complete_refund(refund.id).expect("complete refund");
    assert_eq!(status(&commerce, paid.id), PaymentTransactionStatus::Refunded);

    let err = commerce
        .payments()
        .mark_failed(paid.id, "late decline", None)
        .expect_err("a fully refunded payment is terminal");
    assert_conflict(&err, "mark_failed(refunded)");
    assert_eq!(status(&commerce, paid.id), PaymentTransactionStatus::Refunded);
}

#[test]
fn update_rejects_illegal_status_write() {
    let commerce = commerce();
    let cancelled = payment(&commerce, None, dec!(25.00));
    commerce.payments().cancel(cancelled.id).expect("pending -> cancelled");

    // `Cancelled` is terminal: no status write out of it is legal.
    let err = commerce
        .payments()
        .update(
            cancelled.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Completed),
                ..Default::default()
            },
        )
        .expect_err("resurrecting a cancelled payment must be refused");
    assert_conflict(&err, "update(cancelled -> completed)");
    assert_eq!(status(&commerce, cancelled.id), PaymentTransactionStatus::Cancelled);
}

#[test]
fn mark_completed_rejects_cancelled_payment() {
    let commerce = commerce();
    let cancelled = payment(&commerce, None, dec!(25.00));
    commerce.payments().cancel(cancelled.id).expect("pending -> cancelled");

    let err = commerce
        .payments()
        .mark_completed(cancelled.id)
        .expect_err("a cancelled payment cannot be completed");
    assert_conflict(&err, "mark_completed(cancelled)");
    assert_eq!(status(&commerce, cancelled.id), PaymentTransactionStatus::Cancelled);
}

// ============================================================================
// The money scenario: cancel-then-recapture double capture
// ============================================================================

#[test]
fn cancelling_a_completed_payment_cannot_free_capture_capacity() {
    let commerce = commerce();
    let order_id = order_totalling(&commerce, dec!(100.00));

    // 1. Capture the whole order.
    let first = completed_payment(&commerce, Some(order_id), dec!(100.00));
    assert_eq!(captured_total(&commerce, order_id), dec!(100.00));

    // 2. Cancelling the settled capture would release its slice of the order
    //    total. It must be refused.
    let err = commerce.payments().cancel(first.id).expect_err("cancel of settled money");
    assert_conflict(&err, "cancel(completed) in the double-capture scenario");
    assert_eq!(status(&commerce, first.id), PaymentTransactionStatus::Completed);

    // 3. A fresh full-amount capture must therefore still be refused by the
    //    over-capture guard: the order is already fully captured.
    let second = commerce.payments().create(CreatePayment {
        order_id: Some(order_id),
        payment_method: PaymentMethodType::CreditCard,
        amount: dec!(100.00),
        ..Default::default()
    });
    let err = second.expect_err("the order is fully captured; a second capture must be refused");
    assert_eq!(
        err.invariant_code(),
        Some("commerce.capture.exceeds_order_total"),
        "expected the over-capture invariant, got {err:?}"
    );

    // 4. The order was never captured twice.
    assert_eq!(
        captured_total(&commerce, order_id),
        dec!(100.00),
        "total captured must never exceed the order total"
    );
    assert_eq!(
        commerce.payments().for_order(order_id).expect("list").len(),
        1,
        "the refused capture must not have written a payment row"
    );
}

#[test]
fn failing_a_completed_payment_cannot_free_capture_capacity() {
    let commerce = commerce();
    let order_id = order_totalling(&commerce, dec!(80.00));
    let first = completed_payment(&commerce, Some(order_id), dec!(80.00));

    let err = commerce
        .payments()
        .mark_failed(first.id, "chargeback-ish", None)
        .expect_err("failing settled money must be refused");
    assert_conflict(&err, "mark_failed(completed) in the double-capture scenario");

    let err = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(80.00),
            ..Default::default()
        })
        .expect_err("the order is fully captured");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{err:?}");
    assert_eq!(captured_total(&commerce, order_id), dec!(80.00));
}

// ============================================================================
// Every legal transition still succeeds
// ============================================================================

#[test]
fn legal_transition_pending_to_processing_to_completed() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));

    let processing = commerce.payments().mark_processing(p.id).expect("pending -> processing");
    assert_eq!(processing.status, PaymentTransactionStatus::Processing);

    let completed = commerce.payments().mark_completed(p.id).expect("processing -> completed");
    assert_eq!(completed.status, PaymentTransactionStatus::Completed);
    assert!(completed.paid_at.is_some());
}

#[test]
fn legal_transition_pending_to_completed_single_shot_capture() {
    // Auto-capture: many processors settle in one call, with no intermediate
    // `processing` step. This edge is the backend's one documented addition to
    // the core state machine.
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));
    let completed = commerce.payments().mark_completed(p.id).expect("pending -> completed");
    assert_eq!(completed.status, PaymentTransactionStatus::Completed);
}

#[test]
fn legal_transition_pending_to_cancelled() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));
    let cancelled = commerce.payments().cancel(p.id).expect("pending -> cancelled");
    assert_eq!(cancelled.status, PaymentTransactionStatus::Cancelled);
}

#[test]
fn legal_transition_pending_to_failed() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));
    let failed =
        commerce.payments().mark_failed(p.id, "card declined", Some("declined")).expect("fail");
    assert_eq!(failed.status, PaymentTransactionStatus::Failed);
    assert_eq!(failed.failure_reason, Some("card declined".into()));
    assert_eq!(failed.failure_code, Some("declined".into()));
}

#[test]
fn legal_transition_processing_to_cancelled_and_failed() {
    let commerce = commerce();

    let a = payment(&commerce, None, dec!(10.00));
    commerce.payments().mark_processing(a.id).expect("pending -> processing");
    assert_eq!(
        commerce.payments().cancel(a.id).expect("processing -> cancelled").status,
        PaymentTransactionStatus::Cancelled
    );

    let b = payment(&commerce, None, dec!(10.00));
    commerce.payments().mark_processing(b.id).expect("pending -> processing");
    assert_eq!(
        commerce
            .payments()
            .mark_failed(b.id, "timeout", None)
            .expect("processing -> failed")
            .status,
        PaymentTransactionStatus::Failed
    );
}

#[test]
fn legal_transition_requires_action_round_trip() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));
    commerce.payments().mark_processing(p.id).expect("pending -> processing");

    let ra = commerce
        .payments()
        .update(
            p.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::RequiresAction),
                ..Default::default()
            },
        )
        .expect("processing -> requires_action");
    assert_eq!(ra.status, PaymentTransactionStatus::RequiresAction);

    let completed = commerce.payments().mark_completed(p.id).expect("requires_action -> completed");
    assert_eq!(completed.status, PaymentTransactionStatus::Completed);
}

#[test]
fn legal_transition_completed_to_disputed_and_back() {
    let commerce = commerce();
    // Against a real order, so the dispute's effect on capture capacity is
    // observable: a disputed capture is contested money, not a settled loss,
    // and must keep consuming its slice of the order total (D1).
    let order_id = order_totalling(&commerce, dec!(50.00));
    let paid = completed_payment(&commerce, Some(order_id), dec!(50.00));

    let disputed = commerce
        .payments()
        .update(
            paid.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Disputed),
                ..Default::default()
            },
        )
        .expect("completed -> disputed");
    assert_eq!(disputed.status, PaymentTransactionStatus::Disputed);

    // While disputed, the order is still fully captured: a fresh full-amount
    // capture must be refused by the over-capture guard.
    let err = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(50.00),
            ..Default::default()
        })
        .expect_err("a disputed capture still consumes the order total");
    assert_eq!(
        err.invariant_code(),
        Some("commerce.capture.exceeds_order_total"),
        "expected the over-capture invariant, got {err:?}"
    );

    // The dispute can be resolved in the merchant's favour, or against it.
    let resolved = commerce.payments().mark_completed(paid.id).expect("disputed -> completed");
    assert_eq!(resolved.status, PaymentTransactionStatus::Completed);
    assert_eq!(commerce.payments().for_order(order_id).expect("list").len(), 1);
}

#[test]
fn dispute_resolved_via_update_cannot_double_capture_the_order() {
    let commerce = commerce();
    let order_id = order_totalling(&commerce, dec!(100.00));
    let paid = completed_payment(&commerce, Some(order_id), dec!(100.00));

    commerce
        .payments()
        .update(
            paid.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Disputed),
                ..Default::default()
            },
        )
        .expect("completed -> disputed");

    let second = commerce.payments().create(CreatePayment {
        order_id: Some(order_id),
        payment_method: PaymentMethodType::CreditCard,
        amount: dec!(100.00),
        ..Default::default()
    });
    assert_eq!(
        second.expect_err("second capture while disputed").invariant_code(),
        Some("commerce.capture.exceeds_order_total")
    );

    // Resolving through the generic `update` path (not `mark_completed`).
    let resolved = commerce
        .payments()
        .update(
            paid.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Completed),
                ..Default::default()
            },
        )
        .expect("disputed -> completed via update");
    assert_eq!(resolved.status, PaymentTransactionStatus::Completed);
    assert_eq!(
        captured_total(&commerce, order_id),
        dec!(100.00),
        "total captured must never exceed the order total"
    );
}

#[test]
fn idempotent_self_transitions_are_accepted() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(10.00));
    commerce.payments().cancel(p.id).expect("pending -> cancelled");
    // Re-cancelling is a no-op, not a conflict (the state machine allows the
    // self-transition), so a retried webhook does not error.
    let again = commerce.payments().cancel(p.id).expect("cancel is idempotent");
    assert_eq!(again.status, PaymentTransactionStatus::Cancelled);
}

#[test]
fn update_without_a_status_change_never_conflicts() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(10.00));

    let updated = commerce
        .payments()
        .update(
            paid.id,
            UpdatePayment { external_id: Some("pi_abc123".into()), ..Default::default() },
        )
        .expect("a metadata-only update on a completed payment is legal");
    assert_eq!(updated.external_id, Some("pi_abc123".into()));
    assert_eq!(updated.status, PaymentTransactionStatus::Completed);
}

// ============================================================================
// Refunds interact correctly with cancellation
// ============================================================================

#[test]
fn refund_against_cancelled_payment_is_rejected() {
    let commerce = commerce();
    let p = payment(&commerce, None, dec!(100.00));
    commerce.payments().cancel(p.id).expect("pending -> cancelled");

    let err = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: p.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect_err("a cancelled payment never captured anything to refund");
    assert!(
        matches!(err, CommerceError::ValidationError(_)),
        "expected ValidationError, got {err:?}"
    );
    assert!(commerce.payments().get_refunds(p.id).expect("get refunds").is_empty());
}

#[test]
fn cancel_cannot_strand_an_in_flight_refund() {
    let commerce = commerce();
    let paid = completed_payment(&commerce, None, dec!(100.00));
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: paid.id,
            amount: Some(dec!(30.00)),
            ..Default::default()
        })
        .expect("create refund");

    // Cancelling the payment while a refund is in flight would leave a refund
    // pointing at money the ledger says was never taken.
    let err = commerce.payments().cancel(paid.id).expect_err("cancel with an in-flight refund");
    assert_conflict(&err, "cancel(completed) with an in-flight refund");

    // The refund still completes against the intact payment.
    commerce.payments().complete_refund(refund.id).expect("complete refund");
    let after = commerce.payments().get(paid.id).expect("get").expect("exists");
    assert_eq!(after.status, PaymentTransactionStatus::PartiallyRefunded);
    assert_eq!(after.amount_refunded, dec!(30.00));
}

// ============================================================================
// Concurrency: two OS threads cannot both win
// ============================================================================

#[test]
fn concurrent_complete_and_cancel_cannot_both_win() {
    // `pending` can go to EITHER `completed` or `cancelled`, but never both:
    // whichever thread commits first must lock the other out, because
    // `completed -> cancelled` and `cancelled -> completed` are both illegal.
    // Without the status-guarded UPDATE, both threads write and the payment ends
    // up in whichever state raced last — while the loser's caller believes it
    // succeeded (the double-capture bug's precondition).
    //
    // Repeated over several rounds so both interleavings are exercised.
    for round in 0..8 {
        let commerce = Arc::new(commerce());
        let p = payment(&commerce, None, dec!(100.00));
        let barrier = Arc::new(Barrier::new(2));

        let completer = {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                commerce.payments().mark_completed(p.id)
            })
        };
        let canceller = {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                commerce.payments().cancel(p.id)
            })
        };

        let complete_result = completer.join().expect("completer thread panicked");
        let cancel_result = canceller.join().expect("canceller thread panicked");

        let winners = usize::from(complete_result.is_ok()) + usize::from(cancel_result.is_ok());
        assert_eq!(
            winners, 1,
            "round {round}: exactly one of complete/cancel may win, got \
             complete={complete_result:?} cancel={cancel_result:?}"
        );

        let loser = complete_result
            .as_ref()
            .err()
            .or_else(|| cancel_result.as_ref().err())
            .expect("one thread must lose");
        assert_conflict(loser, &format!("round {round}: losing thread"));

        let final_status = status(&commerce, p.id);
        let expected = if complete_result.is_ok() {
            PaymentTransactionStatus::Completed
        } else {
            PaymentTransactionStatus::Cancelled
        };
        assert_eq!(
            final_status, expected,
            "round {round}: the persisted status must match the winning thread"
        );
    }
}

#[test]
fn concurrent_cancels_of_a_completed_payment_never_free_capacity() {
    // Four threads race to cancel the same settled capture and then re-capture
    // the order. Every cancel must be refused and the order must end up captured
    // exactly once.
    let commerce = Arc::new(commerce());
    let order_id = order_totalling(&commerce, dec!(100.00));
    let paid = completed_payment(&commerce, Some(order_id), dec!(100.00));

    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let cancelled = commerce.payments().cancel(paid.id);
                let recaptured = commerce.payments().create(CreatePayment {
                    order_id: Some(order_id),
                    payment_method: PaymentMethodType::CreditCard,
                    amount: dec!(100.00),
                    ..Default::default()
                });
                (cancelled, recaptured)
            })
        })
        .collect();

    for handle in handles {
        let (cancelled, recaptured) = handle.join().expect("thread panicked");
        assert!(cancelled.is_err(), "no thread may cancel a settled capture: {cancelled:?}");
        assert!(recaptured.is_err(), "no thread may re-capture a settled order: {recaptured:?}");
    }

    assert_eq!(status(&commerce, paid.id), PaymentTransactionStatus::Completed);
    assert_eq!(
        captured_total(&commerce, order_id),
        dec!(100.00),
        "the order must never be captured twice"
    );
}
