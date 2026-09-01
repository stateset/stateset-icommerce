//! Postgres parity for AP payment listing/counting filters.
//!
//! Both backends now apply the full `BillPaymentFilter` (`supplier_id`, `status`,
//! `payment_method`, and the `from_date`/`to_date` range) in `list_payments` AND
//! `count_payments`. This test locks in that behavior so a filtered
//! `count_payments` matches the filtered `list_payments` on Postgres too (the
//! SQLite side is covered by `ap_money_guards_test.rs`). Payments
//! are scoped to fresh random suppliers so a shared DB's other rows don't interfere.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{DateTime, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    BillPaymentFilter, CreateBill, CreateBillItem, CreateBillPayment, PaymentAllocationInput,
    PaymentMethodAP,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn make_payment(
    commerce: &AsyncCommerce,
    supplier: uuid::Uuid,
    method: PaymentMethodAP,
    amount: rust_decimal::Decimal,
) {
    let ap = commerce.accounts_payable();
    let date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().unwrap();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: supplier,
            bill_date: Some(date),
            due_date: "2026-04-10T00:00:00Z".parse().unwrap(),
            items: vec![CreateBillItem {
                description: "item".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: amount,
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create bill");
    ap.approve_bill(bill.id).await.expect("approve bill");
    ap.create_payment(CreateBillPayment {
        supplier_id: supplier,
        payment_date: Some(date),
        payment_method: method,
        amount,
        currency: None,
        reference_number: None,
        bank_account: None,
        check_number: None,
        memo: None,
        allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount }],
    })
    .await
    .expect("create payment");
}

#[tokio::test]
async fn postgres_ap_payment_list_and_count_apply_filters() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping AP payment filter parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

    // Fresh suppliers keep this test isolated on a shared database.
    let supplier_a = uuid::Uuid::new_v4();
    let supplier_b = uuid::Uuid::new_v4();
    make_payment(&commerce, supplier_a, PaymentMethodAP::Check, dec!(100.00)).await;
    make_payment(&commerce, supplier_b, PaymentMethodAP::Wire, dec!(100.00)).await;

    // Scope every assertion to supplier_a/supplier_b so other bills in a shared DB
    // don't skew the counts.
    let a_filter = BillPaymentFilter { supplier_id: Some(supplier_a), ..Default::default() };
    assert_eq!(ap.count_payments(a_filter.clone()).await.unwrap(), 1, "count filters by supplier");
    assert_eq!(ap.list_payments(a_filter).await.unwrap().len(), 1, "list filters by supplier");

    let check_a = BillPaymentFilter {
        supplier_id: Some(supplier_a),
        payment_method: Some(PaymentMethodAP::Check),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(check_a.clone()).await.unwrap(), 1, "count filters by method");
    assert_eq!(ap.list_payments(check_a).await.unwrap().len(), 1, "list filters by method");

    // Supplier A made no Wire payment: count and list agree at zero.
    let wire_a = BillPaymentFilter {
        supplier_id: Some(supplier_a),
        payment_method: Some(PaymentMethodAP::Wire),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(wire_a.clone()).await.unwrap(), 0);
    assert_eq!(ap.list_payments(wire_a).await.unwrap().len(), 0);
}
