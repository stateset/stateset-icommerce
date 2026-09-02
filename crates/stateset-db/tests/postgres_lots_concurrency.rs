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

// ============================================================================
// L1–L5: reservation / quarantine / expiry guards (Postgres parity)
// ============================================================================

fn assert_validation_mentions(err: &CommerceError, needles: &[&str]) {
    match err {
        CommerceError::ValidationError(msg) => {
            for needle in needles {
                assert!(msg.contains(needle), "expected {needle:?} in {msg:?}");
            }
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

async fn create_lot(
    db: &PostgresDatabase,
    sku: &str,
    qty: rust_decimal::Decimal,
    expiration_date: Option<chrono::DateTime<chrono::Utc>>,
) -> stateset_core::Lot {
    db.lots()
        .create_async(CreateLot {
            sku: sku.into(),
            lot_number: Some(format!("L-{}", Uuid::new_v4().simple())),
            quantity: qty,
            expiration_date,
            ..Default::default()
        })
        .await
        .expect("create lot")
}

async fn reserve_units(
    db: &PostgresDatabase,
    lot: &stateset_core::Lot,
    qty: rust_decimal::Decimal,
    expires_in_seconds: Option<i64>,
) -> Uuid {
    db.lots()
        .reserve_async(stateset_core::ReserveLot {
            lot_id: lot.id,
            quantity: qty,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds,
        })
        .await
        .expect("reserve")
}

/// L1: confirming a reservation on a quarantined lot is refused until the lot
/// is released; the reservation survives quarantine.
#[tokio::test]
async fn postgres_confirm_reservation_refuses_quarantined_lot_until_released() {
    use stateset_core::LotStatus;
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let lot = create_lot(&db, &format!("L1-{}", Uuid::new_v4().simple()), dec!(100), None).await;
    let res = reserve_units(&db, &lot, dec!(30), None).await;
    let q = db.lots().quarantine_async(lot.id, "qc fail").await.expect("quarantine");
    assert_eq!(q.quantity_quarantined, dec!(70));

    let err = db.lots().confirm_reservation_async(res).await.expect_err("blocked stock");
    assert_validation_mentions(&err, &[&lot.lot_number, "quarantine"]);
    let after = db.lots().get_async(lot.id).await.expect("ok").expect("found");
    assert_eq!(after.quantity_remaining, dec!(100));
    assert_eq!(after.quantity_reserved, dec!(30));

    db.lots().release_quarantine_async(lot.id).await.expect("release");
    db.lots().confirm_reservation_async(res).await.expect("confirm after release");
    let done = db.lots().get_async(lot.id).await.expect("ok").expect("found");
    assert_eq!(done.quantity_remaining, dec!(70));
    assert_eq!(done.quantity_reserved, dec!(0));
    assert_eq!(done.status, LotStatus::Active);
}

/// Releasing a reservation while quarantined folds the units into the
/// quarantined count.
#[tokio::test]
async fn postgres_release_reservation_under_quarantine_folds_into_quarantine() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let lot = create_lot(&db, &format!("L1R-{}", Uuid::new_v4().simple()), dec!(100), None).await;
    let res = reserve_units(&db, &lot, dec!(30), None).await;
    db.lots().quarantine_async(lot.id, "qc").await.expect("quarantine");
    db.lots().release_reservation_async(res).await.expect("release");
    let after = db.lots().get_async(lot.id).await.expect("ok").expect("found");
    assert_eq!(after.quantity_reserved, dec!(0));
    assert_eq!(after.quantity_quarantined, dec!(100));
}

/// L2: quarantine only from Active/OnHold; release only from Quarantine.
#[tokio::test]
async fn postgres_quarantine_and_release_are_state_guarded() {
    use stateset_core::{LotStatus, LotTransactionType, UpdateLot};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("L2-{}", Uuid::new_v4().simple());

    // Double quarantine keeps the count and writes one transaction.
    let lot = create_lot(&db, &sku, dec!(50), None).await;
    db.lots().quarantine_async(lot.id, "first").await.expect("quarantine");
    let err = db.lots().quarantine_async(lot.id, "again").await.expect_err("double");
    assert_validation_mentions(&err, &[&lot.lot_number, "quarantine"]);
    let after = db.lots().get_async(lot.id).await.expect("ok").expect("found");
    assert_eq!(after.quantity_quarantined, dec!(50));
    let n = db
        .lots()
        .get_transactions_async(lot.id, 50)
        .await
        .expect("txns")
        .iter()
        .filter(|t| t.transaction_type == LotTransactionType::Quarantined)
        .count();
    assert_eq!(n, 1);

    // OnHold may be quarantined; terminal states may not.
    let held = create_lot(&db, &sku, dec!(5), None).await;
    db.lots()
        .update_async(held.id, UpdateLot { status: Some(LotStatus::OnHold), ..Default::default() })
        .await
        .expect("hold");
    assert_eq!(
        db.lots().quarantine_async(held.id, "escalate").await.expect("ok").status,
        LotStatus::Quarantine
    );
    for status in
        [LotStatus::Scrapped, LotStatus::Consumed, LotStatus::Recalled, LotStatus::Expired]
    {
        let l = create_lot(&db, &sku, dec!(5), None).await;
        db.lots()
            .update_async(l.id, UpdateLot { status: Some(status), ..Default::default() })
            .await
            .expect("set");
        let err = db.lots().quarantine_async(l.id, "nope").await.expect_err("refuse");
        assert_validation_mentions(&err, &[&l.lot_number, &status.to_string()]);
        // …and none of them can be resurrected through release_quarantine.
        let err = db.lots().release_quarantine_async(l.id).await.expect_err("no resurrection");
        assert_validation_mentions(&err, &[&l.lot_number, &status.to_string()]);
        assert_eq!(db.lots().get_async(l.id).await.unwrap().unwrap().status, status);
    }
    let active = create_lot(&db, &sku, dec!(5), None).await;
    let err = db.lots().release_quarantine_async(active.id).await.expect_err("not quarantined");
    assert_validation_mentions(&err, &[&active.lot_number, "active"]);
    assert!(matches!(
        db.lots().release_quarantine_async(Uuid::new_v4()).await.expect_err("unknown"),
        CommerceError::NotFound
    ));
}

/// L3: expiry enforced on consume / reserve / confirm before the sweeper runs;
/// `expire_lots` flips only Active lots past expiry.
#[tokio::test]
async fn postgres_expiry_is_enforced_and_swept() {
    use chrono::{Duration, Utc};
    use stateset_core::{LotStatus, UpdateLot};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("L3-{}", Uuid::new_v4().simple());

    let lot = create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(1))).await;
    let res = reserve_units(&db, &lot, dec!(3), None).await;
    db.lots()
        .update_async(
            lot.id,
            UpdateLot {
                expiration_date: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            },
        )
        .await
        .expect("expire");
    let err = db
        .lots()
        .consume_async(ConsumeLot {
            lot_id: lot.id,
            quantity: dec!(1),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
        .await
        .expect_err("consume expired");
    assert_validation_mentions(&err, &[&lot.lot_number, "expired"]);
    let err = db
        .lots()
        .reserve_async(stateset_core::ReserveLot {
            lot_id: lot.id,
            quantity: dec!(1),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .await
        .expect_err("reserve expired");
    assert_validation_mentions(&err, &[&lot.lot_number, "expired"]);
    let err = db.lots().confirm_reservation_async(res).await.expect_err("confirm expired lot");
    assert_validation_mentions(&err, &[&lot.lot_number, "expired"]);
    assert_eq!(db.lots().get_async(lot.id).await.unwrap().unwrap().quantity_remaining, dec!(10));

    // Sweeper: our expired lot flips; a quarantined-but-expired lot does not.
    let quarantined = create_lot(&db, &sku, dec!(10), Some(Utc::now() - Duration::days(1))).await;
    db.lots().quarantine_async(quarantined.id, "qc").await.expect("quarantine");
    let future = create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(30))).await;
    let flipped = db.lots().expire_lots_async(Utc::now()).await.expect("sweep");
    assert!(flipped >= 1, "at least our lot; other tests' lots may be swept too");
    assert_eq!(db.lots().get_async(lot.id).await.unwrap().unwrap().status, LotStatus::Expired);
    assert_eq!(
        db.lots().get_async(quarantined.id).await.unwrap().unwrap().status,
        LotStatus::Quarantine
    );
    assert_eq!(db.lots().get_async(future.id).await.unwrap().unwrap().status, LotStatus::Active);
}

/// L4: FEFO picking order, excluding expired / blocked / fully reserved lots.
#[tokio::test]
async fn postgres_available_lots_for_sku_is_fefo() {
    use chrono::{Duration, Utc};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("L4-{}", Uuid::new_v4().simple());
    let no_exp_old = create_lot(&db, &sku, dec!(10), None).await;
    let late = create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(60))).await;
    let soon = create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(5))).await;
    let no_exp_new = create_lot(&db, &sku, dec!(10), None).await;
    let _expired = create_lot(&db, &sku, dec!(10), Some(Utc::now() - Duration::days(1))).await;
    let fully_reserved =
        create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(2))).await;
    reserve_units(&db, &fully_reserved, dec!(10), None).await;
    let quarantined = create_lot(&db, &sku, dec!(10), Some(Utc::now() + Duration::days(1))).await;
    db.lots().quarantine_async(quarantined.id, "qc").await.expect("quarantine");

    let picked: Vec<Uuid> = db
        .lots()
        .get_available_lots_for_sku_async(&sku)
        .await
        .expect("ok")
        .iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(picked, vec![soon.id, late.id, no_exp_old.id, no_exp_new.id]);
}

/// L5: confirming the last units marks the lot Consumed; an expired
/// reservation cannot be confirmed but can be released.
#[tokio::test]
async fn postgres_confirm_marks_consumed_and_refuses_expired_reservation() {
    use stateset_core::LotStatus;
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("L5-{}", Uuid::new_v4().simple());

    let lot = create_lot(&db, &sku, dec!(10), None).await;
    let res = reserve_units(&db, &lot, dec!(10), None).await;
    db.lots().confirm_reservation_async(res).await.expect("confirm");
    let after = db.lots().get_async(lot.id).await.unwrap().unwrap();
    assert_eq!(after.quantity_remaining, dec!(0));
    assert_eq!(after.status, LotStatus::Consumed);

    let lot = create_lot(&db, &sku, dec!(10), None).await;
    let res = reserve_units(&db, &lot, dec!(4), Some(-60)).await;
    let err = db.lots().confirm_reservation_async(res).await.expect_err("expired reservation");
    assert_validation_mentions(&err, &["expired"]);
    let mid = db.lots().get_async(lot.id).await.unwrap().unwrap();
    assert_eq!(mid.quantity_reserved, dec!(4), "held until released");
    db.lots().release_reservation_async(res).await.expect("release frees units");
    let done = db.lots().get_async(lot.id).await.unwrap().unwrap();
    assert_eq!(done.quantity_reserved, dec!(0));
    assert_eq!(done.quantity_remaining, dec!(10));
}
