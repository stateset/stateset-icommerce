#![cfg(feature = "sqlite")]

//! Regression tests for warehouse/WMS atomicity and status guards
//! (SQLite backend, sync `Commerce` engine).
//!
//! The module's worst axis was atomicity: `adjust_inventory` was an unguarded
//! read-modify-write (SELECT on-hand → add in Rust → UPDATE) on a pooled
//! connection with no transaction, so two concurrent `+5` adjustments both read
//! 10 and both wrote 15 — one adjustment silently lost. The same class of
//! defect ran through the cycle-count lifecycle transitions, the partial-update
//! merges (`update_warehouse` / `update_location` / `update_zone`) and the
//! check-then-delete guards.
//!
//! Covers:
//! - `adjust_inventory` loses no increments under N OS threads (the
//!   concurrency proof), including the insert-if-missing branch where two
//!   threads race to create the same primary key;
//! - concurrent mixed +/- adjustments never drive on-hand negative and the
//!   final quantity equals the exact sum of the adjustments that succeeded;
//! - a rejected adjustment writes no `inventory_movements` row (the movement
//!   insert shares the adjustment's transaction);
//! - negative-inventory rejection still works on both branches, with the
//!   original error strings;
//! - cycle-count lifecycle guards: double start / double complete / cancel
//!   after complete / record-after-complete are all rejected, and concurrent
//!   completes apply the variance exactly once;
//! - partial updates run read-modify-write inside one transaction, so
//!   concurrent field updates do not clobber each other;
//! - check-then-delete guards for locations with stock and warehouses with
//!   locations still hold.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    AdjustLocationInventory, Commerce, CreateCycleCount, CreateCycleCountLine, CreateLocation,
    CreateWarehouse, CycleCount, CycleCountStatus, Location, LocationInventoryFilter, LocationType,
    MovementFilter, MovementType, RecordCycleCountLine, UpdateLocation, UpdateWarehouse, Warehouse,
    WarehouseAddress, WarehouseType,
};
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn addr() -> WarehouseAddress {
    WarehouseAddress {
        street1: "1 Test St".into(),
        street2: None,
        city: "Test City".into(),
        state: "TC".into(),
        postal_code: "00000".into(),
        country: "US".into(),
        phone: None,
    }
}

fn make_warehouse(commerce: &Commerce, code: &str) -> Warehouse {
    commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: code.into(),
            name: format!("WH {code}"),
            warehouse_type: WarehouseType::Distribution,
            address: addr(),
            timezone: Some("America/Los_Angeles".into()),
        })
        .expect("create warehouse")
}

fn make_location(commerce: &Commerce, warehouse_id: i32, code: &str) -> Location {
    commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id,
            code: Some(code.into()),
            location_type: LocationType::Bulk,
            zone: Some("A".into()),
            aisle: Some("01".into()),
            rack: None,
            level: None,
            bin: None,
            max_weight_kg: None,
            max_volume_m3: None,
            is_pickable: Some(true),
            is_receivable: Some(true),
        })
        .expect("create location")
}

fn adjustment(location_id: i32, sku: &str, quantity: Decimal) -> AdjustLocationInventory {
    AdjustLocationInventory {
        location_id,
        sku: sku.into(),
        lot_id: None,
        quantity,
        reason: "test adjustment".into(),
        reference_type: None,
        reference_id: None,
        performed_by: Some("tester".into()),
    }
}

/// Read the exact stored on-hand for one (location, sku), including zero rows
/// (`has_quantity: None` does not filter them out).
fn on_hand(commerce: &Commerce, location_id: i32, sku: &str) -> Decimal {
    commerce
        .warehouse()
        .list_location_inventory(LocationInventoryFilter {
            location_id: Some(location_id),
            sku: Some(sku.into()),
            ..Default::default()
        })
        .expect("list location inventory")
        .iter()
        .map(|entry| entry.quantity_on_hand)
        .sum()
}

fn inventory_rows(commerce: &Commerce, location_id: i32, sku: &str) -> usize {
    commerce
        .warehouse()
        .list_location_inventory(LocationInventoryFilter {
            location_id: Some(location_id),
            sku: Some(sku.into()),
            ..Default::default()
        })
        .expect("list location inventory")
        .len()
}

fn adjustment_movements(commerce: &Commerce, warehouse_id: i32) -> u64 {
    commerce
        .warehouse()
        .count_movements(MovementFilter {
            warehouse_id: Some(warehouse_id),
            movement_type: Some(MovementType::Adjustment),
            ..Default::default()
        })
        .expect("count movements")
}

/// A cycle count for `sku` at `location_id` moved to `in_progress` with
/// `counted` recorded against an expected quantity of `expected`.
fn in_progress_count(
    commerce: &Commerce,
    warehouse_id: i32,
    location_id: i32,
    sku: &str,
    expected: Decimal,
    counted: Decimal,
) -> CycleCount {
    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id,
            location_id: Some(location_id),
            counted_by: Some("counter".into()),
            lines: vec![CreateCycleCountLine {
                sku: sku.into(),
                lot_id: None,
                expected_quantity: expected,
            }],
            ..Default::default()
        })
        .expect("create cycle count");
    commerce.warehouse().start_cycle_count(count.id).expect("start cycle count");
    commerce
        .warehouse()
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine { sku: sku.into(), lot_id: None, counted_quantity: counted }],
        )
        .expect("record counts")
}

// ============================================================================
// Concurrency proof: adjust_inventory loses no updates
// ============================================================================

/// N OS threads each applying `+1` to the same (location, SKU) must sum
/// exactly. Before the fix each thread `SELECTed` on-hand on its own pooled
/// connection, added in Rust and `UPDATEd` with no transaction, so overlapping
/// threads read the same value and the later write erased the earlier one.
#[test]
fn concurrent_adjustments_lose_no_updates() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 20;

    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-CONC");
    let loc = make_location(&commerce, wh.id, "L-CONC");
    let sku = "CONC-SKU";

    commerce.warehouse().adjust_inventory(adjustment(loc.id, sku, dec!(100))).expect("seed");

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            let location_id = loc.id;
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..PER_THREAD {
                    commerce
                        .warehouse()
                        .adjust_inventory(adjustment(location_id, sku, dec!(1)))
                        .expect("concurrent adjustment must succeed");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("adjustment thread panicked");
    }

    let expected = dec!(100) + Decimal::from(THREADS * PER_THREAD);
    assert_eq!(
        on_hand(&commerce, loc.id, sku),
        expected,
        "lost update: {THREADS} threads x {PER_THREAD} increments must sum exactly"
    );
    // Every adjustment also wrote exactly one movement, inside the same tx.
    assert_eq!(adjustment_movements(&commerce, wh.id), (THREADS * PER_THREAD + 1) as u64);
}

/// The insert-if-missing branch under concurrency: threads racing to create the
/// same primary key `(location_id, sku, lot_id)` must not violate it, and the
/// row must converge to the exact total.
#[test]
fn concurrent_first_adjustment_on_new_sku_converges() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 10;

    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-NEW");
    let loc = make_location(&commerce, wh.id, "L-NEW");
    let sku = "BRAND-NEW-SKU";

    assert_eq!(inventory_rows(&commerce, loc.id, sku), 0, "sku must not exist yet");

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            let location_id = loc.id;
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..PER_THREAD {
                    commerce
                        .warehouse()
                        .adjust_inventory(adjustment(location_id, sku, dec!(1)))
                        .expect("racing creates must not violate the primary key");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("insert-race thread panicked");
    }

    assert_eq!(inventory_rows(&commerce, loc.id, sku), 1, "exactly one inventory row");
    assert_eq!(on_hand(&commerce, loc.id, sku), Decimal::from(THREADS * PER_THREAD));
}

/// Concurrent mixed +/- adjustments must never drive on-hand negative, and the
/// stored quantity must equal the exact arithmetic sum of the adjustments that
/// were accepted (no lost update in either direction).
#[test]
fn concurrent_mixed_adjustments_never_go_negative() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;

    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-MIX");
    let loc = make_location(&commerce, wh.id, "L-MIX");
    let sku = "MIX-SKU";

    commerce.warehouse().adjust_inventory(adjustment(loc.id, sku, dec!(10))).expect("seed");

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|thread_index| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            let location_id = loc.id;
            let delta = if thread_index % 2 == 0 { dec!(1) } else { dec!(-1) };
            thread::spawn(move || {
                barrier.wait();
                let mut applied = Decimal::ZERO;
                for _ in 0..PER_THREAD {
                    match commerce.warehouse().adjust_inventory(adjustment(location_id, sku, delta))
                    {
                        Ok(inventory) => {
                            assert!(
                                inventory.quantity_on_hand >= Decimal::ZERO,
                                "adjust_inventory returned negative on-hand: {}",
                                inventory.quantity_on_hand
                            );
                            applied += delta;
                        }
                        Err(e) => assert!(
                            e.to_string().contains("negative inventory"),
                            "unexpected adjustment error: {e}"
                        ),
                    }
                }
                applied
            })
        })
        .collect();

    let applied: Decimal =
        handles.into_iter().map(|h| h.join().expect("mixed thread panicked")).sum();

    let final_on_hand = on_hand(&commerce, loc.id, sku);
    assert!(final_on_hand >= Decimal::ZERO, "on-hand went negative: {final_on_hand}");
    assert_eq!(
        final_on_hand,
        dec!(10) + applied,
        "final on-hand must equal the exact sum of accepted adjustments"
    );
}

// ============================================================================
// Negative-inventory guards (error strings preserved)
// ============================================================================

#[test]
fn adjustment_below_zero_on_existing_row_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-NEG1");
    let loc = make_location(&commerce, wh.id, "L-NEG1");

    commerce.warehouse().adjust_inventory(adjustment(loc.id, "NEG-SKU", dec!(5))).expect("seed");

    let err = commerce
        .warehouse()
        .adjust_inventory(adjustment(loc.id, "NEG-SKU", dec!(-6)))
        .expect_err("over-decrement must be rejected");
    assert!(
        err.to_string().contains("Adjustment would result in negative inventory"),
        "unexpected error: {err}"
    );
    assert_eq!(
        on_hand(&commerce, loc.id, "NEG-SKU"),
        dec!(5),
        "rejected adjustment must not write"
    );
}

#[test]
fn negative_adjustment_on_missing_row_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-NEG2");
    let loc = make_location(&commerce, wh.id, "L-NEG2");

    let err = commerce
        .warehouse()
        .adjust_inventory(adjustment(loc.id, "MISSING-SKU", dec!(-1)))
        .expect_err("creating negative inventory must be rejected");
    assert!(
        err.to_string().contains("Cannot create negative inventory"),
        "unexpected error: {err}"
    );
    assert_eq!(inventory_rows(&commerce, loc.id, "MISSING-SKU"), 0);
}

/// A rejected adjustment must leave no audit movement behind: the movement
/// insert has to share the adjustment's transaction, not run after it.
#[test]
fn rejected_adjustment_writes_no_movement() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-MV");
    let loc = make_location(&commerce, wh.id, "L-MV");

    commerce.warehouse().adjust_inventory(adjustment(loc.id, "MV-SKU", dec!(2))).expect("seed");
    assert_eq!(adjustment_movements(&commerce, wh.id), 1);

    commerce
        .warehouse()
        .adjust_inventory(adjustment(loc.id, "MV-SKU", dec!(-3)))
        .expect_err("must be rejected");
    commerce
        .warehouse()
        .adjust_inventory(adjustment(loc.id, "OTHER-SKU", dec!(-1)))
        .expect_err("must be rejected");

    assert_eq!(
        adjustment_movements(&commerce, wh.id),
        1,
        "rejected adjustments must not record movements"
    );
}

// ============================================================================
// Cycle-count lifecycle guards
// ============================================================================

#[test]
fn start_cycle_count_twice_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CC1");
    let loc = make_location(&commerce, wh.id, "L-CC1");

    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: wh.id,
            location_id: Some(loc.id),
            lines: vec![CreateCycleCountLine {
                sku: "CC-SKU".into(),
                lot_id: None,
                expected_quantity: dec!(1),
            }],
            ..Default::default()
        })
        .expect("create cycle count");

    let started = commerce.warehouse().start_cycle_count(count.id).expect("first start");
    assert_eq!(started.status, CycleCountStatus::InProgress);

    let err = commerce.warehouse().start_cycle_count(count.id).expect_err("second start");
    assert!(err.to_string().contains("in_progress"), "error must name the status: {err}");
}

#[test]
fn complete_cycle_count_twice_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CC2");
    let loc = make_location(&commerce, wh.id, "L-CC2");
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "CC-SKU", dec!(10))).expect("seed");

    let count = in_progress_count(&commerce, wh.id, loc.id, "CC-SKU", dec!(10), dec!(12));
    let completed = commerce.warehouse().complete_cycle_count(count.id).expect("first complete");
    assert_eq!(completed.status, CycleCountStatus::Completed);
    assert_eq!(on_hand(&commerce, loc.id, "CC-SKU"), dec!(12));

    let err = commerce.warehouse().complete_cycle_count(count.id).expect_err("second complete");
    assert!(err.to_string().contains("completed"), "error must name the status: {err}");
    assert_eq!(
        on_hand(&commerce, loc.id, "CC-SKU"),
        dec!(12),
        "a re-complete must not re-apply the variance"
    );
}

#[test]
fn cancel_after_complete_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CC3");
    let loc = make_location(&commerce, wh.id, "L-CC3");
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "CC-SKU", dec!(4))).expect("seed");

    let count = in_progress_count(&commerce, wh.id, loc.id, "CC-SKU", dec!(4), dec!(4));
    commerce.warehouse().complete_cycle_count(count.id).expect("complete");

    let err = commerce.warehouse().cancel_cycle_count(count.id).expect_err("cancel completed");
    assert!(err.to_string().contains("completed"), "error must name the status: {err}");
}

#[test]
fn cancel_cycle_count_twice_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CC4");
    let loc = make_location(&commerce, wh.id, "L-CC4");

    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: wh.id,
            location_id: Some(loc.id),
            lines: vec![CreateCycleCountLine {
                sku: "CC-SKU".into(),
                lot_id: None,
                expected_quantity: dec!(1),
            }],
            ..Default::default()
        })
        .expect("create cycle count");

    commerce.warehouse().cancel_cycle_count(count.id).expect("first cancel");
    let err = commerce.warehouse().cancel_cycle_count(count.id).expect_err("second cancel");
    assert!(err.to_string().contains("cancelled"), "error must name the status: {err}");
}

#[test]
fn record_counts_after_complete_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CC5");
    let loc = make_location(&commerce, wh.id, "L-CC5");
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "CC-SKU", dec!(7))).expect("seed");

    let count = in_progress_count(&commerce, wh.id, loc.id, "CC-SKU", dec!(7), dec!(7));
    commerce.warehouse().complete_cycle_count(count.id).expect("complete");

    let err = commerce
        .warehouse()
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine {
                sku: "CC-SKU".into(),
                lot_id: None,
                counted_quantity: dec!(99),
            }],
        )
        .expect_err("recording onto a completed count");
    assert!(err.to_string().contains("completed"), "error must name the status: {err}");

    let reread = commerce.warehouse().get_cycle_count(count.id).expect("get").expect("present");
    assert_eq!(reread.lines[0].counted_quantity, Some(dec!(7)), "lines must be untouched");
}

/// Concurrent completes of one cycle count must apply the variance exactly
/// once. Reading the status outside the write transaction let every racing
/// caller pass the `can_transition_to` guard and each apply the same variance
/// adjustment, multiplying stock.
#[test]
fn concurrent_complete_applies_variance_exactly_once() {
    const THREADS: usize = 6;

    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-CC6");
    let loc = make_location(&commerce, wh.id, "L-CC6");
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "CC-SKU", dec!(10))).expect("seed");

    let count = in_progress_count(&commerce, wh.id, loc.id, "CC-SKU", dec!(10), dec!(15));

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            let id = count.id;
            thread::spawn(move || {
                barrier.wait();
                commerce.warehouse().complete_cycle_count(id).is_ok()
            })
        })
        .collect();

    let winners: usize = handles
        .into_iter()
        .map(|handle| usize::from(handle.join().expect("complete thread panicked")))
        .sum();

    assert_eq!(winners, 1, "exactly one concurrent complete may win");
    assert_eq!(
        on_hand(&commerce, loc.id, "CC-SKU"),
        dec!(15),
        "the +5 variance must be applied exactly once"
    );
}

/// Concurrent starts of one draft cycle count: exactly one may transition it.
#[test]
fn concurrent_start_cycle_count_only_one_wins() {
    const THREADS: usize = 6;

    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-CC7");
    let loc = make_location(&commerce, wh.id, "L-CC7");

    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: wh.id,
            location_id: Some(loc.id),
            lines: vec![CreateCycleCountLine {
                sku: "CC-SKU".into(),
                lot_id: None,
                expected_quantity: dec!(1),
            }],
            ..Default::default()
        })
        .expect("create cycle count");

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            let id = count.id;
            thread::spawn(move || {
                barrier.wait();
                commerce.warehouse().start_cycle_count(id).is_ok()
            })
        })
        .collect();

    let winners: usize = handles
        .into_iter()
        .map(|handle| usize::from(handle.join().expect("start thread panicked")))
        .sum();

    assert_eq!(winners, 1, "exactly one concurrent start may win");
    let reread = commerce.warehouse().get_cycle_count(count.id).expect("get").expect("present");
    assert_eq!(reread.status, CycleCountStatus::InProgress);
}

// ============================================================================
// Partial-update merges are atomic
// ============================================================================

/// `update_warehouse` merges the caller's `Option` fields over the stored row.
/// With the read and the write in separate statements, two concurrent partial
/// updates each wrote back the fields they had read, so the later write
/// reverted the earlier one's field.
#[test]
fn concurrent_warehouse_updates_do_not_lose_fields() {
    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-UPD");

    let barrier = Arc::new(Barrier::new(2));
    let name_writer = {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        let id = wh.id;
        thread::spawn(move || {
            barrier.wait();
            commerce
                .warehouse()
                .update_warehouse(
                    id,
                    UpdateWarehouse { name: Some("Renamed".into()), ..Default::default() },
                )
                .expect("rename");
        })
    };
    let tz_writer = {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        let id = wh.id;
        thread::spawn(move || {
            barrier.wait();
            commerce
                .warehouse()
                .update_warehouse(
                    id,
                    UpdateWarehouse { timezone: Some("UTC".into()), ..Default::default() },
                )
                .expect("retimezone");
        })
    };
    name_writer.join().expect("name thread panicked");
    tz_writer.join().expect("timezone thread panicked");

    let reread = commerce.warehouse().get_warehouse(wh.id).expect("get").expect("present");
    assert_eq!(reread.name, "Renamed", "the rename was lost by the concurrent update");
    assert_eq!(
        reread.timezone.as_deref(),
        Some("UTC"),
        "the timezone change was lost by the concurrent update"
    );
}

#[test]
fn concurrent_location_updates_do_not_lose_fields() {
    let commerce = Arc::new(commerce());
    let wh = make_warehouse(&commerce, "WH-LUPD");
    let loc = make_location(&commerce, wh.id, "L-UPD");

    let barrier = Arc::new(Barrier::new(2));
    let pickable_writer = {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        let id = loc.id;
        thread::spawn(move || {
            barrier.wait();
            commerce
                .warehouse()
                .update_location(
                    id,
                    UpdateLocation { is_pickable: Some(false), ..Default::default() },
                )
                .expect("set pickable");
        })
    };
    let zone_writer = {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        let id = loc.id;
        thread::spawn(move || {
            barrier.wait();
            commerce
                .warehouse()
                .update_location(
                    id,
                    UpdateLocation { zone: Some("Z9".into()), ..Default::default() },
                )
                .expect("set zone");
        })
    };
    pickable_writer.join().expect("pickable thread panicked");
    zone_writer.join().expect("zone thread panicked");

    let reread = commerce.warehouse().get_location(loc.id).expect("get").expect("present");
    assert!(!reread.is_pickable, "the is_pickable change was lost");
    assert_eq!(reread.zone.as_deref(), Some("Z9"), "the zone change was lost");
}

#[test]
fn update_missing_warehouse_and_location_report_not_found() {
    let commerce = commerce();
    assert!(commerce.warehouse().update_warehouse(999_999, UpdateWarehouse::default()).is_err());
    assert!(commerce.warehouse().update_location(999_999, UpdateLocation::default()).is_err());
}

// ============================================================================
// Check-then-delete guards
// ============================================================================

#[test]
fn delete_location_with_inventory_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-DEL1");
    let loc = make_location(&commerce, wh.id, "L-DEL1");
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "DEL-SKU", dec!(3))).expect("seed");

    let err = commerce.warehouse().delete_location(loc.id).expect_err("delete with stock");
    assert!(
        err.to_string().contains("Cannot delete location with inventory"),
        "unexpected error: {err}"
    );
    assert!(commerce.warehouse().get_location(loc.id).expect("get").is_some());

    // Draining the stock clears the WMS guard, but the location still cannot be
    // deleted: `inventory_movements.to_location_id` references `locations(id)`
    // without a cascade, so the audit trail pins the row. That is schema-level,
    // pre-existing behaviour (the pre-transaction code hit the identical
    // foreign-key error) — asserted here so a later schema change is a
    // deliberate decision rather than a silent audit-trail deletion.
    commerce.warehouse().adjust_inventory(adjustment(loc.id, "DEL-SKU", dec!(-3))).expect("drain");
    let err = commerce.warehouse().delete_location(loc.id).expect_err("movement history pins it");
    assert!(err.to_string().contains("FOREIGN KEY"), "unexpected error: {err}");
    assert!(commerce.warehouse().get_location(loc.id).expect("get").is_some());

    // A location that never moved stock deletes cleanly.
    let untouched = make_location(&commerce, wh.id, "L-DEL1-B");
    commerce.warehouse().delete_location(untouched.id).expect("delete untouched location");
    assert!(commerce.warehouse().get_location(untouched.id).expect("get").is_none());
}

#[test]
fn delete_warehouse_with_locations_is_rejected() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-DEL2");
    let loc = make_location(&commerce, wh.id, "L-DEL2");

    let err = commerce.warehouse().delete_warehouse(wh.id).expect_err("delete with locations");
    assert!(
        err.to_string().contains("Cannot delete warehouse with existing locations"),
        "unexpected error: {err}"
    );

    commerce.warehouse().delete_location(loc.id).expect("delete location");
    commerce.warehouse().delete_warehouse(wh.id).expect("delete warehouse");
}

// ============================================================================
// Cycle-count creation is all-or-nothing
// ============================================================================

#[test]
fn create_cycle_count_persists_header_and_all_lines() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CCA");
    let loc = make_location(&commerce, wh.id, "L-CCA");

    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: wh.id,
            location_id: Some(loc.id),
            lines: (0..5)
                .map(|i| CreateCycleCountLine {
                    sku: format!("SKU-{i}"),
                    lot_id: None,
                    expected_quantity: Decimal::from(i),
                })
                .collect(),
            ..Default::default()
        })
        .expect("create cycle count");

    let reread = commerce.warehouse().get_cycle_count(count.id).expect("get").expect("present");
    assert_eq!(reread.lines.len(), 5, "header and every line commit together");
    assert_eq!(reread.status, CycleCountStatus::Draft);
}

#[test]
fn create_cycle_count_rejects_empty_lines_without_writing_a_header() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-CCB");

    let err = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: wh.id,
            location_id: None,
            lines: vec![],
            ..Default::default()
        })
        .expect_err("empty cycle count");
    assert!(err.to_string().contains("at least one line"), "unexpected error: {err}");

    assert!(
        commerce
            .warehouse()
            .list_cycle_counts(stateset_embedded::CycleCountFilter {
                warehouse_id: Some(wh.id),
                ..Default::default()
            })
            .expect("list")
            .is_empty(),
        "no header may be written for a rejected create"
    );
}

// ============================================================================
// Sanity: a lot-scoped adjustment is a distinct row
// ============================================================================

#[test]
fn lot_scoped_adjustments_are_independent_rows() {
    let commerce = commerce();
    let wh = make_warehouse(&commerce, "WH-LOT");
    let loc = make_location(&commerce, wh.id, "L-LOT");
    let lot = Uuid::new_v4();

    commerce.warehouse().adjust_inventory(adjustment(loc.id, "LOT-SKU", dec!(4))).expect("no lot");
    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
            lot_id: Some(lot),
            ..adjustment(loc.id, "LOT-SKU", dec!(6))
        })
        .expect("with lot");

    assert_eq!(inventory_rows(&commerce, loc.id, "LOT-SKU"), 2);
    assert_eq!(on_hand(&commerce, loc.id, "LOT-SKU"), dec!(10));
}
