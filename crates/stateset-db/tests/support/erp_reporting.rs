use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;
fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

pub(crate) fn periods_and_earnings(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |n| gl.get_account_by_number(n).unwrap().unwrap().id;
    let p = gl
        .create_period(CreateGlPeriod {
            period_name: "Fiscal year".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: date("2025-07-01"),
            end_date: date("2026-06-30"),
        })
        .unwrap();
    assert!(matches!(gl.close_period(p.id, "test"), Err(CommerceError::Conflict(_))));
    assert!(matches!(gl.lock_period(p.id, "test"), Err(CommerceError::Conflict(_))));
    assert!(matches!(gl.reopen_period(p.id), Err(CommerceError::Conflict(_))));
    assert!(matches!(gl.open_period(Uuid::new_v4()), Err(CommerceError::NotFound)));
    gl.open_period(p.id).unwrap();
    assert!(matches!(gl.open_period(p.id), Err(CommerceError::Conflict(_))));
    let contra = gl
        .create_account(CreateGlAccount {
            account_number: "1599".into(),
            name: "Accumulated depreciation".into(),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::AccumulatedDepreciation),
            parent_account_id: None,
            description: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap();
    let entry = |when, lines| {
        gl.create_journal_entry(CreateJournalEntry {
            entry_date: date(when),
            description: "Stored activity".into(),
            entry_type: None,
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(true),
            fx: None,
            lines,
        })
        .unwrap()
    };
    entry(
        "2025-08-01",
        vec![
            CreateJournalEntryLine::debit(account("1010"), dec!(100), None),
            CreateJournalEntryLine::credit(account("4010"), dec!(100), None),
        ],
    );
    entry(
        "2026-02-01",
        vec![
            CreateJournalEntryLine::debit(account("5010"), dec!(20), None),
            CreateJournalEntryLine::credit(contra.id, dec!(20), None),
        ],
    );
    let balance = gl.get_balance_sheet(date("2026-06-30")).unwrap();
    assert_eq!(balance.total_assets, dec!(80));
    assert_eq!(balance.total_equity, dec!(80));
    assert_eq!(balance.total_assets, balance.total_equity + balance.total_liabilities);
    assert_eq!(
        balance
            .equity
            .iter()
            .find(|l| l.account_name == "Current-year unclosed earnings")
            .unwrap()
            .balance,
        dec!(80)
    );
    assert_eq!(
        balance.assets.iter().find(|l| l.account_id == contra.id).unwrap().balance,
        dec!(-20)
    );
    gl.close_period(p.id, "test").unwrap();
    assert!(matches!(gl.close_period(p.id, "test"), Err(CommerceError::Conflict(_))));
    gl.reopen_period(p.id).unwrap();
    assert!(matches!(gl.reopen_period(p.id), Err(CommerceError::Conflict(_))));
    gl.run_period_close(p.id, "test").unwrap();
    let closed = gl.get_balance_sheet(p.end_date).unwrap();
    assert_eq!(closed.total_assets, dec!(80));
    assert_eq!(closed.total_equity, dec!(80));
    assert!(!closed.equity.iter().any(|l| l.account_name.ends_with("unclosed earnings")));
    gl.lock_period(p.id, "test").unwrap();
    assert!(matches!(gl.lock_period(p.id, "test"), Err(CommerceError::Conflict(_))));
    assert!(matches!(gl.reopen_period(p.id), Err(CommerceError::Conflict(_))));
}

pub(crate) fn schedule_precision(db: &dyn Database) {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("schedule-{}@example.com", Uuid::new_v4()),
            first_name: "A".into(),
            last_name: "B".into(),
            ..Default::default()
        })
        .unwrap();
    for (currency, amount, expected) in [
        (CurrencyCode::JPY, dec!(100), dec!(33)),
        ("KWD".parse().unwrap(), dec!(1), dec!(0.333)),
        (CurrencyCode::USD, dec!(1), dec!(0.33)),
    ] {
        let contract = db
            .revenue_recognition()
            .create_contract(CreateRevenueContract {
                contract_number: None,
                customer_id: customer.id.into(),
                order_id: None,
                invoice_id: None,
                transaction_price: amount,
                currency: Some(currency),
                effective_date: date("2026-01-01"),
                obligations: vec![CreatePerformanceObligation {
                    description: "Service".into(),
                    standalone_selling_price: None,
                    allocated_amount: amount,
                    recognition_method: RecognitionMethod::RatableOverTime {
                        start: date("2026-01-01"),
                        end: date("2026-03-31"),
                    },
                }],
            })
            .unwrap();
        let schedule =
            db.revenue_recognition().generate_schedule(contract.obligations[0].id).unwrap();
        assert_eq!(schedule.entries[0].amount, expected);
        assert_eq!(
            schedule.entries.iter().map(|e| e.amount).sum::<rust_decimal::Decimal>(),
            amount
        );
    }
}

pub(crate) fn stored_reports(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |n| gl.get_account_by_number(n).unwrap().unwrap().id;
    let p = gl
        .create_period(CreateGlPeriod {
            period_name: "Reports".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: date("2026-01-01"),
            end_date: date("2026-12-31"),
        })
        .unwrap();
    gl.open_period(p.id).unwrap();
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: date("2026-01-05"),
        entry_type: None,
        description: "Manual service revenue".into(),
        lines: vec![
            CreateJournalEntryLine::debit(account("1010"), dec!(100), None),
            CreateJournalEntryLine::credit(account("4010"), dec!(100), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(true),
        fx: None,
    })
    .unwrap();
    let report =
        db.cost_accounting().transaction_cogs(date("2026-01-01"), date("2026-01-31")).unwrap();
    assert_eq!(report.total_revenue, dec!(100));
    assert_eq!(report.rows[0].sku, "__unallocated__");
    assert_eq!(
        db.cost_accounting()
            .transaction_cogs(date("2026-02-01"), date("2026-02-28"))
            .unwrap()
            .total_revenue,
        dec!(0)
    );
    assert!(db.cost_accounting().close_books_check().unwrap().is_ready);
    gl.run_period_close(p.id, "test").unwrap();
    assert_eq!(
        db.cost_accounting().transaction_cogs(p.start_date, p.end_date).unwrap().total_revenue,
        dec!(100)
    );
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "UNVALUED".into(),
            name: "Unvalued".into(),
            initial_quantity: Some(dec!(2)),
            ..Default::default()
        })
        .unwrap();
    let readiness = db.cost_accounting().close_books_check().unwrap();
    assert!(!readiness.is_ready);
    assert_eq!(readiness.zero_valuation_items[0].sku, "UNVALUED");
}
