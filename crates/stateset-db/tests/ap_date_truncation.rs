#![cfg(feature = "sqlite")]

//! Regression: SQLite stored accounts-payable `bill_date`/`due_date`/`payment_date`
//! as full RFC3339 timestamps (keeping time-of-day), while Postgres stores them in
//! `DATE` columns that drop the time and read back at midnight UTC. So a bill or
//! payment created with a timed date read back differently on the two backends.
//! SQLite now truncates these date columns to midnight UTC before storing (keeping
//! the RFC3339 format so all readers still work), matching Postgres.

use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, CreateBill, CreateBillItem, CreateBillPayment,
    PaymentAllocationInput, PaymentMethodAP,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_ap_dates_truncate_to_midnight_matching_postgres() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    let dt = |s: &str| s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
    let midnight = |d: &str| dt(&format!("{d}T00:00:00Z"));

    let supplier = uuid::Uuid::new_v4();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: supplier,
            bill_date: Some(dt("2026-03-10T14:30:00Z")),
            due_date: dt("2026-04-10T09:15:00Z"),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: dec!(100),
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");

    assert_eq!(bill.bill_date, midnight("2026-03-10"), "bill_date must truncate to midnight");
    assert_eq!(bill.due_date, midnight("2026-04-10"), "due_date must truncate to midnight");

    // payment_date on ap_payments is also a DATE on Postgres.
    ap.approve_bill(bill.id).expect("approve bill");
    let payment = ap
        .create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: Some(dt("2026-03-15T18:45:00Z")),
            payment_method: PaymentMethodAP::Check,
            amount: dec!(100),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(100) }],
        })
        .expect("create payment");

    let stored = ap.get_payment(payment.id).expect("get payment").expect("payment exists");
    assert_eq!(
        stored.payment_date,
        midnight("2026-03-15"),
        "payment_date must truncate to midnight"
    );
}
