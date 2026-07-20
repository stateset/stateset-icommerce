//! Regression test for the Postgres lot-consumption oversell guard.
//!
//! `consume_async`/`reserve_async`/`adjust_async` loaded the lot row inside a
//! transaction but with a plain `SELECT` (no `FOR UPDATE`), then checked
//! availability in application code and wrote the new `quantity_remaining`. Under
//! concurrency two consumers could both read the same remaining quantity, both
//! pass `can_consume`, and both write — over-consuming the lot (a TOCTOU race).
//! The sibling `confirm_reservation`/`transfer` already used `FOR UPDATE`, and the
//! SQLite backend serializes via its single `conn.transaction()`. All three now
//! lock the lot row.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CommerceError, ConsumeLot, CreateLot};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// Sequential guard: consuming the whole lot then one more unit must fail with
/// `InsufficientStock`, never over-consume.
#[tokio::test]
async fn postgres_consume_rejects_over_available() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("LOT-{}", Uuid::new_v4().simple());
    let lot = db
        .lots()
        .create_async(CreateLot { sku, quantity: dec!(5), ..Default::default() })
        .await
        .expect("create lot");

    let consume = |qty| ConsumeLot {
        lot_id: lot.id,
        quantity: qty,
        reference_type: "test".into(),
        reference_id: Uuid::new_v4(),
        location_id: None,
        performed_by: None,
    };

    db.lots().consume_async(consume(dec!(5))).await.expect("consume full lot");
    let err = db
        .lots()
        .consume_async(consume(dec!(1)))
        .await
        .expect_err("consuming beyond the lot must fail");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");
}

/// Many concurrent consumers, each taking one unit from a 10-unit lot, must never
/// consume more than exists: exactly 10 succeed and the lot ends at zero, never
/// negative. Before the `FOR UPDATE` fix the race let more than 10 succeed.
#[tokio::test]
async fn postgres_concurrent_consume_does_not_over_consume() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let sku = format!("LOT-CONC-{}", Uuid::new_v4().simple());
    let available = 10u32;
    let lot = db
        .lots()
        .create_async(CreateLot { sku, quantity: dec!(10), ..Default::default() })
        .await
        .expect("create lot");

    let contenders = 25u32;
    let mut handles = Vec::with_capacity(contenders as usize);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let lot_id = lot.id;
        handles.push(tokio::spawn(async move {
            db.lots()
                .consume_async(ConsumeLot {
                    lot_id,
                    quantity: dec!(1),
                    reference_type: "test".into(),
                    reference_id: Uuid::new_v4(),
                    location_id: None,
                    performed_by: None,
                })
                .await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join consume task").is_ok() {
            successes += 1;
        }
    }

    assert_eq!(
        successes, available,
        "exactly the available units should consume; got {successes} for {available} (over-consume if greater)"
    );

    let lot = db.lots().get_async(lot.id).await.expect("get lot").expect("lot row");
    assert_eq!(
        lot.quantity_remaining,
        dec!(0),
        "lot remaining must be exactly zero, never negative"
    );
}
