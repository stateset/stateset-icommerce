#![cfg(feature = "sqlite")]

//! SQLite side of the general-ledger list ordering parity (see
//! `postgres_gl_list_ordering`). SQLite already orders `list_periods` by
//! `(fiscal_year, period_number) DESC` and `list_journal_entries` by
//! `(entry_date, entry_number) DESC`; Postgres was aligned to match. These lock in
//! the SQLite behavior so the two backends stay in agreement.

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, GeneralLedgerRepository,
    GlPeriodFilter, JournalEntryFilter,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_list_periods_orders_by_fiscal_year_and_period_number() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();

    // Period 1 starts LATER than period 2, so start_date order and
    // (fiscal_year, period_number) order disagree.
    gl.create_period(CreateGlPeriod {
        period_name: "p1".into(),
        fiscal_year: 2099,
        period_number: 1,
        start_date: NaiveDate::from_ymd_opt(2099, 6, 1).unwrap(),
        end_date: NaiveDate::from_ymd_opt(2099, 6, 30).unwrap(),
    })
    .expect("create period 1");
    gl.create_period(CreateGlPeriod {
        period_name: "p2".into(),
        fiscal_year: 2099,
        period_number: 2,
        start_date: NaiveDate::from_ymd_opt(2099, 1, 1).unwrap(),
        end_date: NaiveDate::from_ymd_opt(2099, 1, 31).unwrap(),
    })
    .expect("create period 2");

    let periods = gl
        .list_periods(GlPeriodFilter { fiscal_year: Some(2099), ..Default::default() })
        .expect("list periods");
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0].period_number, 2, "highest period_number sorts first");
    assert_eq!(periods[1].period_number, 1);
}

#[test]
fn sqlite_list_journal_entries_break_ties_by_entry_number() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart");

    let cash = gl.get_account_by_number("1010").unwrap().unwrap().id;
    let revenue = gl.get_account_by_number("4010").unwrap().unwrap().id;

    let entry_date = NaiveDate::from_ymd_opt(2099, 3, 10).unwrap();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "je".into(),
            fiscal_year: 2099,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2099, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2099, 3, 31).unwrap(),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    for i in 0..3 {
        gl.create_journal_entry(CreateJournalEntry {
            entry_date,
            entry_type: None,
            description: format!("entry-{i}"),
            lines: vec![
                CreateJournalEntryLine::debit(cash, dec!(1), None),
                CreateJournalEntryLine::credit(revenue, dec!(1), None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(false),
        })
        .expect("create entry");
    }

    let entries = gl.list_journal_entries(JournalEntryFilter::default()).expect("list entries");
    assert_eq!(entries.len(), 3);
    for pair in entries.windows(2) {
        assert!(
            pair[0].entry_number >= pair[1].entry_number,
            "same-date entries must be ordered by entry_number DESC"
        );
    }
}
