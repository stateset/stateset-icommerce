//! Postgres parity for accounts-payable bill money rounding.
//!
//! Postgres rounds AP money to its `NUMERIC(12,4)` column scale (`NUMERIC(12,6)`
//! for `tax_rate`) on insert. SQLite now rounds the same way before storing TEXT, so
//! a bill item with sub-4dp inputs reads back identically on both backends. This
//! asserts the Postgres values, locking in the shared rounded result.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CreateBill, CreateBillItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_ap_bill_money_rounds_to_numeric_scale() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping AP bill rounding test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

    let bill = ap
        .create_bill(CreateBill {
            supplier_id: uuid::Uuid::new_v4(),
            bill_date: Some("2026-03-10T00:00:00Z".parse().unwrap()),
            due_date: "2026-04-10T00:00:00Z".parse().unwrap(),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: dec!(10.12345),
                tax_rate: Some(dec!(8.5)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create bill");

    let items = ap.get_bill_items(bill.id).await.expect("get bill items");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.unit_price, dec!(10.1235), "unit_price rounds to 4dp");
    assert_eq!(item.amount, dec!(10.1235), "amount rounds to 4dp");
    assert_eq!(item.tax_amount, dec!(0.8605), "tax_amount rounds to 4dp");

    assert_eq!(bill.subtotal, dec!(10.1235), "subtotal 4dp");
    assert_eq!(bill.tax_amount, dec!(0.8605), "tax 4dp");
    assert_eq!(bill.total_amount, dec!(10.984), "total = subtotal + tax");
}
