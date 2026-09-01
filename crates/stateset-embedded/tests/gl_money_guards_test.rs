//! GL money-integrity guards, against the SQLite backend:
//! auto-post idempotency, period guards on post/void, closing-entry
//! exclusion from the income statement, and double-close protection.

// Uses the sync `Commerce` engine, which only exists with the sqlite backend.
#![cfg(feature = "sqlite")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateAutoPostingConfig, CreateCustomer, CreateGlPeriod, CreateInvoice, CreateInvoiceItem,
    CreateJournalEntry, CreateJournalEntryLine, JournalEntryFilter, JournalEntryStatus,
    PeriodStatus,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("create in-memory Commerce")
}

/// Standard chart of accounts, auto-posting config, and a wide open period
/// (GL auto-posting stamps entries with the source document's date, which
/// must fall inside an open period).
fn setup(commerce: &Commerce) -> Uuid {
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart");
    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "GL guards test".into(),
        cash_account_id: by_number("1010"),
        accounts_receivable_account_id: by_number("1100"),
        inventory_account_id: by_number("1200"),
        accounts_payable_account_id: by_number("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: by_number("4010"),
        shipping_revenue_account_id: None,
        cogs_account_id: by_number("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .expect("set auto posting config");

    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "FY-wide".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: date(2020, 1, 1),
            end_date: date(2030, 12, 31),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");
    period.id
}

fn create_invoice(commerce: &Commerce) -> Uuid {
    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("gl-guards-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id;
    commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Services".into(),
                quantity: dec!(1),
                unit_price: dec!(100.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create invoice")
        .id
        .into()
}

/// A posted $100 revenue entry (debit Cash, credit Sales Revenue).
fn post_revenue_entry(commerce: &Commerce, entry_date: NaiveDate) -> Uuid {
    let gl = commerce.general_ledger();
    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    gl.create_journal_entry(CreateJournalEntry {
        entry_date,
        description: "Cash sale".into(),
        lines: vec![
            CreateJournalEntryLine::debit(by_number("1010"), dec!(100.00), None),
            CreateJournalEntryLine::credit(by_number("4010"), dec!(100.00), None),
        ],
        entry_type: None,
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(true),
    })
    .expect("create journal entry")
    .id
}

#[test]
fn auto_post_invoice_is_idempotent() {
    let commerce = new_commerce();
    setup(&commerce);
    let invoice_id = create_invoice(&commerce);
    let gl = commerce.general_ledger();

    let first = gl.auto_post_invoice(invoice_id).expect("first auto-post");
    let second = gl.auto_post_invoice(invoice_id).expect("second auto-post");
    assert_eq!(first.id, second.id, "retry must return the existing entry, not post again");

    let entries = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("invoice".into()),
            source_document_id: Some(invoice_id),
            ..Default::default()
        })
        .expect("list entries");
    assert_eq!(entries.len(), 1, "the invoice must have exactly one journal entry");
}

#[test]
fn post_into_closed_period_is_rejected() {
    let commerce = new_commerce();
    let period_id = setup(&commerce);
    let gl = commerce.general_ledger();
    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;

    let draft = gl
        .create_journal_entry(CreateJournalEntry {
            entry_date: date(2026, 1, 15),
            description: "Draft in open period".into(),
            lines: vec![
                CreateJournalEntryLine::debit(by_number("1010"), dec!(50.00), None),
                CreateJournalEntryLine::credit(by_number("4010"), dec!(50.00), None),
            ],
            entry_type: None,
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(false),
        })
        .expect("create draft");

    gl.close_period(period_id, "tester").expect("close period");

    let err = gl.post_journal_entry(draft.id, "tester");
    assert!(err.is_err(), "posting a draft into a closed period must fail");

    // Reopening the period makes the draft postable again.
    gl.reopen_period(period_id).expect("reopen period");
    gl.post_journal_entry(draft.id, "tester").expect("post after reopen");
}

#[test]
fn void_in_closed_period_is_rejected() {
    let commerce = new_commerce();
    let period_id = setup(&commerce);
    let gl = commerce.general_ledger();

    let entry_id = post_revenue_entry(&commerce, date(2026, 1, 15));
    gl.close_period(period_id, "tester").expect("close period");

    let err = gl.void_journal_entry(entry_id);
    assert!(err.is_err(), "voiding an entry in a closed period must fail");

    // Reopening the period makes the entry voidable again.
    gl.reopen_period(period_id).expect("reopen period");
    let voided = gl.void_journal_entry(entry_id).expect("void after reopen");
    assert_eq!(voided.status, JournalEntryStatus::Voided);
}

#[test]
fn income_statement_excludes_closing_entries() {
    let commerce = new_commerce();
    let period_id = setup(&commerce);
    let gl = commerce.general_ledger();

    post_revenue_entry(&commerce, date(2026, 1, 15));
    gl.run_period_close(period_id, "tester").expect("run period close");

    let statement =
        gl.get_income_statement(date(2020, 1, 1), date(2030, 12, 31)).expect("income statement");
    assert_eq!(
        statement.total_revenue,
        dec!(100.00),
        "a closed period's P&L must still report its revenue"
    );
    assert_eq!(statement.net_income, dec!(100.00));
}

#[test]
fn run_period_close_twice_is_rejected() {
    let commerce = new_commerce();
    let period_id = setup(&commerce);
    let gl = commerce.general_ledger();

    post_revenue_entry(&commerce, date(2026, 1, 15));
    let closing = gl.run_period_close(period_id, "tester").expect("first close");
    assert_eq!(
        gl.get_period(period_id).expect("get period").expect("period").status,
        PeriodStatus::Closed
    );

    // Reopen for adjustments: a second close must NOT double the closing
    // entry while the first one still stands.
    gl.reopen_period(period_id).expect("reopen period");
    let err = gl.run_period_close(period_id, "tester");
    assert!(err.is_err(), "re-close with a standing closing entry must fail");

    // Voiding the standing closing entry makes re-close legitimate.
    gl.void_journal_entry(closing.id).expect("void closing entry");
    gl.run_period_close(period_id, "tester").expect("re-close after voiding closing entry");
}
