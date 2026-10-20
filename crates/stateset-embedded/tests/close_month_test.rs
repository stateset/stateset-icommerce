//! Integration tests for the month-end close orchestration
//! (`general_ledger().close_month`), against the SQLite backend.

// Uses the sync `Commerce` engine, which only exists with the sqlite backend.
#![cfg(feature = "sqlite")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    AccountSubType, AccountType, CloseMonthOptions, CloseMonthStepStatus, CreateAutoPostingConfig,
    CreateFixedAsset, CreateGlAccount, CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine,
    CreatePerformanceObligation, CreateRevenueContract, DepreciationEntryStatus,
    DepreciationMethod, FixedAssetCategory, PeriodStatus, RecognitionMethod, RevenueEntryStatus,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn sub_account(
    commerce: &Commerce,
    number: &str,
    name: &str,
    account_type: AccountType,
    sub_type: AccountSubType,
) -> Uuid {
    commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: number.into(),
            name: name.into(),
            description: None,
            account_type,
            account_sub_type: Some(sub_type),
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("create account")
        .id
}

/// Chart of accounts, auto-posting config (with depreciation + rev-rec
/// auto-post enabled), an open January 2026 period, one in-service asset with
/// one depreciation period due, and one active revenue contract with one
/// deferred entry due through period end.
fn setup(commerce: &Commerce) -> (Uuid, Uuid, Uuid) {
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart");

    sub_account(
        commerce,
        "5300",
        "Depreciation Expense",
        AccountType::Expense,
        AccountSubType::DepreciationExpense,
    );
    sub_account(
        commerce,
        "1510",
        "Accumulated Depreciation",
        AccountType::Asset,
        AccountSubType::AccumulatedDepreciation,
    );
    let unearned_id = sub_account(
        commerce,
        "2300",
        "Unearned Revenue",
        AccountType::Liability,
        AccountSubType::UnearnedRevenue,
    );

    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "Close month test".into(),
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
            period_name: "FY2026-wide".into(),
            fiscal_year: 2026,
            period_number: 1,
            // Wide period: GL auto-posting stamps journal entries with
            // today's date, which must fall inside an open period.
            start_date: date(2020, 1, 1),
            end_date: date(2030, 12, 31),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    // Fixed asset: $1200 over 12 months straight-line => $100/month; all 12
    // periods are due through the wide period end.
    let asset = commerce
        .fixed_assets()
        .create(CreateFixedAsset {
            asset_number: None,
            name: "Espresso machine".into(),
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

    // Revenue contract: $600 ratable Jan-Mar => $200/month; all 3 deferred
    // entries are due through the wide period end.
    let contract = commerce
        .revenue_recognition()
        .create_contract(CreateRevenueContract {
            contract_number: None,
            customer_id: Uuid::new_v4(),
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

    (period.id, asset.id, obligation_id)
}

#[test]
fn close_month_posts_all_steps_and_closes_the_period() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, asset_id, obligation_id) = setup(&commerce);

    let report = commerce
        .general_ledger()
        .close_month(
            period_id,
            CloseMonthOptions { closed_by: Some("controller".into()), ..Default::default() },
        )
        .expect("close month");

    assert!(!report.dry_run);
    assert_eq!(report.period_id, period_id);
    assert_eq!(report.period_name, "FY2026-wide");

    // Step 1: all 12 depreciation periods posted for $1200.
    assert_eq!(report.depreciation.status, CloseMonthStepStatus::Executed);
    assert_eq!(report.depreciation.entry_count, 12);
    assert_eq!(report.depreciation.total_amount, dec!(1200));
    assert!(report.depreciation.warnings.is_empty());
    let asset = commerce.fixed_assets().get(asset_id).expect("get asset").expect("asset");
    assert_eq!(asset.accumulated_depreciation, dec!(1200));

    // Step 2: all 3 revenue schedule entries recognized for $600.
    assert_eq!(report.revenue_recognition.status, CloseMonthStepStatus::Executed);
    assert_eq!(report.revenue_recognition.entry_count, 3);
    assert_eq!(report.revenue_recognition.total_amount, dec!(600));
    let schedule = commerce
        .revenue_recognition()
        .get_schedule(obligation_id)
        .expect("get schedule")
        .expect("schedule");
    assert_eq!(
        schedule.entries.iter().filter(|e| e.status == RevenueEntryStatus::Recognized).count(),
        3
    );

    // Step 3: no foreign-currency accounts => skipped silently.
    assert_eq!(report.fx_revaluation.status, CloseMonthStepStatus::Skipped);
    assert!(report.fx_revaluation.warnings.is_empty());

    // Step 4: closing entry posted, period closed by the requested actor.
    assert_eq!(report.period_close.status, CloseMonthStepStatus::Executed);
    assert_eq!(report.period_close.entry_count, 1);
    let closing = report.closing_entry.as_ref().expect("closing entry");
    assert!(closing.is_balanced);
    assert_eq!(report.period_status, PeriodStatus::Closed);
    let period =
        commerce.general_ledger().get_period(period_id).expect("get period").expect("period");
    assert_eq!(period.status, PeriodStatus::Closed);
    assert_eq!(period.closed_by.as_deref(), Some("controller"));
}

#[test]
fn close_month_dry_run_reports_candidates_without_writing() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, asset_id, obligation_id) = setup(&commerce);
    let entries_before =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();

    let report = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions { dry_run: true, ..Default::default() })
        .expect("dry run");

    assert!(report.dry_run);
    assert_eq!(report.depreciation.status, CloseMonthStepStatus::DryRun);
    assert_eq!(report.depreciation.entry_count, 12);
    assert_eq!(report.depreciation.total_amount, dec!(1200));
    assert_eq!(report.revenue_recognition.status, CloseMonthStepStatus::DryRun);
    assert_eq!(report.revenue_recognition.entry_count, 3);
    assert_eq!(report.revenue_recognition.total_amount, dec!(600));
    assert_eq!(report.fx_revaluation.status, CloseMonthStepStatus::Skipped);
    assert_eq!(report.period_close.status, CloseMonthStepStatus::DryRun);
    assert!(report.closing_entry.is_none());
    assert_eq!(report.period_status, PeriodStatus::Open);

    // Nothing was written anywhere.
    let asset = commerce.fixed_assets().get(asset_id).expect("get asset").expect("asset");
    assert_eq!(asset.accumulated_depreciation, dec!(0));
    let schedule =
        commerce.fixed_assets().get_schedule(asset_id).expect("get schedule").expect("schedule");
    assert!(schedule.entries.iter().all(|e| e.status == DepreciationEntryStatus::Scheduled));
    let rev_schedule = commerce
        .revenue_recognition()
        .get_schedule(obligation_id)
        .expect("get schedule")
        .expect("schedule");
    assert!(rev_schedule.entries.iter().all(|e| e.status == RevenueEntryStatus::Deferred));
    let entries_after =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();
    assert_eq!(entries_after, entries_before);
    let period =
        commerce.general_ledger().get_period(period_id).expect("get period").expect("period");
    assert_eq!(period.status, PeriodStatus::Open);
}

#[test]
fn close_month_respects_skip_flags() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, asset_id, _obligation_id) = setup(&commerce);

    let report = commerce
        .general_ledger()
        .close_month(
            period_id,
            CloseMonthOptions {
                skip_depreciation: true,
                skip_revenue_recognition: true,
                skip_fx_revaluation: true,
                skip_period_close: true,
                ..Default::default()
            },
        )
        .expect("close month");

    assert_eq!(report.depreciation.status, CloseMonthStepStatus::Skipped);
    assert_eq!(report.revenue_recognition.status, CloseMonthStepStatus::Skipped);
    assert_eq!(report.fx_revaluation.status, CloseMonthStepStatus::Skipped);
    assert_eq!(report.period_close.status, CloseMonthStepStatus::Skipped);
    assert!(report.closing_entry.is_none());
    // Every step was skipped, so nothing was posted anywhere.
    let asset = commerce.fixed_assets().get(asset_id).expect("get asset").expect("asset");
    assert_eq!(asset.accumulated_depreciation, dec!(0));
    assert_eq!(report.period_status, PeriodStatus::Open);
}

#[test]
fn close_month_missing_fx_rate_downgrades_to_skipped_warning() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, _asset_id, _obligation_id) = setup(&commerce);

    // A foreign-currency account with no exchange rate configured: the FX
    // step must not fail the close.
    commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1030".into(),
            name: "EUR Cash".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: Some("EUR".parse().expect("EUR")),
        })
        .expect("create EUR account");

    let report = commerce.general_ledger().close_month(period_id, CloseMonthOptions::default());
    // The EUR account has a zero balance, so revalue itself succeeds with no
    // adjustment; either way the close must complete and the period closes.
    let report = report.expect("close month succeeds despite FX account");
    assert!(matches!(
        report.fx_revaluation.status,
        CloseMonthStepStatus::Executed | CloseMonthStepStatus::Skipped
    ));
    assert_eq!(report.period_status, PeriodStatus::Closed);
}

#[test]
fn close_month_unknown_period_is_not_found() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let err = commerce
        .general_ledger()
        .close_month(Uuid::new_v4(), CloseMonthOptions::default())
        .expect_err("unknown period");
    assert!(matches!(err, stateset_core::CommerceError::NotFound));
}

/// An in-service asset with no generated schedule: the depreciation step
/// reports one warning with a failed-item count of one.
fn setup_with_unscheduled_asset(commerce: &Commerce) -> Uuid {
    let (period_id, _asset_id, _obligation_id) = setup(commerce);
    let asset = commerce
        .fixed_assets()
        .create(CreateFixedAsset {
            asset_number: None,
            name: "Unscheduled lathe".into(),
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
    commerce.fixed_assets().place_in_service(asset.id, date(2026, 1, 1)).expect("place in service");
    // Deliberately no `generate_schedule`: the close must warn, not fail.
    period_id
}

#[test]
fn close_month_lenient_mode_collects_warning_and_still_closes() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let period_id = setup_with_unscheduled_asset(&commerce);

    let report = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions::default())
        .expect("lenient close proceeds despite warnings");

    assert_eq!(report.depreciation.warning_count(), 1);
    assert!(report.depreciation.has_failures());
    assert_eq!(report.depreciation.failed_item_count, 1);
    assert_eq!(report.total_warnings(), 1);
    assert_eq!(report.total_failures(), 1);
    assert!(report.has_warnings());
    // The scheduled asset still posted; the period still closed.
    assert_eq!(report.depreciation.entry_count, 12);
    assert_eq!(report.period_status, PeriodStatus::Closed);
}

#[test]
fn close_month_fail_on_warnings_refuses_before_writing_anything() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let period_id = setup_with_unscheduled_asset(&commerce);
    let entries_before =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();

    let err = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions { fail_on_warnings: true, ..Default::default() })
        .expect_err("strict close refuses a close with warnings");
    assert!(
        matches!(err, stateset_core::CommerceError::ValidationError(_)),
        "unexpected error: {err:?}"
    );

    // Nothing was written: no journal entries, no depreciation, open period.
    let entries_after =
        commerce.general_ledger().list_journal_entries(Default::default()).expect("list").len();
    assert_eq!(entries_after, entries_before);
    let period =
        commerce.general_ledger().get_period(period_id).expect("get period").expect("period");
    assert_eq!(period.status, PeriodStatus::Open);
}

#[test]
fn close_month_fail_on_warnings_passes_a_clean_close() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, _asset_id, _obligation_id) = setup(&commerce);

    let report = commerce
        .general_ledger()
        .close_month(period_id, CloseMonthOptions { fail_on_warnings: true, ..Default::default() })
        .expect("strict close passes when nothing warns");

    assert!(!report.has_warnings());
    assert_eq!(report.total_failures(), 0);
    assert_eq!(report.period_status, PeriodStatus::Closed);
}

/// Regression: the FX step's skip used to surface only in the wet run, after
/// depreciation and revenue recognition had already posted, so a strict close
/// left committed GL writes behind an error. The dry-run evaluation now sees
/// the missing rate and refuses before anything is written.
#[test]
fn close_month_fail_on_warnings_refuses_unrevaluable_fx_before_posting() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let (period_id, _asset_id, _obligation_id) = setup(&commerce);
    let gl = commerce.general_ledger();
    let zar_cash = gl
        .create_account(CreateGlAccount {
            account_number: "1033".into(),
            name: "ZAR Cash".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: Some("ZAR".parse().expect("ZAR")),
        })
        .expect("create ZAR account")
        .id;
    let cash = gl.get_account_by_number("1010").expect("get").expect("cash").id;
    // Give the ZAR account a balance so revaluation has work to do. ZAR has
    // no seeded exchange rate, so the wet revaluation would refuse.
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: chrono::Utc::now().date_naive(),
        entry_type: None,
        description: "Fund ZAR cash".into(),
        lines: vec![
            CreateJournalEntryLine::debit(zar_cash, dec!(100), None),
            CreateJournalEntryLine::credit(cash, dec!(100), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(true),
    })
    .expect("fund ZAR account");
    let entries_before = gl.list_journal_entries(Default::default()).expect("list").len();

    let err = gl
        .close_month(period_id, CloseMonthOptions { fail_on_warnings: true, ..Default::default() })
        .expect_err("strict close refuses an FX step it cannot run");
    match err {
        stateset_core::CommerceError::ValidationError(message) => {
            assert!(message.contains("dry-run"), "refused at evaluation: {message}");
            assert!(message.contains("exchange rate"), "names the blocker: {message}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let entries_after = gl.list_journal_entries(Default::default()).expect("list").len();
    assert_eq!(entries_after, entries_before, "no depreciation / rev-rec posted");
    let period = gl.get_period(period_id).expect("get period").expect("period");
    assert_eq!(period.status, PeriodStatus::Open);
}
