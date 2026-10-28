use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

fn date(value: &str) -> NaiveDate {
    value.parse().unwrap()
}

pub(crate) fn ledger_guards(db: &dyn Database, inject_legacy_overlap: impl Fn(Uuid)) {
    let gl = db.general_ledger();
    let period = |name: &str, number, start: &str, end: &str| CreateGlPeriod {
        period_name: name.into(),
        fiscal_year: 2026,
        period_number: number,
        start_date: date(start),
        end_date: date(end),
    };
    assert!(gl.create_period(period("Invalid", 99, "2026-02-01", "2026-01-01")).is_err());
    let jan = gl.create_period(period("January", 1, "2026-01-01", "2026-01-31")).unwrap();
    assert_eq!(
        gl.get_period_for_date(date("2026-01-15")).unwrap().unwrap().status,
        PeriodStatus::Future
    );
    for (start, end) in
        [("2026-01-31", "2026-02-02"), ("2026-01-05", "2026-01-10"), ("2025-12-01", "2026-02-01")]
    {
        assert!(gl.create_period(period("Overlap", 99, start, end)).is_err());
    }
    let feb = gl.create_period(period("February", 2, "2026-02-01", "2026-02-28")).unwrap();
    gl.open_period(jan.id).unwrap();
    gl.open_period(feb.id).unwrap();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let create = |number: &str, header, posting| {
        gl.create_account(CreateGlAccount {
            account_number: number.into(),
            name: number.into(),
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            description: None,
            is_header: Some(header),
            is_posting: Some(posting),
            currency: None,
        })
        .unwrap()
    };
    let header = create("1095", true, true);
    let nonposting = create("1096", false, false);
    let inactive = create("1097", false, true);
    gl.update_account(
        inactive.id,
        UpdateGlAccount { status: Some(AccountStatus::Inactive), ..Default::default() },
    )
    .unwrap();
    let input = |debit, auto| CreateJournalEntry {
        entry_date: date("2026-01-15"),
        entry_type: None,
        description: "Guard test".into(),
        lines: vec![
            CreateJournalEntryLine::debit(debit, dec!(100), None),
            CreateJournalEntryLine::credit(account("4010"), dec!(100), None),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(auto),
        fx: None,
    };
    for invalid in [header.id, nonposting.id, inactive.id] {
        assert!(gl.create_journal_entry(input(invalid, false)).is_err());
        assert!(gl.create_journal_entry(input(invalid, true)).is_err());
    }
    let dormant = create("1098", false, true);
    let draft = gl.create_journal_entry(input(dormant.id, false)).unwrap();
    gl.update_account(
        dormant.id,
        UpdateGlAccount { status: Some(AccountStatus::Inactive), ..Default::default() },
    )
    .unwrap();
    assert!(
        gl.post_journal_entry(draft.id, "tester").is_err(),
        "Posting must revalidate accounts after draft creation"
    );
    assert_eq!(gl.get_journal_entry(draft.id).unwrap().unwrap().status, JournalEntryStatus::Draft);
    let entry = gl.create_journal_entry(input(account("1010"), true)).unwrap();
    for status in [AccountStatus::Inactive, AccountStatus::Archived] {
        assert!(
            gl.update_account(
                account("1010"),
                UpdateGlAccount { status: Some(status), ..Default::default() }
            )
            .is_err()
        );
    }
    gl.close_period(jan.id, "tester").unwrap();
    assert_eq!(
        gl.get_period_for_date(date("2026-01-15")).unwrap().unwrap().status,
        PeriodStatus::Closed
    );
    gl.reverse_journal_entry(entry.id, date("2026-02-01")).unwrap();
    gl.update_account(
        account("1010"),
        UpdateGlAccount { status: Some(AccountStatus::Inactive), ..Default::default() },
    )
    .unwrap();
    let historical = gl.get_trial_balance(date("2026-01-31")).unwrap();
    assert!(
        historical.is_balanced,
        "Deactivation at a later zero balance must preserve historical reports"
    );
    assert_eq!(historical.total_debits, dec!(100));
    let tiny = create("1099", false, true);
    let mut tiny_input = input(tiny.id, true);
    tiny_input.entry_date = date("2026-02-10");
    tiny_input.lines[0].debit_amount = dec!(0.003);
    tiny_input.lines[1].credit_amount = dec!(0.003);
    tiny_input.fx = Some(JournalEntryFx { currency: CurrencyCode::EUR, rate: dec!(0.1) });
    gl.create_journal_entry(tiny_input).unwrap();
    assert_eq!(gl.get_account(tiny.id).unwrap().unwrap().current_balance, dec!(0));
    assert!(
        gl.update_account(
            tiny.id,
            UpdateGlAccount { status: Some(AccountStatus::Inactive), ..Default::default() }
        )
        .is_err(),
        "Foreign units still constitute a balance when their base value rounds to zero"
    );
    inject_legacy_overlap(jan.id);
    assert!(gl.get_period_for_date(date("2026-01-15")).is_err());
    assert!(
        gl.create_journal_entry(input(tiny.id, true)).is_err(),
        "An overlapping open period must not bypass a closed month"
    );
}
