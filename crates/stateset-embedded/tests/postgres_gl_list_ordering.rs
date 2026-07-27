//! Postgres parity for general-ledger list ordering.
//!
//! - `list_periods`: SQLite orders by `fiscal_year DESC, period_number DESC`;
//!   Postgres ordered by `start_date DESC`. These disagree when a lower-numbered
//!   period has a later start date. Postgres now matches SQLite's period-identity
//!   ordering (which is deterministic — `(fiscal_year, period_number)` is unique).
//! - `list_journal_entries`: SQLite breaks ties by `entry_number DESC`; Postgres had
//!   no tiebreak (non-deterministic for same-date entries). Postgres now adds the
//!   same tiebreak.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

mod common;

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, GlPeriodFilter, JournalEntryFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_list_periods_orders_by_fiscal_year_and_period_number() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping GL period ordering test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();

    // Use a distinctive fiscal year to isolate on a shared database. Period 1 has a
    // LATER start date than period 2, so ordering by start_date vs (fiscal_year,
    // period_number) disagree.
    let year = 2099;
    common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: "p1".into(),
            fiscal_year: year,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(year, 6, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(year, 6, 30).unwrap(),
        },
    )
    .await;
    common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: "p2".into(),
            fiscal_year: year,
            period_number: 2,
            start_date: NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(year, 1, 31).unwrap(),
        },
    )
    .await;

    let periods = gl
        .list_periods(GlPeriodFilter { fiscal_year: Some(year), ..Default::default() })
        .await
        .expect("list periods");
    assert_eq!(periods.len(), 2);
    assert_eq!(
        periods[0].period_number, 2,
        "highest period_number must sort first (not the later start_date)"
    );
    assert_eq!(periods[1].period_number, 1);
}

#[tokio::test]
async fn postgres_list_journal_entries_break_ties_by_entry_number() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping GL entry ordering test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().await.expect("init chart");

    let cash = gl.get_account_by_number("1010").await.unwrap().unwrap().id;
    let revenue = gl.get_account_by_number("4010").await.unwrap().unwrap().id;

    // Fiscal 2097, NOT 2099: the sibling test in this binary asserts that
    // fiscal-2099 contains exactly its own two periods, and both tests run
    // concurrently — sharing the year made that count race (2 vs 3).
    let entry_date = NaiveDate::from_ymd_opt(2097, 3, 10).unwrap();
    common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: "je".into(),
            fiscal_year: 2097,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2097, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2097, 3, 31).unwrap(),
        },
    )
    .await;

    // Several entries on the SAME date; entry_number is the only tiebreaker.
    let token = uuid::Uuid::new_v4().simple().to_string();
    for i in 0..3 {
        gl.create_journal_entry(CreateJournalEntry {
            entry_date,
            entry_type: None,
            description: format!("{token}-{i}"),
            lines: vec![
                CreateJournalEntryLine::debit(cash, dec!(1), None),
                CreateJournalEntryLine::credit(revenue, dec!(1), None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(false),
        })
        .await
        .expect("create entry");
    }

    let entries = gl
        .list_journal_entries(JournalEntryFilter { search: Some(token), ..Default::default() })
        .await
        .expect("list entries");
    assert_eq!(entries.len(), 3);
    // Deterministic: same date, so ordered by entry_number DESC (non-increasing).
    for pair in entries.windows(2) {
        assert!(
            pair[0].entry_number >= pair[1].entry_number,
            "entries with equal dates must be ordered by entry_number DESC: {} then {}",
            pair[0].entry_number,
            pair[1].entry_number
        );
    }
}
