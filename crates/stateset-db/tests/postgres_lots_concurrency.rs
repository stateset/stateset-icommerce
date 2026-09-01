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

/// Regression: `merge_async` ignored the source lots' status and hard-coded the
/// merged lot as `active`, so merging a quarantined lot with an active one
/// laundered the quarantined units into sellable stock. Merge now refuses any
/// non-active source and leaves both lots untouched.
#[tokio::test]
async fn postgres_merge_refuses_quarantined_source_lot() {
    use stateset_core::{LotStatus, MergeLots};

    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("LOT-MERGE-{}", Uuid::new_v4().simple());
    let active = db
        .lots()
        .create_async(CreateLot {
            sku: sku.clone(),
            lot_number: Some(format!("ACTIVE-{}", Uuid::new_v4().simple())),
            quantity: dec!(30),
            ..Default::default()
        })
        .await
        .expect("create active lot");
    let quarantined = db
        .lots()
        .create_async(CreateLot {
            sku,
            lot_number: Some(format!("QUAR-{}", Uuid::new_v4().simple())),
            quantity: dec!(20),
            ..Default::default()
        })
        .await
        .expect("create second lot");
    db.lots().quarantine_async(quarantined.id, "qc fail").await.expect("quarantine");

    let target = format!("MERGED-{}", Uuid::new_v4().simple());
    let err = db
        .lots()
        .merge_async(MergeLots {
            source_lot_ids: vec![active.id, quarantined.id],
            target_lot_number: Some(target.clone()),
            reason: None,
        })
        .await
        .expect_err("quarantined stock must not be merged into an active lot");
    match &err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(&quarantined.lot_number), "got {msg}");
            assert!(msg.contains("quarantine"), "got {msg}");
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }

    let a = db.lots().get_async(active.id).await.expect("ok").expect("found");
    let q = db.lots().get_async(quarantined.id).await.expect("ok").expect("found");
    assert_eq!(a.status, LotStatus::Active);
    assert_eq!(a.quantity_remaining, dec!(30));
    assert_eq!(q.status, LotStatus::Quarantine);
    assert_eq!(q.quantity_remaining, dec!(20));
    assert!(db.lots().get_by_number_async(&target).await.expect("ok").is_none());
}
