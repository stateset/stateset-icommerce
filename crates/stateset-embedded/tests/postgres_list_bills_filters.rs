//! Postgres side of the AP `list_bills` filter parity guard.
//!
//! SQLite `list_bills` used to ignore `purchase_order_id` / `min_amount` / `max_amount`
//! / offset; Postgres applies them. This asserts the Postgres behavior the SQLite
//! backend now matches (see
//! `sqlite/accounts_payable.rs::list_bills_honors_po_amount_and_offset_filters`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
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
    unit_price: Decimal,
) -> uuid::Uuid {
    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            bill_number: None,
            supplier_id: supplier,
            purchase_order_id: po,
            bill_date: None,
            due_date: Utc::now() + Duration::days(30),
            payment_terms: None,
            currency: None,
            reference_number: None,
            memo: None,
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price,
                tax_rate: None,
                po_line_id: None,
            }],
        })
        .await
        .expect("create bill");
    bill.id
}

#[tokio::test]
async fn postgres_list_bills_honors_po_amount_and_offset_filters() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping list_bills filter test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let ap = commerce.accounts_payable();

    let supplier = uuid::Uuid::new_v4();
    let po = uuid::Uuid::new_v4();
    let _small = make_bill(&commerce, supplier, None, dec!(20)).await;
    let big = make_bill(&commerce, supplier, None, dec!(200)).await;
    let with_po = make_bill(&commerce, supplier, Some(po), dec!(50)).await;

    // Scope to this supplier so a shared DB's other bills don't interfere.
    let base = BillFilter { supplier_id: Some(supplier), ..Default::default() };
    assert_eq!(ap.list_bills(base.clone()).await.expect("all").len(), 3);

    let over = ap
        .list_bills(BillFilter { min_amount: Some(dec!(100)), ..base.clone() })
        .await
        .expect("min_amount");
    assert_eq!(over.len(), 1);
    assert_eq!(over[0].id, big);

    let by_po = ap
        .list_bills(BillFilter { purchase_order_id: Some(po), ..base.clone() })
        .await
        .expect("po");
    assert_eq!(by_po.len(), 1);
    assert_eq!(by_po[0].id, with_po);

    let offset1 = ap.list_bills(BillFilter { offset: Some(1), ..base }).await.expect("offset");
    assert_eq!(offset1.len(), 2, "offset must skip rows");
}
