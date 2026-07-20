//! Postgres parity: `count_bills` must match `list_bills` across filters.
//!
//! Postgres `count_bills` applied only `supplier_id/status/overdue_only`, while
//! `list_bills` also applies `purchase_order_id` and min/max amount — so a filtered
//! count by PO or amount did not match the filtered list. This test seeds bills for
//! a fresh supplier and asserts `count_bills == list_bills().len()` for each filter.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{BillFilter, CreateBill, CreateBillItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn make_bill(
    commerce: &AsyncCommerce,
    supplier: uuid::Uuid,
    po: Option<uuid::Uuid>,
    amount: Decimal,
) {
    commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier,
            purchase_order_id: po,
            bill_date: Some("2026-03-10T00:00:00Z".parse().unwrap()),
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
}

#[tokio::test]
async fn postgres_count_bills_matches_list_bills_across_filters() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping count_bills parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    // Fresh supplier keeps this test isolated on a shared database.
    let supplier = uuid::Uuid::new_v4();
    let po = uuid::Uuid::new_v4();
    make_bill(&commerce, supplier, Some(po), dec!(100.00)).await;
    make_bill(&commerce, supplier, None, dec!(300.00)).await;
    make_bill(&commerce, supplier, Some(po), dec!(50.00)).await;

    let ap = commerce.accounts_payable();
    // Every filter is scoped to `supplier` so other bills in a shared DB don't skew
    // the counts. For each, count_bills must equal the filtered list length.
    let base = || BillFilter { supplier_id: Some(supplier), ..Default::default() };

    for (label, filter, expected) in [
        ("supplier only", base(), 3u64),
        ("purchase_order", BillFilter { purchase_order_id: Some(po), ..base() }, 2),
        ("min_amount>=100", BillFilter { min_amount: Some(dec!(100)), ..base() }, 2),
        ("max_amount<=100", BillFilter { max_amount: Some(dec!(100)), ..base() }, 2),
        ("min_amount>=200", BillFilter { min_amount: Some(dec!(200)), ..base() }, 1),
    ] {
        let count = ap.count_bills(filter.clone()).await.expect("count");
        let listed = ap.list_bills(filter).await.expect("list").len() as u64;
        assert_eq!(count, expected, "count_bills wrong for {label}");
        assert_eq!(count, listed, "count_bills must match list_bills length for {label}");
    }
}
