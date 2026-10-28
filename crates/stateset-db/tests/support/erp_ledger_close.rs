use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn close_is_atomic(db: &dyn Database, fail_close: impl Fn(bool)) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "January".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: period.start_date,
        entry_type: None,
        description: "January revenue".into(),
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
    fail_close(true);
    assert!(gl.run_period_close(period.id, "tester").is_err());
    assert_eq!(gl.get_period(period.id).unwrap().unwrap().status, PeriodStatus::Open);
    assert_eq!(gl.get_account(account("4010")).unwrap().unwrap().current_balance, dec!(100));
    assert!(
        gl.list_journal_entries(JournalEntryFilter {
            source_document_type: Some("period_close".into()),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
    fail_close(false);
    let entry = gl.run_period_close(period.id, "tester").unwrap();
    assert_eq!(entry.status, JournalEntryStatus::Posted);
    assert_eq!(entry.posted_by.as_deref(), Some("tester"));
    assert_eq!(gl.get_period(period.id).unwrap().unwrap().status, PeriodStatus::Closed);
    assert_eq!(gl.get_account(account("4010")).unwrap().unwrap().current_balance, dec!(0));
    assert!(gl.get_trial_balance(period.end_date).unwrap().is_balanced);
    assert_eq!(
        gl.get_income_statement(period.start_date, period.end_date).unwrap().net_income,
        dec!(100)
    );
    assert!(gl.run_period_close(period.id, "tester").is_err());
}
