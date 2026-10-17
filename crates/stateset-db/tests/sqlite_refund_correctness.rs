#![cfg(feature = "sqlite")]
//! Regression tests for payment refund correctness against the live SQLite
//! engine.
//!
//! These cover two verified defects:
//!
//! 1. **Over-refund / invalid-status refund.** `create_refund` previously
//!    inserted any requested amount with no bounds check and no payment-status
//!    check, so a `Pending`/`Failed` payment could be refunded and a payment
//!    could be refunded for more than its remaining balance.
//! 2. **Money arithmetic on TEXT columns.** `complete_refund` did
//!    `amount_refunded + ?` and `>= amount` directly in SQL on TEXT columns,
//!    which SQLite coerces to IEEE-754 floats (e.g. `'0.10' + '0.20'` yields
//!    `0.30000000000000004`). The arithmetic is now done in Rust with
//!    `rust_decimal::Decimal` and written back as exact TEXT.

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreatePayment, CreateRefund, PaymentMethodType, PaymentRepository,
};
use stateset_db::SqliteDatabase;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

#[test]
fn concurrent_identical_full_refund_creates_one_refund_and_one_event() {
    let db = std::sync::Arc::new(db());
    let payment = completed_payment(&db, dec!(1.00));
    let request = CreateRefund {
        payment_id: payment.id,
        amount: Some(dec!(1.00)),
        idempotency_key: Some("concurrent-full-refund".into()),
        ..Default::default()
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let db = db.clone();
            let barrier = barrier.clone();
            let request = request.clone();
            std::thread::spawn(move || {
                barrier.wait();
                db.payments().create_refund(request).unwrap()
            })
        })
        .collect();
    let refunds: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
    assert!(refunds.iter().all(|refund| refund.id == refunds[0].id));
    assert_eq!(db.payments().get_refunds(payment.id).unwrap().len(), 1);
    let events: i64 = db.conn().unwrap().query_row(
        "SELECT COUNT(*) FROM kernel_outbox WHERE idempotency_key='concurrent-full-refund' AND event_type='payments.refund_created.v1'",
        [], |row| row.get(0)).unwrap();
    assert_eq!(events, 1);
}

#[test]
fn implicit_amount_retry_preserves_the_original_resolved_remaining_refund() {
    let db = db();
    let payment = completed_payment(&db, dec!(1.00));
    let partial = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.20)),
            ..Default::default()
        })
        .unwrap();
    db.payments().complete_refund(partial.id).unwrap();
    let request = CreateRefund {
        payment_id: payment.id,
        amount: None,
        idempotency_key: Some("implicit-remaining".into()),
        ..Default::default()
    };
    let original = db.payments().create_refund(request.clone()).unwrap();
    assert_eq!(original.amount, dec!(0.80));
    db.payments().complete_refund(original.id).unwrap();
    assert_eq!(db.payments().create_refund(request).unwrap().id, original.id);
}

#[test]
fn refund_retry_key_cannot_switch_payment_amount_or_immutable_details() {
    let db = db();
    let payment = completed_payment(&db, dec!(1.00));
    let other = completed_payment(&db, dec!(1.00));
    let request = CreateRefund {
        payment_id: payment.id,
        amount: Some(dec!(0.10)),
        reason: Some("verified return".into()),
        external_id: Some("processor-refund".into()),
        notes: Some("original request".into()),
        idempotency_key: Some("refund-bound-request".into()),
    };
    let original = db.payments().create_refund(request.clone()).unwrap();
    assert_eq!(db.payments().create_refund(request.clone()).unwrap().id, original.id);
    for field in ["payment", "amount", "reason", "external", "notes"] {
        let mut changed = request.clone();
        match field {
            "payment" => changed.payment_id = other.id,
            "amount" => changed.amount = Some(dec!(0.20)),
            "reason" => changed.reason = None,
            "external" => changed.external_id = None,
            _ => changed.notes = None,
        }
        assert!(
            matches!(db.payments().create_refund(changed), Err(CommerceError::Conflict(_))),
            "{field}"
        );
    }
    assert_eq!(db.payments().get_refunds(payment.id).unwrap().len(), 1);
    assert!(db.payments().get_refunds(other.id).unwrap().is_empty());
}

/// Create a payment and advance it to `Completed` (the only state from which a
/// payment normally becomes refundable).
fn completed_payment(db: &SqliteDatabase, amount: rust_decimal::Decimal) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("mark payment completed")
}

#[test]
fn refund_settlement_requires_the_transition_balance_and_fact_to_commit_together() {
    let db = db();
    for complete in [false, true] {
        for fault in ["fact_error", "fact_ignore", "refund_ignore", "payment_ignore"] {
            if !complete && fault == "payment_ignore" {
                continue;
            }
            let payment = completed_payment(&db, dec!(0.30));
            let refund = db
                .payments()
                .create_refund(CreateRefund {
                    payment_id: payment.id,
                    amount: Some(dec!(0.10)),
                    ..Default::default()
                })
                .unwrap();
            let event =
                if complete { "payments.refund_completed.v1" } else { "payments.refund_failed.v1" };
            let status = if complete { "completed" } else { "failed" };
            let before = serde_json::json!({ "payment": db.payments().get(payment.id).unwrap(), "refund": refund });
            let (table, operation, predicate) = match fault {
                "refund_ignore" => (
                    "refunds",
                    "UPDATE",
                    format!("NEW.id = '{}' AND NEW.status = '{status}'", refund.id),
                ),
                "payment_ignore" => ("payments", "UPDATE", format!("NEW.id = '{}'", payment.id)),
                _ => (
                    "kernel_outbox",
                    "INSERT",
                    format!("NEW.aggregate_id = '{}' AND NEW.event_type = '{event}'", refund.id),
                ),
            };
            let refusal = if fault == "fact_error" {
                "RAISE(ABORT, 'settlement fact unavailable')"
            } else {
                "RAISE(IGNORE)"
            };
            db.conn()
                .unwrap()
                .execute_batch(&format!(
                    "CREATE TRIGGER reject_settlement BEFORE {operation} ON {table}
                 WHEN {predicate} BEGIN SELECT {refusal}; END;"
                ))
                .unwrap();
            let settle = || {
                if complete {
                    db.payments().complete_refund(refund.id)
                } else {
                    db.payments().fail_refund(refund.id, "private processor reason")
                }
            };
            let result = settle();
            db.conn().unwrap().execute_batch("DROP TRIGGER reject_settlement").unwrap();
            assert!(result.is_err(), "complete={complete}, fault={fault}");
            assert_eq!(
                serde_json::json!({ "payment": db.payments().get(payment.id).unwrap(), "refund": db.payments().get_refund(refund.id).unwrap() }),
                before
            );
            let count = || {
                db.conn().unwrap().query_row(
                "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = ?1 AND event_type = ?2",
                rusqlite::params![refund.id.to_string(), event], |row| row.get::<_, i64>(0),
            ).unwrap()
            };
            assert_eq!(count(), 0);
            let settled = settle().unwrap();
            assert_eq!(settled.status.to_string(), status);
            assert_eq!(
                serde_json::to_value(settle().unwrap()).unwrap(),
                serde_json::to_value(&settled).unwrap()
            );
            assert_eq!(count(), 1);
            let payload: String = db
                .conn()
                .unwrap()
                .query_row(
                    "SELECT payload FROM kernel_outbox WHERE aggregate_id = ?1 AND event_type = ?2",
                    rusqlite::params![refund.id.to_string(), event],
                    |row| row.get(0),
                )
                .unwrap();
            let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(payload["refund_id"], refund.id.to_string());
            assert_eq!(payload["payment_id"], payment.id.to_string());
            assert_eq!(payload["status"], status);
            assert_eq!(payload["previous_status"], "pending");
            assert_eq!(
                payload["amount"].as_str().unwrap().parse::<rust_decimal::Decimal>().unwrap(),
                dec!(0.10)
            );
            assert_eq!(payload["currency"], "USD");
            assert!(!payload.to_string().contains("private processor reason"));
            assert_eq!(
                db.payments().get(payment.id).unwrap().unwrap().amount_refunded,
                if complete { dec!(0.10) } else { dec!(0) }
            );
            let opposite = if complete {
                db.payments().fail_refund(refund.id, "late failure")
            } else {
                db.payments().complete_refund(refund.id)
            };
            assert!(opposite.is_err());
            assert_eq!(count(), 1);
        }
    }
}

#[test]
fn competing_refund_completion_and_failure_emit_one_terminal_fact() {
    let db = std::sync::Arc::new(db());
    let payment = completed_payment(&db, dec!(0.30));
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|operation| {
            let db = db.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if operation % 2 == 0 {
                    db.payments().complete_refund(refund.id)
                } else {
                    db.payments().fail_refund(refund.id, "processor refusal")
                }
            })
        })
        .collect();
    for worker in workers {
        match worker.join().unwrap() {
            Ok(_) | Err(CommerceError::ValidationError(_)) => {}
            other => panic!("unexpected settlement outcome: {other:?}"),
        }
    }
    let settled = db.payments().get_refund(refund.id).unwrap().unwrap();
    assert!(matches!(
        settled.status,
        stateset_core::RefundStatus::Completed | stateset_core::RefundStatus::Failed
    ));
    let count: i64 = db.conn().unwrap().query_row(
        "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = ? AND event_type IN ('payments.refund_completed.v1', 'payments.refund_failed.v1')",
        [refund.id.to_string()], |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        db.payments().get(payment.id).unwrap().unwrap().amount_refunded,
        if settled.status == stateset_core::RefundStatus::Completed { dec!(0.10) } else { dec!(0) }
    );
}

/// An over-refund must surface as the typed invariant error carrying the
/// stable code `commerce.refund.exceeds_captured` — never as a stringly
/// `ValidationError` an agent cannot branch on.
#[track_caller]
fn assert_over_refund(err: &CommerceError) {
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "got {err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.refund.exceeds_captured"), "got {err:?}");
}

#[test]
fn refund_amount_must_be_positive() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.00)),
            ..Default::default()
        })
        .expect_err("zero-amount refund must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(-5.00)),
            ..Default::default()
        })
        .expect_err("negative-amount refund must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn refund_exceeding_remaining_is_rejected() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(150.00)),
            ..Default::default()
        })
        .expect_err("refund larger than payment must be rejected");
    assert_over_refund(&err);

    // No refund row should have been persisted.
    assert!(db.payments().get_refunds(payment.id).expect("list refunds").is_empty());
    // And the payment's refunded amount stays at zero.
    let reloaded = db.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(reloaded.amount_refunded, dec!(0));
}

#[test]
fn over_refund_across_two_refunds_is_rejected() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    // First partial refund of 60 succeeds and completes.
    let r1 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .expect("first refund");
    db.payments().complete_refund(r1.id).expect("complete first refund");

    // A second refund of 60 would push total refunded to 120 > 100 and must be
    // rejected: only 40 remains refundable.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .expect_err("second refund exceeding remaining must be rejected");
    assert_over_refund(&err);
}

#[test]
fn refunding_pending_payment_is_rejected() {
    let db = db();
    // Freshly created payment is `Pending` (not captured/completed).
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("create payment");

    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect_err("refunding a pending payment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn refunding_failed_payment_is_rejected() {
    let db = db();
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_failed(payment.id, "card declined", Some("declined")).expect("mark failed");

    let err = db
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, amount: None, ..Default::default() })
        .expect_err("refunding a failed payment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn exact_full_refund_succeeds_and_flips_to_refunded() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    // Omitting the amount means "full remaining refund".
    let refund = db
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, amount: None, ..Default::default() })
        .expect("full refund");
    assert_eq!(refund.amount, dec!(100.00));

    db.payments().complete_refund(refund.id).expect("complete full refund");

    let reloaded = db.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(reloaded.amount_refunded, dec!(100.00));
    assert_eq!(reloaded.status, stateset_core::PaymentTransactionStatus::Refunded);
}

#[test]
fn two_partial_refunds_sum_to_exact_decimal_and_flip_to_refunded() {
    let db = db();
    // 0.10 + 0.20 == 0.30 exactly, but '0.10' + '0.20' in SQLite float math is
    // 0.30000000000000004. This is the core money-precision regression.
    let payment = completed_payment(&db, dec!(0.30));

    let r1 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("first partial refund");
    db.payments().complete_refund(r1.id).expect("complete first partial refund");

    // After the first partial refund the payment is partially refunded.
    let mid = db.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(mid.amount_refunded, dec!(0.10));
    assert_eq!(mid.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);

    let r2 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.20)),
            ..Default::default()
        })
        .expect("second partial refund");
    db.payments().complete_refund(r2.id).expect("complete second partial refund");

    let reloaded = db.payments().get(payment.id).expect("get").expect("payment present");
    // Exact decimal: must be 0.30, never 0.30000000000000004.
    assert_eq!(reloaded.amount_refunded, dec!(0.30));
    assert_eq!(
        reloaded.amount_refunded.to_string(),
        "0.30",
        "amount_refunded must be exact decimal text, not a float-coerced value"
    );
    // And the status must flip to fully refunded.
    assert_eq!(reloaded.status, stateset_core::PaymentTransactionStatus::Refunded);
}

/// Atomicity / in-flight-reservation regression.
///
/// `create_refund` now reads the payment, validates the over-refund guard, and
/// inserts the refund inside a single `IMMEDIATE` transaction, and the guard
/// counts *in-flight* (`Pending`/`Processing`) refunds against the remaining
/// balance — not just the already-committed `amount_refunded`.
///
/// Previously the over-refund check only saw `amount_refunded`, which is only
/// updated on `complete_refund`. So two refunds could be *created* (both
/// `Pending`) that together exceeded the payment, and completing both would
/// over-refund the payment. This test pins the new behavior: a second pending
/// refund that would exceed the balance is rejected at creation time even
/// though the first refund has NOT been completed yet.
#[test]
fn second_pending_refund_exceeding_remaining_is_rejected_before_completion() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    // First refund of 60 is created but deliberately left Pending (not completed),
    // so `amount_refunded` is still 0 on the payment row.
    let r1 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .expect("first pending refund");

    // The committed balance is untouched: amount_refunded is still 0.
    let mid = db.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(mid.amount_refunded, dec!(0));

    // A second refund of 60 would, once both complete, push the total to 120 >
    // 100. Because the first 60 is reserved as an in-flight refund, the guard
    // must reject this at creation time — without it, both pending refunds would
    // persist and later over-refund the payment.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .expect_err("second pending refund exceeding remaining must be rejected");
    assert_over_refund(&err);

    // Only the first refund should exist.
    let refunds = db.payments().get_refunds(payment.id).expect("list refunds");
    assert_eq!(refunds.len(), 1, "over-refunding second pending refund must not persist");
    assert_eq!(refunds[0].id, r1.id);
}

/// A second pending refund within the remaining balance is allowed, and the
/// reservation only consumes what each in-flight refund actually requests.
#[test]
fn second_pending_refund_within_remaining_is_allowed() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    // 40 + 50 = 90 <= 100: both fit even though neither is completed yet.
    db.payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .expect("first pending refund");

    db.payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .expect("second pending refund within remaining");

    // A third refund of 20 would push the reserved total to 110 > 100 and is
    // rejected, confirming the reservation accumulates across in-flight refunds.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(20.00)),
            ..Default::default()
        })
        .expect_err("third refund exceeding reserved remaining must be rejected");
    assert_over_refund(&err);

    let refunds = db.payments().get_refunds(payment.id).expect("list refunds");
    assert_eq!(refunds.len(), 2, "only the two fitting refunds should persist");
}

/// A failed (terminal) refund releases its reservation, so the remaining
/// balance becomes available to a new refund again.
#[test]
fn failed_refund_releases_its_reservation() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    // Reserve the whole balance with a pending refund...
    let r1 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(100.00)),
            ..Default::default()
        })
        .expect("full pending refund");

    // ...while it is pending, no further refund can be created.
    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect_err("balance fully reserved by pending refund");
    assert_over_refund(&err);

    // Fail the first refund: its reservation is released.
    db.payments().fail_refund(r1.id, "processor declined").expect("fail refund");

    // Now the full balance is refundable again.
    let r2 = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(100.00)),
            ..Default::default()
        })
        .expect("refund after failed reservation released");
    db.payments().complete_refund(r2.id).expect("complete refund");

    let reloaded = db.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(reloaded.amount_refunded, dec!(100.00));
    assert_eq!(reloaded.status, stateset_core::PaymentTransactionStatus::Refunded);
}

/// Completing the same refund twice (e.g. a duplicated payment-processor webhook
/// or a retry) must be idempotent: the refund amount is folded into the
/// payment's `amount_refunded` exactly once, never doubled.
#[test]
fn complete_refund_is_idempotent() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .expect("create refund");

    db.payments().complete_refund(refund.id).expect("first completion");
    let once = db.payments().get(payment.id).expect("get").expect("present");
    assert_eq!(once.amount_refunded, dec!(50.00));
    assert_eq!(once.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);

    // Second completion must be a no-op, not a double-count.
    db.payments().complete_refund(refund.id).expect("second completion is idempotent");
    let twice = db.payments().get(payment.id).expect("get").expect("present");
    assert_eq!(
        twice.amount_refunded,
        dec!(50.00),
        "duplicate complete_refund must NOT double-count into amount_refunded"
    );
    assert_eq!(
        twice.status,
        stateset_core::PaymentTransactionStatus::PartiallyRefunded,
        "payment must not flip to fully Refunded on a duplicate completion"
    );
}

/// A failed (terminal) refund cannot be completed, and completing it must not
/// fold its amount into the payment.
#[test]
fn complete_refund_rejects_failed_refund() {
    let db = db();
    let payment = completed_payment(&db, dec!(100.00));

    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .expect("create refund");
    db.payments().fail_refund(refund.id, "processor declined").expect("fail refund");

    let err =
        db.payments().complete_refund(refund.id).expect_err("a failed refund cannot be completed");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let reloaded = db.payments().get(payment.id).expect("get").expect("present");
    assert_eq!(
        reloaded.amount_refunded,
        dec!(0.00),
        "a failed refund must not fold its amount into the payment"
    );
}
