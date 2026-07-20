//! Postgres parity for the inspection-result quantity guard.
//!
//! `record_inspection_result` historically wrote `quantity_passed`/
//! `quantity_failed` with no validation, so a caller could record passing/failing
//! more units than were inspected (or negative counts) on both backends. The
//! guard (`passed >= 0`, `failed >= 0`, `passed + failed <= quantity_inspected`)
//! is now enforced on both; this pins the Postgres side.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateInspection, CreateInspectionItem, InspectionResult, InspectionType,
    RecordInspectionResult,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_inspection_result_rejects_over_and_negative() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let inspection = commerce
        .quality()
        .create_inspection(CreateInspection {
            inspection_type: InspectionType::Incoming,
            reference_type: "receipt".into(),
            reference_id: uuid::Uuid::new_v4(),
            inspector_id: Some("QC-001".into()),
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: "QC-SKU".into(),
                lot_number: None,
                serial_number: None,
                quantity_to_inspect: dec!(10),
            }],
        })
        .await
        .expect("create inspection");

    let items = commerce.quality().get_inspection_items(inspection.id).await.expect("get items");
    let item_id = items[0].id;

    // passed + failed (13) exceeds inspected (10) → rejected.
    let err = commerce
        .quality()
        .record_inspection_result(RecordInspectionResult {
            item_id,
            quantity_passed: dec!(8),
            quantity_failed: dec!(5),
            result: InspectionResult::Fail,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect_err("over-inspection must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // negative → rejected.
    let err = commerce
        .quality()
        .record_inspection_result(RecordInspectionResult {
            item_id,
            quantity_passed: dec!(-1),
            quantity_failed: dec!(0),
            result: InspectionResult::Pass,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect_err("negative quantity must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Exactly the inspected quantity is accepted.
    let ok = commerce
        .quality()
        .record_inspection_result(RecordInspectionResult {
            item_id,
            quantity_passed: dec!(7),
            quantity_failed: dec!(3),
            result: InspectionResult::ConditionalPass,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect("valid result accepted");
    assert_eq!(ok.quantity_passed, dec!(7));
    assert_eq!(ok.quantity_failed, dec!(3));
}
