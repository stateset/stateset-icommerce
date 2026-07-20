//! Postgres parity for `list_journal_entries` `account_id` / `search` filters.
//!
//! SQLite dropped both; Postgres joins `gl_journal_entry_lines` for `account_id`
//! (`SELECT DISTINCT`) and `ILIKE`s the entry number/description for `search`. This
//! locks in that behavior so the two backends agree.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, JournalEntryFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_list_journal_entries_applies_account_and_search_filters() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping journal-entry filter test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().await.expect("init chart of accounts");

    let acct = |number: &'static str| {
        let gl = &gl;
        async move {
            gl.get_account_by_number(number)
                .await
                .expect("query account")
                .unwrap_or_else(|| panic!("account {number} exists"))
                .id
        }
    };
    let cash = acct("1010").await;
    let ar = acct("1100").await;
    let revenue = acct("4010").await;

    let entry_date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-03".into(),
            fiscal_year: 2026,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        })
        .await
        .expect("create period");
    gl.open_period(period.id).await.expect("open period");

    // Use a unique description token so the search assertion is isolated on a
    // shared database.
    let token = uuid::Uuid::new_v4().simple().to_string();
    let alpha_desc = format!("Alpha-{token}");

    gl.create_journal_entry(CreateJournalEntry {
        entry_date,
        entry_type: None,
        description: alpha_desc.clone(),
        lines: vec![
            CreateJournalEntryLine::debit(cash, dec!(100), None),
            CreateJournalEntryLine::credit(revenue, dec!(100), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .await
    .expect("create entry A");
    gl.create_journal_entry(CreateJournalEntry {
        entry_date,
        entry_type: None,
        description: format!("Beta-{token}"),
        lines: vec![
            CreateJournalEntryLine::debit(cash, dec!(50), None),
            CreateJournalEntryLine::credit(ar, dec!(50), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .await
    .expect("create entry B");

    // account_id: revenue only in A, AR only in B — each returns exactly one entry,
    // with no duplicate rows from the lines join.
    let by_revenue = gl
        .list_journal_entries(JournalEntryFilter {
            account_id: Some(revenue),
            search: Some(token.clone()),
            ..Default::default()
        })
        .await
        .expect("list by revenue");
    assert_eq!(by_revenue.len(), 1, "account_id must return only entries posting to it");
    assert_eq!(by_revenue[0].description, alpha_desc);

    let by_ar = gl
        .list_journal_entries(JournalEntryFilter {
            account_id: Some(ar),
            search: Some(token.clone()),
            ..Default::default()
        })
        .await
        .expect("list by ar");
    assert_eq!(by_ar.len(), 1);

    // search alone (scoped by the unique token) returns both entries.
    let both = gl
        .list_journal_entries(JournalEntryFilter { search: Some(token), ..Default::default() })
        .await
        .expect("list by search");
    assert_eq!(both.len(), 2, "search must match both entries by their shared token");
}
