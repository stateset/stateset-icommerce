#![cfg(feature = "sqlite")]

//! Regression: SQLite `auto_post_bill_payment` was doubly broken.
//!
//! 1. It read the payment with `SELECT amount, payment_date FROM bill_payments`,
//!    but there is no `bill_payments` table — AP payments live in `ap_payments`
//!    (Postgres reads `ap_payments`). So the query failed at runtime with a
//!    "no such table" error.
//! 2. It parsed `payment_date` (stored on SQLite as a full RFC3339 timestamp)
//!    directly as a `NaiveDate`, which cannot parse a timestamp — so even with the
//!    table fixed the date parse would fail.
//!
//! The result: posting a bill payment to the general ledger was completely broken
//! on SQLite while Postgres worked. This test creates a configured chart of
//! accounts and an AP payment, then asserts `auto_post_bill_payment` produces a
//! balanced, correctly-dated journal entry (debit Accounts Payable, credit Cash).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, CreateAutoPostingConfig, CreateBill, CreateBillItem,
    CreateBillPayment, CreateGlPeriod, GeneralLedgerRepository, PaymentAllocationInput,
    PaymentMethodAP,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_auto_post_bill_payment_posts_balanced_entry() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart of accounts");

    let acct = |number: &str| {
        gl.get_account_by_number(number)
            .expect("query account")
            .unwrap_or_else(|| panic!("account {number} exists"))
            .id
    };

    let payment_date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().expect("parse date");
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-03".into(),
            fiscal_year: 2026,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "default".into(),
        cash_account_id: acct("1010"),
        accounts_receivable_account_id: acct("1100"),
        inventory_account_id: acct("1200"),
        accounts_payable_account_id: acct("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: acct("4010"),
        shipping_revenue_account_id: None,
        cogs_account_id: acct("5010"),
        bad_debt_expense_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .expect("set auto-posting config");

    // A bill payment requires at least one allocation to a payable (approved) bill.
    let supplier_id = uuid::Uuid::new_v4();
    let ap = db.accounts_payable();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id,
            bill_date: Some(payment_date),
            due_date: "2026-04-10T00:00:00Z".parse().expect("parse due date"),
            items: vec![CreateBillItem {
                description: "Raw material".into(),
                account_code: None,
                quantity: dec!(2),
                unit_price: dec!(50.00),
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");
    ap.approve_bill(bill.id).expect("approve bill");

    let payment = ap
        .create_payment(CreateBillPayment {
            supplier_id,
            payment_date: Some(payment_date),
            payment_method: PaymentMethodAP::Check,
            amount: dec!(100.00),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(100.00) }],
        })
        .expect("create bill payment");

    let entry = gl.auto_post_bill_payment(payment.id).expect("auto_post_bill_payment must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "AP debit should equal the payment amount");
    assert_eq!(entry.total_credits, dec!(100.00), "Cash credit should equal the payment amount");
    assert_eq!(
        entry.entry_date,
        payment_date.date_naive(),
        "entry date should be the payment date"
    );
}
