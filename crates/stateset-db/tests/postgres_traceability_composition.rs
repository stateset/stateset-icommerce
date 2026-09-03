//! Postgres parity for the traceability *composition* guarantees:
//!
//! * a lot quarantine (direct, or via a failed inspection) quarantines the
//!   lot's `Available`/`Reserved` serials in the same transaction and releasing
//!   the lot returns them; shipped serials are untouched;
//! * `split` and `merge` re-attribute placements between lots (and record the
//!   parent/child genealogy) without moving stock, so the invariant survives
//!   both and a merged lot traces back to every source;
//! * lot movements are mirrored onto `inventory_balances` for the lot's
//!   `(sku, location)` — the invariant `Σ active lots available == inventory
//!   available` holds after every operation;
//! * expired lot reservations are released lazily by `reserve` / `confirm`
//!   and by the `release_expired_reservations` sweeper;
//! * failed-inspection lot resolution is scoped by the item's SKU;
//! * `update` refuses quarantine transitions; a confirmed-but-unshipped serial
//!   reservation can still be released.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustLot, CommerceError, ConsumeLot, CreateInspection, CreateInspectionItem,
    CreateInventoryItem, CreateLot, CreateSerialNumber, InspectionResult, InspectionStatus,
    InspectionType, LotFilter, LotRelationship, LotStatus, MergeLots, RecordInspectionResult,
    ReserveLot, ReserveSerialNumber, SerialStatus, SplitLot, TraceNodeType, TransactionType,
    TransferLot, UpdateLot,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

macro_rules! db_or_skip {
    () => {
        match postgres_url() {
            Some(url) => PostgresDatabase::connect(&url).await.expect("connect + migrate"),
            None => {
                eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

fn sku(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn make_lot(
    db: &PostgresDatabase,
    sku: &str,
    qty: Decimal,
    location: Option<i32>,
) -> stateset_core::Lot {
    db.lots()
        .create_async(CreateLot {
            sku: sku.into(),
            lot_number: Some(format!("TC-{}", Uuid::new_v4().simple())),
            quantity: qty,
            initial_location_id: location,
            ..Default::default()
        })
        .await
        .expect("create lot")
}

async fn make_serial(db: &PostgresDatabase, sku: &str, lot: &stateset_core::Lot) -> Uuid {
    db.serials()
        .create_async(CreateSerialNumber {
            serial: Some(format!("SN-{}", Uuid::new_v4().simple())),
            sku: sku.to_string(),
            lot_id: Some(lot.id),
            lot_number: Some(lot.lot_number.clone()),
            location_id: Some(1),
            ..Default::default()
        })
        .await
        .expect("create serial")
        .id
}

async fn serial_status(db: &PostgresDatabase, id: Uuid) -> SerialStatus {
    db.serials().get_async(id).await.expect("get").expect("exists").status
}

async fn reserve_lot(db: &PostgresDatabase, lot_id: Uuid, qty: Decimal, ttl: Option<i64>) -> Uuid {
    db.lots()
        .reserve_async(ReserveLot {
            lot_id,
            quantity: qty,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: ttl,
        })
        .await
        .expect("reserve")
}

async fn inventory_item(db: &PostgresDatabase, sku: &str) -> i64 {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Item {sku}"),
            description: None,
            unit_of_measure: None,
            initial_quantity: None,
            location_id: Some(1),
            reorder_point: None,
            safety_stock: None,
        })
        .await
        .expect("inventory item")
        .id
}

async fn balance(db: &PostgresDatabase, item_id: i64, location: i32) -> (Decimal, Decimal) {
    let b = db
        .inventory()
        .get_balance_async(item_id, location)
        .await
        .expect("balance")
        .unwrap_or_else(|| panic!("balance {item_id}@{location}"));
    (b.quantity_on_hand, b.quantity_available)
}

async fn assert_invariant(
    db: &PostgresDatabase,
    sku: &str,
    item_id: i64,
    location: i32,
    step: &str,
) {
    let lots = db
        .lots()
        .list_async(LotFilter { sku: Some(sku.into()), ..Default::default() })
        .await
        .expect("list");
    let mut expected = Decimal::ZERO;
    for lot in lots.iter().filter(|l| l.status == LotStatus::Active) {
        if db.lots().get_quantity_at_location_async(lot.id, location).await.unwrap().is_some() {
            expected += lot.quantity_available();
        }
    }
    let (_, available) = balance(db, item_id, location).await;
    assert_eq!(available, expected, "invariant broken after {step}");
}

/// Available + reserved + shipped serials; quarantine → the first two are
/// quarantined (reservation closed), shipped untouched; release restores.
#[tokio::test]
async fn postgres_lot_quarantine_cascades_to_serials_and_release_restores() {
    let db = db_or_skip!();
    let sku = sku("TC-QSER");
    let lot = make_lot(&db, &sku, dec!(3), Some(1)).await;
    let available = make_serial(&db, &sku, &lot).await;
    let reserved = make_serial(&db, &sku, &lot).await;
    let shipped = make_serial(&db, &sku, &lot).await;
    let reservation = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            serial_id: reserved,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            reserved_by: None,
            expires_in_seconds: None,
        })
        .await
        .expect("reserve serial");
    db.serials().mark_shipped_async(shipped, Uuid::new_v4()).await.expect("ship");

    db.lots().quarantine_async(lot.id, "recall").await.expect("quarantine");
    assert_eq!(serial_status(&db, available).await, SerialStatus::Quarantined);
    assert_eq!(serial_status(&db, reserved).await, SerialStatus::Quarantined);
    assert_eq!(serial_status(&db, shipped).await, SerialStatus::Shipped);
    let res = db.serials().get_reservation_async(reservation.id).await.unwrap().unwrap();
    assert!(res.released_at.is_some(), "open reservation closed by the quarantine");

    db.lots().release_quarantine_async(lot.id).await.expect("release");
    assert_eq!(serial_status(&db, available).await, SerialStatus::Available);
    assert_eq!(serial_status(&db, reserved).await, SerialStatus::Available);
    assert_eq!(serial_status(&db, shipped).await, SerialStatus::Shipped);
}

#[tokio::test]
async fn postgres_failed_inspection_quarantines_serials_and_holds_inventory() {
    let db = db_or_skip!();
    let sku = sku("TC-QINS");
    let item = inventory_item(&db, &sku).await;
    let lot = make_lot(&db, &sku, dec!(2), Some(1)).await;
    let serial = make_serial(&db, &sku, &lot).await;
    assert_eq!(balance(&db, item, 1).await, (dec!(2), dec!(2)));

    let insp = db
        .quality()
        .create_inspection_async(CreateInspection {
            inspection_type: InspectionType::Incoming,
            reference_type: "lot".into(),
            reference_id: lot.id,
            inspector_id: None,
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: sku.clone(),
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity_to_inspect: dec!(2),
            }],
        })
        .await
        .expect("inspection");
    db.quality().start_inspection_async(insp.id).await.expect("start");
    db.quality()
        .record_inspection_result_async(RecordInspectionResult {
            item_id: insp.items[0].id,
            quantity_passed: dec!(0),
            quantity_failed: dec!(2),
            result: InspectionResult::Fail,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect("record");
    let done = db.quality().complete_inspection_async(insp.id).await.expect("complete");
    assert_eq!(done.status, InspectionStatus::Failed);

    assert_eq!(db.lots().get_async(lot.id).await.unwrap().unwrap().status, LotStatus::Quarantine);
    assert_eq!(serial_status(&db, serial).await, SerialStatus::Quarantined);
    assert_eq!(balance(&db, item, 1).await, (dec!(2), dec!(0)), "held, still on hand");

    db.lots().release_quarantine_async(lot.id).await.expect("release");
    assert_eq!(serial_status(&db, serial).await, SerialStatus::Available);
    assert_eq!(balance(&db, item, 1).await, (dec!(2), dec!(2)));
}

#[tokio::test]
async fn postgres_lot_lifecycle_keeps_inventory_in_step() {
    let db = db_or_skip!();
    let sku = sku("TC-LINK");
    let item = inventory_item(&db, &sku).await;
    let lots = db.lots();

    let lot = make_lot(&db, &sku, dec!(100), Some(1)).await;
    assert_eq!(balance(&db, item, 1).await, (dec!(100), dec!(100)));
    assert_invariant(&db, &sku, item, 1, "create").await;

    let res = reserve_lot(&db, lot.id, dec!(30), None).await;
    assert_eq!(balance(&db, item, 1).await.1, dec!(70));
    assert_invariant(&db, &sku, item, 1, "reserve").await;

    lots.consume_async(ConsumeLot {
        lot_id: lot.id,
        quantity: dec!(10),
        reference_type: "work_order".into(),
        reference_id: Uuid::new_v4(),
        location_id: None,
        performed_by: None,
    })
    .await
    .expect("consume");
    assert_eq!(balance(&db, item, 1).await.0, dec!(90));
    assert_invariant(&db, &sku, item, 1, "consume").await;

    lots.adjust_async(AdjustLot {
        lot_id: lot.id,
        quantity_change: dec!(-5),
        reason: "damaged".into(),
        ..Default::default()
    })
    .await
    .expect("adjust down");
    lots.adjust_async(AdjustLot {
        lot_id: lot.id,
        quantity_change: dec!(2),
        reason: "found".into(),
        ..Default::default()
    })
    .await
    .expect("adjust up");
    assert_eq!(balance(&db, item, 1).await.0, dec!(87));
    assert_invariant(&db, &sku, item, 1, "adjust").await;

    lots.release_reservation_async(res).await.expect("release");
    assert_invariant(&db, &sku, item, 1, "release").await;

    let res2 = reserve_lot(&db, lot.id, dec!(20), None).await;
    lots.confirm_reservation_async(res2).await.expect("confirm");
    assert_eq!(balance(&db, item, 1).await.0, dec!(67));
    assert_invariant(&db, &sku, item, 1, "confirm").await;

    let res3 = reserve_lot(&db, lot.id, dec!(7), None).await;
    lots.quarantine_async(lot.id, "qa hold").await.expect("quarantine");
    assert_eq!(balance(&db, item, 1).await, (dec!(67), dec!(0)));
    assert_invariant(&db, &sku, item, 1, "quarantine").await;

    lots.release_reservation_async(res3).await.expect("release under quarantine");
    assert_eq!(balance(&db, item, 1).await.1, dec!(0));
    assert_invariant(&db, &sku, item, 1, "release under quarantine").await;

    lots.release_quarantine_async(lot.id).await.expect("release quarantine");
    assert_eq!(balance(&db, item, 1).await.1, dec!(67));
    assert_invariant(&db, &sku, item, 1, "release quarantine").await;

    let other = make_lot(&db, &sku, dec!(10), Some(1)).await;
    assert_invariant(&db, &sku, item, 1, "second lot").await;
    // Inventory can only mirror placements at registered locations.
    let err = lots
        .transfer_async(TransferLot {
            lot_id: other.id,
            from_location_id: 1,
            to_location_id: 99_999,
            quantity: dec!(4),
            reason: None,
            performed_by: None,
        })
        .await
        .expect_err("unregistered location");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("not an inventory location"))
    );
    sqlx::query(
        "INSERT INTO inventory_locations (id, name, code) VALUES (2, 'Two', 'TWO')
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(db.pool())
    .await
    .expect("register location 2");
    lots.transfer_async(TransferLot {
        lot_id: other.id,
        from_location_id: 1,
        to_location_id: 2,
        quantity: dec!(4),
        reason: None,
        performed_by: None,
    })
    .await
    .expect("transfer");
    assert_eq!(balance(&db, item, 1).await.0, dec!(73));
    assert_eq!(balance(&db, item, 2).await.0, dec!(4));

    lots.update_async(
        other.id,
        UpdateLot {
            expiration_date: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            ..Default::default()
        },
    )
    .await
    .expect("backdate expiry");
    assert!(lots.expire_lots_async(chrono::Utc::now()).await.expect("sweep") >= 1);
    assert_invariant(&db, &sku, item, 1, "expire").await;
    assert_eq!(balance(&db, item, 1).await.1, dec!(67));

    let txs = db.inventory().get_transactions_async(item, 50).await.expect("transactions");
    assert!(txs.iter().all(|t| t.reference_type.as_deref() == Some("lot")));
    assert!(txs.iter().any(|t| t.transaction_type == TransactionType::Allocation
        && t.reason.as_deref().is_some_and(|r| r.contains("qa hold"))));
}

#[tokio::test]
async fn postgres_unlinked_lots_leave_inventory_alone() {
    let db = db_or_skip!();
    let sku = sku("TC-FREE");
    let item = inventory_item(&db, &sku).await;
    let lot = make_lot(&db, &sku, dec!(50), None).await;
    db.lots().quarantine_async(lot.id, "x").await.expect("quarantine");
    assert_eq!(balance(&db, item, 1).await, (dec!(0), dec!(0)));
}

#[tokio::test]
async fn postgres_expired_lot_reservations_are_released_lazily_and_by_the_sweeper() {
    let db = db_or_skip!();
    let sku = sku("TC-EXP");
    let lot = make_lot(&db, &sku, dec!(10), Some(1)).await;

    // confirm of an expired reservation releases it on the spot.
    let stale = reserve_lot(&db, lot.id, dec!(4), Some(-60)).await;
    let err = db.lots().confirm_reservation_async(stale).await.expect_err("expired");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("released")),
        "{err:?}"
    );
    assert_eq!(db.lots().get_async(lot.id).await.unwrap().unwrap().quantity_reserved, dec!(0));
    assert!(matches!(
        db.lots().release_reservation_async(stale).await,
        Err(CommerceError::NotFound)
    ));

    // reserve sweeps stale reservations on the lot first.
    let stale2 = reserve_lot(&db, lot.id, dec!(8), Some(-1)).await;
    reserve_lot(&db, lot.id, dec!(6), None).await;
    assert_eq!(db.lots().get_async(lot.id).await.unwrap().unwrap().quantity_reserved, dec!(6));
    assert!(matches!(
        db.lots().release_reservation_async(stale2).await,
        Err(CommerceError::NotFound)
    ));

    // the global sweeper catches lots nobody touched (the live reservation is
    // taken first: reserving afterwards would sweep the stale one lazily).
    let other = make_lot(&db, &sku, dec!(10), Some(1)).await;
    let live = reserve_lot(&db, other.id, dec!(2), None).await;
    reserve_lot(&db, other.id, dec!(3), Some(-1)).await;
    assert!(
        db.lots().release_expired_reservations_async(chrono::Utc::now()).await.expect("sweep") >= 1
    );
    assert_eq!(db.lots().get_async(other.id).await.unwrap().unwrap().quantity_reserved, dec!(2));
    db.lots().release_reservation_async(live).await.expect("live reservation survives the sweep");
}

#[tokio::test]
async fn postgres_failed_item_lot_number_is_scoped_to_the_items_sku() {
    let db = db_or_skip!();
    let shared = format!("SHARED-{}", Uuid::new_v4().simple());
    let other = db
        .lots()
        .create_async(CreateLot {
            sku: sku("TC-OTHER"),
            lot_number: Some(shared.clone()),
            quantity: dec!(10),
            ..Default::default()
        })
        .await
        .expect("other lot");
    let insp = db
        .quality()
        .create_inspection_async(CreateInspection {
            inspection_type: InspectionType::Receiving,
            reference_type: "receipt".into(),
            reference_id: Uuid::new_v4(),
            inspector_id: None,
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: sku("TC-MINE"),
                lot_number: Some(shared),
                serial_number: None,
                quantity_to_inspect: dec!(10),
            }],
        })
        .await
        .expect("inspection");
    db.quality().start_inspection_async(insp.id).await.expect("start");
    db.quality()
        .record_inspection_result_async(RecordInspectionResult {
            item_id: insp.items[0].id,
            quantity_passed: dec!(0),
            quantity_failed: dec!(10),
            result: InspectionResult::Fail,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .await
        .expect("record");
    let err = db.quality().complete_inspection_async(insp.id).await.expect_err("wrong SKU");
    assert!(matches!(err, CommerceError::Conflict(ref m) if m.contains("TC-OTHER")), "{err:?}");
    assert_eq!(
        db.quality().get_inspection_async(insp.id).await.unwrap().unwrap().status,
        InspectionStatus::InProgress
    );
    assert_eq!(db.lots().get_async(other.id).await.unwrap().unwrap().status, LotStatus::Active);
}

#[tokio::test]
async fn postgres_update_refuses_quarantine_transitions() {
    let db = db_or_skip!();
    let lot = make_lot(&db, &sku("TC-UPD"), dec!(5), None).await;
    let err = db
        .lots()
        .update_async(
            lot.id,
            UpdateLot { status: Some(LotStatus::Quarantine), ..Default::default() },
        )
        .await
        .expect_err("quarantine via update");
    assert!(matches!(err, CommerceError::ValidationError(ref m) if m.contains("use quarantine")));
    db.lots().quarantine_async(lot.id, "x").await.expect("quarantine");
    let err = db
        .lots()
        .update_async(lot.id, UpdateLot { status: Some(LotStatus::Active), ..Default::default() })
        .await
        .expect_err("release via update");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("release_quarantine"))
    );
}

#[tokio::test]
async fn postgres_confirmed_serial_reservation_can_be_released_until_shipped() {
    let db = db_or_skip!();
    let sku = sku("TC-SRC");
    let lot = make_lot(&db, &sku, dec!(2), None).await;
    let serial = make_serial(&db, &sku, &lot).await;
    let res = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            serial_id: serial,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            reserved_by: None,
            expires_in_seconds: None,
        })
        .await
        .expect("reserve");
    db.serials().confirm_reservation_async(res.id).await.expect("confirm");
    db.serials().release_reservation_async(res.id).await.expect("release after confirm");
    assert_eq!(serial_status(&db, serial).await, SerialStatus::Available);

    let shipped = make_serial(&db, &sku, &lot).await;
    let res2 = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            serial_id: shipped,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            reserved_by: None,
            expires_in_seconds: None,
        })
        .await
        .expect("reserve");
    db.serials().mark_shipped_async(shipped, Uuid::new_v4()).await.expect("ship");
    assert!(matches!(
        db.serials().release_reservation_async(res2.id).await,
        Err(CommerceError::Conflict(_))
    ));
}

// ==========================================================================
// Split / merge: placements, inventory and genealogy (Postgres parity)
// ==========================================================================

/// `split` moves the placement with the units, so the child is a real, placed
/// lot and the invariant holds across the operation.
#[tokio::test]
async fn postgres_split_moves_placement_and_keeps_inventory_in_step() {
    let db = db_or_skip!();
    let sku = sku("TC-SPLIT");
    let item = inventory_item(&db, &sku).await;
    let lots = db.lots();
    let lot = make_lot(&db, &sku, dec!(100), Some(1)).await;
    assert_invariant(&db, &sku, item, 1, "create").await;

    let child = lots
        .split_async(SplitLot { lot_id: lot.id, quantity: dec!(30), ..Default::default() })
        .await
        .expect("split");
    assert_invariant(&db, &sku, item, 1, "split").await;
    assert_eq!(lots.get_quantity_at_location_async(lot.id, 1).await.unwrap(), Some(dec!(70)));
    assert_eq!(lots.get_quantity_at_location_async(child.id, 1).await.unwrap(), Some(dec!(30)));
    assert_eq!(balance(&db, item, 1).await.0, dec!(100), "split moves nothing on hand");

    lots.consume_async(ConsumeLot {
        lot_id: child.id,
        quantity: dec!(10),
        reference_type: "work_order".into(),
        reference_id: Uuid::new_v4(),
        location_id: None,
        performed_by: None,
    })
    .await
    .expect("consume child");
    assert_eq!(balance(&db, item, 1).await.0, dec!(90), "consuming a split child moves stock");
    assert_invariant(&db, &sku, item, 1, "consume split child").await;
}

/// `merge` moves every source placement onto the target, so the merged lot
/// stays visible to inventory and the invariant holds.
#[tokio::test]
async fn postgres_merge_moves_placements_and_keeps_inventory_in_step() {
    let db = db_or_skip!();
    let sku = sku("TC-MERGE");
    let item = inventory_item(&db, &sku).await;
    let lots = db.lots();
    let a = make_lot(&db, &sku, dec!(40), Some(1)).await;
    let b = make_lot(&db, &sku, dec!(60), Some(1)).await;
    assert_invariant(&db, &sku, item, 1, "two lots").await;

    let merged = lots
        .merge_async(MergeLots {
            source_lot_ids: vec![a.id, b.id],
            // Explicit and unique: the generated `MERGED-<timestamp>`
            // collides when two merges land in the same second.
            target_lot_number: Some(format!("MERGED-{}", Uuid::new_v4().simple())),
            reason: Some("consolidate".into()),
        })
        .await
        .expect("merge");
    assert_eq!(merged.quantity_remaining, dec!(100));
    assert_invariant(&db, &sku, item, 1, "merge").await;
    assert_eq!(lots.get_quantity_at_location_async(merged.id, 1).await.unwrap(), Some(dec!(100)));
    assert_eq!(lots.get_quantity_at_location_async(a.id, 1).await.unwrap(), None);
    assert_eq!(lots.get_quantity_at_location_async(b.id, 1).await.unwrap(), None);
    assert_eq!(balance(&db, item, 1).await.0, dec!(100), "merge moves nothing on hand");

    lots.consume_async(ConsumeLot {
        lot_id: merged.id,
        quantity: dec!(25),
        reference_type: "order".into(),
        reference_id: Uuid::new_v4(),
        location_id: None,
        performed_by: None,
    })
    .await
    .expect("consume merged");
    assert_eq!(balance(&db, item, 1).await.0, dec!(75), "consuming a merged lot moves stock");
    assert_invariant(&db, &sku, item, 1, "consume merged").await;
}

/// A merged lot is traceable back to every source lot and the supplier or work
/// order each came from, even though its own row can only carry one.
#[tokio::test]
async fn postgres_merge_records_genealogy_for_every_source() {
    let db = db_or_skip!();
    let sku = sku("TC-GEN");
    let lots = db.lots();
    let po = Uuid::new_v4();
    let wo = Uuid::new_v4();
    let a = lots
        .create_async(CreateLot {
            sku: sku.clone(),
            lot_number: Some(format!("TC-{}", Uuid::new_v4().simple())),
            quantity: dec!(10),
            supplier_lot: Some("SUP-A".into()),
            purchase_order_id: Some(po),
            ..Default::default()
        })
        .await
        .expect("lot a");
    let b = lots
        .create_async(CreateLot {
            sku: sku.clone(),
            lot_number: Some(format!("TC-{}", Uuid::new_v4().simple())),
            quantity: dec!(5),
            supplier_lot: Some("SUP-B".into()),
            work_order_id: Some(wo),
            ..Default::default()
        })
        .await
        .expect("lot b");

    let merged = lots
        .merge_async(MergeLots {
            source_lot_ids: vec![b.id, a.id], // reverse order: locks are canonical
            // Explicit and unique: the generated `MERGED-<timestamp>`
            // collides when two merges land in the same second.
            target_lot_number: Some(format!("MERGED-{}", Uuid::new_v4().simple())),
            reason: None,
        })
        .await
        .expect("merge");

    let parents = lots.get_lot_parents_async(merged.id).await.expect("parents");
    assert_eq!(parents.len(), 2);
    assert!(parents.iter().all(|p| p.relationship == LotRelationship::Merge));
    assert!(parents.iter().any(|p| p.parent_lot_id == a.id && p.quantity == dec!(10)));
    assert!(parents.iter().any(|p| p.parent_lot_id == b.id && p.quantity == dec!(5)));
    assert_eq!(lots.get_lot_children_async(a.id).await.expect("children").len(), 1);

    // Sources disagree on both documents, so neither is inherited onto the row.
    assert_eq!(merged.purchase_order_id, None);
    assert_eq!(merged.work_order_id, None);
    assert_eq!(merged.supplier_lot, None);

    let trace = lots.trace_async(merged.id).await.expect("trace");
    let lot_nodes: Vec<_> =
        trace.upstream.iter().filter(|n| n.node_type == TraceNodeType::Lot).collect();
    assert_eq!(lot_nodes.len(), 2, "one node per ancestor lot");
    assert!(lot_nodes.iter().any(|n| n.entity_name.as_deref() == Some("SUP-A")));
    assert!(lot_nodes.iter().any(|n| n.entity_name.as_deref() == Some("SUP-B")));
    assert!(
        trace
            .upstream
            .iter()
            .any(|n| n.node_type == TraceNodeType::PurchaseOrder && n.node_id == po)
    );
    assert!(
        trace.upstream.iter().any(|n| n.node_type == TraceNodeType::WorkOrder && n.node_id == wo)
    );
}

/// Split genealogy is transitive: a grandchild traces back to the original
/// receipt through both hops.
#[tokio::test]
async fn postgres_split_genealogy_is_transitive() {
    let db = db_or_skip!();
    let sku = sku("TC-GEN3");
    let lots = db.lots();
    let po = Uuid::new_v4();
    let root = lots
        .create_async(CreateLot {
            sku: sku.clone(),
            lot_number: Some(format!("TC-{}", Uuid::new_v4().simple())),
            quantity: dec!(100),
            purchase_order_id: Some(po),
            ..Default::default()
        })
        .await
        .expect("root");
    let child = lots
        .split_async(SplitLot { lot_id: root.id, quantity: dec!(40), ..Default::default() })
        .await
        .expect("split");
    let grandchild = lots
        .split_async(SplitLot { lot_id: child.id, quantity: dec!(10), ..Default::default() })
        .await
        .expect("split again");

    let parents = lots.get_lot_parents_async(grandchild.id).await.expect("parents");
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0].parent_lot_id, child.id);
    assert_eq!(parents[0].relationship, LotRelationship::Split);

    let trace = lots.trace_async(grandchild.id).await.expect("trace");
    let ancestors: Vec<Uuid> = trace
        .upstream
        .iter()
        .filter(|n| n.node_type == TraceNodeType::Lot)
        .map(|n| n.node_id)
        .collect();
    assert!(ancestors.contains(&child.id));
    assert!(ancestors.contains(&root.id));
    assert!(
        trace
            .upstream
            .iter()
            .any(|n| n.node_type == TraceNodeType::PurchaseOrder && n.node_id == po)
    );
}
