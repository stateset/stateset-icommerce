//! Postgres parity for `auto_post_inventory_cost`.
//!
//! SQLite `auto_post_inventory_cost` was triply broken: it selected a nonexistent
//! `transaction_date` column (the date is `created_at`), parsed the RFC3339 date as
//! a bare `NaiveDate`, and treated only `"sale"` (not `"issue"`) as a COGS-debit
//! issue — so an `"issue"` transaction posted with debit/credit reversed. Postgres
//! reads `created_at`, calls `.date_naive()`, and treats `"issue"` OR `"sale"` as an
//! issue, so it was already correct — this test locks in that behavior so the two
//! backends stay in agreement: an `"issue"` cost transaction posts a balanced entry
//! with COGS debited and Inventory credited.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{CostTransactionType, CreateAutoPostingConfig, CreateGlPeriod};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_auto_post_inventory_cost_issue_posts_cogs_debit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping inventory-cost auto-post test");
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
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: format!("{}-full", today.year()),
            fiscal_year: today.year(),
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap(),
        })
        .await
        .expect("create period");
    gl.open_period(period.id).await.expect("open period");

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
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .await
    .expect("set auto-posting config");

    let txn = commerce
        .cost_accounting()
        .record_cost_transaction(
            "WIDGET-1",
            CostTransactionType::Issue,
            dec!(2),
            dec!(50.00),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("record cost transaction");
    assert_eq!(txn.total_cost, dec!(100.00), "total cost should be quantity * unit cost");

    let entry =
        gl.auto_post_inventory_cost(txn.id).await.expect("auto_post_inventory_cost must succeed");

    assert_eq!(entry.total_debits, dec!(100.00));
    assert_eq!(entry.total_credits, dec!(100.00));

    let lines = gl.get_journal_entry_lines(entry.id).await.expect("get journal entry lines");
    let line_for = |number: &str| {
        lines
            .iter()
            .find(|l| l.account_number.as_deref() == Some(number))
            .unwrap_or_else(|| panic!("a line posts to account {number}"))
    };

    let cogs = line_for("5010");
    assert_eq!(cogs.debit_amount, dec!(100.00), "COGS must be debited for an issue");
    assert_eq!(cogs.credit_amount, Decimal::ZERO, "COGS must not be credited for an issue");

    let inventory = line_for("1200");
    assert_eq!(inventory.credit_amount, dec!(100.00), "Inventory must be credited for an issue");
    assert_eq!(inventory.debit_amount, Decimal::ZERO, "Inventory must not be debited for an issue");
}
