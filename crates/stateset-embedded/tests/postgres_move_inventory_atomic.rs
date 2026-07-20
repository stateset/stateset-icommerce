//! Postgres side of the warehouse `move_inventory` atomicity parity guard.
//!
//! A move is a source-decrement + destination-increment + movement-insert. If the
//! destination write fails, the whole move must roll back so the source keeps its
//! stock. Postgres already wraps the move in a transaction; SQLite now does too
//! (see `sqlite/warehouse.rs::move_inventory_is_atomic_when_destination_write_fails`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AdjustLocationInventory, CreateLocation, CreateWarehouse, LocationType, MoveInventory,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_move_inventory_rolls_back_on_destination_failure() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping move_inventory atomicity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let wh = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &unique[..8]),
            name: "Atomic".into(),
            ..Default::default()
        })
        .await
        .expect("create warehouse");
    let src = commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some(format!("SRC-{}", &unique[..8])),
            location_type: LocationType::Bulk,
            ..Default::default()
        })
        .await
        .expect("create location");

    let sku = format!("SKU-{}", &unique[..8]);
    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
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
        .expect("seed inventory");

    // Move to a destination location id that does not exist → the destination
    // write fails and the whole move must roll back.
    let result = commerce
        .warehouse()
        .move_inventory(MoveInventory {
            from_location_id: src.id,
            to_location_id: 999_999,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(3),
            reason: None,
            performed_by: None,
        })
        .await;
    assert!(result.is_err(), "move to a non-existent destination must fail");

    let src_inv = commerce.warehouse().get_location_inventory(src.id).await.expect("inventory");
    let row = src_inv.iter().find(|i| i.sku == sku).expect("source inventory still exists");
    assert_eq!(row.quantity_on_hand, dec!(10), "source must not lose stock on a failed move");
}
