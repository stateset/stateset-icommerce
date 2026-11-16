use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

fn date(value: &str) -> NaiveDate {
    value.parse().unwrap()
}

pub(crate) fn ledger_fx(db: &dyn Database, correct_february_rate: impl Fn(&str)) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    for month in 1..=4 {
        let start = NaiveDate::from_ymd_opt(2026, month, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, month + 1, 1).unwrap().pred_opt().unwrap();
        let period = gl
            .create_period(CreateGlPeriod {
                period_name: format!("FX {month}"),
                fiscal_year: 2026,
                period_number: month as i32,
                start_date: start,
                end_date: end,
            })
            .unwrap();
        gl.open_period(period.id).unwrap();
    }
    let fx = gl
        .create_account(CreateGlAccount {
            account_number: "6999".into(),
            name: "FX gain/loss".into(),
            account_type: AccountType::Expense,
            account_sub_type: Some(AccountSubType::OtherExpense),
            description: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap();
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "FX".into(),
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
        fx_gain_loss_account_id: Some(fx.id),
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("fx-{}@example.com", Uuid::new_v4()),
            first_name: "FX".into(),
            last_name: "Customer".into(),
            ..Default::default()
        })
        .unwrap();
    let invoice = |day: &str, currency| {
        let invoice = db
            .invoices()
            .create(CreateInvoice {
                customer_id: customer.id,
                invoice_date: Some(format!("{day}T12:00:00Z").parse().unwrap()),
                currency: Some(currency),
                items: vec![CreateInvoiceItem {
                    description: "Goods".into(),
                    quantity: dec!(1),
                    unit_price: dec!(1000),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        db.invoices().send(invoice.id).unwrap();
        invoice
    };
    let invoice1 = invoice("2026-01-15", CurrencyCode::EUR);
    let first = gl.auto_post_invoice(invoice1.id).unwrap();
    let mut settings = db.currency().get_settings().unwrap();
    settings.base_currency = Currency::EUR;
    assert!(
        db.currency().update_settings(settings).is_err(),
        "Functional currency is fixed once the ledger has entries"
    );
    assert_eq!(first.total_debits, dec!(1200));
    assert!(first.lines.iter().all(|line| line.currency == CurrencyCode::USD
        && line.transaction_currency == Some(CurrencyCode::EUR)
        && line.exchange_rate == Some(dec!(1.2))));
    assert_eq!(
        first.lines.iter().map(|line| line.transaction_debit_amount.unwrap()).sum::<Decimal>(),
        dec!(1000)
    );
    assert_eq!(first.id, gl.auto_post_invoice(invoice1.id).unwrap().id);

    // Future activity is present before historical revaluation is requested.
    let invoice2 = invoice("2026-03-15", CurrencyCode::EUR);
    let future = gl.auto_post_invoice(invoice2.id).unwrap();
    assert_eq!(future.total_debits, dec!(1800));
    let unchanged = gl.revalue(date("2026-01-31"), None).unwrap();
    assert_eq!(unchanged.total_unrealized_gain_loss, dec!(0));
    assert!(unchanged.journal_entry.is_none());
    assert_eq!(
        unchanged.lines.len(),
        1,
        "Only monetary positions are revalued; sales stay at the historical rate"
    );
    assert_eq!(unchanged.lines[0].carrying_value, dec!(1200));
    let before_preview = gl.list_journal_entries(JournalEntryFilter::default()).unwrap().len();
    let preview = gl.preview_revaluation(date("2026-02-28"), None).unwrap();
    assert_eq!(preview.total_unrealized_gain_loss, dec!(100));
    assert!(preview.journal_entry.is_none());
    assert_eq!(
        gl.list_journal_entries(JournalEntryFilter::default()).unwrap().len(),
        before_preview
    );
    let changed = gl.revalue(date("2026-02-28"), None).unwrap();
    assert_eq!(changed.total_unrealized_gain_loss, dec!(100));
    let revaluation = changed.journal_entry.unwrap();
    assert!(revaluation.lines.iter().all(|line| line.transaction_debit_amount == Some(dec!(0))
        && line.transaction_credit_amount == Some(dec!(0))));
    let retry = gl.revalue(date("2026-02-28"), None).unwrap();
    assert_eq!(retry.journal_entry.unwrap().id, revaluation.id);
    assert_eq!(retry.total_unrealized_gain_loss, dec!(100));
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(3100));

    // Reversal uses the original 1.8 rate even though April's market rate is 2.
    let reversed = gl.reverse_journal_entry(future.id, date("2026-04-10")).unwrap();
    assert_eq!(reversed.total_debits, dec!(1800));
    assert!(reversed.lines.iter().all(|line| line.exchange_rate == Some(dec!(1.8))));
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(1300));
    let march = gl.revalue(date("2026-03-31"), None).unwrap();
    assert_eq!(
        march.lines[0].foreign_balance,
        dec!(2000),
        "Later reversal must not erase the original from March"
    );
    assert_eq!(march.total_unrealized_gain_loss, dec!(500));
    let april = gl.revalue(date("2026-04-30"), None).unwrap();
    assert_eq!(april.lines[0].foreign_balance, dec!(1000));
    assert_eq!(april.lines[0].carrying_value, dec!(1800));
    assert_eq!(april.total_unrealized_gain_loss, dec!(200));
    assert!(gl.get_trial_balance(date("2026-04-30")).unwrap().is_balanced);

    // No rate may be borrowed from the future when a document predates history.
    let unknown = invoice("2026-01-10", CurrencyCode::GBP);
    assert!(gl.auto_post_invoice(unknown.id).is_err());
    let count = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("invoice".into()),
            source_document_id: Some(unknown.id.into()),
            ..Default::default()
        })
        .unwrap();
    assert!(count.is_empty());

    // Tiny transaction lines can round to zero without losing their foreign units.
    let tiny = gl
        .create_journal_entry(CreateJournalEntry {
            entry_date: date("2026-04-20"),
            entry_type: None,
            description: "Tiny FX".into(),
            lines: vec![
                CreateJournalEntryLine::debit(account("1010"), dec!(0.03), None),
                CreateJournalEntryLine::credit(account("4010"), dec!(0.01), None),
                CreateJournalEntryLine::credit(account("4010"), dec!(0.01), None),
                CreateJournalEntryLine::credit(account("4010"), dec!(0.01), None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(true),
            fx: Some(JournalEntryFx { currency: CurrencyCode::EUR, rate: dec!(0.5) }),
        })
        .unwrap();
    assert_eq!(tiny.total_debits, dec!(0.02));
    assert_eq!(tiny.total_credits, dec!(0.02));
    assert!(tiny.lines.iter().all(|line| line.is_valid()));
    let undo = gl.reverse_journal_entry(tiny.id, date("2026-04-21")).unwrap();
    for (line, original) in undo.lines.iter().zip(&tiny.lines) {
        assert_eq!(line.debit_amount, original.credit_amount);
        assert_eq!(line.transaction_debit_amount, original.transaction_credit_amount);
    }
    // New backdated transactions invalidate a previously saved close result.
    let late = invoice("2026-01-20", CurrencyCode::EUR);
    gl.auto_post_invoice(late.id).unwrap();
    let updated = gl.revalue(date("2026-02-28"), None).unwrap();
    assert_eq!(updated.lines[0].foreign_balance, dec!(2000));
    assert_eq!(updated.total_unrealized_gain_loss, dec!(100));
    let updated_id = updated.journal_entry.unwrap().id;
    assert_ne!(updated_id, revaluation.id);
    assert_eq!(gl.revalue(date("2026-02-28"), None).unwrap().journal_entry.unwrap().id, updated_id);
    correct_february_rate("1.4");
    let proposal = gl.preview_revaluation(date("2026-02-28"), None).unwrap();
    assert_eq!(proposal.total_unrealized_gain_loss, dec!(200));
    assert!(proposal.journal_entry.is_none());
    let corrected = gl.revalue(date("2026-02-28"), None).unwrap();
    assert_eq!(corrected.total_unrealized_gain_loss, dec!(200));
    assert_ne!(corrected.journal_entry.unwrap().id, updated_id);
    // Returning to an earlier rate must adjust the current carrying balance,
    // rather than resurrecting the result cached for that historical rate.
    correct_february_rate("1.3");
    let restored = gl.revalue(date("2026-02-28"), None).unwrap();
    assert_eq!(restored.total_unrealized_gain_loss, dec!(-200));
    assert_eq!(
        gl.revalue(date("2026-02-28"), None).unwrap().journal_entry.unwrap().id,
        restored.journal_entry.unwrap().id
    );
}
