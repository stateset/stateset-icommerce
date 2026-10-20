//! Month-end close against PostgreSQL: backend-parity proof for the close
//! orchestration (`general_ledger().close_month`), including strict
//! (`fail_on_warnings`) mode.
//!
//! Requires the `postgres` feature and a live database:
//! `POSTGRES_URL` (or `DATABASE_URL`). Without one the tests print a notice
//! and pass, exactly like `crates/stateset-saga/tests/postgres_saga.rs`.
//!
//! The shared database is safe: every fixture uses unique account numbers,
//! config names, period names and fiscal years, and auto-numbered assets and
//! contracts, so parallel tests (here or in other suites) never collide.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    AccountSubType, AccountType, CloseMonthOptions, CreateAutoPostingConfig, CreateFixedAsset,
    CreateGlAccount, CreateGlPeriod, CreatePerformanceObligation, CreateRevenueContract,
    DepreciationMethod, FixedAssetCategory, PeriodStatus, RecognitionMethod,
};
use stateset_embedded::Commerce;
use std::env;
use std::sync::atomic::{AtomicU32, Ordering};

/// Distinct numeric account-number space per test (standard chart numbers
/// like `1010` come from the idempotent chart init and are shared).
static TAG: AtomicU32 = AtomicU32::new(0);

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

/// Connect to a FRESH database per test process (`{base}_pgclose_{pid}`),
/// so parallel suites, reruns and other developers sharing one Postgres
/// server never see each other's chart, periods or ledger rows. Returns
/// `None` (and the test passes vacuously) when no URL is configured.
fn pg_commerce() -> Option<Commerce> {
    let base = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok());
    let base = match base {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres close test");
            return None;
        }
    };
    // Fresh database per TEST (not per process): the tests in this file run
    // in parallel threads sharing one process, and concurrent migration runs
    // against the same database deadlock the pool. UUID-hex names never
    // collide across reruns or developers; stale databases are tiny and
    // safe to drop by hand.
    let db_name = format!("pgclose_{}", uuid::Uuid::new_v4().simple());
    assert!(
        db_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "generated database name must be identifier-safe"
    );
    let admin_url = admin_url(&base);
    stateset_embedded_block_on(async {
        use sqlx::Connection as _;
        let mut conn = sqlx::PgConnection::connect(&admin_url).await.expect("admin connect");
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(&db_name)
                .fetch_one(&mut conn)
                .await
                .expect("check database");
        if !exists {
            sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
                .execute(&mut conn)
                .await
                .expect("create database");
        }
        Ok::<(), stateset_core::CommerceError>(())
    })
    .expect("provision database");
    Some(Commerce::with_postgres(&with_database(&base, &db_name)).expect("connect to postgres"))
}

/// Split `postgres://…/db?params` into (server part, `?params` or empty).
fn split_url(url: &str) -> (&str, &str) {
    let (without_query, query) = match url.find('?') {
        // NB: `?` cannot appear before the path in a connection URL, so the
        // first `?` starts the query string.
        Some(q) => (&url[..q], &url[q..]),
        None => (url, ""),
    };
    let (server, _) = without_query.rsplit_once('/').expect("url has database path");
    (server, query)
}

/// The same server, `postgres` maintenance database.
fn admin_url(url: &str) -> String {
    let (server, _) = split_url(url);
    format!("{server}/postgres")
}

/// The same server and query string, different database.
fn with_database(url: &str, db: &str) -> String {
    let (server, query) = split_url(url);
    format!("{server}/{db}{query}")
}

fn stateset_embedded_block_on<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(f)
}

/// Get-or-create: reruns against the same database reuse the accounts from
/// the previous run instead of failing on duplicates.
fn sub_account(
    commerce: &Commerce,
    number: String,
    name: &str,
    ty: AccountType,
    sub_type: AccountSubType,
) -> uuid::Uuid {
    let gl = commerce.general_ledger();
    if let Some(existing) = gl.get_account_by_number(&number).expect("get account") {
        return existing.id;
    }
    gl.create_account(CreateGlAccount {
        account_number: number,
        name: name.into(),
        description: None,
        account_type: ty,
        account_sub_type: Some(sub_type),
        parent_account_id: None,
        is_header: None,
        is_posting: Some(true),
        currency: None,
    })
    .expect("create account")
    .id
}

/// Full close fixture on a unique fiscal year: chart (idempotent), posting
/// config, open wide period, one scheduled asset and one active contract.
/// Returns `(period_id, tag)`.
fn setup(commerce: &Commerce, fiscal_year: i32, label: &str) -> (uuid::Uuid, u32) {
    let tag = TAG.fetch_add(1, Ordering::SeqCst);
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart");

    let num = |base: u32| (base + tag).to_string();
    sub_account(
        commerce,
        num(5390),
        "Depreciation Expense",
        AccountType::Expense,
        AccountSubType::DepreciationExpense,
    );
    sub_account(
        commerce,
        num(1590),
        "Accumulated Depreciation",
        AccountType::Asset,
        AccountSubType::AccumulatedDepreciation,
    );
    let unearned_id = sub_account(
        commerce,
        num(2390),
        "Unearned Revenue",
        AccountType::Liability,
        AccountSubType::UnearnedRevenue,
    );

    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: format!("pg close {label} {tag}"),
        cash_account_id: by_number("1010"),
        accounts_receivable_account_id: by_number("1100"),
        inventory_account_id: by_number("1200"),
        accounts_payable_account_id: by_number("2010"),
        unearned_revenue_account_id: Some(unearned_id),
        sales_revenue_account_id: by_number("4010"),
        shipping_revenue_account_id: None,
        cogs_account_id: by_number("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: true,
        auto_post_revenue_recognition: true,
    })
    .expect("set auto posting config");

    let period = gl
        .create_period(CreateGlPeriod {
            period_name: format!("pg-close-{label}-{tag}"),
            fiscal_year,
            period_number: 1,
            start_date: date(2020, 1, 1),
            end_date: date(2030, 12, 31),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    let asset = commerce
        .fixed_assets()
        .create(CreateFixedAsset {
            asset_number: None,
            name: format!("pg press {label} {tag}"),
            description: None,
            category: FixedAssetCategory::Machinery,
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
        })
        .expect("create asset");
    let asset = commerce
        .fixed_assets()
        .place_in_service(asset.id, date(2026, 1, 1))
        .expect("place in service");
    commerce.fixed_assets().generate_schedule(asset.id).expect("generate schedule");

    let contract = commerce
        .revenue_recognition()
        .create_contract(CreateRevenueContract {
            contract_number: None,
            customer_id: uuid::Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(600),
            currency: None,
            effective_date: date(2026, 1, 1),
            obligations: vec![CreatePerformanceObligation {
                description: "Q1 support".into(),
                standalone_selling_price: None,
                allocated_amount: dec!(600),
                recognition_method: RecognitionMethod::RatableOverTime {
                    start: date(2026, 1, 1),
                    end: date(2026, 3, 31),
                },
            }],
        })
        .expect("create contract");
    commerce
        .revenue_recognition()
        .update_contract(
            contract.id,
            stateset_core::UpdateRevenueContract {
                status: Some(stateset_core::RevenueContractStatus::Active),
                ..Default::default()
            },
        )
        .expect("activate contract");
    let obligation_id = contract.obligations[0].id;
    commerce.revenue_recognition().generate_schedule(obligation_id).expect("generate schedule");

    (period.id, tag)
}

#[test]
fn pg_close_month_posts_all_steps_and_closes_the_period() {
    let Some(commerce) = pg_commerce() else { return };
    let (period_id, _tag) = setup(&commerce, 2097, "clean");

    let report = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions::default())
        .expect("close succeeds on postgres");

    // Same numbers as the SQLite twin (close_month_test.rs): 12 depreciation
    // periods ($100) and 3 recognized revenue entries ($600), exact decimals,
    // period sealed.
    assert_eq!(report.depreciation.entry_count, 12);
    assert_eq!(report.depreciation.total_amount, dec!(1200));
    assert_eq!(report.revenue_recognition.entry_count, 3);
    assert_eq!(report.revenue_recognition.total_amount, dec!(600));
    assert!(!report.has_warnings());
    assert_eq!(report.period_status, PeriodStatus::Closed);
}

#[test]
fn pg_close_month_fail_on_warnings_refuses_before_writing_anything() {
    let Some(commerce) = pg_commerce() else { return };
    let (period_id, _tag) = setup(&commerce, 2098, "strict");

    // In-service asset with deliberately no generated schedule.
    let unscheduled = commerce
        .fixed_assets()
        .create(CreateFixedAsset {
            asset_number: None,
            name: "pg unscheduled lathe".into(),
            description: None,
            category: FixedAssetCategory::Machinery,
            acquisition_date: date(2026, 1, 1),
            acquisition_cost: dec!(600),
            salvage_value: dec!(0),
            useful_life_months: 6,
            depreciation_method: DepreciationMethod::StraightLine,
            in_service_date: None,
            location_id: None,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            currency: None,
        })
        .expect("create asset");
    commerce
        .fixed_assets()
        .place_in_service(unscheduled.id, date(2026, 1, 1))
        .expect("place in service");

    let entries_before =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();
    let err = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions { fail_on_warnings: true, ..Default::default() })
        .expect_err("strict close refuses on postgres");
    assert!(
        matches!(err, stateset_core::CommerceError::ValidationError(_)),
        "unexpected error: {err:?}"
    );
    let entries_after =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();
    assert_eq!(entries_after, entries_before);

    // Lenient mode on the same period collects the warning and still closes.
    let report = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions::default())
        .expect("lenient close proceeds on postgres");
    assert_eq!(report.depreciation.failed_item_count, 1);
    assert_eq!(report.total_warnings(), 1);
    assert_eq!(report.period_status, PeriodStatus::Closed);
}
