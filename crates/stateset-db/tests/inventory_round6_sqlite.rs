//! Inventory round 6 (SQLite): the database-enforced balance identity
//! (migration 092), the clamp paths repairing drift instead of only logging
//! it, real `std::thread` races on reserve / release / adjust, and
//! `auto_allocate_inventory` skipping a starved candidate instead of failing
//! the whole batch.
//!
//! Round 5 shipped the non-negative triggers and the expiry sweeper but left
//! two holes this file closes: nothing enforced
//! `quantity_available == quantity_on_hand - quantity_allocated`, and the
//! SQLite backend had NO concurrency coverage at all
//! (`inventory_round5_sqlite.rs` never spawns a thread).

#![cfg(feature = "sqlite")]

use std::sync::{Arc, Barrier};

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustInventory, BackorderPriority, BackorderRepository, CommerceError, CreateBackorder,
    CreateInventoryItem, InventoryRepository, ReservationStatus, ReserveInventory,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("in-memory sqlite")
}

fn item(sku: &str, qty: Decimal) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: sku.into(),
        name: format!("Item {sku}"),
        initial_quantity: Some(qty),
        ..Default::default()
    }
}

fn reserve(sku: &str, qty: Decimal, reference: &str, expires: Option<i64>) -> ReserveInventory {
    ReserveInventory {
        sku: sku.into(),
        location_id: None,
        quantity: qty,
        reference_type: "cart".into(),
        reference_id: reference.into(),
        expires_in_seconds: expires,
    }
}

fn balance(db: &SqliteDatabase, sku: &str) -> (Decimal, Decimal, Decimal) {
    let stock = db.inventory().get_stock(sku).expect("stock").expect("exists");
    (stock.total_on_hand, stock.total_allocated, stock.total_available)
}

fn item_id(db: &SqliteDatabase, sku: &str) -> i64 {
    db.inventory().get_item_by_sku(sku).expect("item").expect("exists").id
}

/// BOTH balance identities, for every balance row:
///
/// 1. `quantity_allocated == SUM(open reservations)` (round 5), and
/// 2. `quantity_available == quantity_on_hand - quantity_allocated` (round 6).
fn assert_balance_identities(db: &SqliteDatabase) {
    let conn = db.pool().get().expect("conn");
    let mut stmt = conn
        .prepare(
            "SELECT b.item_id, b.location_id, b.quantity_on_hand, b.quantity_allocated,
                    b.quantity_available,
                    (SELECT COALESCE(GROUP_CONCAT(r.quantity, ','), '') FROM inventory_reservations r
                      WHERE r.item_id = b.item_id AND r.location_id = b.location_id
                        AND r.status IN ('pending', 'confirmed', 'allocated'))
             FROM inventory_balances b",
        )
        .expect("prepare");
    let rows: Vec<(i64, i32, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(!rows.is_empty(), "no balances to check");
    for (item_id, location_id, on_hand, allocated, available, held) in rows {
        let on_hand: Decimal = on_hand.parse().expect("on_hand decimal");
        let allocated: Decimal = allocated.parse().expect("allocated decimal");
        let available: Decimal = available.parse().expect("available decimal");
        let held: Decimal = held
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<Decimal>().expect("reservation decimal"))
            .sum();
        assert_eq!(
            allocated, held,
            "item {item_id} @ {location_id}: allocated {allocated} != open reservations {held}"
        );
        assert_eq!(
            available,
            on_hand - allocated,
            "item {item_id} @ {location_id}: available {available} != {on_hand} - {allocated}"
        );
    }
}

fn set_reservation_expiry_in_past(db: &SqliteDatabase, reservation_id: Uuid) {
    let conn = db.pool().get().expect("conn");
    conn.execute(
        "UPDATE inventory_reservations SET expires_at = ? WHERE id = ?",
        rusqlite::params![
            (Utc::now() - Duration::minutes(5)).to_rfc3339(),
            reservation_id.to_string()
        ],
    )
    .expect("backdate expiry");
}

// ---------------------------------------------------------------------------
// #2: the balance identity is enforced by the database (migration 092)
// ---------------------------------------------------------------------------

#[test]
fn sqlite_migration_092_rejects_an_incoherent_balance_update() {
    let db = db();
    db.inventory().create_item(item("IDENT-1", dec!(10))).expect("create");
    let conn = db.pool().get().expect("conn");

    // Move on-hand without recomputing available: exactly the raw-SQL write
    // that used to leave the row lying to every future `reserve`.
    let err = conn
        .execute(
            "UPDATE inventory_balances SET quantity_on_hand = '20'
             WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'IDENT-1')",
            [],
        )
        .expect_err("the identity trigger must reject this");
    assert!(
        err.to_string().contains("quantity_available must equal"),
        "expected the identity trigger, got {err}"
    );

    // A coherent write of the same on-hand change is accepted.
    conn.execute(
        "UPDATE inventory_balances SET quantity_on_hand = '20', quantity_available = '20'
         WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'IDENT-1')",
        [],
    )
    .expect("coherent update");
    assert_eq!(balance(&db, "IDENT-1"), (dec!(20), dec!(0), dec!(20)));
    assert_balance_identities(&db);
}

#[test]
fn sqlite_migration_092_rejects_an_incoherent_balance_insert() {
    let db = db();
    db.inventory().create_item(item("IDENT-2", dec!(5))).expect("create");
    let conn = db.pool().get().expect("conn");
    let id = item_id(&db, "IDENT-2");
    conn.execute("INSERT INTO inventory_locations (id, name, code) VALUES (77, 'R6', 'R6')", [])
        .expect("location");

    let err = conn
        .execute(
            "INSERT INTO inventory_balances
                (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available)
             VALUES (?, 77, '10', '0', '4')",
            rusqlite::params![id],
        )
        .expect_err("incoherent insert must be rejected");
    assert!(
        err.to_string().contains("quantity_available must equal"),
        "expected the identity trigger, got {err}"
    );
}

#[test]
fn sqlite_migration_092_is_legacy_safe_for_an_already_drifted_row() {
    let db = db();
    db.inventory().create_item(item("IDENT-3", dec!(10))).expect("create");
    let conn = db.pool().get().expect("conn");

    // Install drift the way a pre-092 deployment would have: the trigger fires
    // only on quantity columns, so writing them via a column list it does not
    // watch is impossible — instead drop the trigger the way the migration's
    // own "legacy" contract describes and re-create the state.
    conn.execute_batch("DROP TRIGGER trg_inventory_balances_identity_update")
        .expect("drop trigger");
    conn.execute(
        "UPDATE inventory_balances SET quantity_on_hand = '20'
         WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'IDENT-3')",
        [],
    )
    .expect("legacy drift");
    conn.execute_batch(
        "CREATE TRIGGER trg_inventory_balances_identity_update
         BEFORE UPDATE OF quantity_on_hand, quantity_allocated, quantity_available ON inventory_balances
         WHEN ABS(CAST(NEW.quantity_available AS REAL)
                  - (CAST(NEW.quantity_on_hand AS REAL) - CAST(NEW.quantity_allocated AS REAL)))
              > 0.000001
          AND ABS(CAST(OLD.quantity_available AS REAL)
                  - (CAST(OLD.quantity_on_hand AS REAL) - CAST(OLD.quantity_allocated AS REAL)))
              <= 0.000001
         BEGIN
             SELECT RAISE(ABORT, 'inventory_balances: quantity_available must equal quantity_on_hand - quantity_allocated');
         END;",
    )
    .expect("recreate trigger");

    // The drifted row still loads and can still be written (repaired).
    assert_eq!(balance(&db, "IDENT-3"), (dec!(20), dec!(0), dec!(10)));
    conn.execute(
        "UPDATE inventory_balances SET quantity_available = '20'
         WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'IDENT-3')",
        [],
    )
    .expect("a drifted row must stay repairable");
    assert_balance_identities(&db);
}

#[test]
fn sqlite_release_repairs_a_drifted_balance_instead_of_only_clamping() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("DRIFT-1", dec!(10))).expect("create");
    let keep = inv.reserve(reserve("DRIFT-1", dec!(3), "keep", None)).expect("reserve keep");
    let drop = inv.reserve(reserve("DRIFT-1", dec!(4), "drop", None)).expect("reserve drop");
    assert_eq!(balance(&db, "DRIFT-1"), (dec!(10), dec!(7), dec!(3)));

    // Simulate pre-fix drift: allocated forgets both holds (coherently, so the
    // identity trigger allows the write) while the reservations stay open.
    {
        let conn = db.pool().get().expect("conn");
        conn.execute(
            "UPDATE inventory_balances
             SET quantity_allocated = '1', quantity_available = '9'
             WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'DRIFT-1')",
            [],
        )
        .expect("install drift");
    }

    // Releasing `drop` (4 units) needs more than the recorded 1: the old code
    // clamped allocated to 0 and left `keep`'s 3 units unaccounted for, so the
    // next reserve could oversell. The repair path rebuilds allocated from the
    // reservations that stay open.
    inv.release_reservation(drop.id).expect("release");
    assert_eq!(
        balance(&db, "DRIFT-1"),
        (dec!(10), dec!(3), dec!(7)),
        "the drifted balance must be repaired to the units `keep` still holds"
    );
    assert_eq!(inv.get_reservation(keep.id).unwrap().unwrap().status, ReservationStatus::Pending);
    assert_balance_identities(&db);
}

#[test]
fn sqlite_mixed_operations_plus_a_sweep_keep_both_identities() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("MIX-1", dec!(20))).expect("create");
    inv.create_item(item("MIX-2", dec!(7.5))).expect("create");

    let a = inv.reserve(reserve("MIX-1", dec!(4), "a", Some(3600))).expect("a");
    let b = inv.reserve(reserve("MIX-1", dec!(2.25), "b", None)).expect("b");
    let c = inv.reserve(reserve("MIX-2", dec!(1.5), "c", Some(3600))).expect("c");
    inv.confirm_reservation(b.id).expect("confirm b");
    inv.adjust(AdjustInventory {
        sku: "MIX-1".into(),
        location_id: None,
        quantity: dec!(5),
        reason: "restock".into(),
        reference_type: None,
        reference_id: None,
    })
    .expect("adjust");
    inv.release_reservation(a.id).expect("release a");
    let d = inv.reserve(reserve("MIX-1", dec!(3), "d", Some(3600))).expect("d");
    set_reservation_expiry_in_past(&db, c.id);
    set_reservation_expiry_in_past(&db, d.id);
    assert_balance_identities(&db);

    // The sweeper (nothing else touches MIX-2) reclaims both expired holds.
    let expired = inv.expire_reservations(Utc::now(), 100).expect("sweep");
    assert_eq!(expired, 2, "both expired holds must be swept");
    assert_eq!(balance(&db, "MIX-1"), (dec!(25), dec!(2.25), dec!(22.75)));
    assert_eq!(balance(&db, "MIX-2"), (dec!(7.5), dec!(0), dec!(7.5)));
    assert_balance_identities(&db);
}

// ---------------------------------------------------------------------------
// #5: real SQLite concurrency
// ---------------------------------------------------------------------------

#[test]
fn sqlite_concurrent_reserves_never_oversell() {
    let db = Arc::new(db());
    for round in 0..10 {
        let sku = format!("RACE-RESERVE-{round}");
        db.inventory().create_item(item(&sku, dec!(10))).expect("create");

        let contenders = 8;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles: Vec<_> = (0..contenders)
            .map(|i| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let sku = sku.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    db.inventory().reserve(reserve(&sku, dec!(2), &format!("c{i}"), None))
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();

        let winners = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(winners, 5, "10 units / 2 per hold = 5 winners, got {results:?}");
        for result in &results {
            if let Err(e) = result {
                assert!(
                    matches!(e, CommerceError::InsufficientStock { .. }),
                    "losers must be InsufficientStock, got {e:?}"
                );
            }
        }
        assert_eq!(balance(&db, &sku), (dec!(10), dec!(10), dec!(0)));
    }
    assert_balance_identities(&db);
}

#[test]
fn sqlite_concurrent_releases_are_idempotent_and_keep_the_identities() {
    let db = Arc::new(db());
    db.inventory().create_item(item("RACE-RELEASE", dec!(10))).expect("create");
    let target =
        db.inventory().reserve(reserve("RACE-RELEASE", dec!(4), "a", None)).expect("reserve");
    let other =
        db.inventory().reserve(reserve("RACE-RELEASE", dec!(1), "b", None)).expect("reserve");

    let contenders = 8;
    let barrier = Arc::new(Barrier::new(contenders + 1));
    let mut handles: Vec<_> = (0..contenders)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.inventory().release_reservation(target.id)
            })
        })
        .collect();
    // A concurrent release of a DIFFERENT hold on the same balance must not
    // surface as a VersionConflict either.
    handles.push({
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            db.inventory().release_reservation(other.id)
        })
    });

    for handle in handles {
        handle.join().expect("thread").expect("release must be Ok even when racing / repeated");
    }
    assert_eq!(balance(&db, "RACE-RELEASE"), (dec!(10), dec!(0), dec!(10)));
    assert_eq!(
        db.inventory().get_reservation(target.id).unwrap().unwrap().status,
        ReservationStatus::Released
    );
    assert_balance_identities(&db);
}

#[test]
fn sqlite_concurrent_adjusts_do_not_lose_updates() {
    let db = Arc::new(db());
    db.inventory().create_item(item("RACE-ADJUST", dec!(0))).expect("create");

    let contenders = 8;
    let per_thread = 5;
    let barrier = Arc::new(Barrier::new(contenders));
    let handles: Vec<_> = (0..contenders)
        .map(|i| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for n in 0..per_thread {
                    db.inventory()
                        .adjust(AdjustInventory {
                            sku: "RACE-ADJUST".into(),
                            location_id: None,
                            quantity: dec!(1),
                            reason: format!("thread {i} adjust {n}"),
                            reference_type: None,
                            reference_id: None,
                        })
                        .expect("adjust must not lose an update");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread");
    }

    let expected = Decimal::from(contenders as i64 * per_thread);
    assert_eq!(balance(&db, "RACE-ADJUST"), (expected, dec!(0), expected));
    assert_balance_identities(&db);
}

// ---------------------------------------------------------------------------
// #4: auto-allocation skips a starved candidate instead of failing the batch
// ---------------------------------------------------------------------------

#[test]
fn sqlite_auto_allocate_gives_stock_to_every_candidate_it_can() {
    let db = db();
    db.inventory().create_item(item("BO-SKU", dec!(6))).expect("create");
    let backorders = db.backorder();

    let mut ids = Vec::new();
    for (i, priority) in
        [BackorderPriority::Critical, BackorderPriority::High, BackorderPriority::Normal]
            .into_iter()
            .enumerate()
    {
        ids.push(
            backorders
                .create_backorder(CreateBackorder {
                    order_id: Uuid::new_v4(),
                    order_line_id: None,
                    customer_id: Uuid::new_v4(),
                    sku: "BO-SKU".into(),
                    quantity: Decimal::from(4),
                    priority: Some(priority),
                    expected_date: None,
                    promised_date: None,
                    source_location_id: None,
                    notes: Some(format!("bo {i}")),
                })
                .expect("create backorder")
                .id,
        );
    }

    // 6 units for 3 backorders wanting 4 each: the first two get 4 and 2, the
    // third gets nothing — and that must not fail the call.
    let created = backorders.auto_allocate_inventory("BO-SKU").expect("auto allocate");
    assert_eq!(created.len(), 2, "the starved candidate must be skipped, not fatal: {created:?}");
    assert_eq!(created[0].quantity, dec!(4));
    assert_eq!(created[1].quantity, dec!(2));
    assert_eq!(balance(&db, "BO-SKU"), (dec!(6), dec!(6), dec!(0)));
    assert_balance_identities(&db);
    let _ = ids;
}
