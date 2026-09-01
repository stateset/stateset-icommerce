//! Postgres parity for the AP list `from_date`/`to_date` filters.
//!
//! SQLite dropped these; Postgres filters `bill_date`/`payment_date` by the date
//! range. This locks in that behavior so both backends agree now that SQLite stores
//! AP dates at date (midnight-UTC) granularity.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    BillFilter, BillPaymentFilter, CreateBill, CreateBillItem, CreateBillPayment, CreatePaymentRun,
    PaymentAllocationInput, PaymentMethodAP, PaymentRunFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse().unwrap()
}

async fn make_bill(commerce: &AsyncCommerce, supplier: uuid::Uuid, date: &str) -> uuid::Uuid {
    let ap = commerce.accounts_payable();
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
        .await
        .expect("create bill");
    bill.id
}

#[tokio::test]
async fn postgres_ap_list_methods_filter_by_date() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping AP date-filter parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

    // --- bills / count_bills, scoped to a fresh supplier ---
    let supplier = uuid::Uuid::new_v4();
    make_bill(&commerce, supplier, "2026-01-15T00:00:00Z").await;
    make_bill(&commerce, supplier, "2026-03-15T00:00:00Z").await;
    let bill_from = BillFilter {
        supplier_id: Some(supplier),
        from_date: Some(dt("2026-02-01T00:00:00Z")),
        ..Default::default()
    };
    assert_eq!(ap.list_bills(bill_from.clone()).await.unwrap().len(), 1, "bills from_date");
    assert_eq!(ap.count_bills(bill_from).await.unwrap(), 1, "count_bills from_date");

    // --- payments, scoped to another fresh supplier ---
    let pay_supplier = uuid::Uuid::new_v4();
    for date in ["2026-01-15T00:00:00Z", "2026-03-15T00:00:00Z"] {
        let bill_id = make_bill(&commerce, pay_supplier, date).await;
        ap.approve_bill(bill_id).await.expect("approve");
        ap.create_payment(CreateBillPayment {
            supplier_id: pay_supplier,
            payment_date: Some(dt(date)),
            payment_method: PaymentMethodAP::Check,
            amount: dec!(10),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id, amount: dec!(10) }],
        })
        .await
        .expect("create payment");
    }
    let pay_from = BillPaymentFilter {
        supplier_id: Some(pay_supplier),
        from_date: Some(dt("2026-02-01T00:00:00Z")),
        ..Default::default()
    };
    assert_eq!(ap.list_payments(pay_from).await.unwrap().len(), 1, "payments from_date");

    // --- payment runs (no supplier scope; asserts the date window narrows).
    // Each run needs a payable (approved) bill: `create_payment_run` now
    // validates bill_ids. ---
    let run_supplier = uuid::Uuid::new_v4();
    for date in ["2020-01-15T00:00:00Z", "2020-03-15T00:00:00Z"] {
        let bill_id = make_bill(&commerce, run_supplier, date).await;
        ap.approve_bill(bill_id).await.expect("approve");
        ap.create_payment_run(CreatePaymentRun {
            payment_date: dt(date),
            bill_ids: vec![bill_id],
            ..Default::default()
        })
        .await
        .expect("create run");
    }
    let runs = ap
        .list_payment_runs(PaymentRunFilter {
            from_date: Some(dt("2020-02-01T00:00:00Z")),
            to_date: Some(dt("2020-12-31T00:00:00Z")),
            ..Default::default()
        })
        .await
        .expect("list runs");
    assert_eq!(runs.len(), 1, "payment runs date window selects only the March 2020 run");
}
