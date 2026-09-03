//! Postgres regression tests for the WMS guards:
//!
//! - W1 `create_put_away` caps planned quantity at the line's received
//!   quantity, checks the line belongs to the receipt, rejects non-positive
//!   quantities, and serializes concurrent put-aways with `FOR UPDATE`;
//! - W2 `waves.pick_count` is maintained on pick insert/cancel and
//!   `complete_wave` refuses while picks are still open;
//! - W3 a line with `expected_quantity = 0` is a blind receipt;
//! - W4 `delete_location` refuses reserved stock and movement history with a
//!   `ValidationError`, and reports a missing id as `NotFound`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AdjustLocationInventory, CommerceError, CompletePick, CompletePutAway, CreateLocation,
    CreatePickTask, CreatePutAway, CreateReceipt, CreateReceiptItem, CreateWarehouse, CreateWave,
    FulfillmentId, LocationType, OrderId, OrderItemId, ReceiptItemStatus, ReceiveItemLine,
    ReceiveItems, UpdateLocation, WarehouseType, WaveStatus,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

/// A fresh warehouse with one receivable/pickable location: `(wh_id, loc_id)`.
async fn seed_warehouse(db: &PostgresDatabase) -> (i32, i32) {
    let tag = Uuid::new_v4().simple().to_string();
    let wh = db
        .warehouse()
        .create_warehouse_async(CreateWarehouse {
            code: format!("WMS-{}", &tag[..12]),
            name: "WMS guard test".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .await
        .expect("create warehouse");
    let loc = db
        .warehouse()
        .create_location_async(CreateLocation {
            warehouse_id: wh.id,
            code: Some(format!("L-{}", &tag[..8])),
            location_type: LocationType::Bulk,
            is_pickable: Some(true),
            is_receivable: Some(true),
            ..Default::default()
        })
        .await
        .expect("create location");
    (wh.id, loc.id)
}

async fn receipt_with_one_item(
    db: &PostgresDatabase,
    warehouse_id: i32,
    expected: rust_decimal::Decimal,
) -> (Uuid, Uuid) {
    let receipt = db
        .receiving()
        .create_receipt_async(CreateReceipt {
            receipt_type: stateset_core::ReceiptType::PurchaseOrder,
            warehouse_id,
            items: vec![CreateReceiptItem {
                sku: "SKU-1".into(),
                expected_quantity: expected,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create receipt");
    let items = db.receiving().get_receipt_items_async(receipt.id).await.expect("items");
    (receipt.id, items[0].id)
}

fn receive_line(receipt_id: Uuid, item_id: Uuid, qty: rust_decimal::Decimal) -> ReceiveItems {
    ReceiveItems {
        receipt_id,
        items: vec![ReceiveItemLine {
            receipt_item_id: item_id,
            quantity_received: qty,
            quantity_rejected: None,
            rejection_reason: None,
            lot_number: None,
            serial_numbers: None,
            expiration_date: None,
            notes: None,
        }],
        receiving_location_id: None,
        received_by: None,
    }
}

fn put_away(
    receipt_id: Uuid,
    item_id: Uuid,
    loc: i32,
    qty: rust_decimal::Decimal,
) -> CreatePutAway {
    CreatePutAway {
        receipt_id,
        receipt_item_id: item_id,
        sku: "SKU-1".into(),
        from_location_id: None,
        to_location_id: loc,
        quantity: qty,
        lot_id: None,
        assigned_to: None,
        notes: None,
    }
}

fn pick(
    wave: Option<FulfillmentId>,
    order: OrderId,
    wh: i32,
    loc: i32,
    sku: &str,
) -> CreatePickTask {
    CreatePickTask {
        wave_id: wave,
        order_id: order,
        order_item_id: OrderItemId::new(),
        warehouse_id: wh,
        sku: sku.into(),
        product_name: None,
        source_location_id: loc,
        quantity_requested: dec!(5),
        lot_id: None,
        serial_number: None,
        priority: None,
        notes: None,
    }
}

// ---------------------------------------------------------------- W1

#[tokio::test]
async fn postgres_create_put_away_validates_quantity_and_ownership() {
    let Some(db) = connect().await else { return };
    let (wh, loc) = seed_warehouse(&db).await;
    let (rid_a, iid_a) = receipt_with_one_item(&db, wh, dec!(10)).await;
    let (rid_b, _) = receipt_with_one_item(&db, wh, dec!(10)).await;
    db.receiving()
        .receive_items_async(receive_line(rid_a, iid_a, dec!(10)))
        .await
        .expect("receive");

    for qty in [dec!(0), dec!(-1)] {
        let err = db
            .receiving()
            .create_put_away_async(put_away(rid_a, iid_a, loc, qty))
            .await
            .expect_err("non-positive");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    }
    let err = db
        .receiving()
        .create_put_away_async(put_away(rid_b, iid_a, loc, dec!(1)))
        .await
        .expect_err("line belongs to another receipt");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("does not belong")),
        "got {err:?}"
    );
    let err = db
        .receiving()
        .create_put_away_async(put_away(rid_a, Uuid::new_v4(), loc, dec!(1)))
        .await
        .expect_err("unknown line");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

#[tokio::test]
async fn postgres_create_put_away_caps_cumulative_planned_at_received() {
    let Some(db) = connect().await else { return };
    let (wh, loc) = seed_warehouse(&db).await;
    let (rid, iid) = receipt_with_one_item(&db, wh, dec!(10)).await;
    db.receiving().receive_items_async(receive_line(rid, iid, dec!(10))).await.expect("receive");

    let first =
        db.receiving().create_put_away_async(put_away(rid, iid, loc, dec!(6))).await.expect("6");
    let err = db
        .receiving()
        .create_put_away_async(put_away(rid, iid, loc, dec!(5)))
        .await
        .expect_err("6 + 5 > 10");
    match err {
        CommerceError::ValidationError(m) => {
            assert!(m.contains("received"), "{m}");
            assert!(m.contains("already planned"), "{m}");
            assert!(m.contains("available"), "{m}");
        }
        other => panic!("got {other:?}"),
    }
    let second =
        db.receiving().create_put_away_async(put_away(rid, iid, loc, dec!(4))).await.expect("4");

    // Cancelling frees capacity; completing does not.
    db.receiving().cancel_put_away_async(second.id).await.expect("cancel");
    let second = db
        .receiving()
        .create_put_away_async(put_away(rid, iid, loc, dec!(4)))
        .await
        .expect("re-plan");

    for id in [first.id, second.id] {
        db.receiving()
            .complete_put_away_async(CompletePutAway {
                put_away_id: id,
                actual_location_id: None,
                notes: None,
                completed_by: None,
            })
            .await
            .expect("complete");
    }
    let receipt = db.receiving().get_receipt_async(rid).await.expect("get").expect("exists");
    assert_eq!(receipt.put_away_quantity, dec!(10));
}

/// Two concurrent 10-unit put-aways on a 10-unit receipt line: exactly one
/// succeeds. Before the `FOR UPDATE` + cap both were accepted.
#[tokio::test]
async fn postgres_concurrent_put_aways_cannot_exceed_received() {
    let Some(db) = connect().await else { return };
    let db = Arc::new(db);
    let (wh, loc) = seed_warehouse(&db).await;
    let (rid, iid) = receipt_with_one_item(&db, wh, dec!(10)).await;
    db.receiving().receive_items_async(receive_line(rid, iid, dec!(10))).await.expect("receive");

    let rounds = 5;
    for round in 0..rounds {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let db = Arc::clone(&db);
                tokio::spawn(async move {
                    db.receiving().create_put_away_async(put_away(rid, iid, loc, dec!(10))).await
                })
            })
            .collect();
        let mut ok = 0;
        let mut rejected = 0;
        for h in handles {
            match h.await.expect("task") {
                Ok(_) => ok += 1,
                Err(CommerceError::ValidationError(_)) => rejected += 1,
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert_eq!((ok, rejected), (1, 1), "round {round}: exactly one put-away may win");

        // Reset for the next round: cancel the winner.
        let winners = db
            .receiving()
            .list_put_aways_async(stateset_core::PutAwayFilter {
                receipt_id: Some(rid),
                status: Some(stateset_core::PutAwayStatus::Pending),
                ..Default::default()
            })
            .await
            .expect("list");
        assert_eq!(winners.len(), 1);
        db.receiving().cancel_put_away_async(winners[0].id).await.expect("cancel winner");
    }
}

// ---------------------------------------------------------------- W3

#[tokio::test]
async fn postgres_blind_receipt_line_accepts_any_positive_quantity() {
    let Some(db) = connect().await else { return };
    let (wh, _) = seed_warehouse(&db).await;
    let (rid, iid) = receipt_with_one_item(&db, wh, dec!(0)).await;

    db.receiving().receive_items_async(receive_line(rid, iid, dec!(7))).await.expect("blind 7");
    db.receiving().receive_items_async(receive_line(rid, iid, dec!(3))).await.expect("blind +3");
    let items = db.receiving().get_receipt_items_async(rid).await.expect("items");
    assert_eq!(items[0].received_quantity, dec!(10));
    assert_eq!(items[0].status, ReceiptItemStatus::Received);

    // A non-blind line is still capped.
    let (rid, iid) = receipt_with_one_item(&db, wh, dec!(5)).await;
    let err = db
        .receiving()
        .receive_items_async(receive_line(rid, iid, dec!(6)))
        .await
        .expect_err("over-receipt");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

/// Put `qty` units of `sku` on the shelf at both ledger levels (bin and
/// warehouse balance) so a pick can take them.
async fn seed_stock(
    db: &PostgresDatabase,
    warehouse_id: i32,
    location_id: i32,
    sku: &str,
    qty: rust_decimal::Decimal,
) {
    sqlx::query(
        "INSERT INTO location_inventory (location_id, sku, quantity_on_hand, quantity_reserved)
         VALUES ($1, $2, $3, 0)
         ON CONFLICT (location_id, sku, lot_id) DO UPDATE SET quantity_on_hand = EXCLUDED.quantity_on_hand",
    )
    .bind(location_id)
    .bind(sku)
    .bind(qty)
    .execute(db.pool())
    .await
    .expect("seed bin");
    sqlx::query("INSERT INTO inventory_items (sku, name) VALUES ($1, $1) ON CONFLICT DO NOTHING")
        .bind(sku)
        .execute(db.pool())
        .await
        .expect("seed item");
    sqlx::query(
        "INSERT INTO inventory_locations (id, name, code)
         SELECT id, name, code FROM warehouses WHERE id = $1 ON CONFLICT DO NOTHING",
    )
    .bind(warehouse_id)
    .execute(db.pool())
    .await
    .expect("seed inventory location");
    sqlx::query(
        "INSERT INTO inventory_balances
         (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available)
         SELECT id, $2, $3, 0, $3 FROM inventory_items WHERE sku = $1
         ON CONFLICT (item_id, location_id) DO UPDATE SET
             quantity_on_hand = EXCLUDED.quantity_on_hand,
             quantity_available = EXCLUDED.quantity_available",
    )
    .bind(sku)
    .bind(warehouse_id)
    .bind(qty)
    .execute(db.pool())
    .await
    .expect("seed warehouse balance");
}

// ---------------------------------------------------------------- W2

#[tokio::test]
async fn postgres_wave_pick_count_and_completion_gate() {
    let Some(db) = connect().await else { return };
    let (wh, loc) = seed_warehouse(&db).await;
    let order = OrderId::new();
    let wave = db
        .fulfillment()
        .create_wave_async(CreateWave {
            warehouse_id: wh,
            order_ids: vec![order],
            priority: None,
            notes: None,
            created_by: None,
        })
        .await
        .expect("wave");
    assert_eq!(wave.pick_count, 0);

    let p1 = db
        .fulfillment()
        .create_pick_async(pick(Some(wave.id), order, wh, loc, "SKU-A"))
        .await
        .expect("p1");
    let p2 = db
        .fulfillment()
        .create_pick_async(pick(Some(wave.id), order, wh, loc, "SKU-B"))
        .await
        .expect("p2");
    let w = db.fulfillment().get_wave_async(wave.id.into()).await.expect("get").expect("exists");
    assert_eq!(w.pick_count, 2);

    db.fulfillment().release_wave_async(wave.id.into()).await.expect("release");
    let err =
        db.fulfillment().complete_wave_async(wave.id.into()).await.expect_err("0 of 2 picks done");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("still open")),
        "got {err:?}"
    );
    let w = db.fulfillment().get_wave_async(wave.id.into()).await.expect("get").expect("exists");
    assert_eq!(w.status, WaveStatus::Released);

    // Completing a pick now takes the units out of the bin and allocates them
    // at warehouse level, so the shelf has to hold them first.
    seed_stock(&db, wh, loc, "SKU-A", dec!(5)).await;
    db.fulfillment()
        .complete_pick_async(CompletePick {
            pick_id: p1.id,
            quantity_picked: dec!(5),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .await
        .expect("complete p1");
    assert!(db.fulfillment().complete_wave_async(wave.id.into()).await.is_err(), "1 of 2 done");

    db.fulfillment().cancel_pick_async(p2.id).await.expect("cancel p2");
    let done = db.fulfillment().complete_wave_async(wave.id.into()).await.expect("all finalized");
    assert_eq!(done.status, WaveStatus::Completed);
    assert_eq!(done.pick_count, 1);
    assert_eq!(done.completed_pick_count, 1);

    // No more picks on a completed wave; the insert is rolled back.
    let err = db
        .fulfillment()
        .create_pick_async(pick(Some(wave.id), order, wh, loc, "SKU-LATE"))
        .await
        .expect_err("wave completed");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let picks = db.fulfillment().get_picks_for_wave_async(wave.id.into()).await.expect("picks");
    assert_eq!(picks.len(), 2, "late pick must not have been inserted");

    let err = db
        .fulfillment()
        .create_pick_async(pick(Some(FulfillmentId::new()), order, wh, loc, "SKU-X"))
        .await
        .expect_err("no such wave");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

// ---------------------------------------------------------------- W4

#[tokio::test]
async fn postgres_delete_location_guards() {
    let Some(db) = connect().await else { return };
    let (_wh, loc) = seed_warehouse(&db).await;

    let err = db.warehouse().delete_location_async(i32::MAX).await.expect_err("no such id");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    // Reserved stock (zero on hand) blocks deletion.
    sqlx::query(
        "INSERT INTO location_inventory (location_id, sku, quantity_on_hand, quantity_reserved)
         VALUES ($1, 'RES-SKU', 0, 2)",
    )
    .bind(loc)
    .execute(db.pool())
    .await
    .expect("seed reserved");
    let err = db.warehouse().delete_location_async(loc).await.expect_err("reserved");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("reserved")),
        "got {err:?}"
    );
    sqlx::query("DELETE FROM location_inventory WHERE location_id = $1")
        .bind(loc)
        .execute(db.pool())
        .await
        .expect("clear");

    // Movement history (net zero on hand) blocks deletion with a clear error
    // rather than a foreign-key database error.
    for qty in [dec!(4), dec!(-4)] {
        db.warehouse()
            .adjust_inventory_async(AdjustLocationInventory {
                location_id: loc,
                sku: "MOV-SKU".into(),
                lot_id: None,
                quantity: qty,
                reason: "test".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .await
            .expect("adjust");
    }
    let err = db.warehouse().delete_location_async(loc).await.expect_err("history");
    assert!(
        matches!(err, CommerceError::ValidationError(ref m) if m.contains("movement history")),
        "got {err:?}"
    );
    assert!(db.warehouse().get_location_async(loc).await.expect("get").is_some());
    let updated = db
        .warehouse()
        .update_location_async(loc, UpdateLocation { is_active: Some(false), ..Default::default() })
        .await
        .expect("deactivate");
    assert!(!updated.is_active);

    // An untouched location deletes cleanly.
    let (_, fresh) = seed_warehouse(&db).await;
    db.warehouse().delete_location_async(fresh).await.expect("delete");
    assert!(db.warehouse().get_location_async(fresh).await.expect("get").is_none());
}

// ---------------------------------------------------------------- W5

/// `delete_location` used to read zero stock and then DELETE with no row lock.
/// `FOR UPDATE` cannot lock a `location_inventory` row that does not exist yet,
/// so a concurrent first adjustment for a new SKU inserted its row after the
/// read and had it cascaded away by the DELETE — both calls reporting success
/// and the stock silently vanishing.
///
/// The fix pins the `locations` row: the delete takes it `FOR UPDATE`, every
/// stock writer takes it `FOR SHARE`. These two tests hold each lock from the
/// test's own transaction and assert the other side waits.
///
/// The load-bearing half is
/// `postgres_stock_writes_block_while_a_delete_holds_the_location`: before the
/// fix the adjustment ignored the delete entirely, inserted its row and then
/// failed with a raw foreign-key database error (or, once the delete had
/// committed, had the row cascaded away). The other direction always blocked at
/// the `DELETE` statement itself; it is pinned here so the ordering cannot
/// regress.
#[tokio::test]
async fn postgres_delete_location_blocks_while_a_stock_writer_holds_the_location() {
    let Some(db) = connect().await else { return };
    let (_wh, loc) = seed_warehouse(&db).await;

    // Stand in for a stock writer mid-transaction.
    let mut writer = db.pool().begin().await.expect("begin");
    sqlx::query("SELECT id FROM locations WHERE id = $1 FOR SHARE")
        .bind(loc)
        .fetch_one(writer.as_mut())
        .await
        .expect("hold the location");

    let blocked = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        db.warehouse().delete_location_async(loc),
    )
    .await;
    assert!(
        blocked.is_err(),
        "delete_location must wait for the stock writer to finish; it returned {blocked:?}"
    );

    writer.rollback().await.expect("rollback");
    // With the writer gone the delete goes through.
    db.warehouse().delete_location_async(loc).await.expect("delete once nothing holds it");
    assert!(db.warehouse().get_location_async(loc).await.expect("get").is_none());
}

#[tokio::test]
async fn postgres_stock_writes_block_while_a_delete_holds_the_location() {
    let Some(db) = connect().await else { return };
    let (_wh, loc) = seed_warehouse(&db).await;

    // Stand in for `delete_location_async` mid-transaction.
    let mut deleter = db.pool().begin().await.expect("begin");
    sqlx::query("SELECT id FROM locations WHERE id = $1 FOR UPDATE")
        .bind(loc)
        .fetch_one(deleter.as_mut())
        .await
        .expect("hold the location");

    let adjust = AdjustLocationInventory {
        location_id: loc,
        sku: format!("RACE-{}", Uuid::new_v4().simple()),
        lot_id: None,
        quantity: dec!(7),
        reason: "race".into(),
        reference_type: None,
        reference_id: None,
        performed_by: None,
    };
    let blocked = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        db.warehouse().adjust_inventory_async(adjust.clone()),
    )
    .await;
    assert!(
        blocked.is_err(),
        "an adjustment must wait for a delete holding the location; it returned {blocked:?}"
    );

    // Let the "delete" win, and the adjustment must then find no location
    // rather than writing a row the cascade would swallow.
    sqlx::query("DELETE FROM locations WHERE id = $1")
        .bind(loc)
        .execute(deleter.as_mut())
        .await
        .expect("delete");
    deleter.commit().await.expect("commit");

    let err =
        db.warehouse().adjust_inventory_async(adjust).await.expect_err("the location is gone");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM location_inventory WHERE location_id = $1")
            .bind(loc)
            .fetch_one(db.pool())
            .await
            .expect("count");
    assert_eq!(rows, 0, "no stock row may survive a deleted location");
}
