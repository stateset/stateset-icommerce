//! Postgres `move_inventory` must not over-transfer under concurrency.
//!
//! The source-inventory `SELECT` had no `FOR UPDATE`, so concurrent moves of the same
//! (location, sku, lot) each read the same stale `quantity_on_hand`, all passed the
//! `quantity > available` guard, and all applied the relative
//! `quantity_on_hand = quantity_on_hand - $1` decrement — driving the source
//! negative (over-transfer). SQLite serializes writers via an IMMEDIATE transaction
//! and is safe. With `FOR UPDATE`, exactly one concurrent move may succeed.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AdjustLocationInventory, CreateLocation, CreateWarehouse, LocationType, MoveInventory,
    WarehouseAddress, WarehouseType,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_move_inventory_never_over_transfers_under_concurrency() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping move_inventory concurrency test");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let wh = db.warehouse();

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let warehouse = wh
        .create_warehouse_async(CreateWarehouse {
            code: format!("WH-{}", &unique[..8]),
            name: "Test WH".into(),
            warehouse_type: WarehouseType::default(),
            address: WarehouseAddress {
                street1: "1 Main St".into(),
                street2: None,
                city: "Town".into(),
                state: "CA".into(),
                postal_code: "00000".into(),
                country: "US".into(),
                phone: None,
            },
            timezone: None,
        })
        .await
        .expect("create warehouse");

    let src = wh
        .create_location_async(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(format!("SRC-{}", &unique[..8])),
            location_type: LocationType::default(),
            ..Default::default()
        })
        .await
        .expect("create source location");
    let dst = wh
        .create_location_async(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(format!("DST-{}", &unique[..8])),
            location_type: LocationType::default(),
            ..Default::default()
        })
        .await
        .expect("create dest location");

    let sku = format!("SKU-{}", &unique[..8]);
    wh.adjust_inventory_async(AdjustLocationInventory {
        location_id: src.id,
        sku: sku.clone(),
        lot_id: None,
        quantity: dec!(10),
        reason: "seed".into(),
        reference_type: None,
        reference_id: None,
        performed_by: None,
    })
    .await
    .expect("seed source inventory");

    // Fire 8 concurrent moves of 8 each; only one can succeed against 10 available.
    let mut handles = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let sku = sku.clone();
        let (from, to) = (src.id, dst.id);
        handles.push(tokio::spawn(async move {
            db.warehouse()
                .move_inventory_async(MoveInventory {
                    from_location_id: from,
                    to_location_id: to,
                    sku,
                    lot_id: None,
                    quantity: dec!(8),
                    reason: None,
                    performed_by: None,
                })
                .await
        }));
    }

    let mut successes = 0;
    for h in handles {
        if h.await.expect("task join").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "exactly one move of 8 from 10 available may succeed");

    // The one successful move leaves 2 on hand at the source (never negative).
    let src_inv = wh.get_location_inventory_async(src.id).await.expect("source inventory");
    let on_hand = src_inv.iter().find(|i| i.sku == sku).map(|i| i.quantity_on_hand);
    assert_eq!(on_hand, Some(dec!(2)), "source on_hand must be 10 - 8 = 2 (never negative)");
}
