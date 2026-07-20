#![cfg(feature = "sqlite")]

//! Regression: SQLite `get_income_statement` summed the TEXT `debit_amount` /
//! `credit_amount` columns with the built-in SQL `SUM()` and then read the
//! result back as a `String`. `SUM()` over a TEXT column returns a REAL/INTEGER,
//! so the `row.get::<String>` read fails at runtime — the income statement (and
//! `run_period_close`, which calls it) was broken on SQLite while Postgres, with
//! its `NUMERIC` columns, worked. The fix uses the exact `decimal_sum` aggregate
//! (which returns TEXT), so the report both parses and stays penny-exact.

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, GeneralLedgerRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_income_statement_sums_revenue_exactly() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();

    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-01".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: start,
            end_date: end,
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");
    let cash = gl.get_account_by_number("1010").expect("query cash").expect("cash account exists");
    let revenue = gl
        .get_account_by_number("4010")
        .expect("query revenue")
        .expect("sales revenue account exists");

    let entry_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();

    // Two revenue postings whose amounts drift under float SUM: 0.10 + 0.20.
    for amount in [dec!(0.10), dec!(0.20)] {
        let entry = gl
            .create_journal_entry(CreateJournalEntry {
                entry_date,
                entry_type: None,
                description: "revenue".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, amount, None),
                    CreateJournalEntryLine::credit(revenue.id, amount, None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .expect("create journal entry");
        gl.post_journal_entry(entry.id, "tester").expect("post journal entry");
    }

    let statement = gl.get_income_statement(start, end).expect("income statement must succeed");

    assert_eq!(statement.total_revenue, dec!(0.30), "revenue must be penny-exact");
    assert_eq!(statement.total_expenses, dec!(0));
    assert_eq!(statement.net_income, dec!(0.30));
}
