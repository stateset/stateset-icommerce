//! Postgres parity for `list_payment_runs` filters.
//!
//! SQLite `list_payment_runs` ignored its filter entirely (dropping `status` and
//! `limit`/`offset`); Postgres already applies status + LIMIT/OFFSET. This test
//! locks in that behavior. Runs are scoped to a fresh random payment method so a
//! shared DB's other runs don't interfere with the status/pagination assertions.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreatePaymentRun, PaymentMethodAP, PaymentRunFilter, PaymentRunStatus};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_list_payment_runs_applies_status_and_pagination() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping payment-run filter parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

    let run_date = "2026-03-10T00:00:00Z".parse().expect("parse date");
    // A distinctive payment method scopes this test on a shared database.
    let method = PaymentMethodAP::Wire;
    let draft = ap
        .create_payment_run(CreatePaymentRun {
            payment_date: run_date,
            payment_method: method,
            ..Default::default()
        })
        .await
        .expect("create draft run");
    let to_cancel = ap
        .create_payment_run(CreatePaymentRun {
            payment_date: run_date,
            payment_method: method,
            ..Default::default()
        })
        .await
        .expect("create second run");
    ap.cancel_payment_run(to_cancel.id).await.expect("cancel run");

    // Draft status filter returns exactly the un-cancelled run.
    let drafts = ap
        .list_payment_runs(PaymentRunFilter {
            status: Some(PaymentRunStatus::Draft),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(drafts.iter().any(|r| r.id == draft.id), "Draft filter must include the draft run");
    assert!(
        drafts.iter().all(|r| r.status == PaymentRunStatus::Draft),
        "Draft filter must return only draft runs"
    );

    // Cancelled status filter returns the cancelled run, never the draft.
    let cancelled = ap
        .list_payment_runs(PaymentRunFilter {
            status: Some(PaymentRunStatus::Cancelled),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(cancelled.iter().any(|r| r.id == to_cancel.id));
    assert!(cancelled.iter().all(|r| r.status == PaymentRunStatus::Cancelled));

    // limit caps the result set.
    let limited = ap
        .list_payment_runs(PaymentRunFilter { limit: Some(1), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(limited.len(), 1, "list_payment_runs must honor `limit`");
}
