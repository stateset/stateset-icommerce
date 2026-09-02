//! Postgres parity for the quality-control guards:
//!
//! * Q1 — completing an inspection as `Failed` quarantines the inspected lot
//!   (header `reference_type = "lot"` and/or item `lot_number`) in the same
//!   transaction as the verdict;
//! * Q2 — `start_inspection` only from Pending/Scheduled, `complete_inspection`
//!   only from `InProgress` and only once every item has a result, and a quality
//!   hold can only be released once.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateInspection, CreateInspectionItem, CreateLot, CreateQualityHold,
    InspectionResult, InspectionStatus, InspectionType, LotStatus, LotTransactionType,
    RecordInspectionResult, ReleaseQualityHold,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

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

async fn make_lot(db: &PostgresDatabase, sku: &str) -> stateset_core::Lot {
    db.lots()
        .create_async(CreateLot {
            sku: sku.into(),
            lot_number: Some(format!("QL-{}", Uuid::new_v4().simple())),
            quantity: dec!(100),
            ..Default::default()
        })
        .await
        .expect("create lot")
}

async fn inspection_for(
    db: &PostgresDatabase,
    reference_type: &str,
    reference_id: Uuid,
    lots: &[&stateset_core::Lot],
) -> stateset_core::Inspection {
    db.quality()
        .create_inspection_async(CreateInspection {
            inspection_type: InspectionType::Incoming,
            reference_type: reference_type.into(),
            reference_id,
            inspector_id: Some("qa".into()),
            scheduled_at: None,
            notes: None,
            items: lots
                .iter()
                .map(|l| CreateInspectionItem {
                    sku: l.sku.clone(),
                    lot_number: Some(l.lot_number.clone()),
                    serial_number: None,
                    quantity_to_inspect: dec!(10),
                })
                .collect(),
        })
        .await
        .expect("create inspection")
}

async fn record(db: &PostgresDatabase, item_id: Uuid, result: InspectionResult) {
    let (passed, failed) = match result {
        InspectionResult::Pass => (dec!(10), dec!(0)),
        _ => (dec!(0), dec!(10)),
    };
    db.quality()
        .record_inspection_result_async(RecordInspectionResult {
            item_id,
            quantity_passed: passed,
            quantity_failed: failed,
            result,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect("record");
}

#[tokio::test]
async fn postgres_failed_inspection_quarantines_lots() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("Q1-{}", Uuid::new_v4().simple());

    // Header references the lot; item names it too.
    let lot = make_lot(&db, &sku).await;
    let insp = inspection_for(&db, "lot", lot.id, &[&lot]).await;
    db.quality().start_inspection_async(insp.id).await.expect("start");
    record(&db, insp.items[0].id, InspectionResult::Fail).await;
    let done = db.quality().complete_inspection_async(insp.id).await.expect("complete");
    assert_eq!(done.status, InspectionStatus::Failed);
    let after = db.lots().get_async(lot.id).await.unwrap().unwrap();
    assert_eq!(after.status, LotStatus::Quarantine);
    assert_eq!(after.quantity_quarantined, dec!(100));
    let q = db
        .lots()
        .get_transactions_async(lot.id, 10)
        .await
        .expect("txns")
        .into_iter()
        .find(|t| t.transaction_type == LotTransactionType::Quarantined)
        .expect("quarantine transaction");
    assert!(q.reason.unwrap_or_default().contains(&insp.inspection_number));

    // Partial pass: only the failing item's lot is quarantined.
    let bad = make_lot(&db, &sku).await;
    let good = make_lot(&db, &sku).await;
    let insp = inspection_for(&db, "receipt", Uuid::new_v4(), &[&bad, &good]).await;
    db.quality().start_inspection_async(insp.id).await.expect("start");
    let bad_item = insp.items.iter().find(|i| i.lot_number.as_deref() == Some(&bad.lot_number));
    let good_item = insp.items.iter().find(|i| i.lot_number.as_deref() == Some(&good.lot_number));
    record(&db, bad_item.unwrap().id, InspectionResult::Fail).await;
    record(&db, good_item.unwrap().id, InspectionResult::Pass).await;
    let done = db.quality().complete_inspection_async(insp.id).await.expect("complete");
    assert_eq!(done.status, InspectionStatus::PartialPass);
    assert_eq!(db.lots().get_async(bad.id).await.unwrap().unwrap().status, LotStatus::Quarantine);
    assert_eq!(db.lots().get_async(good.id).await.unwrap().unwrap().status, LotStatus::Active);

    // Passed leaves the lot alone; already-quarantined lot is untouched.
    let fine = make_lot(&db, &sku).await;
    let insp = inspection_for(&db, "lot", fine.id, &[&fine]).await;
    db.quality().start_inspection_async(insp.id).await.expect("start");
    record(&db, insp.items[0].id, InspectionResult::Pass).await;
    db.quality().complete_inspection_async(insp.id).await.expect("complete");
    assert_eq!(db.lots().get_async(fine.id).await.unwrap().unwrap().status, LotStatus::Active);
}

#[tokio::test]
async fn postgres_inspection_transitions_are_guarded() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let sku = format!("Q2-{}", Uuid::new_v4().simple());
    let lot = make_lot(&db, &sku).await;
    let insp = inspection_for(&db, "receipt", Uuid::new_v4(), &[&lot]).await;

    let err = db.quality().complete_inspection_async(insp.id).await.expect_err("not started");
    assert_validation_mentions(&err, &[&insp.inspection_number, "pending"]);

    let started = db.quality().start_inspection_async(insp.id).await.expect("start");
    let started_at = started.started_at.expect("started_at");
    let err = db.quality().start_inspection_async(insp.id).await.expect_err("restart");
    assert_validation_mentions(&err, &[&insp.inspection_number, "in_progress"]);
    assert_eq!(
        db.quality().get_inspection_async(insp.id).await.unwrap().unwrap().started_at,
        Some(started_at)
    );

    let err = db.quality().complete_inspection_async(insp.id).await.expect_err("pending item");
    assert_validation_mentions(&err, &[&insp.inspection_number, "pending"]);
    let mid = db.quality().get_inspection_async(insp.id).await.unwrap().unwrap();
    assert_eq!(mid.status, InspectionStatus::InProgress);
    assert!(mid.completed_at.is_none());

    record(&db, insp.items[0].id, InspectionResult::Pass).await;
    let done = db.quality().complete_inspection_async(insp.id).await.expect("complete");
    assert_eq!(done.status, InspectionStatus::Passed);
    let err = db.quality().complete_inspection_async(insp.id).await.expect_err("twice");
    assert_validation_mentions(&err, &["passed"]);
    let err = db.quality().start_inspection_async(insp.id).await.expect_err("restart passed");
    assert_validation_mentions(&err, &["passed"]);
    assert!(matches!(
        db.quality().start_inspection_async(Uuid::new_v4()).await.expect_err("unknown"),
        CommerceError::NotFound
    ));
}

#[tokio::test]
async fn postgres_release_hold_only_once() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let hold = db
        .quality()
        .create_hold_async(CreateQualityHold {
            sku: format!("HOLD-{}", Uuid::new_v4().simple()),
            quantity: dec!(1),
            reason: "r".into(),
            placed_by: "qa".into(),
            ..Default::default()
        })
        .await
        .expect("hold");
    let released = db
        .quality()
        .release_hold_async(
            hold.id,
            ReleaseQualityHold { released_by: "qa-1".into(), release_notes: None },
        )
        .await
        .expect("release");
    let released_at = released.released_at.expect("released_at");
    let err = db
        .quality()
        .release_hold_async(
            hold.id,
            ReleaseQualityHold { released_by: "qa-2".into(), release_notes: None },
        )
        .await
        .expect_err("re-release");
    assert_validation_mentions(&err, &["already released"]);
    let after = db.quality().get_hold_async(hold.id).await.unwrap().unwrap();
    assert_eq!(after.released_by.as_deref(), Some("qa-1"));
    assert_eq!(after.released_at, Some(released_at));
    assert!(matches!(
        db.quality()
            .release_hold_async(
                Uuid::new_v4(),
                ReleaseQualityHold { released_by: "x".into(), release_notes: None }
            )
            .await
            .expect_err("unknown"),
        CommerceError::NotFound
    ));
}
