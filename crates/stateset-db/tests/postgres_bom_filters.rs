//! Postgres parity for `bom::list`/`count` filters.
//!
//! Two Postgres bugs, both fixed here:
//!  - `list_async`/`count_async` used `if product_id {} else if status {}`, so
//!    `status` was silently dropped when `product_id` was also set (SQLite ANDs
//!    them). A `list(product_id=X, status=Active)` returned every status for X.
//!  - `search` was ignored entirely on Postgres (SQLite applies
//!    `name`/`bom_number` LIKE). Search was a no-op returning the whole set.
//!
//! Both now compose cumulatively (`product_id` AND status AND search), matching
//! SQLite, and `count_async` mirrors `list_async` so filtered counts agree.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{BomFilter, BomStatus, CreateBom, ProductId};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_bom_list_and_count_compose_product_status_and_search() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping bom filter test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let bom = db.bom();

    // manufacturing_boms.product_id has no FK; fresh ids isolate this test on a
    // shared database.
    let product_a = ProductId::new();
    let product_b = ProductId::new();

    let make = |product: ProductId, name: &str| {
        let bom = &bom;
        let name = name.to_string();
        async move {
            bom.create_async(CreateBom {
                product_id: product,
                name,
                description: None,
                revision: None,
                components: None,
                created_by: None,
            })
            .await
            .expect("create bom")
        }
    };

    // product_a: one Draft "Widget", one "Gadget" promoted to Active.
    make(product_a, "Widget Assembly").await;
    let gadget = make(product_a, "Gadget Assembly").await;
    bom.activate_async(gadget.id).await.expect("activate");
    // product_b: unrelated Draft "Widget" (must never leak into product_a queries).
    make(product_b, "Widget Assembly").await;

    // Assert list length and matching count for a filter in one shot.
    let check = |filter: BomFilter, expected: usize, label: &'static str| {
        let bom = &bom;
        async move {
            let listed = bom.list_async(filter.clone()).await.expect("list");
            assert_eq!(listed.len(), expected, "list {label}");
            assert_eq!(
                bom.count_async(filter).await.expect("count") as usize,
                expected,
                "count {label} must match the filtered list",
            );
        }
    };

    // Baseline: product_a has exactly 2 BOMs.
    check(BomFilter { product_id: Some(product_a), ..Default::default() }, 2, "product_a").await;

    // product_id AND status compose (was: status dropped → 2).
    check(
        BomFilter {
            product_id: Some(product_a),
            status: Some(BomStatus::Active),
            ..Default::default()
        },
        1,
        "product_a + Active",
    )
    .await;
    check(
        BomFilter {
            product_id: Some(product_a),
            status: Some(BomStatus::Draft),
            ..Default::default()
        },
        1,
        "product_a + Draft",
    )
    .await;

    // search composes with product_id (was: search ignored → 2).
    check(
        BomFilter {
            product_id: Some(product_a),
            search: Some("Widget".into()),
            ..Default::default()
        },
        1,
        "product_a + search Widget",
    )
    .await;
    check(
        BomFilter {
            product_id: Some(product_a),
            search: Some("Nonexistent".into()),
            ..Default::default()
        },
        0,
        "product_a + search Nonexistent",
    )
    .await;
}
