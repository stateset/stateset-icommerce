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
fn auto_post_invoice_concurrent_race_is_idempotent() {
    // Two OS threads auto-post the same invoice at the same time. The
    // idempotency check runs inside one immediate (write-locked) transaction,
    // so both calls must succeed, return the SAME entry, and leave exactly one
    // journal entry for the source document.
    use std::sync::{Arc, Barrier};
    use std::thread;

    let commerce = Arc::new(new_commerce());
    setup(&commerce);
    let invoice_id = create_invoice(&commerce);

    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                commerce.general_ledger().auto_post_invoice(invoice_id)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread panicked")).collect();

    let entries: Vec<_> = results
        .into_iter()
        .map(|r| r.expect("concurrent auto-post must succeed idempotently"))
        .collect();
    assert_eq!(entries[0].id, entries[1].id, "both threads must get the same journal entry");

    let listed = commerce
        .general_ledger()
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("invoice".into()),
            source_document_id: Some(invoice_id),
            ..Default::default()
        })
        .expect("list entries");
    assert_eq!(listed.len(), 1, "the racing auto-posts must leave exactly one journal entry");
    assert!(listed[0].is_balanced, "the surviving entry must balance");
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
fn reports_respect_as_of_date() {
    let commerce = new_commerce();
    setup(&commerce);
    let gl = commerce.general_ledger();
    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    let cash_id = by_number("1010");

    post_revenue_entry(&commerce, date(2026, 1, 15)); // $100
    post_revenue_entry(&commerce, date(2026, 2, 10)); // $100 more

    // Trial balance as of Jan 31 must exclude the February entry.
    let tb_jan = gl.get_trial_balance(date(2026, 1, 31)).expect("trial balance jan");
    assert_eq!(tb_jan.total_debits, dec!(100.00));
    assert_eq!(tb_jan.total_credits, dec!(100.00));
    assert!(tb_jan.is_balanced);
    let tb_feb = gl.get_trial_balance(date(2026, 2, 28)).expect("trial balance feb");
    assert_eq!(tb_feb.total_debits, dec!(200.00));

    // Balance sheet cash line follows the same cutoff.
    let bs_jan = gl.get_balance_sheet(date(2026, 1, 31)).expect("balance sheet jan");
    let cash_jan = bs_jan.assets.iter().find(|l| l.account_id == cash_id).expect("cash line");
    assert_eq!(cash_jan.balance, dec!(100.00));
    let bs_feb = gl.get_balance_sheet(date(2026, 2, 28)).expect("balance sheet feb");
    let cash_feb = bs_feb.assets.iter().find(|l| l.account_id == cash_id).expect("cash line");
    assert_eq!(cash_feb.balance, dec!(200.00));

    // get_account_balance: dated queries derive; None stays the live balance.
    assert_eq!(
        gl.get_account_balance(cash_id, Some(date(2026, 1, 31))).expect("dated balance"),
        Some(dec!(100.00))
    );
    assert_eq!(gl.get_account_balance(cash_id, None).expect("live balance"), Some(dec!(200.00)));
}

#[test]
fn reports_exclude_voided_and_draft_entries() {
    let commerce = new_commerce();
    setup(&commerce);
    let gl = commerce.general_ledger();
    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;

    let posted = post_revenue_entry(&commerce, date(2026, 1, 15)); // $100
    // A draft entry must not appear in reports.
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: date(2026, 1, 20),
        description: "Draft".into(),
        lines: vec![
            CreateJournalEntryLine::debit(by_number("1010"), dec!(40.00), None),
            CreateJournalEntryLine::credit(by_number("4010"), dec!(40.00), None),
        ],
        entry_type: None,
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .expect("create draft");

    let tb = gl.get_trial_balance(date(2026, 1, 31)).expect("trial balance");
    assert_eq!(tb.total_debits, dec!(100.00), "draft entries must not count");

    gl.void_journal_entry(posted).expect("void");
    let tb = gl.get_trial_balance(date(2026, 1, 31)).expect("trial balance after void");
    assert_eq!(tb.total_debits, dec!(0.00), "voided entries must not count");
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

#[test]
fn reclose_period_encodes_the_adjustment_flow() {
    let commerce = new_commerce();
    let period_id = setup(&commerce);
    let gl = commerce.general_ledger();

    post_revenue_entry(&commerce, date(2026, 1, 15)); // $100
    gl.run_period_close(period_id, "tester").expect("first close");

    // Adjustments arrive late: reopen, post, and re-close in one call.
    gl.reopen_period(period_id).expect("reopen period");
    post_revenue_entry(&commerce, date(2026, 1, 20)); // $100 more
    let closing = gl.reclose_period(period_id, "tester").expect("reclose");

    // The fresh closing entry sweeps the FULL adjusted activity ($200),
    // the old closing entry is voided, and the period is closed again.
    assert_eq!(closing.total_debits, dec!(200.00));
    assert_eq!(
        gl.get_period(period_id).expect("get period").expect("period").status,
        PeriodStatus::Closed
    );
    let posted_closings = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("period_close".into()),
            source_document_id: Some(period_id),
            status: Some(JournalEntryStatus::Posted),
            ..Default::default()
        })
        .expect("list closing entries");
    assert_eq!(posted_closings.len(), 1, "exactly one closing entry stands");
}
