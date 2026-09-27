//! Regression test for the Postgres work-order completion lost-update race.
//!
//! `complete_async` read `quantity_completed` on a pooled connection, added the
//! new units in application code, and wrote the total back on another pooled
//! connection — a read-modify-write with no row lock. Under concurrency two
//! completions could both read the same starting quantity and one overwrite the
//! other, under-counting completed units (a lost update). SQLite serializes via
//! its `IMMEDIATE` transaction; the Postgres path now reads with
//! `SELECT … FOR UPDATE` inside one transaction.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{AddWorkOrderMaterial, CommerceError, CreateWorkOrder, ProductId};
use stateset_db::PostgresDatabase;
use std::sync::Arc;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_concurrent_completions_are_not_lost() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));

    let wo = db
        .work_orders()
        .create_async(CreateWorkOrder {
            product_id: ProductId::new(),
            quantity_to_build: dec!(1000),
            ..Default::default()
        })
        .await
        .expect("create work order");

    // Fire many concurrent single-unit completions at the same work order.
    let contenders = 25u32;
    let mut handles = Vec::with_capacity(contenders as usize);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let id = wo.id;
        handles
            .push(tokio::spawn(async move { db.work_orders().complete_async(id, dec!(1)).await }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join completion task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, contenders, "every completion should succeed: {successes}/{contenders}");

    let wo = db.work_orders().get_async(wo.id).await.expect("get").expect("work order");
    // Each completion adds exactly one unit; with the read+write serialized every
    // one lands, so the total equals the number of completions (no lost updates).
    assert_eq!(wo.quantity_completed, dec!(25), "completions were lost to a race (expected 25)");
}

#[tokio::test]
async fn postgres_competing_completions_cannot_exceed_plan() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let wo = db
        .work_orders()
        .create_async(CreateWorkOrder {
            product_id: ProductId::new(),
            quantity_to_build: dec!(3),
            ..Default::default()
        })
        .await
        .expect("create work order");
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let id = wo.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.work_orders().complete_async(id, dec!(2)).await
        }));
    }
    let mut successes = 0;
    for handle in handles {
        if handle.await.expect("join completion").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1);
    let stored = db.work_orders().get_async(wo.id).await.expect("get").expect("work order");
    assert_eq!(stored.quantity_completed, dec!(2));
}

#[tokio::test]
async fn postgres_material_consumption_is_bounded_and_serialized() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let repo = db.work_orders();
    let wo = repo
        .create_async(CreateWorkOrder {
            product_id: ProductId::new(),
            quantity_to_build: dec!(1),
            ..Default::default()
        })
        .await
        .expect("create work order");
    let mat = repo
        .add_material_async(
            wo.id,
            AddWorkOrderMaterial {
                component_id: None,
                component_sku: "POSTGRES-MATERIAL-RACE".into(),
                component_name: "Concurrent material".into(),
                quantity: dec!(10),
            },
        )
        .await
        .expect("add material");

    let contenders = 10usize;
    let barrier = Arc::new(tokio::sync::Barrier::new(contenders));
    let mut handles = Vec::with_capacity(contenders);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let id = mat.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.work_orders().consume_material_async(id, dec!(1)).await
        }));
    }
    for handle in handles {
        handle.await.expect("join consumer").expect("consume reserved unit");
    }
    let stored = db.work_orders().get_async(wo.id).await.expect("get").expect("work order");
    assert_eq!(stored.materials[0].consumed_quantity, dec!(10));
    let err = db.work_orders().consume_material_async(mat.id, dec!(1)).await.expect_err("full");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let err = db.work_orders().consume_material_async(mat.id, dec!(0)).await.expect_err("zero");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}
