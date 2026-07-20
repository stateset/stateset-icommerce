//! Postgres parity for the credit concurrency fixes (SQLite covered by the
//! `concurrent_charges_cannot_exceed_credit_limit` and
//! `concurrent_reservations_cannot_exceed_available_credit` unit tests in
//! sqlite/credit.rs). Charging and reserving credit are both check-then-act on
//! the account row (`current_balance`/`credit_limit`/`hold_amount`); concurrent
//! operations must serialize (via `FOR UPDATE`) so they cannot each pass the
//! check against a stale read and together exceed the credit line.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{CreateCreditAccount, RiskRating};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_charges_cannot_exceed_credit_limit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping credit concurrency test");
        return;
    };
    let commerce = Arc::new(
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations"),
    );

    // credit_accounts.customer_id is UNIQUE with no customers FK, so a random id
    // is fine and keeps runs isolated.
    let customer_id = uuid::Uuid::new_v4();
    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id: customer_id.into(),
            credit_limit: dec!(100),
            currency: None,
            payment_terms: Some("NET30".into()),
            risk_rating: Some(RiskRating::Low),
            notes: Some("concurrency test".into()),
        })
        .await
        .expect("create credit account");

    // Ten $20 charges race; only five fit under the $100 limit.
    let task_count = 10;
    let barrier = Arc::new(tokio::sync::Barrier::new(task_count));
    let mut handles = Vec::new();
    for _ in 0..task_count {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            commerce.credit().charge_credit(customer_id, uuid::Uuid::new_v4(), dec!(20)).await
        }));
    }

    let mut successes = 0;
    for h in handles {
        if h.await.expect("join").is_ok() {
            successes += 1;
        }
    }

    let acct = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .await
        .expect("get")
        .expect("found");
    assert!(
        acct.current_balance <= dec!(100),
        "credit limit exceeded under concurrency: balance {}",
        acct.current_balance
    );
    assert_eq!(successes, 5, "exactly five $20 charges fit under the $100 limit");
    assert_eq!(acct.current_balance, dec!(100), "balance must equal the five successful charges");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_reservations_cannot_exceed_available_credit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping reservation concurrency test");
        return;
    };
    let commerce = Arc::new(
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations"),
    );

    let customer_id = uuid::Uuid::new_v4();
    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id: customer_id.into(),
            credit_limit: dec!(100),
            currency: None,
            payment_terms: Some("NET30".into()),
            risk_rating: Some(RiskRating::Low),
            notes: Some("concurrency test".into()),
        })
        .await
        .expect("create credit account");

    // Ten $20 reservations race against $100 available — only five may win.
    let task_count = 10;
    let barrier = Arc::new(tokio::sync::Barrier::new(task_count));
    let mut handles = Vec::new();
    for _ in 0..task_count {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            commerce.credit().reserve_credit(customer_id, uuid::Uuid::new_v4(), dec!(20)).await
        }));
    }

    let mut successes = 0;
    for h in handles {
        if h.await.expect("join").is_ok() {
            successes += 1;
        }
    }

    let acct = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .await
        .expect("get")
        .expect("found");
    assert!(
        acct.hold_amount <= dec!(100),
        "available credit over-reserved under concurrency: holds {}",
        acct.hold_amount
    );
    assert_eq!(successes, 5, "exactly five $20 reservations fit under $100");
    assert_eq!(acct.hold_amount, dec!(100), "holds must equal the five successful reservations");
    assert_eq!(acct.available_credit, Decimal::ZERO);
}
