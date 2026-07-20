#![cfg(feature = "sqlite")]

//! Regression: SQLite `auto_post_inventory_cost` was triply broken.
//!
//! 1. It read the transaction with
//!    `SELECT total_cost, transaction_date, transaction_type FROM cost_transactions`,
//!    but the table has no `transaction_date` column — the date is `created_at`
//!    (Postgres reads `created_at`). So the query failed at runtime with a
//!    "no such column" error.
//! 2. It parsed that date (a full RFC3339 timestamp) directly as a `NaiveDate`,
//!    which cannot parse a timestamp.
//! 3. It treated only `transaction_type == "sale"` as a COGS-debit issue; Postgres
//!    treats `"issue"` OR `"sale"`. So an `"issue"` cost transaction posted with the
//!    debit and credit REVERSED on SQLite (Inventory debited, COGS credited).
//!
//! This test records an `"issue"` cost transaction and asserts `auto_post_inventory_cost`
//! produces a balanced journal entry with the correct direction: COGS debited,
//! Inventory credited.

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CostAccountingRepository, CostTransactionType, CreateAutoPostingConfig, CreateGlPeriod,
    GeneralLedgerRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_auto_post_inventory_cost_issue_posts_cogs_debit() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };

    // The entry date comes from the transaction's `created_at` (today); use a
    // full-year period so the covering period is robust regardless of run date.
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
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .expect("set auto-posting config");

    // An inventory issue: total_cost = 2 * 50.00 = 100.00.
    let txn = db
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
        .expect("record cost transaction");
    assert_eq!(txn.total_cost, dec!(100.00), "total cost should be quantity * unit cost");

    let entry = gl.auto_post_inventory_cost(txn.id).expect("auto_post_inventory_cost must succeed");

    assert_eq!(entry.total_debits, dec!(100.00));
    assert_eq!(entry.total_credits, dec!(100.00));

    let lines = gl.get_journal_entry_lines(entry.id).expect("get journal entry lines");
    let line_for = |number: &str| {
        lines
            .iter()
            .find(|l| l.account_number.as_deref() == Some(number))
            .unwrap_or_else(|| panic!("a line posts to account {number}"))
    };

    // An "issue" moves cost from Inventory to COGS: debit COGS, credit Inventory.
    let cogs = line_for("5010");
    assert_eq!(cogs.debit_amount, dec!(100.00), "COGS must be debited for an issue");
    assert_eq!(cogs.credit_amount, Decimal::ZERO, "COGS must not be credited for an issue");

    let inventory = line_for("1200");
    assert_eq!(inventory.credit_amount, dec!(100.00), "Inventory must be credited for an issue");
    assert_eq!(inventory.debit_amount, Decimal::ZERO, "Inventory must not be debited for an issue");
}
