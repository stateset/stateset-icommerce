//! Inventory round 6 (Postgres): the database-enforced balance identity
//! (migration 099), the clamp path repairing drift instead of only logging it,
//! the backorder writes that used to bypass the retry wrapper, and
//! `auto_allocate` taking the balance lock so a losing candidate skips
//! instead of aborting the whole batch.
//!
//! Requires a live Postgres (`POSTGRES_URL` / `DATABASE_URL`); skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustInventory, BackorderPriority, CommerceError, CreateBackorder, CreateInventoryItem,
    ReservationStatus, ReserveInventory,
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

fn sku(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
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

async fn seed(inv: &PgInventoryRepository, sku: &str, qty: Decimal) -> i64 {
    inv.create_item_async(CreateInventoryItem {
        sku: sku.to_string(),
        name: format!("Round 6 {sku}"),
        initial_quantity: Some(qty),
        ..Default::default()
    })
    .await
    .expect("create item")
    .id
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

/// BOTH balance identities for the given items:
///
/// 1. `quantity_allocated = SUM(open reservations)` (round 5), and
/// 2. `quantity_available = quantity_on_hand - quantity_allocated` (round 6).
async fn assert_balance_identities(db: &PostgresDatabase, item_ids: &[i64]) {
    let rows: Vec<(i64, i32, Decimal, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "SELECT b.item_id, b.location_id, b.quantity_on_hand, b.quantity_allocated,
                b.quantity_available,
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
    for (item_id, location_id, on_hand, allocated, available, held) in rows {
        assert_eq!(
            allocated, held,
            "item {item_id} @ {location_id}: allocated {allocated} != open holds {held}"
        );
        assert_eq!(
            available,
            on_hand - allocated,
            "item {item_id} @ {location_id}: available {available} != {on_hand} - {allocated}"
        );
    }
}

// ---------------------------------------------------------------------------
// #2: the balance identity is enforced by the database (migration 099)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_migration_099_rejects_an_incoherent_balance_write() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("PG-IDENT");
    let item_id = seed(&inv, &sku, dec!(10)).await;

    // Move on-hand without recomputing available: exactly the raw-SQL write
    // that used to leave the row lying to every future `reserve`.
    let err = sqlx::query("UPDATE inventory_balances SET quantity_on_hand = 20 WHERE item_id = $1")
        .bind(item_id)
        .execute(db.pool())
        .await
        .expect_err("the identity CHECK must reject this");
    assert!(
        err.to_string().contains("chk_inventory_balances_identity"),
        "expected the identity CHECK, got {err}"
    );

    // The coherent form of the same change is accepted.
    sqlx::query(
        "UPDATE inventory_balances SET quantity_on_hand = 20, quantity_available = 20
         WHERE item_id = $1",
    )
    .bind(item_id)
    .execute(db.pool())
    .await
    .expect("coherent update");
    assert_eq!(balance(&inv, &sku).await, (dec!(20), dec!(0), dec!(20)));
    assert_balance_identities(&db, &[item_id]).await;
}

#[tokio::test]
async fn postgres_migration_099_is_recorded_and_validated_on_a_clean_database() {
    let db = require_pg!();
    let applied: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _migrations WHERE name = $1)")
            .bind("099_inventory_balance_identity")
            .fetch_one(db.pool())
            .await
            .expect("migration lookup");
    assert!(applied, "099 must be applied");

    // On a clean test database the NOT VALID constraint is promoted to VALID.
    let convalidated: bool = sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint
         WHERE conname = 'chk_inventory_balances_identity'
           AND conrelid = 'inventory_balances'::regclass",
    )
    .fetch_one(db.pool())
    .await
    .expect("constraint lookup");
    assert!(convalidated, "a clean database must end up with a VALIDATEd constraint");
}

#[tokio::test]
async fn postgres_release_repairs_a_drifted_balance_instead_of_only_clamping() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku = sku("PG-DRIFT");
    let item_id = seed(&inv, &sku, dec!(10)).await;
    let keep = inv.reserve_async(reserve(&sku, dec!(3), "keep", None)).await.expect("keep");
    let drop = inv.reserve_async(reserve(&sku, dec!(4), "drop", None)).await.expect("drop");
    assert_eq!(balance(&inv, &sku).await, (dec!(10), dec!(7), dec!(3)));

    // Simulate pre-fix drift: allocated forgets both holds (coherently, so the
    // identity CHECK allows the write) while the reservations stay open.
    sqlx::query(
        "UPDATE inventory_balances SET quantity_allocated = 1, quantity_available = 9
         WHERE item_id = $1",
    )
    .bind(item_id)
    .execute(db.pool())
    .await
    .expect("install drift");

    // Releasing `drop` (4 units) needs more than the recorded 1: the old code
    // clamped allocated to 0 and left `keep`'s 3 units unaccounted for.
    inv.release_reservation_async(drop.id).await.expect("release");
    assert_eq!(
        balance(&inv, &sku).await,
        (dec!(10), dec!(3), dec!(7)),
        "the drifted balance must be repaired to the units `keep` still holds"
    );
    assert_eq!(
        inv.get_reservation_async(keep.id).await.unwrap().unwrap().status,
        ReservationStatus::Pending
    );
    assert_balance_identities(&db, &[item_id]).await;
}

#[tokio::test]
async fn postgres_mixed_operations_plus_a_sweep_keep_both_identities() {
    let db = require_pg!();
    let inv = db.inventory();
    let sku1 = sku("PG-MIX-A");
    let sku2 = sku("PG-MIX-B");
    let id1 = seed(&inv, &sku1, dec!(20)).await;
    let id2 = seed(&inv, &sku2, dec!(7.5)).await;

    let a = inv.reserve_async(reserve(&sku1, dec!(4), "a", Some(3600))).await.unwrap();
    let b = inv.reserve_async(reserve(&sku1, dec!(2.25), "b", None)).await.unwrap();
    let c = inv.reserve_async(reserve(&sku2, dec!(1.5), "c", Some(3600))).await.unwrap();
    inv.confirm_reservation_async(b.id).await.unwrap();
    inv.adjust_async(AdjustInventory {
        sku: sku1.clone(),
        location_id: None,
        quantity: dec!(5),
        reason: "restock".into(),
        reference_type: None,
        reference_id: None,
    })
    .await
    .expect("adjust");
    inv.release_reservation_async(a.id).await.unwrap();
    let d = inv.reserve_async(reserve(&sku1, dec!(3), "d", Some(3600))).await.unwrap();
    backdate(&db, c.id).await;
    backdate(&db, d.id).await;
    assert_balance_identities(&db, &[id1, id2]).await;

    // The sweeper (nothing else touches either SKU) reclaims both holds.
    let expired = inv.expire_reservations_async(Utc::now(), 100).await.expect("sweep");
    assert!(expired >= 2, "both expired holds must be swept, got {expired}");
    assert_eq!(balance(&inv, &sku1).await, (dec!(25), dec!(2.25), dec!(22.75)));
    assert_eq!(balance(&inv, &sku2).await, (dec!(7.5), dec!(0), dec!(7.5)));
    assert_balance_identities(&db, &[id1, id2]).await;
}

// ---------------------------------------------------------------------------
// #3 / #4: backorder writes are retried, and auto-allocation locks + skips
// ---------------------------------------------------------------------------

async fn create_backorder(bo: &PgBackorderRepository, sku: &str, qty: Decimal) -> Uuid {
    bo.create_backorder_async(CreateBackorder {
        order_id: Uuid::new_v4(),
        order_line_id: None,
        customer_id: Uuid::new_v4(),
        sku: sku.to_string(),
        quantity: qty,
        priority: Some(BackorderPriority::Normal),
        expected_date: None,
        promised_date: None,
        source_location_id: None,
        notes: None,
    })
    .await
    .expect("create backorder")
    .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_auto_allocate_survives_a_concurrent_reserve_on_the_same_sku() {
    let db = require_pg!();
    let inv = db.inventory();
    let bo = db.backorder();

    // Several tight rounds so a scheduling fluke cannot mask the race: stock
    // is exactly enough for the backorders, and carts compete for the same
    // units. Before the fix `auto_allocate` read `quantity_available`
    // UNLOCKED, so a cart could take the units between that read and
    // `reserve_in_tx` — and the resulting `InsufficientStock` aborted the
    // WHOLE batch. Auto-allocation must therefore never fail here: a losing
    // candidate skips, only a cart may come away empty-handed.
    for round in 0..12 {
        let sku = sku("PG-AUTO-RACE");
        let item_id = seed(&inv, &sku, dec!(8)).await;
        for _ in 0..8 {
            create_backorder(&bo, &sku, dec!(1)).await;
        }

        let mut carts = Vec::new();
        for i in 0..6 {
            let inv = inv.clone();
            let sku = sku.clone();
            carts.push(tokio::spawn(async move {
                inv.reserve_async(reserve(&sku, dec!(1), &format!("cart-{i}"), None)).await
            }));
        }
        let mut allocators = Vec::new();
        for _ in 0..3 {
            let bo = bo.clone();
            let sku = sku.clone();
            allocators
                .push(tokio::spawn(async move { bo.auto_allocate_inventory_async(&sku).await }));
        }

        for cart in carts {
            match cart.await.expect("join") {
                Ok(_) | Err(CommerceError::InsufficientStock { .. }) => {}
                Err(other) => panic!("round {round}: a racing cart reserve failed: {other:?}"),
            }
        }
        for allocator in allocators {
            allocator.await.expect("join").unwrap_or_else(|e| {
                panic!(
                    "round {round}: auto_allocate must skip a candidate that loses a race, \
                     not abort the batch: {e:?}"
                )
            });
        }

        let (on_hand, allocated, available) = balance(&inv, &sku).await;
        assert_eq!(on_hand, dec!(8));
        assert!(allocated <= on_hand, "round {round}: oversold {allocated} of {on_hand}");
        assert_eq!(available, on_hand - allocated);
        assert_balance_identities(&db, &[item_id]).await;
    }
}

#[tokio::test]
async fn postgres_auto_allocate_skips_a_starved_candidate_without_failing_the_batch() {
    let db = require_pg!();
    let inv = db.inventory();
    let bo = db.backorder();
    let sku = sku("PG-AUTO-SKIP");
    let item_id = seed(&inv, &sku, dec!(6)).await;
    for _ in 0..3 {
        create_backorder(&bo, &sku, dec!(4)).await;
    }

    // 6 units for 3 backorders of 4: 4 + 2, and the third gets nothing.
    let created = bo.auto_allocate_inventory_async(&sku).await.expect("auto allocate");
    assert_eq!(created.len(), 2, "the starved candidate must be skipped, not fatal: {created:?}");
    assert_eq!(created.iter().map(|a| a.quantity).sum::<Decimal>(), dec!(6));
    assert_eq!(balance(&inv, &sku).await, (dec!(6), dec!(6), dec!(0)));
    assert_balance_identities(&db, &[item_id]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_backorder_allocation_writes_are_retried_under_contention() {
    let db = require_pg!();
    let inv = db.inventory();
    let bo = db.backorder();
    let sku = sku("PG-BO-RETRY");
    let item_id = seed(&inv, &sku, dec!(40)).await;
    for _ in 0..6 {
        create_backorder(&bo, &sku, dec!(3)).await;
    }
    let allocations = bo.auto_allocate_inventory_async(&sku).await.expect("allocate");
    assert_eq!(allocations.len(), 6);

    // Release / confirm / expire every allocation concurrently while carts and
    // another auto-allocation contend for the same balance row. All of these
    // used to open bare transactions, so a 40P01 deadlock across the multi-row
    // FOR UPDATE loop reached the caller instead of being retried.
    let mut tasks = Vec::new();
    for (i, allocation) in allocations.iter().enumerate() {
        let bo = bo.clone();
        let id = allocation.id;
        tasks.push(tokio::spawn(async move {
            if i % 2 == 0 {
                bo.release_allocation_async(id).await.map(|_| ())
            } else {
                bo.confirm_allocation_async(id).await.map(|_| ())
            }
        }));
    }
    for _ in 0..2 {
        let bo = bo.clone();
        tasks.push(tokio::spawn(async move { bo.expire_allocations_async().await.map(|_| ()) }));
    }
    for i in 0..4 {
        let inv = inv.clone();
        let sku = sku.clone();
        tasks.push(tokio::spawn(async move {
            inv.reserve_async(reserve(&sku, dec!(1), &format!("cart-{i}"), None)).await.map(|_| ())
        }));
    }
    let bo_auto = bo.clone();
    let sku_auto = sku.clone();
    tasks.push(tokio::spawn(async move {
        bo_auto.auto_allocate_inventory_async(&sku_auto).await.map(|_| ())
    }));

    for task in tasks {
        match task.await.expect("join") {
            Ok(()) => {}
            Err(CommerceError::InsufficientStock { .. }) => {}
            Err(other) => {
                let message = other.to_string();
                assert!(
                    !message.contains("40P01") && !message.contains("deadlock"),
                    "backorder writes must retry deadlocks, got {other:?}"
                );
                panic!("unexpected error from a contended backorder write: {other:?}");
            }
        }
    }
    assert_balance_identities(&db, &[item_id]).await;
}
