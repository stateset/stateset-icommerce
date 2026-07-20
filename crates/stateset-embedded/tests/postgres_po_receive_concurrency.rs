//! Postgres side of the PO `receive` concurrency parity guard.
//!
//! Concurrent partial receipts against the same PO item must all land. Postgres
//! uses one atomic conditional UPDATE (`quantity_received + $1 WHERE ... <=
//! quantity_ordered`); SQLite now serializes its read-check-write under a retrying
//! IMMEDIATE transaction (see
//! `sqlite/purchase_orders.rs::receive_accumulates_concurrent_partial_receipts_without_lost_updates`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, ReceivePurchaseOrderItem,
    ReceivePurchaseOrderItems,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_receive_accumulates_concurrent_partial_receipts() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping PO receive concurrency test");
        return;
    };
    let commerce = Arc::new(AsyncCommerce::connect(&url).await.expect("connect + migrate"));

    let unique = uuid::Uuid::new_v4().to_string();
    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: format!("RCV-{}", &unique[..8]),
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

    let po = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "SKU-R".into(),
                name: "Item".into(),
                quantity: dec!(100),
                unit_cost: dec!(1),
                unit_of_measure: Some("EA".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create po");
    let po_id = po.id.into_uuid();
    let item_id = po.items[0].id;

    let n = 8;
    let barrier = Arc::new(tokio::sync::Barrier::new(n));
    let mut handles = Vec::new();
    for _ in 0..n {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            commerce
                .purchase_orders()
                .receive(
                    po_id,
                    ReceivePurchaseOrderItems {
                        items: vec![ReceivePurchaseOrderItem {
                            item_id,
                            quantity_received: dec!(2),
                            notes: None,
                        }],
                        notes: None,
                    },
                )
                .await
        }));
    }
    for h in handles {
        h.await.expect("join").expect("receive");
    }

    let po = commerce.purchase_orders().get(po_id).await.expect("get").expect("po exists");
    assert_eq!(
        po.items[0].quantity_received,
        Decimal::from(n as u64) * dec!(2),
        "all {n} concurrent receipts of 2 must accumulate"
    );
}
