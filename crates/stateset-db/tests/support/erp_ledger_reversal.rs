use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn reversal_across_close(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let date = |month, day| NaiveDate::from_ymd_opt(2026, month, day).unwrap();
    let make = |month, end| {
        let period = gl
            .create_period(CreateGlPeriod {
                period_name: format!("2026-{month:02}"),
                fiscal_year: 2026,
                period_number: month as i32,
                start_date: date(month, 1),
                end_date: date(month, end),
            })
            .unwrap();
        gl.open_period(period.id).unwrap();
        period
    };
    let jan = make(1, 31);
    let feb = make(2, 28);
    let accrual = gl
        .create_journal_entry(CreateJournalEntry {
            entry_date: jan.end_date,
            entry_type: Some(JournalEntryType::Adjusting),
            description: "January accrual".into(),
            lines: vec![
                CreateJournalEntryLine::debit(account("5010"), dec!(100), None),
                CreateJournalEntryLine::credit(account("2010"), dec!(100), None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(true),
            fx: None,
        })
        .unwrap();
    gl.close_period(jan.id, "test").unwrap();
    gl.reverse_journal_entry(accrual.id, feb.start_date).unwrap();
    assert_eq!(
        gl.get_income_statement(jan.start_date, jan.end_date).unwrap().total_expenses,
        dec!(100)
    );
    assert_eq!(
        gl.get_income_statement(feb.start_date, feb.end_date).unwrap().total_expenses,
        dec!(-100)
    );
    assert_eq!(
        gl.get_income_statement(jan.start_date, feb.end_date).unwrap().total_expenses,
        dec!(0)
    );
    assert!(gl.get_trial_balance(jan.end_date).unwrap().is_balanced());
    assert!(gl.get_trial_balance(feb.end_date).unwrap().is_balanced());
}
