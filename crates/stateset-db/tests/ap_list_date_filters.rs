#![cfg(feature = "sqlite")]

//! Regression: SQLite dropped the `from_date`/`to_date` filters on the AP list
//! methods (`list_bills`/`count_bills` on `bill_date`, `list_payments` and
//! `list_payment_runs` on `payment_date`). These were deferred while SQLite stored
//! AP dates as full timestamps; now that those columns are truncated to midnight UTC
//! (matching Postgres `DATE`), the `>= from_date` / `<= to_date` comparisons are
//! well-defined and are applied, matching Postgres.

use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, BillFilter, BillPaymentFilter, CreateBill, CreateBillItem,
    CreateBillPayment, CreatePaymentRun, PaymentAllocationInput, PaymentMethodAP, PaymentRunFilter,
};
use stateset_db::SqliteDatabase;

fn dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse().unwrap()
}

#[test]
fn sqlite_list_bills_filters_by_date() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();
    let supplier = uuid::Uuid::new_v4();

    let make = |date: &str| {
        ap.create_bill(CreateBill {
            supplier_id: supplier,
            bill_date: Some(dt(date)),
            due_date: dt("2026-12-31T00:00:00Z"),
            items: vec![CreateBillItem {
                description: "x".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: dec!(10),
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");
    };
    make("2026-01-15T00:00:00Z");
    make("2026-03-15T00:00:00Z");

    let from = |d: &str| BillFilter { from_date: Some(dt(d)), ..Default::default() };
    let to = |d: &str| BillFilter { to_date: Some(dt(d)), ..Default::default() };

    assert_eq!(ap.list_bills(from("2026-02-01T00:00:00Z")).unwrap().len(), 1, "from_date");
    assert_eq!(ap.list_bills(to("2026-02-01T00:00:00Z")).unwrap().len(), 1, "to_date");
    // count_bills shares the matching logic, so it honors dates too.
    assert_eq!(ap.count_bills(from("2026-02-01T00:00:00Z")).unwrap(), 1, "count from_date");
}

#[test]
fn sqlite_list_payment_runs_filters_by_date() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    let make = |date: &str| {
        ap.create_payment_run(CreatePaymentRun { payment_date: dt(date), ..Default::default() })
            .expect("create run");
    };
    make("2026-01-15T00:00:00Z");
    make("2026-03-15T00:00:00Z");

    let f = PaymentRunFilter { from_date: Some(dt("2026-02-01T00:00:00Z")), ..Default::default() };
    assert_eq!(ap.list_payment_runs(f).unwrap().len(), 1, "runs from_date");
}

#[test]
fn sqlite_list_payments_filters_by_date() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();
    let supplier = uuid::Uuid::new_v4();

    let make = |date: &str| {
        let bill = ap
            .create_bill(CreateBill {
                supplier_id: supplier,
                bill_date: Some(dt(date)),
                due_date: dt("2026-12-31T00:00:00Z"),
                items: vec![CreateBillItem {
                    description: "x".into(),
                    account_code: None,
                    quantity: dec!(1),
                    unit_price: dec!(10),
                    tax_rate: None,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create bill");
        ap.approve_bill(bill.id).expect("approve");
        ap.create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: Some(dt(date)),
            payment_method: PaymentMethodAP::Check,
            amount: dec!(10),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(10) }],
        })
        .expect("create payment");
    };
    make("2026-01-15T00:00:00Z");
    make("2026-03-15T00:00:00Z");

    let f = BillPaymentFilter { from_date: Some(dt("2026-02-01T00:00:00Z")), ..Default::default() };
    assert_eq!(ap.list_payments(f).unwrap().len(), 1, "payments from_date");
}
