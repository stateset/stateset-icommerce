#![cfg(feature = "postgres")]
//! The WMS stock ledger against a live Postgres — the twin of
//! `sqlite_wms_put_away_pick_ship_ledger.rs`.
//!
//! Warehouse documents used to be pure paperwork: completing a put-away, a pick
//! or a ship moved no stock anywhere, while return dispositions *did*, so the
//! warehouse was half-ledgered. These tests pin the ledger and its idempotency.
//!
//! Requires `POSTGRES_URL` / `DATABASE_URL`; skipped otherwise.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CompletePutAway, CreateLocation, CreatePutAway, CreateReceipt, CreateReceiptItem,
    CreateWarehouse, LocationType, ReceiveItemLine, ReceiveItems, WarehouseType,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())?;
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

async fn warehouse_with_location(db: &PostgresDatabase) -> (i32, i32) {
    let tag = Uuid::new_v4().simple().to_string();
    let wh = db
        .warehouse()
        .create_warehouse_async(CreateWarehouse {
            code: format!("WMSL-{}", &tag[..10]),
            name: "WMS ledger test".into(),
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

async fn warehouse_stock(db: &PostgresDatabase, sku: &str, wh: i32) -> (Decimal, Decimal) {
    let row: Option<(Decimal, Decimal)> = sqlx::query_as(
        "SELECT b.quantity_on_hand, b.quantity_allocated FROM inventory_balances b
         JOIN inventory_items i ON i.id = b.item_id
         WHERE i.sku = $1 AND b.location_id = $2",
    )
    .bind(sku)
    .bind(wh)
    .fetch_optional(db.pool())
    .await
    .expect("warehouse stock");
    row.unwrap_or((Decimal::ZERO, Decimal::ZERO))
}

async fn bin_stock(db: &PostgresDatabase, location_id: i32, sku: &str) -> Decimal {
    let row: Option<(Decimal,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(quantity_on_hand), 0) FROM location_inventory
         WHERE location_id = $1 AND sku = $2",
    )
    .bind(location_id)
    .bind(sku)
    .fetch_optional(db.pool())
    .await
    .expect("bin stock");
    row.map_or(Decimal::ZERO, |r| r.0)
}

/// Receive `qty` units of `sku` into `loc` and put them away.
async fn plan_put_away(db: &PostgresDatabase, wh: i32, loc: i32, sku: &str, qty: Decimal) -> Uuid {
    let receipt = db
        .receiving()
        .create_receipt_async(CreateReceipt {
            warehouse_id: wh,
            items: vec![CreateReceiptItem {
                sku: sku.into(),
                expected_quantity: qty,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create receipt");
    let line_id = db.receiving().get_receipt_items_async(receipt.id).await.expect("items")[0].id;
    db.receiving().start_receiving_async(receipt.id).await.expect("start");
    db.receiving()
        .receive_items_async(ReceiveItems {
            receipt_id: receipt.id,
            items: vec![ReceiveItemLine {
                receipt_item_id: line_id,
                quantity_received: qty,
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
            receiving_location_id: Some(loc),
            received_by: Some("dock".into()),
        })
        .await
        .expect("receive");
    db.receiving().complete_receiving_async(receipt.id).await.expect("complete receiving");

    let put_away = db
        .receiving()
        .create_put_away_async(CreatePutAway {
            receipt_id: receipt.id,
            receipt_item_id: line_id,
            sku: sku.into(),
            from_location_id: None,
            to_location_id: loc,
            quantity: qty,
            lot_id: None,
            assigned_to: Some("carl".into()),
            notes: None,
        })
        .await
        .expect("create put-away");
    put_away.id
}

async fn first_stock_race(same_location: bool) {
    let Some(db) = connect().await else { return };
    let db = std::sync::Arc::new(db);
    let (wh, loc) = warehouse_with_location(&db).await;
    let sku = format!("RACE-{}", Uuid::new_v4().simple());
    // Pre-create the master so its unique-key insertion cannot serialize the
    // stock writers and conceal the missing balance/location row locks.
    sqlx::query("INSERT INTO inventory_items (sku, name) VALUES ($1, $1)")
        .bind(&sku)
        .execute(db.pool())
        .await
        .unwrap();
    let first = plan_put_away(&db, wh, loc, &sku, dec!(10)).await;
    let second_location = if same_location {
        loc
    } else {
        db.warehouse()
            .create_location_async(CreateLocation {
                warehouse_id: wh,
                code: Some(format!("SECOND-{}", Uuid::new_v4())),
                location_type: LocationType::Bulk,
                is_receivable: Some(true),
                ..Default::default()
            })
            .await
            .unwrap()
            .id
    };
    let second = plan_put_away(&db, wh, second_location, &sku, dec!(10)).await;
    sqlx::query("INSERT INTO inventory_locations (id, name, code) VALUES ($1, 'Race warehouse', $2) ON CONFLICT DO NOTHING")
        .bind(wh).bind(format!("RACE-{wh}")).execute(db.pool()).await.unwrap();
    let table = if same_location { "location_inventory" } else { "inventory_balances" };

    // SHARE allows the old SELECT FOR UPDATE of an absent row, but stops
    // INSERT. A shared shelf now serializes on its location before INSERT;
    // distinct shelves must still contend on the first warehouse balance.
    let mut gate = db.pool().begin().await.unwrap();
    sqlx::query(&format!("LOCK TABLE {table} IN SHARE MODE")).execute(gate.as_mut()).await.unwrap();
    let handles: Vec<_> = [first, second]
        .into_iter()
        .map(|put_away_id| {
            let db = std::sync::Arc::clone(&db);
            tokio::spawn(async move {
                db.receiving()
                    .complete_put_away_async(CompletePutAway {
                        put_away_id,
                        actual_location_id: None,
                        completed_by: None,
                        notes: None,
                    })
                    .await
            })
        })
        .collect();
    let start = std::time::Instant::now();
    loop {
        let waiting: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_locks WHERE relation = to_regclass($1) AND mode = 'RowExclusiveLock' AND NOT granted")
            .bind(table).fetch_one(db.pool()).await.unwrap();
        if waiting >= if same_location { 1 } else { 2 } {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(15),
            "put-aways must reach the stock insertion gate"
        );
        tokio::task::yield_now().await;
    }
    gate.commit().await.unwrap();
    for handle in handles {
        handle.await.unwrap().expect("complete put-away");
    }
    assert_eq!(bin_stock(&db, loc, &sku).await, if same_location { dec!(20) } else { dec!(10) });
    if !same_location {
        assert_eq!(bin_stock(&db, second_location, &sku).await, dec!(10));
    }
    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(20), dec!(0)));
    let moved: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements WHERE sku = $1 AND reference_type = 'put_away'")
        .bind(&sku).fetch_one(db.pool()).await.unwrap();
    assert_eq!(moved, dec!(20));
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_first_put_aways_preserve_both_receipts() {
    first_stock_race(true).await;
    first_stock_race(false).await;
}
