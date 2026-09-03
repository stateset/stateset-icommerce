//! Inventory round 5 (Postgres): transactional `adjust` (balance + ledger in
//! one tx, `FOR UPDATE`, auto-created balance, retry), row locks on
//! release/confirm/expire with idempotent double-release, the reservation
//! expiry sweeper, backorder allocations backed by real reservations, the
//! reorder threshold (safety stock, one row per SKU), ledger-type parity and
//! the non-negative CHECK from migration 090.
//!
//! Requires a live Postgres (`POSTGRES_URL` / `DATABASE_URL`); skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustInventory, AllocateBackorder, AllocationStatus, BackorderStatus, CommerceError,
    CreateBackorder, CreateInventoryItem, FulfillBackorder, FulfillmentSourceType,
    ReservationStatus, ReserveInventory, TransactionType,
};
use stateset_db::PostgresDatabase;
use stateset_db::postgres::{PgBackorderRepository, PgInventoryRepository};
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

macro_rules! require_pg {
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

/// The seeded default location pins id 1 without advancing the sequence, so
/// a plain INSERT can collide; pick an explicit id and tolerate races.
async fn ensure_location(db: &PostgresDatabase, code: &str) -> i32 {
    for _ in 0..10 {
        if let Some(id) =
            sqlx::query_scalar::<_, i32>("SELECT id FROM inventory_locations WHERE code = $1")
                .bind(code)
                .fetch_optional(db.pool())
                .await
                .expect("select location")
        {
            return id;
        }
        let _ = sqlx::query(
            "INSERT INTO inventory_locations (id, name, code)
             SELECT COALESCE(MAX(id), 0) + 1, 'Round 5', $1 FROM inventory_locations
             ON CONFLICT DO NOTHING",
        )
        .bind(code)
        .execute(db.pool())
        .await;
    }
    panic!("could not create inventory location {code}");
}

fn sku(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn seed(inv: &PgInventoryRepository, sku: &str, qty: Decimal) -> i64 {
    inv.create_item_async(CreateInventoryItem {
        sku: sku.to_string(),
        name: format!("Round 5 {sku}"),
        initial_quantity: Some(qty),
        ..Default::default()
    })
    .await
    .expect("create item")
    .id
}

fn adjust(sku: &str, qty: Decimal, reason: &str) -> AdjustInventory {
    AdjustInventory {
        sku: sku.to_string(),
        location_id: None,
        quantity: qty,
        reason: reason.into(),
        reference_type: None,
        reference_id: None,
    }
}

fn reserve(sku: &str, qty: Decimal, reference: &str, expires: Option<i64>) -> ReserveInventory {
    ReserveInventory {
        sku: sku.to_string(),
        location_id: None,
        quantity: qty,
        reference_type: "cart".into(),
        reference_id: reference.into(),
        expires_in_seconds: expires,
    }
}

async fn balance(inv: &PgInventoryRepository, sku: &str) -> (Decimal, Decimal, Decimal) {
    let stock = inv.get_stock_async(sku).await.expect("stock").expect("exists");
    (stock.total_on_hand, stock.total_allocated, stock.total_available)
}

async fn backdate(db: &PostgresDatabase, reservation_id: Uuid) {
    sqlx::query("UPDATE inventory_reservations SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::minutes(5))
        .bind(reservation_id)
        .execute(db.pool())
        .await
        .expect("backdate");
}

/// `SUM(open reservations)` == `quantity_allocated` for every balance of the
/// given items (the database is shared with other tests, so scope it).
async fn assert_allocation_invariant(db: &PostgresDatabase, item_ids: &[i64]) {
    let rows: Vec<(i64, i32, Decimal, Decimal)> = sqlx::query_as(
        "SELECT b.item_id, b.location_id, b.quantity_allocated,
                COALESCE((SELECT SUM(r.quantity) FROM inventory_reservations r
                          WHERE r.item_id = b.item_id AND r.location_id = b.location_id
                            AND r.status IN ('pending', 'confirmed', 'allocated')), 0)
         FROM inventory_balances b WHERE b.item_id = ANY($1)",
    )
    .bind(item_ids)
    .fetch_all(db.pool())
    .await
    .expect("invariant query");
    assert!(!rows.is_empty());
    for (item_id, location_id, allocated, held) in rows {
        assert_eq!(allocated, held, "item {item_id} @ {location_id}: allocated != open holds");
    }
}

// ---------------------------------------------------------------------------
// #1: adjust is transactional, validated, auto-creating and retried
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_adjust_rolls_back_balance_when_ledger_insert_fails() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("ADJ-ATOMIC");
    let item_id = seed(&inv, &sku, dec!(10)).await;

    // Make the audit INSERT fail for one specific reason string.
    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION round5_fail_audit() RETURNS trigger AS $$
         BEGIN
           IF NEW.reason = '__round5_fail_audit__' THEN
             RAISE EXCEPTION 'round5: audit insert refused';
           END IF;
           RETURN NEW;
         END $$ LANGUAGE plpgsql;
         DROP TRIGGER IF EXISTS round5_fail_audit ON inventory_transactions;
         CREATE TRIGGER round5_fail_audit BEFORE INSERT ON inventory_transactions
           FOR EACH ROW EXECUTE FUNCTION round5_fail_audit();",
    )
    .execute(db.pool())
    .await
    .expect("install failing trigger");

    let err = inv.adjust_async(adjust(&sku, dec!(-4), "__round5_fail_audit__")).await;
    sqlx::raw_sql("DROP TRIGGER IF EXISTS round5_fail_audit ON inventory_transactions")
        .execute(db.pool())
        .await
        .expect("drop trigger");
    assert!(err.is_err(), "adjust must fail when its ledger row cannot be written");

    // The balance is untouched: no units moved without a ledger row.
    assert_eq!(balance(&inv, &sku).await, (dec!(10), dec!(0), dec!(10)));
    let ledger = inv.get_transactions_async(item_id, 10).await.unwrap();
    assert_eq!(ledger.len(), 1, "only the initial receipt");
    let version: i32 =
        sqlx::query_scalar("SELECT version FROM inventory_balances WHERE item_id = $1")
            .bind(item_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(version, 1, "no partial version bump either");
}

#[tokio::test]
async fn postgres_concurrent_adjusts_all_land_with_one_ledger_row_each() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("ADJ-CONC");
    let item_id = seed(&inv, &sku, dec!(100)).await;

    let mut tasks = Vec::new();
    for i in 0..16 {
        let inv = inv.clone();
        let sku = sku.clone();
        tasks.push(tokio::spawn(async move {
            let qty = if i % 2 == 0 { dec!(1.5) } else { dec!(-0.5) };
            inv.adjust_async(adjust(&sku, qty, "concurrent")).await
        }));
    }
    for task in tasks {
        task.await.expect("join").expect("every adjust succeeds (retry absorbs conflicts)");
    }
    // 8 * 1.5 - 8 * 0.5 = 8
    assert_eq!(balance(&inv, &sku).await, (dec!(108), dec!(0), dec!(108)));
    let ledger = inv.get_transactions_async(item_id, 50).await.unwrap();
    assert_eq!(ledger.len(), 17, "initial receipt + 16 adjustments");
    let sum: Decimal = ledger.iter().map(|t| t.quantity).sum();
    assert_eq!(sum, dec!(108), "ledger reconciles to on-hand");
}

#[tokio::test]
async fn postgres_adjust_validates_and_auto_creates_missing_balance() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("ADJ-VALID");
    let item_id = seed(&inv, &sku, dec!(3)).await;

    for bad in [adjust(&sku, dec!(0), "zero"), adjust(&sku, dec!(1), "  ")] {
        let err = inv.adjust_async(bad).await;
        assert!(matches!(err, Err(CommerceError::ValidationError(_))), "got {err:?}");
    }
    let missing = inv.adjust_async(adjust("NO-SUCH-SKU-ROUND5", dec!(1), "x")).await;
    assert!(matches!(missing, Err(CommerceError::InventoryItemNotFound(_))));
    let over = inv.adjust_async(adjust(&sku, dec!(-4), "too much")).await;
    assert!(matches!(over, Err(CommerceError::InsufficientStock { .. })));

    // A second location with no balance row yet: SQLite auto-creates, and
    // now so does Postgres (previously NotFound).
    let location_id = ensure_location(&db, "ROUND5-LOC").await;
    let receipt = inv
        .adjust_async(AdjustInventory {
            location_id: Some(location_id),
            ..adjust(&sku, dec!(2), "transfer in")
        })
        .await
        .expect("auto-created balance");
    assert_eq!(
        receipt.transaction_type,
        TransactionType::Receipt,
        "positive => receipt (SQLite parity)"
    );
    let down = inv.adjust_async(adjust(&sku, dec!(-1), "damage")).await.unwrap();
    assert_eq!(down.transaction_type, TransactionType::Adjustment);
    let second = inv.get_balance_async(item_id, location_id).await.unwrap().expect("balance row");
    assert_eq!((second.quantity_on_hand, second.quantity_available), (dec!(2), dec!(2)));
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(0), dec!(4)));
}

// ---------------------------------------------------------------------------
// #5: locks on release/confirm/expire, idempotent double release
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_concurrent_releases_of_one_reservation_are_idempotent() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("REL-CONC");
    let item_id = seed(&inv, &sku, dec!(10)).await;
    let r = inv.reserve_async(reserve(&sku, dec!(4), "a", None)).await.unwrap();
    let other = inv.reserve_async(reserve(&sku, dec!(1), "b", None)).await.unwrap();

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let inv = inv.clone();
        tasks.push(tokio::spawn(async move { inv.release_reservation_async(r.id).await }));
    }
    // A concurrent release of ANOTHER reservation on the same balance must
    // not surface as VersionConflict either.
    let inv2 = inv.clone();
    tasks.push(tokio::spawn(async move { inv2.release_reservation_async(other.id).await }));
    for task in tasks {
        task.await.unwrap().expect("release is Ok even when racing / repeated");
    }
    assert_eq!(balance(&inv, &sku).await, (dec!(10), dec!(0), dec!(10)));
    assert_eq!(
        inv.get_reservation_async(r.id).await.unwrap().unwrap().status,
        ReservationStatus::Released
    );
    assert_allocation_invariant(&db, &[item_id]).await;
}

/// The name says "hold row locks", so the calls must actually RACE: the
/// original version ran every confirm/expire/release sequentially and proved
/// only idempotence. Confirms, expiries and releases now contend on the same
/// balance row from several tasks on a real multi-threaded runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_confirm_and_expire_hold_row_locks_and_keep_invariant() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("CONF");
    let item_id = seed(&inv, &sku, dec!(10)).await;
    let a = inv.reserve_async(reserve(&sku, dec!(2), "a", Some(3600))).await.unwrap();
    let b = inv.reserve_async(reserve(&sku, dec!(3), "b", None)).await.unwrap();
    backdate(&db, a.id).await;

    // Confirming an expired hold reports it expired (and hands units back);
    // several racing confirms must all agree on that, and none may surface a
    // VersionConflict from the balance row they all touch.
    let mut expiring = Vec::new();
    for _ in 0..6 {
        let inv = inv.clone();
        expiring.push(tokio::spawn(async move { inv.confirm_reservation_async(a.id).await }));
    }
    let mut expired_reports = 0;
    for task in expiring {
        match task.await.unwrap() {
            Err(CommerceError::ReservationExpired(_)) => expired_reports += 1,
            // Once the hold is `expired`, a later confirm reports the terminal
            // state rather than re-expiring it.
            Ok(()) => {}
            other => panic!("racing confirm of an expired hold must not fail: {other:?}"),
        }
    }
    assert!(expired_reports >= 1, "at least one confirm must report the expiry");
    assert_eq!(balance(&inv, &sku).await, (dec!(10), dec!(3), dec!(7)));

    // Confirms of a live hold and releases of the expired one race on the same
    // balance row; both are idempotent and neither may error.
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let inv = inv.clone();
        tasks.push(tokio::spawn(async move { inv.confirm_reservation_async(b.id).await }));
    }
    for _ in 0..6 {
        let inv = inv.clone();
        tasks.push(tokio::spawn(async move { inv.release_reservation_async(a.id).await }));
    }
    // ... and a sweeper runs over the same rows at the same time.
    for _ in 0..2 {
        let inv = inv.clone();
        tasks.push(tokio::spawn(async move {
            inv.expire_reservations_async(Utc::now(), 100).await.map(|_| ())
        }));
    }
    for task in tasks {
        task.await.unwrap().expect("racing confirm/release/sweep must all be Ok");
    }

    assert_eq!(balance(&inv, &sku).await, (dec!(10), dec!(3), dec!(7)));
    assert_eq!(
        inv.get_reservation_async(a.id).await.unwrap().unwrap().status,
        ReservationStatus::Expired
    );
    assert_eq!(
        inv.get_reservation_async(b.id).await.unwrap().unwrap().status,
        ReservationStatus::Confirmed
    );
    assert_allocation_invariant(&db, &[item_id]).await;
}

// ---------------------------------------------------------------------------
// #3: sweeper
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_expire_reservations_sweeps_idle_skus_and_keeps_invariant() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku1 = sku("SWEEP-A");
    let sku2 = sku("SWEEP-B");
    let id1 = seed(&inv, &sku1, dec!(20)).await;
    let id2 = seed(&inv, &sku2, dec!(5.5)).await;

    let stale: Vec<Uuid> = {
        let mut v = Vec::new();
        v.push(inv.reserve_async(reserve(&sku1, dec!(4), "a", Some(60))).await.unwrap().id);
        v.push(inv.reserve_async(reserve(&sku1, dec!(0.25), "b", Some(60))).await.unwrap().id);
        v.push(inv.reserve_async(reserve(&sku2, dec!(1.5), "c", Some(60))).await.unwrap().id);
        v
    };
    let live = inv.reserve_async(reserve(&sku2, dec!(2), "live", Some(3600))).await.unwrap();
    let confirmed = inv.reserve_async(reserve(&sku1, dec!(1), "conf", None)).await.unwrap();
    inv.confirm_reservation_async(confirmed.id).await.unwrap();
    for id in &stale {
        backdate(&db, *id).await;
    }
    assert_allocation_invariant(&db, &[id1, id2]).await;

    // Other tests may have stale rows of their own in the shared database,
    // so count only what lands on our items.
    let mut total = 0;
    loop {
        let n = inv.expire_reservations_async(Utc::now(), 2).await.unwrap();
        total += n;
        if n < 2 {
            break;
        }
    }
    assert!(total >= 3, "swept at least our three stale holds (got {total})");
    assert_eq!(inv.expire_reservations_async(Utc::now(), 0).await.unwrap(), 0);

    assert_eq!(balance(&inv, &sku1).await, (dec!(20), dec!(1), dec!(19)));
    assert_eq!(balance(&inv, &sku2).await, (dec!(5.5), dec!(2), dec!(3.5)));
    for id in stale {
        assert_eq!(
            inv.get_reservation_async(id).await.unwrap().unwrap().status,
            ReservationStatus::Expired
        );
    }
    assert_eq!(
        inv.get_reservation_async(live.id).await.unwrap().unwrap().status,
        ReservationStatus::Pending
    );
    assert_allocation_invariant(&db, &[id1, id2]).await;
}

// ---------------------------------------------------------------------------
// #6/#7: reorder parity and CHECK constraint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_reorder_needed_dedups_skus_and_adds_safety_stock() {
    let db = require_pg!();
    let inv = db.inventory();
    let below = sku("REO-BELOW");
    let above = sku("REO-ABOVE");
    let nopoint = sku("REO-NONE");
    let below_id = inv
        .create_item_async(CreateInventoryItem {
            reorder_point: Some(dec!(5)),
            safety_stock: Some(dec!(3)),
            initial_quantity: Some(dec!(7)),
            sku: below.clone(),
            name: "below".into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .id;
    inv.create_item_async(CreateInventoryItem {
        reorder_point: Some(dec!(5)),
        safety_stock: Some(dec!(3)),
        initial_quantity: Some(dec!(8)),
        sku: above.clone(),
        name: "above".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    inv.create_item_async(CreateInventoryItem {
        reorder_point: None,
        initial_quantity: Some(dec!(0)),
        sku: nopoint.clone(),
        name: "no point".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    // A second below-threshold balance for the same SKU must not duplicate it.
    let location_id = ensure_location(&db, "ROUND5-LOC").await;
    sqlx::query(
        "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock)
         VALUES ($1, $2, 1, 0, 1, 5, 3)",
    )
    .bind(below_id)
    .bind(location_id)
    .execute(db.pool())
    .await
    .unwrap();

    let skus: Vec<String> =
        inv.get_reorder_needed_async().await.unwrap().into_iter().map(|s| s.sku).collect();
    assert_eq!(skus.iter().filter(|s| **s == below).count(), 1, "listed exactly once");
    assert!(!skus.contains(&above), "8 >= 5 + 3 does not reorder");
    assert!(!skus.contains(&nopoint), "no reorder point never reorders (COALESCE(…,0) bug)");
}

#[tokio::test]
async fn postgres_check_constraint_rejects_negative_balances() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("CHK");
    let item_id = seed(&inv, &sku, dec!(5)).await;
    // Each write is COHERENT (available = on_hand - allocated, migration 099)
    // so it can only trip the non-negative CHECK, not the identity one.
    for assignment in [
        "quantity_on_hand = -1, quantity_available = -1",
        "quantity_allocated = -1, quantity_available = 6",
    ] {
        let err =
            sqlx::query(&format!("UPDATE inventory_balances SET {assignment} WHERE item_id = $1"))
                .bind(item_id)
                .execute(db.pool())
                .await
                .expect_err("negative balance must be rejected by CHECK");
        assert!(err.to_string().contains("chk_inventory_balances_non_negative"), "{err}");
    }
    let validated: bool = sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint WHERE conname = 'chk_inventory_balances_non_negative'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(validated, "clean database gets the fully validated constraint");
}

// ---------------------------------------------------------------------------
// #4: backorder allocations
// ---------------------------------------------------------------------------

async fn create_backorder(bo: &PgBackorderRepository, sku: &str, qty: Decimal) -> Uuid {
    bo.create_backorder_async(CreateBackorder {
        order_id: Uuid::new_v4(),
        order_line_id: None,
        customer_id: Uuid::new_v4(),
        sku: sku.to_string(),
        quantity: qty,
        priority: None,
        expected_date: None,
        promised_date: None,
        source_location_id: None,
        notes: None,
    })
    .await
    .expect("create backorder")
    .id
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

const fn fulfil(
    backorder_id: Uuid,
    qty: Decimal,
    source: FulfillmentSourceType,
) -> FulfillBackorder {
    FulfillBackorder {
        backorder_id,
        quantity: qty,
        source_type: source,
        source_id: None,
        notes: None,
        fulfilled_by: None,
    }
}

#[tokio::test]
async fn postgres_backorder_allocation_reserves_stock_and_blocks_cart_reserve() {
    let db = require_pg!();
    let inv = db.inventory();
    let bo = db.backorder();
    let sku = sku("BO");
    let item_id = seed(&inv, &sku, dec!(5)).await;
    let backorder = create_backorder(&bo, &sku, dec!(8)).await;

    let allocation =
        bo.allocate_backorder_async(allocate(backorder, dec!(5))).await.expect("allocate 5");
    let reservation_id = allocation.reservation_id.expect("backed by a reservation");
    let reservation = inv.get_reservation_async(reservation_id).await.unwrap().unwrap();
    assert_eq!(
        (reservation.reference_type.as_str(), reservation.reference_id.as_str()),
        ("backorder", backorder.to_string().as_str())
    );
    assert_eq!(balance(&inv, &sku).await, (dec!(5), dec!(5), dec!(0)));
    assert_eq!(
        bo.get_backorder_async(backorder).await.unwrap().unwrap().status,
        BackorderStatus::Allocated
    );

    let refused = inv.reserve_async(reserve(&sku, dec!(5), "cart", None)).await;
    assert!(matches!(refused, Err(CommerceError::InsufficientStock { .. })), "got {refused:?}");
    let over = bo.allocate_backorder_async(allocate(backorder, dec!(4))).await;
    assert!(matches!(over, Err(CommerceError::ValidationError(_))), "got {over:?}");
    assert_allocation_invariant(&db, &[item_id]).await;

    let released = bo.release_allocation_async(allocation.id).await.unwrap();
    assert_eq!(released.status, AllocationStatus::Released);
    assert_eq!(
        bo.release_allocation_async(allocation.id).await.unwrap().status,
        AllocationStatus::Released
    );
    assert_eq!(balance(&inv, &sku).await, (dec!(5), dec!(0), dec!(5)));
    assert_eq!(
        bo.get_backorder_async(backorder).await.unwrap().unwrap().status,
        BackorderStatus::Pending
    );
    inv.reserve_async(reserve(&sku, dec!(5), "cart", None)).await.expect("cart reserve now ok");

    // Without stock the allocation is refused and no facade row is written.
    let short = create_backorder(&bo, &sku, dec!(3)).await;
    let err = bo.allocate_backorder_async(allocate(short, dec!(1))).await;
    assert!(matches!(err, Err(CommerceError::InsufficientStock { .. })), "got {err:?}");
    assert!(bo.get_allocations_async(short).await.unwrap().is_empty());
    assert_allocation_invariant(&db, &[item_id]).await;
}

#[tokio::test]
async fn postgres_backorder_fulfil_cancel_expire_and_auto_allocate_move_real_stock() {
    let db = require_pg!();
    let inv = db.inventory();
    let bo = db.backorder();
    let sku = sku("BO-FLOW");
    let item_id = seed(&inv, &sku, dec!(10)).await;

    // Fulfilment consumes the allocation (on-hand and allocated both drop),
    // then takes the remainder from available stock.
    let backorder = create_backorder(&bo, &sku, dec!(6)).await;
    let allocation = bo.allocate_backorder_async(allocate(backorder, dec!(4))).await.unwrap();
    assert_eq!(
        bo.confirm_allocation_async(allocation.id).await.unwrap().status,
        AllocationStatus::Confirmed
    );
    bo.fulfill_backorder_async(fulfil(backorder, dec!(3), FulfillmentSourceType::Inventory))
        .await
        .unwrap();
    assert_eq!(balance(&inv, &sku).await, (dec!(7), dec!(1), dec!(6)));
    let done = bo
        .fulfill_backorder_async(fulfil(backorder, dec!(3), FulfillmentSourceType::Inventory))
        .await
        .unwrap();
    assert_eq!(done.status, BackorderStatus::Fulfilled);
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(0), dec!(4)));
    assert_eq!(
        bo.get_allocations_async(backorder).await.unwrap()[0].status,
        AllocationStatus::Fulfilled
    );
    assert_eq!(
        inv.get_reservation_async(allocation.reservation_id.unwrap())
            .await
            .unwrap()
            .unwrap()
            .status,
        ReservationStatus::Fulfilled
    );
    let shipped: Decimal = inv
        .get_transactions_async(item_id, 50)
        .await
        .unwrap()
        .into_iter()
        .filter(|t| t.transaction_type == TransactionType::Shipment)
        .map(|t| t.quantity)
        .sum();
    assert_eq!(shipped, dec!(-6));
    let short = bo
        .fulfill_backorder_async(fulfil(
            create_backorder(&bo, &sku, dec!(9)).await,
            dec!(9),
            FulfillmentSourceType::Inventory,
        ))
        .await;
    assert!(matches!(short, Err(CommerceError::InsufficientStock { .. })), "got {short:?}");
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(0), dec!(4)));
    assert_allocation_invariant(&db, &[item_id]).await;

    // Cancel releases; a fulfilled backorder cannot be cancelled.
    let cancelled = create_backorder(&bo, &sku, dec!(2)).await;
    bo.allocate_backorder_async(allocate(cancelled, dec!(2))).await.unwrap();
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(2), dec!(2)));
    assert_eq!(
        bo.cancel_backorder_async(cancelled).await.unwrap().status,
        BackorderStatus::Cancelled
    );
    bo.cancel_backorder_async(cancelled).await.expect("idempotent");
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(0), dec!(4)));
    assert!(matches!(
        bo.cancel_backorder_async(backorder).await,
        Err(CommerceError::ValidationError(_))
    ));

    // Expiry releases the reservation and drops the backorder back to pending.
    let expiring = create_backorder(&bo, &sku, dec!(2)).await;
    let expiring_alloc = bo
        .allocate_backorder_async(AllocateBackorder {
            expires_at: Some(Utc::now() + Duration::hours(1)),
            ..allocate(expiring, dec!(2))
        })
        .await
        .unwrap();
    sqlx::query("UPDATE backorder_allocations SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::minutes(1))
        .bind(expiring_alloc.id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(bo.expire_allocations_async().await.unwrap() >= 1);
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(0), dec!(4)));
    assert_eq!(
        bo.get_allocations_async(expiring).await.unwrap()[0].status,
        AllocationStatus::Expired
    );
    assert_eq!(
        bo.get_backorder_async(expiring).await.unwrap().unwrap().status,
        BackorderStatus::Pending
    );

    // Auto-allocate: oldest open backorder first, up to what is available.
    let created = bo.auto_allocate_inventory_async(&sku).await.unwrap();
    // `expiring` (2 remaining, created last) and `short` (9 remaining) are
    // open; pending backorders are served oldest first: short gets 4.
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].quantity, dec!(4));
    assert_eq!(balance(&inv, &sku).await, (dec!(4), dec!(4), dec!(0)));
    assert!(bo.auto_allocate_inventory_async(&sku).await.unwrap().is_empty());
    assert_allocation_invariant(&db, &[item_id]).await;
}
