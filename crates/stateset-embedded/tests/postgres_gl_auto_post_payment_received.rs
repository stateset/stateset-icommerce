//! Postgres parity for `auto_post_payment_received`.
//!
//! SQLite `auto_post_payment_received` was doubly broken (selected a nonexistent
//! `payment_date` column, and parsed the RFC3339 date as a bare `NaiveDate`).
//! Postgres reads `COALESCE(paid_at, created_at)` as a `DateTime<Utc>` and reduces
//! it with `.date_naive()`, so it was already correct — this test locks in that
//! behavior so the two backends stay in agreement: posting a received payment
//! yields a balanced, correctly-dated journal entry (debit Cash, credit AR).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{CreateAutoPostingConfig, CreateGlPeriod, CreatePayment};
use stateset_embedded::AsyncCommerce;

mod common;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_auto_post_payment_received_posts_balanced_entry() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping payment auto-post parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();

    gl.initialize_chart_of_accounts().await.expect("init chart of accounts");

    let acct = |number: &'static str| {
        let gl = &gl;
        async move {
            gl.get_account_by_number(number)
                .await
                .expect("query account")
                .unwrap_or_else(|| panic!("account {number} exists"))
                .id
        }
    };

    let today = Utc::now().date_naive();
    let _period = common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: format!("{}-full", today.year()),
            fiscal_year: today.year(),
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap(),
        },
    )
    .await;

    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "default".into(),
        cash_account_id: acct("1010").await,
        accounts_receivable_account_id: acct("1100").await,
        inventory_account_id: acct("1200").await,
        accounts_payable_account_id: acct("2010").await,
        unearned_revenue_account_id: None,
        sales_revenue_account_id: acct("4010").await,
        shipping_revenue_account_id: None,
        cogs_account_id: acct("5010").await,
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .await
    .expect("set auto-posting config");

    let payment = commerce
        .payments()
        .create(CreatePayment { amount: dec!(100.00), ..Default::default() })
        .await
        .expect("create payment");

    let entry = gl
        .auto_post_payment_received(payment.id.into())
        .await
        .expect("auto_post_payment_received must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "Cash debit should equal the payment amount");
    assert_eq!(entry.total_credits, dec!(100.00), "AR credit should equal the payment amount");
    assert_eq!(entry.entry_date, today, "entry date should be the payment date");
}
