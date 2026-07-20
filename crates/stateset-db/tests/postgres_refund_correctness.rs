//! Regression tests for the Postgres `create_refund_async` over-refund guard.
//!
//! The Postgres refund-create path historically did the payment `SELECT` and the
//! over-refund validation on a lock-free pool connection, then issued the refund
//! `INSERT` separately — and it validated only against the committed
//! `amount_refunded`, ignoring not-yet-completed `Pending`/`Processing` refunds.
//! Two concurrent refunds of the same payment could each validate against the
//! same stale balance and both succeed, over-refunding the payment once both
//! completed (a TOCTOU race) — unlike the SQLite backend, which serializes the
//! read+validate+insert inside one `IMMEDIATE` transaction and reserves against
//! in-flight refunds.
//!
//! The fix runs the read (with `SELECT ... FOR UPDATE`), the in-flight
//! reservation sum, `validate_refund`, and the `INSERT` inside one transaction.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`)
//! and are skipped otherwise, so they run only in CI with a provisioned database.

#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{CommerceError, CreatePayment, CreateRefund, PaymentMethodType};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

/// Create a payment and advance it to `Completed` so it becomes refundable.
#[cfg(feature = "postgres")]
async fn completed_payment(
    db: &PostgresDatabase,
    amount: rust_decimal::Decimal,
) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("mark completed")
}

/// A second refund that, together with an in-flight (`Pending`) refund, would
/// exceed the payment balance must be rejected — even though the first refund's
/// amount has not yet folded into `amount_refunded`.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_reserves_against_in_flight() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping in-flight refund test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let payment = completed_payment(&db, dec!(100.00)).await;

    // First refund leaves it Pending (does not touch amount_refunded yet).
    db.payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .await
        .expect("first 60.00 refund");

    // A second 60.00 refund would total 120.00 > 100.00. It must be rejected
    // because the in-flight Pending refund is reserved against the balance.
    let err = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .await
        .expect_err("second refund exceeding remaining-minus-in-flight must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // A refund for the exact remaining 40.00 must still be allowed.
    db.payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .await
        .expect("remaining 40.00 refund");

    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    let total: rust_decimal::Decimal = refunds.iter().map(|r| r.amount).sum();
    assert_eq!(total, dec!(100.00), "total reserved refunds must not exceed the payment amount");
}

/// Many concurrent refunders of the same payment must never reserve more than
/// the payment's balance in aggregate. Before the `FOR UPDATE` + transaction fix
/// this race could let several refunds each validate against the same stale
/// balance and over-refund the payment.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_concurrent_refunds_do_not_over_refund() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping concurrent refund test");
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let payment = completed_payment(&db, dec!(100.00)).await;

    // 10 contenders each ask for 20.00 against a 100.00 payment: at most 5 may
    // succeed; the rest must be rejected as over-refunds.
    let contenders = 10u32;
    let per_refund = dec!(20.00);
    let mut handles = Vec::with_capacity(contenders as usize);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let payment_id = payment.id;
        handles.push(tokio::spawn(async move {
            db.payments()
                .create_refund_async(CreateRefund {
                    payment_id,
                    amount: Some(per_refund),
                    ..Default::default()
                })
                .await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join refund task").is_ok() {
            successes += 1;
        }
    }

    assert_eq!(
        successes, 5,
        "exactly 5 of the 20.00 refunds fit inside the 100.00 payment; got {successes} (over-refund if greater)"
    );

    // The persisted in-flight reservations must total exactly the payment amount,
    // never more.
    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    let total: rust_decimal::Decimal = refunds.iter().map(|r| r.amount).sum();
    assert_eq!(
        total,
        dec!(100.00),
        "aggregate reserved refunds must equal the payment amount, never exceed it"
    );
}

/// Completing the same refund twice (e.g. a duplicated payment-processor webhook)
/// must be idempotent: the amount folds into `amount_refunded` exactly once.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_complete_refund_is_idempotent() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db, dec!(100.00)).await;

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .await
        .expect("create refund");

    db.payments().complete_refund_async(refund.id).await.expect("first completion");
    let once =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(once.amount_refunded, dec!(50.00));
    assert_eq!(once.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);

    db.payments().complete_refund_async(refund.id).await.expect("second completion is idempotent");
    let twice =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(
        twice.amount_refunded,
        dec!(50.00),
        "duplicate complete_refund must NOT double-count into amount_refunded"
    );
    assert_eq!(twice.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);
}

/// A failed (terminal) refund cannot be completed and must not fold its amount.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_complete_refund_rejects_failed_refund() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db, dec!(100.00)).await;

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .await
        .expect("create refund");
    db.payments().fail_refund_async(refund.id, "processor declined").await.expect("fail refund");

    let err = db
        .payments()
        .complete_refund_async(refund.id)
        .await
        .expect_err("a failed refund cannot be completed");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let reloaded =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(reloaded.amount_refunded, dec!(0.00));
}
