//! Postgres concurrency regression for weighted-average-cost updates.
//!
//! `update_average_cost` reads `item_costs.average_cost`, computes the new
//! weighted average, and writes it back. Without a row lock two concurrent
//! receipts for the same SKU both read the same average and one overwrites the
//! other, corrupting the WAC. With identical receipts the correct result is
//! order-independent, so the concurrent final average must equal the sequential
//! result. The fix locks the `item_costs` row with `SELECT … FOR UPDATE`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{CreateInventoryItem, SetItemCost};
use stateset_embedded::AsyncCommerce;
use std::sync::Arc;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn setup(commerce: &AsyncCommerce, sku: &str, on_hand: Decimal) {
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: "WAC widget".into(),
            initial_quantity: Some(on_hand),
            ..Default::default()
        })
        .await
        .expect("create inventory item");
    commerce
        .cost_accounting()
        .set_item_cost(SetItemCost {
            sku: sku.into(),
            standard_cost: Some(dec!(0)),
            ..Default::default()
        })
        .await
        .expect("seed item cost");
}

#[tokio::test]
async fn postgres_wac_concurrent_receipts_match_sequential() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let n = 20usize;
    let on_hand = dec!(1000);
    let qty = dec!(100);
    let unit_cost = dec!(10);

    // Sequential reference (fresh DB via a unique SKU prefix per phase not needed —
    // separate connect() instances would share the DB, so use one SKU here).
    let seq_commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let seq_sku = format!("WAC-SEQ-{}", uuid::Uuid::new_v4().simple());
    setup(&seq_commerce, &seq_sku, on_hand).await;
    for _ in 0..n {
        seq_commerce
            .cost_accounting()
            .update_average_cost(&seq_sku, qty, unit_cost)
            .await
            .expect("seq update");
    }
    let expected = seq_commerce
        .cost_accounting()
        .get_item_cost(&seq_sku)
        .await
        .expect("get")
        .expect("cost")
        .average_cost;

    // Concurrent run.
    let commerce = Arc::new(AsyncCommerce::connect(&url).await.expect("connect + migrate"));
    let sku = format!("WAC-CONC-{}", uuid::Uuid::new_v4().simple());
    setup(&commerce, &sku, on_hand).await;

    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let commerce = Arc::clone(&commerce);
        let sku = sku.clone();
        handles.push(tokio::spawn(async move {
            commerce.cost_accounting().update_average_cost(&sku, qty, unit_cost).await
        }));
    }
    let mut committed = 0usize;
    for handle in handles {
        if handle.await.expect("join update task").is_ok() {
            committed += 1;
        }
    }
    // `FOR UPDATE` blocks rather than failing, so every update commits.
    assert_eq!(committed, n, "every update should commit under FOR UPDATE serialization");

    let got = commerce
        .cost_accounting()
        .get_item_cost(&sku)
        .await
        .expect("get")
        .expect("cost")
        .average_cost;
    assert_eq!(
        got, expected,
        "concurrent weighted-average must equal the serialized result (lost update otherwise)"
    );
}
