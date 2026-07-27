//! Postgres integration coverage for the fixed asset register: asset
//! lifecycle (draft → in-service → fully-depreciated), disposal with
//! gain/loss, depreciation schedule persistence, and GL auto-posting of
//! depreciation — all driven through the public `AsyncCommerce::fixed_assets()`
//! accessor.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

mod common;

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AccountSubType, AccountType, CreateAutoPostingConfig, CreateFixedAsset, CreateGlAccount,
    CreateGlPeriod, DepreciationEntryStatus, DepreciationMethod, FixedAssetCategory,
    FixedAssetFilter, FixedAssetStatus, JournalEntryFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn new_asset_input(name: &str) -> CreateFixedAsset {
    CreateFixedAsset {
        asset_number: None,
        name: name.into(),
        description: Some("pg integration asset".into()),
        category: FixedAssetCategory::Equipment,
        acquisition_date: date(2026, 1, 1),
        acquisition_cost: dec!(1200),
        salvage_value: dec!(0),
        useful_life_months: 12,
        depreciation_method: DepreciationMethod::StraightLine,
        in_service_date: None,
        location_id: None,
        asset_account_id: None,
        accumulated_depreciation_account_id: None,
        depreciation_expense_account_id: None,
        currency: None,
    }
}

#[tokio::test]
async fn postgres_fixed_asset_lifecycle_and_schedule_persistence() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping fixed asset lifecycle test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let assets = commerce.fixed_assets();

    let asset = assets.create(new_asset_input("PG Lathe")).await.expect("create asset");
    assert_eq!(asset.status, FixedAssetStatus::Draft);
    assert!(!asset.asset_number.is_empty(), "asset number must be generated");
    assert_eq!(asset.accumulated_depreciation, Decimal::ZERO);

    // Depreciation cannot be posted before the asset is in service.
    assert!(assets.post_depreciation(asset.id, 1).await.is_err());

    let asset =
        assets.place_in_service(asset.id, date(2026, 2, 1)).await.expect("place in service");
    assert_eq!(asset.status, FixedAssetStatus::InService);
    assert_eq!(asset.in_service_date, Some(date(2026, 2, 1)));

    // Generate and persist the depreciation schedule.
    let schedule = assets.generate_schedule(asset.id).await.expect("generate schedule");
    assert_eq!(schedule.entries.len(), 12);
    assert_eq!(schedule.total_depreciation, dec!(1200));
    assert!(schedule.entries.iter().all(|e| e.status == DepreciationEntryStatus::Scheduled));
    assert_eq!(schedule.entries[0].amount, dec!(100));

    // Schedule must survive a re-read from Postgres.
    let persisted =
        assets.get_schedule(asset.id).await.expect("get schedule").expect("schedule exists");
    assert_eq!(persisted.entries.len(), 12);
    assert_eq!(persisted.total_depreciation, dec!(1200));
    assert_eq!(persisted.entries[11].accumulated, dec!(1200));
    assert_eq!(persisted.entries[11].book_value, dec!(0));

    // Post three periods and verify accumulated depreciation.
    let asset = assets.post_depreciation(asset.id, 3).await.expect("post 3 periods");
    assert_eq!(asset.accumulated_depreciation, dec!(300));
    assert_eq!(asset.status, FixedAssetStatus::InService);

    let persisted =
        assets.get_schedule(asset.id).await.expect("get schedule").expect("schedule exists");
    let posted =
        persisted.entries.iter().filter(|e| e.status == DepreciationEntryStatus::Posted).count();
    assert_eq!(posted, 3, "first three entries must persist as posted");

    // Post the remainder: asset becomes fully depreciated.
    let asset = assets.post_depreciation(asset.id, 9).await.expect("post remaining periods");
    assert_eq!(asset.accumulated_depreciation, dec!(1200));
    assert_eq!(asset.status, FixedAssetStatus::FullyDepreciated);
    assert!(assets.post_depreciation(asset.id, 1).await.is_err(), "no entries left to post");

    // Listing by status must include the asset.
    let listed = assets
        .list(FixedAssetFilter {
            status: Some(FixedAssetStatus::FullyDepreciated),
            ..Default::default()
        })
        .await
        .expect("list assets");
    assert!(listed.iter().any(|a| a.id == asset.id));
}

#[tokio::test]
async fn postgres_fixed_asset_depreciation_auto_posts_journal_entry() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping depreciation auto-post test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();
    let assets = commerce.fixed_assets();

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

    // Dedicated depreciation accounts so the auto-post has explicit targets.
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let expense = gl
        .create_account(CreateGlAccount {
            account_number: format!("6D-{suffix}"),
            name: format!("Depreciation Expense {suffix}"),
            description: None,
            account_type: AccountType::Expense,
            account_sub_type: Some(AccountSubType::DepreciationExpense),
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .await
        .expect("create depreciation expense account");
    let accumulated = gl
        .create_account(CreateGlAccount {
            account_number: format!("1D-{suffix}"),
            name: format!("Accumulated Depreciation {suffix}"),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::AccumulatedDepreciation),
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .await
        .expect("create accumulated depreciation account");

    // An open GL period covering today is required to post the entry. The DB
    // may persist across runs, so tolerate an already-existing period.
    let today = Utc::now().date_naive();
    let start = date(today.year(), today.month(), 1);
    let end = if today.month() == 12 {
        date(today.year() + 1, 1, 1)
    } else {
        date(today.year(), today.month() + 1, 1)
    }
    .pred_opt()
    .expect("previous day");
    common::ensure_open_period(
        &gl,
        CreateGlPeriod {
            period_name: format!("{}-{:02}", today.year(), today.month()),
            fiscal_year: today.year(),
            period_number: today.month() as i32,
            start_date: start,
            end_date: end,
        },
    )
    .await;

    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "default".into(),
        cash_account_id: acct("1010").await,
        accounts_receivable_account_id: acct("1100").await,
        inventory_account_id: acct("1200").await,
        accounts_payable_account_id: acct("2010").await,
        unearned_revenue_account_id: None,
        sales_revenue_account_id: acct("4010").await,
        shipping_revenue_account_id: None,
        cogs_account_id: acct("5010").await,
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: true,
        auto_post_revenue_recognition: false,
    })
    .await
    .expect("set auto-posting config");

    let mut input = new_asset_input("PG Auto-Post Press");
    input.depreciation_expense_account_id = Some(expense.id);
    input.accumulated_depreciation_account_id = Some(accumulated.id);
    let asset = assets.create(input).await.expect("create asset");
    assets.place_in_service(asset.id, date(2026, 2, 1)).await.expect("place in service");
    assets.generate_schedule(asset.id).await.expect("generate schedule");

    let asset = assets.post_depreciation(asset.id, 2).await.expect("post depreciation");
    assert_eq!(asset.accumulated_depreciation, dec!(200));

    // A balanced journal entry must have been created for this asset.
    let entries = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("fixed_asset_depreciation".into()),
            source_document_id: Some(asset.id),
            ..Default::default()
        })
        .await
        .expect("list journal entries");
    assert_eq!(entries.len(), 1, "exactly one depreciation entry for this posting");
    let entry = &entries[0];
    assert_eq!(entry.total_debits, dec!(200));
    assert_eq!(entry.total_credits, dec!(200));

    let lines = gl.get_journal_entry_lines(entry.id).await.expect("entry lines");
    assert_eq!(lines.len(), 2);
    assert!(
        lines.iter().any(|l| l.account_id == expense.id && l.debit_amount == dec!(200)),
        "debit hits the asset's depreciation expense account"
    );
    assert!(
        lines.iter().any(|l| l.account_id == accumulated.id && l.credit_amount == dec!(200)),
        "credit hits the asset's accumulated depreciation account"
    );
}

#[tokio::test]
async fn postgres_fixed_asset_dispose_records_gain_loss() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping fixed asset dispose test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let assets = commerce.fixed_assets();

    let asset = assets.create(new_asset_input("PG Forklift")).await.expect("create asset");
    assets.place_in_service(asset.id, date(2026, 2, 1)).await.expect("place in service");
    assets.generate_schedule(asset.id).await.expect("generate schedule");

    // Depreciate 3 of 12 periods: accumulated = 300, book value = 900.
    let asset = assets.post_depreciation(asset.id, 3).await.expect("post 3 periods");
    assert_eq!(asset.accumulated_depreciation, dec!(300));
    assert_eq!(asset.book_value(), dec!(900));

    // Negative proceeds are rejected and leave the asset untouched.
    assert!(
        assets.dispose(asset.id, date(2026, 6, 1), dec!(-1), None).await.is_err(),
        "negative proceeds must be rejected"
    );

    // Dispose for 1000 against a 900 book value: gain of 100.
    let disposed = assets
        .dispose(asset.id, date(2026, 6, 1), dec!(1000), Some("sold at auction".into()))
        .await
        .expect("dispose asset");
    assert_eq!(disposed.status, FixedAssetStatus::Disposed);
    let disposal = disposed.disposal.expect("disposal details recorded");
    assert_eq!(disposal.disposal_date, date(2026, 6, 1));
    assert_eq!(disposal.proceeds, dec!(1000));
    assert_eq!(disposal.book_value_at_disposal, dec!(900));
    assert_eq!(disposal.gain_loss, dec!(100));
    assert_eq!(disposal.notes.as_deref(), Some("sold at auction"));

    // Disposal is terminal: no further transitions or depreciation.
    assert!(assets.post_depreciation(disposed.id, 1).await.is_err());
    assert!(assets.dispose(disposed.id, date(2026, 7, 1), dec!(1), None).await.is_err());

    // Round-trip: disposal details survive a re-read.
    let fetched = assets.get(disposed.id).await.expect("get").expect("asset exists");
    assert_eq!(fetched.status, FixedAssetStatus::Disposed);
    assert_eq!(fetched.disposal.expect("disposal persisted").gain_loss, dec!(100));
}
