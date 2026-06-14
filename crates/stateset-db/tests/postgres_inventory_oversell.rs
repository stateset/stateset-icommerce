//! Regression tests for the Postgres inventory reservation oversell guard.
//!
//! The Postgres `reserve` path historically did a plain `SELECT` (no `FOR UPDATE`),
//! checked availability in application code, then issued an `UPDATE` whose `WHERE`
//! clause guarded only the optimistic `version`. Under concurrency two transactions
//! could both observe sufficient stock and both succeed, overselling inventory
//! (a TOCTOU race) — unlike the SQLite backend, which atomically re-checks
//! `quantity_available >= ?` inside its `UPDATE`.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`) and
//! are skipped otherwise, so they run only in CI with a provisioned database.

#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{CommerceError, CreateInventoryItem, ReserveInventory};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use std::sync::Arc;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
async fn seed_item(db: &PostgresDatabase, sku: &str, on_hand: rust_decimal::Decimal) {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.to_string(),
            name: "Oversell test widget".into(),
            description: None,
            unit_of_measure: None,
            initial_quantity: Some(on_hand),
            location_id: None,
            reorder_point: None,
            safety_stock: None,
        })
        .await
        .expect("create inventory item");
}

/// Reserving the full stock and then one more unit must fail with `InsufficientStock`,
/// never succeed (which would oversell).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_reserve_rejects_over_available_stock() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping oversell test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let sku = format!("OVERSELL-{}", Uuid::new_v4().simple());
    seed_item(&db, &sku, dec!(5)).await;

    db.inventory()
        .reserve_async(ReserveInventory {
            sku: sku.clone(),
            location_id: None,
            quantity: dec!(5),
            reference_type: "test".into(),
            reference_id: "ref-full".into(),
            expires_in_seconds: None,
        })
        .await
        .expect("reserve full stock");

    let err = db
        .inventory()
        .reserve_async(ReserveInventory {
            sku: sku.clone(),
            location_id: None,
            quantity: dec!(1),
            reference_type: "test".into(),
            reference_id: "ref-over".into(),
            expires_in_seconds: None,
        })
        .await
        .expect_err("reserving beyond available stock must fail");

    assert!(
        matches!(err, CommerceError::InsufficientStock { .. }),
        "expected InsufficientStock, got {err:?}"
    );
}

/// Many concurrent reservers, each requesting one unit, against limited stock must
/// never reserve more units than are on hand. Before the `FOR UPDATE` + stock-guard
/// fix this race could allocate more reservations than available, overselling.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_concurrent_reserve_does_not_oversell() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping concurrent oversell test");
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let sku = format!("OVERSELL-CONC-{}", Uuid::new_v4().simple());
    let available = 10u32;
    seed_item(&db, &sku, dec!(10)).await;

    // Fire more contenders than units exist; each asks for a single unit.
    let contenders = 25u32;
    let mut handles = Vec::with_capacity(contenders as usize);
    for i in 0..contenders {
        let db = Arc::clone(&db);
        let sku = sku.clone();
        handles.push(tokio::spawn(async move {
            db.inventory()
                .reserve_async(ReserveInventory {
                    sku,
                    location_id: None,
                    quantity: dec!(1),
                    reference_type: "test".into(),
                    reference_id: format!("conc-{i}"),
                    expires_in_seconds: None,
                })
                .await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join reserve task").is_ok() {
            successes += 1;
        }
    }

    assert_eq!(
        successes, available,
        "exactly the available units should reserve; got {successes} successes for {available} units (oversell if greater)"
    );

    // The persisted balance must reflect zero remaining availability, never negative.
    let stock = db.inventory().get_stock_async(&sku).await.expect("get stock").expect("stock row");
    assert_eq!(
        stock.total_available,
        dec!(0),
        "available stock must be exactly zero, never negative"
    );
}
