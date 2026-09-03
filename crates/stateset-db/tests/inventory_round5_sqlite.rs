//! Inventory round 5 (SQLite): exact decimal release/expiry arithmetic, the
//! reservation-expiry sweeper, backorder allocations backed by real
//! reservations, adjust validation, reorder threshold with safety stock and
//! the non-negative balance triggers (migration 083).

#![cfg(feature = "sqlite")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustInventory, AllocateBackorder, AllocationStatus, BackorderRepository, BackorderStatus,
    CommerceError, CreateBackorder, CreateInventoryItem, FulfillBackorder, FulfillmentSourceType,
    InventoryRepository, ReservationStatus, ReserveInventory, TransactionType,
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

/// SUM(quantity) of reservations that hold stock must equal
/// `quantity_allocated` for every (item, location).
fn assert_allocation_invariant(db: &SqliteDatabase) {
    let conn = db.pool().get().expect("conn");
    let mut stmt = conn
        .prepare(
            "SELECT b.item_id, b.location_id, b.quantity_allocated,
                    (SELECT COALESCE(GROUP_CONCAT(r.quantity, ','), '') FROM inventory_reservations r
                      WHERE r.item_id = b.item_id AND r.location_id = b.location_id
                        AND r.status IN ('pending', 'confirmed', 'allocated'))
             FROM inventory_balances b",
        )
        .expect("prepare");
    let rows: Vec<(i64, i32, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(!rows.is_empty(), "no balances to check");
    for (item_id, location_id, allocated, held) in rows {
        let allocated: Decimal = allocated.parse().expect("allocated decimal");
        let held: Decimal = held
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<Decimal>().expect("reservation decimal"))
            .sum();
        assert_eq!(
            allocated, held,
            "item {item_id} @ {location_id}: allocated {allocated} != open reservations {held}"
        );
    }
}

// ---------------------------------------------------------------------------
// #2: release/expire compute in Rust decimals, not SQL floats
// ---------------------------------------------------------------------------

#[test]
fn sqlite_fractional_release_round_trips_exactly() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("FRAC-1", dec!(1))).expect("create");

    let r1 = inv.reserve(reserve("FRAC-1", dec!(0.1), "a", None)).expect("reserve 0.1");
    let _r2 = inv.reserve(reserve("FRAC-1", dec!(0.2), "b", None)).expect("reserve 0.2");
    assert_eq!(balance(&db, "FRAC-1"), (dec!(1), dec!(0.3), dec!(0.7)));

    // Release 0.1: SQL float math would leave 0.19999… / 0.80000…1.
    inv.release_reservation(r1.id).expect("release");
    let (on_hand, allocated, available) = balance(&db, "FRAC-1");
    assert_eq!(on_hand, dec!(1));
    assert_eq!(allocated, dec!(0.2));
    assert_eq!(available, dec!(0.8));
    // The stored TEXT must be the exact decimal string as well.
    let conn = db.pool().get().expect("conn");
    let (alloc_text, avail_text): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances b
             JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = 'FRAC-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("row");
    assert_eq!(alloc_text.parse::<Decimal>().unwrap(), dec!(0.2));
    assert_eq!(avail_text.parse::<Decimal>().unwrap(), dec!(0.8));
    assert_allocation_invariant(&db);
}

#[test]
fn sqlite_fractional_expiry_round_trips_exactly() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("FRAC-2", dec!(1))).expect("create");
    let r1 = inv.reserve(reserve("FRAC-2", dec!(0.1), "a", Some(3600))).expect("reserve 0.1");
    inv.reserve(reserve("FRAC-2", dec!(0.2), "b", None)).expect("reserve 0.2");
    set_reservation_expiry_in_past(&db, r1.id);

    // A new reserve on the same balance lazily expires r1 with exact math.
    inv.reserve(reserve("FRAC-2", dec!(0.3), "c", None)).expect("reserve 0.3");
    assert_eq!(balance(&db, "FRAC-2"), (dec!(1), dec!(0.5), dec!(0.5)));
    assert_eq!(inv.get_reservation(r1.id).unwrap().unwrap().status, ReservationStatus::Expired);
    assert_allocation_invariant(&db);
}

// ---------------------------------------------------------------------------
// #3: expiry sweeper
// ---------------------------------------------------------------------------

#[test]
fn sqlite_expire_reservations_sweeps_idle_skus_in_batches() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("IDLE-1", dec!(10))).expect("create");
    inv.create_item(item("IDLE-2", dec!(10))).expect("create");
    let mut expired_ids = Vec::new();
    for (sku, reference) in [("IDLE-1", "a"), ("IDLE-1", "b"), ("IDLE-2", "c")] {
        let r = inv.reserve(reserve(sku, dec!(2), reference, Some(3600))).expect("reserve");
        expired_ids.push(r.id);
    }
    // One live hold that must survive the sweep.
    let live = inv.reserve(reserve("IDLE-2", dec!(1), "live", Some(3600))).expect("reserve");
    // Backdate only now: a later reserve on the same balance would otherwise
    // expire these lazily (correct, but not what this test exercises).
    for id in &expired_ids {
        set_reservation_expiry_in_past(&db, *id);
    }
    assert_eq!(balance(&db, "IDLE-1"), (dec!(10), dec!(4), dec!(6)));
    assert_eq!(balance(&db, "IDLE-2"), (dec!(10), dec!(3), dec!(7)));

    assert_eq!(inv.expire_reservations(Utc::now(), 0).unwrap(), 0, "limit 0 is a no-op");
    assert_eq!(inv.expire_reservations(Utc::now(), 2).unwrap(), 2, "first batch");
    assert_eq!(inv.expire_reservations(Utc::now(), 2).unwrap(), 1, "drains the rest");
    assert_eq!(inv.expire_reservations(Utc::now(), 2).unwrap(), 0, "idempotent");

    assert_eq!(balance(&db, "IDLE-1"), (dec!(10), dec!(0), dec!(10)));
    assert_eq!(balance(&db, "IDLE-2"), (dec!(10), dec!(1), dec!(9)));
    for id in expired_ids {
        assert_eq!(inv.get_reservation(id).unwrap().unwrap().status, ReservationStatus::Expired);
    }
    assert_eq!(inv.get_reservation(live.id).unwrap().unwrap().status, ReservationStatus::Pending);
    assert_allocation_invariant(&db);
}

#[test]
fn sqlite_allocation_invariant_holds_after_mixed_ops_and_sweep() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(item("MIX-1", dec!(100))).expect("create");
    inv.create_item(item("MIX-2", dec!(50.5))).expect("create");

    let a = inv.reserve(reserve("MIX-1", dec!(10), "a", Some(60))).unwrap();
    let b = inv.reserve(reserve("MIX-1", dec!(0.25), "b", None)).unwrap();
    let c = inv.reserve(reserve("MIX-2", dec!(7.75), "c", Some(60))).unwrap();
    let d = inv.reserve(reserve("MIX-2", dec!(1), "d", None)).unwrap();
    inv.confirm_reservation(b.id).unwrap();
    inv.release_reservation(d.id).unwrap();
    inv.release_reservation(d.id).unwrap(); // double release is idempotent
    inv.adjust(AdjustInventory {
        sku: "MIX-1".into(),
        location_id: None,
        quantity: dec!(-5),
        reason: "damage".into(),
        reference_type: None,
        reference_id: None,
    })
    .unwrap();
    set_reservation_expiry_in_past(&db, a.id);
    set_reservation_expiry_in_past(&db, c.id);
    assert_allocation_invariant(&db);

    assert_eq!(inv.expire_reservations(Utc::now(), 100).unwrap(), 2);
    assert_allocation_invariant(&db);
    assert_eq!(balance(&db, "MIX-1"), (dec!(95), dec!(0.25), dec!(94.75)));
    assert_eq!(balance(&db, "MIX-2"), (dec!(50.5), dec!(0), dec!(50.5)));
}

// ---------------------------------------------------------------------------
// #6/#7: adjust validation, reorder threshold, triggers
// ---------------------------------------------------------------------------

#[test]
fn sqlite_adjust_requires_a_reason_and_records_receipt_or_adjustment() {
    let db = db();
    let inv = db.inventory();
    let created = inv.create_item(item("ADJ-1", dec!(5))).expect("create");
    let blank = inv.adjust(AdjustInventory {
        sku: "ADJ-1".into(),
        location_id: None,
        quantity: dec!(1),
        reason: "   ".into(),
        reference_type: None,
        reference_id: None,
    });
    assert!(matches!(blank, Err(CommerceError::ValidationError(_))), "got {blank:?}");
    let zero = inv.adjust(AdjustInventory {
        sku: "ADJ-1".into(),
        location_id: None,
        quantity: dec!(0),
        reason: "count".into(),
        reference_type: None,
        reference_id: None,
    });
    assert!(matches!(zero, Err(CommerceError::ValidationError(_))));

    let up = inv
        .adjust(AdjustInventory {
            sku: "ADJ-1".into(),
            location_id: None,
            quantity: dec!(3),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
        })
        .unwrap();
    assert_eq!(up.transaction_type, TransactionType::Receipt);
    let down = inv
        .adjust(AdjustInventory {
            sku: "ADJ-1".into(),
            location_id: None,
            quantity: dec!(-1),
            reason: "damage".into(),
            reference_type: None,
            reference_id: None,
        })
        .unwrap();
    assert_eq!(down.transaction_type, TransactionType::Adjustment);
    assert_eq!(inv.get_transactions(created.id, 10).unwrap().len(), 3);
}

#[test]
fn sqlite_reorder_threshold_includes_safety_stock() {
    let db = db();
    let inv = db.inventory();
    inv.create_item(CreateInventoryItem {
        reorder_point: Some(dec!(5)),
        safety_stock: Some(dec!(3)),
        ..item("REO-1", dec!(7))
    })
    .unwrap();
    inv.create_item(CreateInventoryItem {
        reorder_point: Some(dec!(5)),
        safety_stock: Some(dec!(3)),
        ..item("REO-2", dec!(8))
    })
    .unwrap();
    inv.create_item(CreateInventoryItem { reorder_point: None, ..item("REO-3", dec!(0)) }).unwrap();

    let skus: Vec<String> = inv.get_reorder_needed().unwrap().into_iter().map(|s| s.sku).collect();
    assert_eq!(skus, vec!["REO-1".to_string()], "7 < 5+3 reorders; 8 does not; no point never");
}

#[test]
fn sqlite_balance_triggers_reject_negative_writes_but_leave_legacy_rows_editable() {
    let db = db();
    db.inventory().create_item(item("TRG-1", dec!(5))).unwrap();
    let conn = db.pool().get().expect("conn");
    // Each write keeps the BALANCE IDENTITY (migration 092:
    // available == on_hand - allocated) so it can only trip the non-negative
    // triggers this test is about, not the identity one.
    let err = conn
        .execute(
            "UPDATE inventory_balances SET quantity_on_hand = '-1', quantity_available = '-1' WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'TRG-1')",
            [],
        )
        .expect_err("negative available must be rejected");
    assert!(err.to_string().contains("must be >= 0"), "{err}");
    let err = conn
        .execute(
            "UPDATE inventory_balances SET quantity_allocated = '-0.5', quantity_available = '5.5' WHERE item_id = (SELECT id FROM inventory_items WHERE sku = 'TRG-1')",
            [],
        )
        .expect_err("negative allocated must be rejected");
    assert!(err.to_string().contains("must be >= 0"), "{err}");

    // A legacy row that already violates the invariant (simulated by
    // disabling the trigger) can still be updated while negative.
    conn.execute_batch("DROP TRIGGER trg_inventory_balances_non_negative_update").unwrap();
    conn.execute(
        "UPDATE inventory_balances SET quantity_on_hand = '-3', quantity_available = '-3'",
        [],
    )
    .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER trg_inventory_balances_non_negative_update
         BEFORE UPDATE OF quantity_available, quantity_allocated ON inventory_balances
         WHEN (CAST(NEW.quantity_available AS REAL) < 0 AND CAST(OLD.quantity_available AS REAL) >= 0)
           OR (CAST(NEW.quantity_allocated AS REAL) < 0 AND CAST(OLD.quantity_allocated AS REAL) >= 0)
         BEGIN SELECT RAISE(ABORT, 'inventory_balances: quantity_available and quantity_allocated must be >= 0'); END",
    )
    .unwrap();
    conn.execute(
        "UPDATE inventory_balances SET quantity_on_hand = '-2', quantity_available = '-2'",
        [],
    )
    .expect("legacy negative row stays editable");
    conn.execute(
        "UPDATE inventory_balances SET quantity_on_hand = '0', quantity_available = '0'",
        [],
    )
    .expect("and can be repaired");
}

// ---------------------------------------------------------------------------
// #4: backorder allocations are real reservations
// ---------------------------------------------------------------------------

fn create_backorder(db: &SqliteDatabase, sku: &str, qty: Decimal) -> stateset_core::Backorder {
    db.backorder()
        .create_backorder(CreateBackorder {
            order_id: Uuid::new_v4(),
            order_line_id: None,
            customer_id: Uuid::new_v4(),
            sku: sku.into(),
            quantity: qty,
            priority: None,
            expected_date: None,
            promised_date: None,
            source_location_id: None,
            notes: None,
        })
        .expect("create backorder")
}

const fn allocate(backorder_id: Uuid, qty: Decimal) -> AllocateBackorder {
    AllocateBackorder {
        backorder_id,
        quantity: qty,
        location_id: None,
        lot_id: None,
        expires_at: None,
    }
}

#[test]
fn sqlite_backorder_allocation_reserves_stock_and_blocks_cart_reserve() {
    let db = db();
    let inv = db.inventory();
    let bo_repo = db.backorder();
    inv.create_item(item("BO-1", dec!(5))).unwrap();
    let bo = create_backorder(&db, "BO-1", dec!(8));

    let allocation = bo_repo.allocate_backorder(allocate(bo.id, dec!(5))).expect("allocate 5");
    assert_eq!(allocation.status, AllocationStatus::Reserved);
    let reservation_id = allocation.reservation_id.expect("backed by a reservation");
    let reservation = inv.get_reservation(reservation_id).unwrap().expect("reservation row");
    assert_eq!(reservation.reference_type, "backorder");
    assert_eq!(reservation.reference_id, bo.id.to_string());
    assert_eq!(balance(&db, "BO-1"), (dec!(5), dec!(5), dec!(0)));
    assert_eq!(bo_repo.get_backorder(bo.id).unwrap().unwrap().status, BackorderStatus::Allocated);

    // A cart trying to take the same 5 units is refused.
    let refused = inv.reserve(reserve("BO-1", dec!(5), "cart-1", None));
    assert!(matches!(refused, Err(CommerceError::InsufficientStock { .. })), "got {refused:?}");
    // And so is over-allocating the backorder itself (8 - 5 = 3 left).
    let over = bo_repo.allocate_backorder(allocate(bo.id, dec!(4)));
    assert!(matches!(over, Err(CommerceError::ValidationError(_))), "got {over:?}");
    let zero = bo_repo.allocate_backorder(allocate(bo.id, dec!(0)));
    assert!(matches!(zero, Err(CommerceError::ValidationError(_))));
    assert_allocation_invariant(&db);

    // Releasing hands the units back; the cart can now reserve.
    let released = bo_repo.release_allocation(allocation.id).expect("release");
    assert_eq!(released.status, AllocationStatus::Released);
    assert_eq!(balance(&db, "BO-1"), (dec!(5), dec!(0), dec!(5)));
    assert_eq!(
        inv.get_reservation(reservation_id).unwrap().unwrap().status,
        ReservationStatus::Released
    );
    assert_eq!(bo_repo.get_backorder(bo.id).unwrap().unwrap().status, BackorderStatus::Pending);
    let again = bo_repo.release_allocation(allocation.id).expect("idempotent release");
    assert_eq!(again.status, AllocationStatus::Released);
    inv.reserve(reserve("BO-1", dec!(5), "cart-1", None)).expect("cart reserve now succeeds");
    assert_allocation_invariant(&db);
}

#[test]
fn sqlite_backorder_allocation_refused_without_stock() {
    let db = db();
    db.inventory().create_item(item("BO-2", dec!(2))).unwrap();
    let bo = create_backorder(&db, "BO-2", dec!(10));
    let err = db.backorder().allocate_backorder(allocate(bo.id, dec!(3)));
    assert!(matches!(err, Err(CommerceError::InsufficientStock { .. })), "got {err:?}");
    assert!(db.backorder().get_allocations(bo.id).unwrap().is_empty(), "no facade row written");
    let missing = db
        .backorder()
        .allocate_backorder(allocate(create_backorder(&db, "NOPE", dec!(1)).id, dec!(1)));
    assert!(matches!(missing, Err(CommerceError::InventoryItemNotFound(_))), "got {missing:?}");
}

#[test]
fn sqlite_backorder_fulfilment_consumes_allocated_stock() {
    let db = db();
    let inv = db.inventory();
    let bo_repo = db.backorder();
    let created = inv.create_item(item("BO-3", dec!(10))).unwrap();
    let bo = create_backorder(&db, "BO-3", dec!(6));
    let allocation = bo_repo.allocate_backorder(allocate(bo.id, dec!(4))).unwrap();
    let confirmed = bo_repo.confirm_allocation(allocation.id).expect("confirm");
    assert_eq!(confirmed.status, AllocationStatus::Confirmed);
    assert_eq!(
        inv.get_reservation(allocation.reservation_id.unwrap()).unwrap().unwrap().status,
        ReservationStatus::Confirmed
    );
    assert_eq!(balance(&db, "BO-3"), (dec!(10), dec!(4), dec!(6)));

    // Fulfil 3 of the 4 allocated: on-hand and allocated both drop by 3.
    bo_repo
        .fulfill_backorder(FulfillBackorder {
            backorder_id: bo.id,
            quantity: dec!(3),
            source_type: FulfillmentSourceType::Inventory,
            source_id: None,
            notes: None,
            fulfilled_by: None,
        })
        .expect("partial fulfil");
    assert_eq!(balance(&db, "BO-3"), (dec!(7), dec!(1), dec!(6)));
    let allocs = bo_repo.get_allocations(bo.id).unwrap();
    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].quantity, dec!(1));
    assert_eq!(allocs[0].status, AllocationStatus::Confirmed);
    assert_allocation_invariant(&db);

    // Fulfil the remaining 3: 1 from the allocation, 2 straight from stock.
    let done = bo_repo
        .fulfill_backorder(FulfillBackorder {
            backorder_id: bo.id,
            quantity: dec!(3),
            source_type: FulfillmentSourceType::Inventory,
            source_id: None,
            notes: None,
            fulfilled_by: None,
        })
        .expect("final fulfil");
    assert_eq!(done.status, BackorderStatus::Fulfilled);
    assert_eq!(balance(&db, "BO-3"), (dec!(4), dec!(0), dec!(4)));
    assert_eq!(bo_repo.get_allocations(bo.id).unwrap()[0].status, AllocationStatus::Fulfilled);
    assert_eq!(
        inv.get_reservation(allocation.reservation_id.unwrap()).unwrap().unwrap().status,
        ReservationStatus::Fulfilled
    );
    let shipments = inv
        .get_transactions(created.id, 20)
        .unwrap()
        .into_iter()
        .filter(|t| t.transaction_type == TransactionType::Shipment)
        .map(|t| t.quantity)
        .sum::<Decimal>();
    assert_eq!(shipments, dec!(-6), "every consumed unit has a shipment ledger row");
    assert_allocation_invariant(&db);
}

#[test]
fn sqlite_backorder_fulfilment_from_inventory_needs_available_stock() {
    let db = db();
    db.inventory().create_item(item("BO-4", dec!(2))).unwrap();
    let bo = create_backorder(&db, "BO-4", dec!(5));
    let err = db.backorder().fulfill_backorder(FulfillBackorder {
        backorder_id: bo.id,
        quantity: dec!(3),
        source_type: FulfillmentSourceType::Inventory,
        source_id: None,
        notes: None,
        fulfilled_by: None,
    });
    assert!(matches!(err, Err(CommerceError::InsufficientStock { .. })), "got {err:?}");
    let after = db.backorder().get_backorder(bo.id).unwrap().unwrap();
    assert_eq!(after.quantity_fulfilled, dec!(0), "nothing recorded when stock is short");
    assert_eq!(balance(&db, "BO-4"), (dec!(2), dec!(0), dec!(2)));

    // From a purchase-order receipt the units pass straight through.
    db.backorder()
        .fulfill_backorder(FulfillBackorder {
            backorder_id: bo.id,
            quantity: dec!(3),
            source_type: FulfillmentSourceType::PurchaseOrder,
            source_id: None,
            notes: None,
            fulfilled_by: None,
        })
        .expect("PO pass-through");
    assert_eq!(balance(&db, "BO-4"), (dec!(2), dec!(0), dec!(2)));
}

#[test]
fn sqlite_backorder_cancel_and_expiry_release_allocations() {
    let db = db();
    let inv = db.inventory();
    let bo_repo = db.backorder();
    inv.create_item(item("BO-5", dec!(10))).unwrap();

    let cancelled = create_backorder(&db, "BO-5", dec!(4));
    bo_repo.allocate_backorder(allocate(cancelled.id, dec!(4))).unwrap();
    assert_eq!(balance(&db, "BO-5"), (dec!(10), dec!(4), dec!(6)));
    let bo = bo_repo.cancel_backorder(cancelled.id).expect("cancel");
    assert_eq!(bo.status, BackorderStatus::Cancelled);
    assert_eq!(balance(&db, "BO-5"), (dec!(10), dec!(0), dec!(10)));
    assert_eq!(
        bo_repo.get_allocations(cancelled.id).unwrap()[0].status,
        AllocationStatus::Released
    );
    bo_repo.cancel_backorder(cancelled.id).expect("cancel is idempotent");

    let expiring = create_backorder(&db, "BO-5", dec!(3));
    let allocation = bo_repo
        .allocate_backorder(AllocateBackorder {
            expires_at: Some(Utc::now() + Duration::hours(1)),
            ..allocate(expiring.id, dec!(3))
        })
        .unwrap();
    assert_eq!(balance(&db, "BO-5"), (dec!(10), dec!(3), dec!(7)));
    {
        let conn = db.pool().get().unwrap();
        conn.execute(
            "UPDATE backorder_allocations SET expires_at = ? WHERE id = ?",
            rusqlite::params![
                (Utc::now() - Duration::minutes(1)).to_rfc3339(),
                allocation.id.to_string()
            ],
        )
        .unwrap();
    }
    assert_eq!(bo_repo.expire_allocations().unwrap(), 1);
    assert_eq!(bo_repo.expire_allocations().unwrap(), 0);
    assert_eq!(balance(&db, "BO-5"), (dec!(10), dec!(0), dec!(10)));
    assert_eq!(bo_repo.get_allocations(expiring.id).unwrap()[0].status, AllocationStatus::Expired);
    assert_eq!(
        bo_repo.get_backorder(expiring.id).unwrap().unwrap().status,
        BackorderStatus::Pending
    );
    assert_allocation_invariant(&db);
}

#[test]
fn sqlite_auto_allocate_serves_oldest_open_backorders_up_to_available() {
    let db = db();
    let inv = db.inventory();
    let bo_repo = db.backorder();
    inv.create_item(item("BO-6", dec!(7))).unwrap();
    let first = create_backorder(&db, "BO-6", dec!(5));
    let second = create_backorder(&db, "BO-6", dec!(5));
    inv.reserve(reserve("BO-6", dec!(1), "cart", None)).unwrap(); // 6 available

    let created = bo_repo.auto_allocate_inventory("BO-6").expect("auto allocate");
    assert_eq!(created.len(), 2);
    assert_eq!((created[0].backorder_id, created[0].quantity), (first.id, dec!(5)));
    assert_eq!((created[1].backorder_id, created[1].quantity), (second.id, dec!(1)));
    assert_eq!(balance(&db, "BO-6"), (dec!(7), dec!(7), dec!(0)));
    assert_eq!(
        bo_repo.get_backorder(first.id).unwrap().unwrap().status,
        BackorderStatus::Allocated
    );

    assert!(bo_repo.auto_allocate_inventory("BO-6").unwrap().is_empty(), "nothing left");
    assert!(bo_repo.auto_allocate_inventory("UNKNOWN-SKU").unwrap().is_empty());

    // More stock arrives: only the still-open remainder (4) is allocated.
    inv.adjust(AdjustInventory {
        sku: "BO-6".into(),
        location_id: None,
        quantity: dec!(10),
        reason: "receipt".into(),
        reference_type: None,
        reference_id: None,
    })
    .unwrap();
    let more = bo_repo.auto_allocate_inventory("BO-6").unwrap();
    assert_eq!(more.len(), 1);
    assert_eq!((more[0].backorder_id, more[0].quantity), (second.id, dec!(4)));
    assert_eq!(balance(&db, "BO-6"), (dec!(17), dec!(11), dec!(6)));
    assert_allocation_invariant(&db);
}
