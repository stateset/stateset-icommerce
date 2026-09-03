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
    AddCarton, AddCartonItem, CommerceError, CompletePick, CompletePutAway, CompleteShip,
    CreateLocation, CreatePackTask, CreatePickTask, CreatePutAway, CreateReceipt,
    CreateReceiptItem, CreateShipTask, CreateWarehouse, LocationType, OrderId, OrderItemId,
    PickStatus, ReceiveItemLine, ReceiveItems, ShipmentId, WarehouseType,
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
async fn receive_and_put_away(
    db: &PostgresDatabase,
    wh: i32,
    loc: i32,
    sku: &str,
    qty: Decimal,
) -> Uuid {
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
    db.receiving()
        .complete_put_away_async(CompletePutAway {
            put_away_id: put_away.id,
            actual_location_id: None,
            completed_by: Some("carl".into()),
            notes: None,
        })
        .await
        .expect("complete put-away");
    put_away.id
}

#[tokio::test]
async fn postgres_receipt_put_away_pick_ship_conserves_quantity() {
    let Some(db) = connect().await else { return };
    let (wh, loc) = warehouse_with_location(&db).await;
    let sku = format!("SKU-WMS-{}", Uuid::new_v4().simple());

    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(0), dec!(0)));
    let put_away_id = receive_and_put_away(&db, wh, loc, &sku, dec!(10)).await;
    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(10), dec!(0)));
    assert_eq!(bin_stock(&db, loc, &sku).await, dec!(10));

    let order = OrderId::new();
    let pick = db
        .fulfillment()
        .create_pick_async(CreatePickTask {
            wave_id: None,
            order_id: order,
            order_item_id: OrderItemId::new(),
            warehouse_id: wh,
            sku: sku.clone(),
            product_name: None,
            source_location_id: loc,
            quantity_requested: dec!(4),
            lot_id: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .await
        .expect("create pick");
    db.fulfillment()
        .complete_pick_async(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(4),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: Some("pia".into()),
        })
        .await
        .expect("complete pick");
    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(10), dec!(4)));
    assert_eq!(bin_stock(&db, loc, &sku).await, dec!(6));

    let pack = db
        .fulfillment()
        .create_pack_async(CreatePackTask { order_id: order, notes: None })
        .await
        .expect("pack");
    let carton = db
        .fulfillment()
        .add_carton_async(AddCarton { pack_task_id: pack.id, ..Default::default() })
        .await
        .expect("carton");
    db.fulfillment()
        .add_carton_item_async(AddCartonItem {
            carton_id: carton.id,
            sku: sku.clone(),
            quantity: dec!(4),
            lot_id: None,
            serial_number: None,
        })
        .await
        .expect("carton item");

    let ship = db
        .fulfillment()
        .create_ship_async(CreateShipTask {
            order_id: order,
            shipment_id: ShipmentId::new(),
            pack_task_id: pack.id,
            carrier: Some("UPS".into()),
            service_level: None,
            notes: None,
        })
        .await
        .expect("create ship");
    db.fulfillment()
        .complete_ship_async(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-TEST".into(),
            shipping_cost: None,
            shipped_by: Some("sam".into()),
        })
        .await
        .expect("complete ship");

    // 10 received = 6 still in stock + 4 shipped, no allocation left dangling.
    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(6), dec!(0)));
    assert_eq!(bin_stock(&db, loc, &sku).await, dec!(6));

    // Idempotency: no document may move stock twice.
    let before = (warehouse_stock(&db, &sku, wh).await, bin_stock(&db, loc, &sku).await);
    let err = db
        .receiving()
        .complete_put_away_async(CompletePutAway {
            put_away_id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .await
        .expect_err("already completed");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    db.fulfillment()
        .complete_pick_async(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(4),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .await
        .expect("re-completing a finalized pick is a no-op");
    let err = db
        .fulfillment()
        .complete_ship_async(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-TEST".into(),
            shipping_cost: None,
            shipped_by: None,
        })
        .await
        .expect_err("already shipped");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(
        (warehouse_stock(&db, &sku, wh).await, bin_stock(&db, loc, &sku).await),
        before,
        "no document may move stock twice"
    );
}

#[tokio::test]
async fn postgres_completing_a_pick_without_stock_is_refused_and_moves_nothing() {
    let Some(db) = connect().await else { return };
    let (wh, loc) = warehouse_with_location(&db).await;
    let sku = format!("SKU-EMPTY-{}", Uuid::new_v4().simple());
    let pick = db
        .fulfillment()
        .create_pick_async(CreatePickTask {
            wave_id: None,
            order_id: OrderId::new(),
            order_item_id: OrderItemId::new(),
            warehouse_id: wh,
            sku: sku.clone(),
            product_name: None,
            source_location_id: loc,
            quantity_requested: dec!(3),
            lot_id: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .await
        .expect("create pick");

    let err = db
        .fulfillment()
        .complete_pick_async(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(3),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .await
        .expect_err("the bin is empty");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");

    assert_ne!(
        db.fulfillment().get_pick_async(pick.id).await.expect("get").expect("exists").status,
        PickStatus::Completed
    );
    assert_eq!(warehouse_stock(&db, &sku, wh).await, (dec!(0), dec!(0)));
    assert_eq!(bin_stock(&db, loc, &sku).await, dec!(0));
}
