use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn asset_accounting(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let date = |month, day| NaiveDate::from_ymd_opt(2025, month, day).unwrap();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "Assets 2025".into(),
            fiscal_year: 2025,
            period_number: 1,
            start_date: date(1, 1),
            end_date: date(12, 31),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();
    let make_account = |number: &str, name: &str, kind, sub_type| {
        gl.create_account(CreateGlAccount {
            account_number: number.into(),
            name: name.into(),
            account_type: kind,
            account_sub_type: Some(sub_type),
            description: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap()
        .id
    };
    let capital = make_account("1501", "Equipment", AccountType::Asset, AccountSubType::FixedAsset);
    let accumulated = make_account(
        "1502",
        "Accumulated depreciation",
        AccountType::Asset,
        AccountSubType::AccumulatedDepreciation,
    );
    let expense = make_account(
        "5101",
        "Depreciation",
        AccountType::Expense,
        AccountSubType::DepreciationExpense,
    );
    let gain_loss = make_account(
        "5102",
        "Disposal gain/loss",
        AccountType::Expense,
        AccountSubType::OtherExpense,
    );
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "Assets".into(),
        cash_account_id: account("1010"),
        accounts_receivable_account_id: account("1100"),
        inventory_account_id: account("1200"),
        accounts_payable_account_id: account("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: account("4010"),
        shipping_revenue_account_id: None,
        sales_tax_payable_account_id: None,
        goods_received_not_invoiced_account_id: None,
        inventory_adjustment_account_id: None,
        work_in_progress_account_id: None,
        cogs_account_id: account("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: true,
        auto_post_revenue_recognition: false,
    })
    .unwrap();
    let input = CreateFixedAsset {
        asset_number: None,
        name: "Machine".into(),
        description: None,
        category: FixedAssetCategory::Machinery,
        acquisition_date: date(1, 1),
        acquisition_cost: dec!(1200),
        salvage_value: dec!(0),
        useful_life_months: 12,
        depreciation_method: DepreciationMethod::StraightLine,
        in_service_date: Some(date(1, 1)),
        location_id: None,
        asset_account_id: Some(capital),
        accumulated_depreciation_account_id: Some(accumulated),
        depreciation_expense_account_id: Some(expense),
        acquisition_offset_account_id: None,
        disposal_gain_loss_account_id: None,
        currency: None,
    };
    let assets = db.fixed_assets();
    let asset = assets.create(input.clone()).unwrap();
    let balance = |id| gl.get_account(id).unwrap().unwrap().current_balance;
    assert_eq!(balance(capital), dec!(1200));
    assert_eq!(balance(account("1010")), dec!(-1200));
    assets.generate_schedule(asset.id).unwrap();
    assets.post_depreciation(asset.id, 2).unwrap();
    let original = assets.get_schedule(asset.id).unwrap().unwrap();
    assert_eq!(original.entries[1].accumulated, dec!(200));
    assets
        .update(
            asset.id,
            UpdateFixedAsset {
                useful_life_months: Some(22),
                salvage_value: Some(dec!(100)),
                ..Default::default()
            },
        )
        .unwrap();
    let revised = assets.get_schedule(asset.id).unwrap().unwrap();
    assert_eq!(revised.entries.len(), 22);
    assert_eq!(revised.entries[0].amount, original.entries[0].amount);
    assert_eq!(revised.entries[1].accumulated, original.entries[1].accumulated);
    assert_eq!(revised.entries[2].amount, dec!(45));
    assert_eq!(revised.entries[21].book_value, dec!(100));
    assert!(
        assets
            .update(
                asset.id,
                UpdateFixedAsset { in_service_date: Some(date(2, 1)), ..Default::default() }
            )
            .is_err()
    );
    assert!(
        assets
            .update(
                asset.id,
                UpdateFixedAsset { salvage_value: Some(dec!(1100)), ..Default::default() }
            )
            .is_err()
    );
    assert!(
        assets
            .update(
                asset.id,
                UpdateFixedAsset { asset_account_id: Some(account("1010")), ..Default::default() }
            )
            .is_err()
    );
    assets.post_depreciation(asset.id, 1).unwrap();
    assert_eq!(assets.get(asset.id).unwrap().unwrap().accumulated_depreciation, dec!(245));
    assert!(
        assets.dispose(asset.id, date(4, 30), dec!(1000), None).is_err(),
        "Missing gain/loss mapping must roll back disposal"
    );
    assert_eq!(assets.get(asset.id).unwrap().unwrap().status, FixedAssetStatus::InService);
    assert_eq!(balance(capital), dec!(1200));
    assets
        .update(
            asset.id,
            UpdateFixedAsset {
                disposal_gain_loss_account_id: Some(gain_loss),
                ..Default::default()
            },
        )
        .unwrap();
    let disposed = assets.dispose(asset.id, date(4, 30), dec!(1000), None).unwrap();
    assert_eq!(disposed.disposal.unwrap().gain_loss, dec!(45));
    assert_eq!(balance(capital), dec!(0));
    assert_eq!(balance(accumulated), dec!(0));
    assert_eq!(balance(expense), dec!(245));
    assert_eq!(balance(gain_loss), dec!(-45));
    assert_eq!(balance(account("1010")), dec!(-200));
    assert!(assets.dispose(asset.id, date(4, 30), dec!(1000), None).is_err());
    let damaged = assets
        .create(CreateFixedAsset {
            disposal_gain_loss_account_id: Some(gain_loss),
            ..input.clone()
        })
        .unwrap();
    assets.write_off(damaged.id, date(2, 28), Some("Damaged".into())).unwrap();
    assert_eq!(balance(capital), dec!(0));
    assert_eq!(balance(gain_loss), dec!(1155));
    let cash_before = balance(account("1010"));
    let gain_before = balance(gain_loss);
    let foreign = assets
        .create(CreateFixedAsset {
            currency: Some(CurrencyCode::EUR),
            disposal_gain_loss_account_id: Some(gain_loss),
            ..input.clone()
        })
        .unwrap();
    assert_eq!(balance(capital), dec!(1440));
    assets.dispose(foreign.id, date(4, 30), dec!(1200), None).unwrap();
    assert_eq!(balance(capital), dec!(0), "Asset cost leaves at its acquisition exchange rate");
    assert_eq!(
        balance(account("1010")) - cash_before,
        dec!(360),
        "Functional-currency cash uses each settlement date's rate"
    );
    assert_eq!(balance(gain_loss) - gain_before, dec!(-360));
    let foreign_entries = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_id: Some(foreign.id),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(foreign_entries.len(), 3);
    assert!(
        foreign_entries
            .iter()
            .flat_map(|entry| &entry.lines)
            .filter(|line| line.account_id == account("1010"))
            .all(|line| line.transaction_currency == Some(CurrencyCode::USD)
                && line.exchange_rate == Some(dec!(1)))
    );
    assert!(gl.get_trial_balance(date(12, 31)).unwrap().is_balanced());
    let count = assets.list(FixedAssetFilter::default()).unwrap().len();
    gl.close_period(period.id, "test").unwrap();
    assert!(assets.create(input).is_err(), "Acquisition journal failure must roll back the asset");
    assert_eq!(assets.list(FixedAssetFilter::default()).unwrap().len(), count);
}
