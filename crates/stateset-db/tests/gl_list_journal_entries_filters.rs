#![cfg(feature = "sqlite")]

//! Regression: SQLite `list_journal_entries` dropped the `account_id` and `search`
//! filters that Postgres applies. `account_id` selects entries that have a line
//! posting to that account (Postgres joins `gl_journal_entry_lines`); `search`
//! matches the entry number or description. Both are now applied, matching Postgres.

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, GeneralLedgerRepository,
    JournalEntryFilter,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_list_journal_entries_applies_account_and_search_filters() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };
    let cash = acct("1010");
    let ar = acct("1100");
    let revenue = acct("4010");

    let entry_date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
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

    // Entry A touches cash + revenue; entry B touches cash + AR.
    gl.create_journal_entry(CreateJournalEntry {
        entry_date,
        entry_type: None,
        description: "Alpha sale".into(),
        lines: vec![
            CreateJournalEntryLine::debit(cash, dec!(100), None),
            CreateJournalEntryLine::credit(revenue, dec!(100), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .expect("create entry A");
    gl.create_journal_entry(CreateJournalEntry {
        entry_date,
        entry_type: None,
        description: "Beta payment".into(),
        lines: vec![
            CreateJournalEntryLine::debit(cash, dec!(50), None),
            CreateJournalEntryLine::credit(ar, dec!(50), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .expect("create entry B");

    let list = |filter: JournalEntryFilter| gl.list_journal_entries(filter).expect("list");

    // Baseline.
    assert_eq!(list(JournalEntryFilter::default()).len(), 2);

    // account_id: revenue is only in A, AR only in B, cash in both.
    assert_eq!(
        list(JournalEntryFilter { account_id: Some(revenue), ..Default::default() }).len(),
        1,
        "account_id filter must return only entries posting to that account"
    );
    assert_eq!(list(JournalEntryFilter { account_id: Some(ar), ..Default::default() }).len(), 1);
    assert_eq!(
        list(JournalEntryFilter { account_id: Some(cash), ..Default::default() }).len(),
        2,
        "cash appears in both entries (no duplicate rows)"
    );

    // search matches entry description.
    let alpha = list(JournalEntryFilter { search: Some("Alpha".into()), ..Default::default() });
    assert_eq!(alpha.len(), 1, "search must filter by description");
    assert_eq!(alpha[0].description, "Alpha sale");
}
