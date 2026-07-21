#![cfg(feature = "sqlite")]

//! Regression: SQLite `auto_post_invoice` was doubly broken.
//!
//! 1. It read the invoice total with `SELECT total_amount FROM invoices`, but the
//!    SQLite `invoices` table's money column is named `total` (Postgres reads
//!    `total`). So the query failed at runtime with a "no such column" error.
//! 2. It parsed `invoice_date` (stored as a full RFC3339 timestamp) directly as a
//!    `NaiveDate`, which cannot parse a timestamp — so even with the column fixed
//!    the date parse would fail.
//!
//! The result: posting an invoice to the general ledger was completely broken on
//! SQLite while Postgres worked. This test creates a configured chart of accounts
//! and an invoice, then asserts `auto_post_invoice` produces a balanced,
//! correctly-dated journal entry.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    CreateAutoPostingConfig, CreateGlPeriod, CreateInvoice, CreateInvoiceItem,
    GeneralLedgerRepository, InvoiceRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_auto_post_invoice_posts_balanced_entry() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };

    // Posting requires an open GL period covering the entry date.
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

    // Invoice dated on a known day; total = 2 * 50.00 = 100.00.
    let invoice_date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().expect("parse date");
    let invoice = db
        .invoices()
        .create(CreateInvoice {
            customer_id: uuid::Uuid::new_v4().into(),
            invoice_date: Some(invoice_date),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(2),
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create invoice");
    assert_eq!(invoice.total, dec!(100.00), "invoice total should foot to 100.00");

    let entry = gl.auto_post_invoice(invoice.id).expect("auto_post_invoice must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "AR debit should equal the invoice total");
    assert_eq!(entry.total_credits, dec!(100.00), "Sales credit should equal the invoice total");
    assert_eq!(
        entry.entry_date,
        invoice_date.date_naive(),
        "entry date should be the invoice date (from the RFC3339 timestamp)"
    );
}
