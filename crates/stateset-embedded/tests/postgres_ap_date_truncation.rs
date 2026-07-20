//! Postgres parity for accounts-payable date truncation.
//!
//! Postgres stores `bill_date`/`due_date`/`payment_date` as `DATE`, dropping the
//! time-of-day and reading them back at midnight UTC. SQLite now truncates the same
//! way. This asserts the Postgres behavior, locking in the shared midnight result.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateBill, CreateBillItem, CreateBillPayment, PaymentAllocationInput, PaymentMethodAP,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_ap_dates_truncate_to_midnight() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping AP date truncation test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

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
        .await
        .expect("create bill");

    assert_eq!(bill.bill_date, midnight("2026-03-10"), "bill_date reads back at midnight");
    assert_eq!(bill.due_date, midnight("2026-04-10"), "due_date reads back at midnight");

    ap.approve_bill(bill.id).await.expect("approve bill");
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
        .await
        .expect("create payment");

    let stored = ap.get_payment(payment.id).await.expect("get payment").expect("payment exists");
    assert_eq!(stored.payment_date, midnight("2026-03-15"), "payment_date reads back at midnight");
}
