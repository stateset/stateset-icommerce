#![cfg(feature = "sqlite")]

//! Regression: SQLite `auto_post_bill` was doubly broken.
//!
//! 1. It read the bill with `SELECT total_amount, bill_date FROM bills`, but there
//!    is no `bills` table — accounts-payable bills live in `ap_bills` (Postgres
//!    reads `ap_bills`). So the query failed at runtime with a "no such table"
//!    error.
//! 2. It parsed `bill_date` (stored on SQLite as a full RFC3339 timestamp) directly
//!    as a `NaiveDate`, which cannot parse a timestamp — so even with the table
//!    fixed the date parse would fail.
//!
//! The result: posting a vendor bill to the general ledger was completely broken on
//! SQLite while Postgres worked. This test creates a configured chart of accounts
//! and an AP bill, then asserts `auto_post_bill` produces a balanced, correctly-
//! dated journal entry (debit Inventory/Expense, credit Accounts Payable).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, CreateAutoPostingConfig, CreateBill, CreateBillItem, CreateGlPeriod,
    GeneralLedgerRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_auto_post_bill_posts_balanced_entry() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };

    let bill_date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().expect("parse date");
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-03".into(),
            fiscal_year: 2026,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
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

    // Bill total = 2 * 50.00 = 100.00.
    let bill = db
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: uuid::Uuid::new_v4(),
            bill_date: Some(bill_date),
            due_date: "2026-04-10T00:00:00Z".parse().expect("parse due date"),
            items: vec![CreateBillItem {
                description: "Raw material".into(),
                account_code: None,
                quantity: dec!(2),
                unit_price: dec!(50.00),
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");
    assert_eq!(bill.total_amount, dec!(100.00), "bill total should foot to 100.00");

    let entry = gl.auto_post_bill(bill.id).expect("auto_post_bill must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "Inventory debit should equal the bill total");
    assert_eq!(entry.total_credits, dec!(100.00), "AP credit should equal the bill total");
    assert_eq!(entry.entry_date, bill_date.date_naive(), "entry date should be the bill date");
}
