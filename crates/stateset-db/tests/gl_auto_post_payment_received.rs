#![cfg(feature = "sqlite")]

//! Regression: SQLite `auto_post_payment_received` was doubly broken.
//!
//! 1. It read the payment with `SELECT amount, payment_date FROM payments`, but the
//!    SQLite `payments` table has no `payment_date` column — it stores `paid_at`
//!    (nullable) and `created_at`. Postgres reads
//!    `COALESCE(paid_at, created_at)`. So the query failed at runtime with a
//!    "no such column" error.
//! 2. It parsed the date (a full RFC3339 timestamp) directly as a `NaiveDate`,
//!    which cannot parse a timestamp — so even with the column fixed the date
//!    parse would fail.
//!
//! The result: posting a received payment to the general ledger was completely
//! broken on SQLite while Postgres worked. This test creates a configured chart of
//! accounts and a payment, then asserts `auto_post_payment_received` produces a
//! balanced, correctly-dated journal entry (debit Cash, credit AR).

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    CreateAutoPostingConfig, CreateGlPeriod, CreatePayment, GeneralLedgerRepository,
    PaymentRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_auto_post_payment_received_posts_balanced_entry() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };

    // A freshly-created payment has `paid_at = NULL`, so its entry date comes from
    // `created_at` (today). Use a full-year period so the covering period is robust
    // regardless of when the test runs.
    let today = Utc::now().date_naive();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: format!("{}-full", today.year()),
            fiscal_year: today.year(),
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap(),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "default".into(),
        cash_account_id: acct("1010"),
        accounts_receivable_account_id: acct("1100"),
        inventory_account_id: acct("1200"),
        accounts_payable_account_id: acct("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: acct("4010"),
        shipping_revenue_account_id: None,
        cogs_account_id: acct("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .expect("set auto-posting config");

    let payment = db
        .payments()
        .create(CreatePayment { amount: dec!(100.00), ..Default::default() })
        .expect("create payment");

    let entry = gl
        .auto_post_payment_received(payment.id.into())
        .expect("auto_post_payment_received must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "Cash debit should equal the payment amount");
    assert_eq!(entry.total_credits, dec!(100.00), "AR credit should equal the payment amount");
    assert_eq!(entry.entry_date, today, "entry date should be the payment date");
}
