//! Postgres integration coverage for cycle counts: draft → in-progress →
//! completed flow, variance computation, and variance application to
//! `location_inventory`.
//!
//! Driven entirely through the public `AsyncWarehouse` cycle count accessors
//! (`create_cycle_count` / `start_cycle_count` / `record_cycle_counts` /
//! `complete_cycle_count` / `cancel_cycle_count`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustLocationInventory, CreateCycleCount, CreateCycleCountLine, CreateLocation,
    CreateWarehouse, CycleCountFilter, CycleCountStatus, LocationType, RecordCycleCountLine,
    WarehouseAddress, WarehouseType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn setup_location(commerce: &AsyncCommerce) -> (i32, i32) {
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("CC-{suffix}"),
            name: format!("Cycle Count WH {suffix}"),
            warehouse_type: WarehouseType::Distribution,
            address: WarehouseAddress {
                street1: "1 Count St".into(),
                street2: None,
                city: "Palo Alto".into(),
                state: "CA".into(),
                postal_code: "94301".into(),
                country: "US".into(),
                phone: None,
            },
            timezone: None,
        })
        .await
        .expect("create warehouse");
    let location = commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(format!("BIN-{suffix}")),
            location_type: LocationType::Pick,
            zone: None,
            aisle: None,
            rack: None,
            level: None,
            bin: None,
            max_weight_kg: None,
            max_volume_m3: None,
            is_pickable: Some(true),
            is_receivable: Some(true),
        })
        .await
        .expect("create location");
    (warehouse.id, location.id)
}

async fn on_hand(commerce: &AsyncCommerce, location_id: i32, sku: &str) -> Decimal {
    commerce
        .warehouse()
        .get_location_inventory(location_id)
        .await
        .expect("get location inventory")
        .into_iter()
        .find(|inv| inv.sku == sku)
        .map(|inv| inv.quantity_on_hand)
        .unwrap_or(Decimal::ZERO)
}

#[tokio::test]
async fn postgres_cycle_count_flow_applies_variance_to_inventory() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping cycle count flow test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let counts = commerce.warehouse();
    let (warehouse_id, location_id) = setup_location(&commerce).await;
    let sku = format!("CC-SKU-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);

    // Seed 100 units at the location.
    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(100),
            reason: "seed for cycle count test".into(),
            reference_type: None,
            reference_id: None,
            performed_by: Some("pg-test".into()),
        })
        .await
        .expect("seed inventory");
    assert_eq!(on_hand(&commerce, location_id, &sku).await, dec!(100));

    let count = counts
        .create_cycle_count(CreateCycleCount {
            warehouse_id,
            location_id: Some(location_id),
            scheduled_date: None,
            counted_by: Some("counter@example.com".into()),
            lines: vec![CreateCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                expected_quantity: dec!(100),
            }],
        })
        .await
        .expect("create cycle count");
    assert_eq!(count.status, CycleCountStatus::Draft);
    assert_eq!(count.lines.len(), 1);

    // Draft counts cannot record lines or complete.
    assert!(
        counts
            .record_cycle_counts(
                count.id,
                vec![RecordCycleCountLine {
                    sku: sku.clone(),
                    lot_id: None,
                    counted_quantity: dec!(90),
                }],
            )
            .await
            .is_err(),
        "recording against a draft count is rejected"
    );
    assert!(
        counts.complete_cycle_count(count.id).await.is_err(),
        "completing a draft count is rejected"
    );

    counts.start_cycle_count(count.id).await.expect("start cycle count");
    let count = counts
        .get_cycle_count(count.id)
        .await
        .expect("get cycle count")
        .expect("cycle count exists");
    assert_eq!(count.status, CycleCountStatus::InProgress);

    // Completing before all lines are counted is rejected.
    assert!(counts.complete_cycle_count(count.id).await.is_err());

    let count = counts
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                counted_quantity: dec!(90),
            }],
        )
        .await
        .expect("record counts");
    assert_eq!(count.lines[0].counted_quantity, Some(dec!(90)));

    let completed = counts.complete_cycle_count(count.id).await.expect("complete count");
    assert_eq!(completed.status, CycleCountStatus::Completed);
    assert!(completed.completed_at.is_some());
    assert_eq!(completed.lines[0].variance, Some(dec!(-10)), "variance = counted - expected");

    // The -10 variance is applied to location inventory.
    assert_eq!(on_hand(&commerce, location_id, &sku).await, dec!(90));

    // Completed counts are terminal.
    assert!(counts.complete_cycle_count(count.id).await.is_err());

    // Round-trip + filter.
    let listed = counts
        .list_cycle_counts(CycleCountFilter {
            warehouse_id: Some(warehouse_id),
            status: Some(CycleCountStatus::Completed),
            ..Default::default()
        })
        .await
        .expect("list cycle counts");
    assert!(listed.iter().any(|c| c.id == count.id));
}

#[tokio::test]
async fn postgres_cycle_count_negative_variance_cannot_underflow_inventory() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping cycle count underflow test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let counts = commerce.warehouse();
    let (warehouse_id, location_id) = setup_location(&commerce).await;
    let sku = format!("CC-SKU-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);

    // Seed only 5 units, but expect 100: counting 0 would apply a -100
    // variance and drive inventory negative, which must be rejected.
    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(5),
            reason: "seed for underflow test".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .await
        .expect("seed inventory");

    let count = counts
        .create_cycle_count(CreateCycleCount {
            warehouse_id,
            location_id: Some(location_id),
            scheduled_date: None,
            counted_by: None,
            lines: vec![CreateCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                expected_quantity: dec!(100),
            }],
        })
        .await
        .expect("create cycle count");
    counts.start_cycle_count(count.id).await.expect("start cycle count");
    counts
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                counted_quantity: dec!(0),
            }],
        )
        .await
        .expect("record counts");

    let err = counts.complete_cycle_count(count.id).await;
    assert!(err.is_err(), "variance that would drive inventory negative is rejected");

    // Inventory and count status are untouched by the failed completion.
    assert_eq!(on_hand(&commerce, location_id, &sku).await, dec!(5));
    let fetched = counts
        .get_cycle_count(count.id)
        .await
        .expect("get cycle count")
        .expect("cycle count exists");
    assert_eq!(fetched.status, CycleCountStatus::InProgress);
}

#[tokio::test]
async fn postgres_cycle_count_start_and_cancel_via_async_api() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping cycle count start/cancel test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let counts = commerce.warehouse();
    let (warehouse_id, location_id) = setup_location(&commerce).await;
    let sku = format!("CC-SKU-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);

    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(50),
            reason: "seed for start/cancel test".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .await
        .expect("seed inventory");

    let count = counts
        .create_cycle_count(CreateCycleCount {
            warehouse_id,
            location_id: Some(location_id),
            scheduled_date: None,
            counted_by: None,
            lines: vec![CreateCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                expected_quantity: dec!(50),
            }],
        })
        .await
        .expect("create cycle count");
    assert_eq!(count.status, CycleCountStatus::Draft);

    // draft → in_progress via the public async API.
    let started = counts.start_cycle_count(count.id).await.expect("start cycle count");
    assert_eq!(started.status, CycleCountStatus::InProgress);

    // Starting again is an invalid transition.
    assert!(counts.start_cycle_count(count.id).await.is_err());

    // Record a variance, then cancel: no adjustment may be applied.
    counts
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                counted_quantity: dec!(10),
            }],
        )
        .await
        .expect("record counts");
    let cancelled = counts.cancel_cycle_count(count.id).await.expect("cancel cycle count");
    assert_eq!(cancelled.status, CycleCountStatus::Cancelled);
    assert_eq!(on_hand(&commerce, location_id, &sku).await, dec!(50), "cancel applies no variance");

    // Cancelled counts are terminal: no restart, complete, or re-cancel.
    assert!(counts.start_cycle_count(count.id).await.is_err());
    assert!(counts.complete_cycle_count(count.id).await.is_err());
    assert!(counts.cancel_cycle_count(count.id).await.is_err());

    // Unknown IDs are NotFound errors.
    assert!(counts.start_cycle_count(uuid::Uuid::new_v4()).await.is_err());
    assert!(counts.cancel_cycle_count(uuid::Uuid::new_v4()).await.is_err());
}
