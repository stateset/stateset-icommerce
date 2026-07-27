//! Postgres side of the income-statement parity guard.
//!
//! SQLite's `get_income_statement` was broken (raw SQL `SUM()` over TEXT money
//! read back as a `String`); Postgres, with `NUMERIC` columns, always worked.
//! This asserts the exact revenue total the SQLite backend now matches (see
//! `crates/stateset-db/tests/gl_income_statement.rs`), guarding against the two
//! backends drifting apart again.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine};
use stateset_embedded::AsyncCommerce;

mod common;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_income_statement_sums_revenue_exactly() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();

    gl.initialize_chart_of_accounts().await.expect("init chart of accounts");

    let start = NaiveDate::from_ymd_opt(2098, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2098, 1, 31).unwrap();
    let _period = common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: "2098-01".into(),
            fiscal_year: 2098,
            period_number: 1,
            start_date: start,
            end_date: end,
        },
    )
    .await;

    let cash =
        gl.get_account_by_number("1010").await.expect("query cash").expect("cash account exists");
    let revenue = gl
        .get_account_by_number("4010")
        .await
        .expect("query revenue")
        .expect("sales revenue account exists");

    // Delta-based assertions: the shared parity database persists across local
    // reruns, so prior runs' entries may already be in the 2098-01 range.
    let before = gl.get_income_statement(start, end).await.expect("income statement (before)");

    let entry_date = NaiveDate::from_ymd_opt(2098, 1, 15).unwrap();
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
            .await
            .expect("create journal entry");
        gl.post_journal_entry(entry.id, "tester").await.expect("post journal entry");
    }

    let statement = gl.get_income_statement(start, end).await.expect("income statement");
    assert_eq!(statement.total_revenue - before.total_revenue, dec!(0.30));
    assert_eq!(statement.total_expenses - before.total_expenses, dec!(0));
    assert_eq!(statement.net_income - before.net_income, dec!(0.30));
}
