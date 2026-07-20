//! Postgres side of the purchase-order `list` pagination parity guard.
//!
//! Postgres applies `offset` and a default page size of 100; SQLite used to
//! ignore `offset` and had no default cap. This asserts the Postgres pagination
//! the SQLite backend now matches (see
//! `sqlite/purchase_orders.rs::list_applies_offset_and_pagination`), guarding the
//! two backends against drifting apart.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PurchaseOrderFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_purchase_order_list_applies_offset() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping PO pagination test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: format!("PAGER-{}", &unique[..8]),
            supplier_code: None,
            contact_name: None,
            email: None,
            phone: None,
            website: None,
            address: None,
            city: None,
            state: None,
            postal_code: None,
            country: Some("US".into()),
            tax_id: None,
            payment_terms: None,
            currency: None,
            lead_time_days: Some(7),
            minimum_order: None,
            notes: None,
        })
        .await
        .expect("create supplier");

    for i in 0..3 {
        commerce
            .purchase_orders()
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![CreatePurchaseOrderItem {
                    sku: format!("SKU-{i}"),
                    name: format!("Item {i}"),
                    quantity: dec!(1),
                    unit_cost: dec!(1),
                    unit_of_measure: Some("EA".into()),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .expect("create po");
    }

    let base = PurchaseOrderFilter { supplier_id: Some(supplier.id), ..Default::default() };

    assert_eq!(commerce.purchase_orders().list(base.clone()).await.expect("all").len(), 3);

    let offset1 = commerce
        .purchase_orders()
        .list(PurchaseOrderFilter { offset: Some(1), ..base.clone() })
        .await
        .expect("offset");
    assert_eq!(offset1.len(), 2, "offset must skip rows");

    let past = commerce
        .purchase_orders()
        .list(PurchaseOrderFilter { offset: Some(10), ..base })
        .await
        .expect("past");
    assert!(past.is_empty(), "offset past the end returns nothing");
}
