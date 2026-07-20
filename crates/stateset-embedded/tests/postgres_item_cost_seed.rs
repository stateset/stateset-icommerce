//! Postgres parity for `set_item_cost` seeding of `average_cost` / `last_cost`.
//!
//! When a brand-new `item_costs` row is created, SQLite seeds `average_cost` and
//! `last_cost` to the `standard_cost` (documented: "`average_cost` starts as
//! standard"), but the Postgres path hardcoded them to zero — so a freshly-costed
//! SKU reported an inventory value of $0 on Postgres vs its real value on SQLite,
//! and the weighted-average cost then diverged permanently. Postgres now seeds
//! both to `standard_cost` too.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CostMethod, SetItemCost};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_set_item_cost_seeds_average_and_last_to_standard() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let sku = format!("COST-{}", uuid::Uuid::new_v4().simple());

    let cost = commerce
        .cost_accounting()
        .set_item_cost(SetItemCost {
            sku: sku.clone(),
            cost_method: Some(CostMethod::Standard),
            standard_cost: Some(dec!(10.00)),
            ..Default::default()
        })
        .await
        .expect("set item cost");

    assert_eq!(cost.standard_cost, dec!(10.00));
    assert_eq!(cost.average_cost, dec!(10.00), "average_cost must seed to standard_cost, not zero");
    assert_eq!(cost.last_cost, dec!(10.00), "last_cost must seed to standard_cost, not zero");

    // Re-fetched from the DB, same seeding.
    let fetched =
        commerce.cost_accounting().get_item_cost(&sku).await.expect("get").expect("exists");
    assert_eq!(fetched.average_cost, dec!(10.00));
    assert_eq!(fetched.last_cost, dec!(10.00));
}
