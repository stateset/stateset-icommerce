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
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

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
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
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
