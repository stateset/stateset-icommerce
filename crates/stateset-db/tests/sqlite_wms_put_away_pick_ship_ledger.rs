#![cfg(feature = "sqlite")]
//! The WMS stock ledger against the live SQLite engine.
//!
//! Warehouse documents used to be pure paperwork: completing a put-away, a pick
//! or a ship moved no stock anywhere, while return dispositions *did*, so the
//! warehouse was half-ledgered — goods could be received and shipped without a
//! single unit ever appearing in or leaving inventory.
//!
//! These tests pin the ledger:
//!
//! - **W-S1**: receipt → put-away → pick → ship conserves quantity. A put-away
//!   is the only producer of received stock (bin + warehouse balance), a pick
//!   takes it off the shelf and allocates it, and a ship consumes exactly what
//!   the picks allocated.
//! - **W-S2**: every effect is idempotent — a second completion of the same
//!   document never moves stock twice.
//! - **W-S3**: a pick cannot take units the bin does not hold.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCarton, AddCartonItem, CommerceError, CompletePick, CompletePutAway, CompleteShip,
    CreateLocation, CreatePackTask, CreatePickTask, CreatePutAway, CreateReceipt,
    CreateReceiptItem, CreateShipTask, CreateWarehouse, FulfillmentRepository, InventoryRepository,
    LocationType, OrderId, OrderItemId, ReceiveItemLine, ReceiveItems, ReceivingRepository,
    ShipmentId, WarehouseRepository, WarehouseType,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("in-memory sqlite")
}

/// A fresh warehouse with one receivable + pickable location.
fn warehouse_with_location(db: &SqliteDatabase) -> (i32, i32) {
    let tag = Uuid::new_v4().simple().to_string();
    let wh = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WMS-{}", &tag[..12]),
            name: "WMS ledger test".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .expect("create warehouse");
    let loc = db
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some(format!("L-{}", &tag[..8])),
            location_type: LocationType::Bulk,
            is_pickable: Some(true),
            is_receivable: Some(true),
            ..Default::default()
        })
        .expect("create location");
    (wh.id, loc.id)
}

/// `(on_hand, allocated)` at the warehouse balance level for `sku`.
fn warehouse_stock(db: &SqliteDatabase, sku: &str, wh: i32) -> (Decimal, Decimal) {
    db.inventory().get_stock(sku).expect("stock").map_or((dec!(0), dec!(0)), |stock| {
        stock
            .locations
            .iter()
            .find(|l| l.location_id == wh)
            .map_or((dec!(0), dec!(0)), |l| (l.on_hand, l.allocated))
    })
}

/// On-hand for `sku` in one bin.
fn bin_stock(db: &SqliteDatabase, location_id: i32, sku: &str) -> Decimal {
    db.warehouse()
        .get_location_inventory(location_id)
        .expect("location inventory")
        .into_iter()
        .filter(|entry| entry.sku == sku)
        .map(|entry| entry.quantity_on_hand)
        .sum()
}

/// Receive `qty` units of `sku` and put them away into `loc`; returns the
/// put-away id.
fn receive_and_put_away(
    db: &SqliteDatabase,
    wh: i32,
    loc: i32,
    sku: &str,
    qty: Decimal,
) -> (Uuid, Uuid) {
    let receipt = db
        .receiving()
        .create_receipt(CreateReceipt {
            warehouse_id: wh,
            items: vec![CreateReceiptItem {
                sku: sku.into(),
                expected_quantity: qty,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create receipt");
    db.receiving().start_receiving(receipt.id).expect("start receiving");
    let line_id = db.receiving().get_receipt_items(receipt.id).expect("receipt items")[0].id;
    db.receiving()
        .receive_items(ReceiveItems {
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
        .expect("receive items");
    db.receiving().complete_receiving(receipt.id).expect("complete receiving");

    let put_away = db
        .receiving()
        .create_put_away(CreatePutAway {
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
        .expect("create put-away");
    db.receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: put_away.id,
            actual_location_id: None,
            completed_by: Some("carl".into()),
            notes: None,
        })
        .expect("complete put-away");
    (receipt.id, put_away.id)
}

/// Receipt → put-away → pick → ship moves every unit exactly once: what was
/// received either sits on the shelf or has left the building.
#[test]
fn receipt_put_away_pick_ship_conserves_quantity() {
    let db = db();
    let (wh, loc) = warehouse_with_location(&db);
    let sku = "SKU-WMS-LEDGER";

    // Nothing exists before the receipt.
    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(0), dec!(0)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(0));

    // Receiving alone still moves nothing: the goods are on the dock.
    let (_, put_away_id) = receive_and_put_away(&db, wh, loc, sku, dec!(10));

    // The put-away is what puts them into stock, at both ledger levels.
    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(10), dec!(0)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(10));

    // Pick 4: off the shelf, allocated to the order, still in the building.
    let order = OrderId::new();
    let pick = db
        .fulfillment()
        .create_pick(CreatePickTask {
            wave_id: None,
            order_id: order,
            order_item_id: OrderItemId::new(),
            warehouse_id: wh,
            sku: sku.into(),
            product_name: None,
            source_location_id: loc,
            quantity_requested: dec!(4),
            lot_id: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .expect("create pick");
    db.fulfillment()
        .complete_pick(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(4),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: Some("pia".into()),
        })
        .expect("complete pick");
    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(10), dec!(4)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(6));

    // Pack the picked units into a carton.
    let pack = db
        .fulfillment()
        .create_pack(CreatePackTask { order_id: order, notes: None })
        .expect("pack");
    let carton = db
        .fulfillment()
        .add_carton(AddCarton { pack_task_id: pack.id, ..Default::default() })
        .expect("carton");
    db.fulfillment()
        .add_carton_item(AddCartonItem {
            carton_id: carton.id,
            sku: sku.into(),
            quantity: dec!(4),
            lot_id: None,
            serial_number: None,
        })
        .expect("carton item");

    // Ship: the carton leaves, releasing exactly what the pick allocated.
    let ship = db
        .fulfillment()
        .create_ship(CreateShipTask {
            order_id: order,
            shipment_id: ShipmentId::new(),
            pack_task_id: pack.id,
            carrier: Some("UPS".into()),
            service_level: None,
            notes: None,
        })
        .expect("create ship");
    db.fulfillment()
        .complete_ship(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-TEST".into(),
            shipping_cost: None,
            shipped_by: Some("sam".into()),
        })
        .expect("complete ship");

    // Conservation: 10 received = 6 still in stock + 4 shipped out, and no
    // allocation is left dangling.
    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(6), dec!(0)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(6));

    // --- W-S2: every effect is idempotent ---------------------------------
    let before = (warehouse_stock(&db, sku, wh), bin_stock(&db, loc, sku));

    let err = db
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect_err("a completed put-away cannot be completed again");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    // Re-completing a finalized pick is an explicit no-op.
    db.fulfillment()
        .complete_pick(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(4),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .expect("re-completing a finalized pick is a no-op");

    let err = db
        .fulfillment()
        .complete_ship(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-TEST".into(),
            shipping_cost: None,
            shipped_by: None,
        })
        .expect_err("a shipped task cannot be shipped again");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    assert_eq!(
        (warehouse_stock(&db, sku, wh), bin_stock(&db, loc, sku)),
        before,
        "no document may move stock twice"
    );
}

/// A pick cannot take units the bin does not hold — the whole point of giving
/// the documents a stock effect.
#[test]
fn completing_a_pick_without_stock_is_refused_and_moves_nothing() {
    let db = db();
    let (wh, loc) = warehouse_with_location(&db);
    let sku = "SKU-WMS-EMPTY-BIN";
    let pick = db
        .fulfillment()
        .create_pick(CreatePickTask {
            wave_id: None,
            order_id: OrderId::new(),
            order_item_id: OrderItemId::new(),
            warehouse_id: wh,
            sku: sku.into(),
            product_name: None,
            source_location_id: loc,
            quantity_requested: dec!(3),
            lot_id: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .expect("create pick");

    let err = db
        .fulfillment()
        .complete_pick(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(3),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .expect_err("the bin is empty");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");

    // The whole transaction rolled back: the pick is still open and no stock
    // record was created.
    assert_ne!(
        db.fulfillment().get_pick(pick.id).expect("get").expect("exists").status,
        stateset_core::PickStatus::Completed
    );
    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(0), dec!(0)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(0));
}

/// A short pick moves only what was actually picked.
#[test]
fn a_short_pick_moves_only_the_picked_units() {
    let db = db();
    let (wh, loc) = warehouse_with_location(&db);
    let sku = "SKU-WMS-SHORT";
    receive_and_put_away(&db, wh, loc, sku, dec!(5));

    let pick = db
        .fulfillment()
        .create_pick(CreatePickTask {
            wave_id: None,
            order_id: OrderId::new(),
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
        })
        .expect("create pick");
    db.fulfillment()
        .complete_pick(CompletePick {
            pick_id: pick.id,
            quantity_picked: dec!(2),
            quantity_short: Some(dec!(3)),
            short_reason: Some("not on the shelf".into()),
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .expect("complete short");

    assert_eq!(warehouse_stock(&db, sku, wh), (dec!(5), dec!(2)));
    assert_eq!(bin_stock(&db, loc, sku), dec!(3));
}
