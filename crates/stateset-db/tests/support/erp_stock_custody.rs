use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn lifecycle(db: &dyn Database) {
    let sku = format!("CUSTODY-{}", Uuid::new_v4());
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let source = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let lot = db
        .lots()
        .create(CreateLot {
            sku: sku.clone(),
            lot_number: Some(sku.clone()),
            quantity: dec!(10),
            ..Default::default()
        })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: source.id,
            sku: sku.clone(),
            lot_id: Some(lot.id),
            quantity: dec!(10),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let order = OrderId::new();
    let reservation = db
        .inventory()
        .reserve(ReserveInventory {
            sku: sku.clone(),
            location_id: Some(warehouse.id),
            quantity: dec!(4),
            reference_type: "order".into(),
            reference_id: order.to_string(),
            expires_in_seconds: None,
        })
        .unwrap();
    let pick = db
        .fulfillment()
        .create_pick(CreatePickTask {
            order_id: order,
            order_item_id: OrderItemId::new(),
            warehouse_id: warehouse.id,
            source_location_id: source.id,
            sku: sku.clone(),
            quantity_requested: dec!(4),
            lot_id: Some(lot.id),
            wave_id: None,
            product_name: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .unwrap();
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
        .unwrap();
    let staged = db
        .warehouse()
        .list_locations(LocationFilter {
            warehouse_id: Some(warehouse.id),
            location_type: Some(LocationType::Staging),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(staged.len(), 1);
    let stage = staged[0].id;
    assert!(!staged[0].is_pickable);
    let assert_stock = |on_hand: Decimal, reserved: Decimal, staged: Decimal| {
        let stock = db.inventory().get_stock(&sku).unwrap().unwrap();
        assert_eq!((stock.total_on_hand, stock.total_allocated), (on_hand, reserved));
        let physical = db.warehouse().get_inventory_for_sku(warehouse.id, &sku).unwrap();
        assert_eq!(physical.iter().map(|row| row.quantity_on_hand).sum::<Decimal>(), on_hand);
        assert_eq!(physical.iter().map(|row| row.quantity_reserved).sum::<Decimal>(), reserved);
        assert_eq!(
            physical
                .iter()
                .filter(|row| row.location_id == stage)
                .map(|row| row.quantity_on_hand)
                .sum::<Decimal>(),
            staged
        );
        let bins = db.bins().get_bin_levels_for_sku(warehouse.id, &sku).unwrap();
        assert_eq!(bins.iter().map(|row| row.quantity_on_hand).sum::<Decimal>(), on_hand);
        assert_eq!(bins.iter().map(|row| row.quantity_allocated).sum::<Decimal>(), reserved);
        let current = db.lots().get(lot.id).unwrap().unwrap();
        assert_eq!((current.quantity_remaining, current.quantity_reserved), (on_hand, reserved));
    };
    assert_stock(dec!(10), dec!(4), dec!(4));
    assert_eq!(
        db.warehouse().get_location_inventory(source.id).unwrap()[0].quantity_on_hand,
        dec!(6)
    );
    let ship = |quantity| {
        let pack =
            db.fulfillment().create_pack(CreatePackTask { order_id: order, notes: None }).unwrap();
        let carton = db
            .fulfillment()
            .add_carton(AddCarton { pack_task_id: pack.id, ..Default::default() })
            .unwrap();
        db.fulfillment()
            .add_carton_item(AddCartonItem {
                carton_id: carton.id,
                sku: sku.clone(),
                quantity,
                lot_id: Some(lot.id),
                serial_number: None,
            })
            .unwrap();
        let task = db
            .fulfillment()
            .create_ship(CreateShipTask {
                order_id: order,
                shipment_id: ShipmentId::new(),
                pack_task_id: pack.id,
                carrier: None,
                service_level: None,
                notes: None,
            })
            .unwrap();
        (
            task.id,
            db.fulfillment().complete_ship(CompleteShip {
                ship_task_id: task.id,
                tracking_number: "custody".into(),
                shipping_cost: None,
                shipped_by: None,
            }),
        )
    };
    let (first, result) = ship(dec!(1.5));
    result.unwrap();
    assert_stock(dec!(8.5), dec!(2.5), dec!(2.5));
    assert!(
        db.fulfillment()
            .complete_ship(CompleteShip {
                ship_task_id: first,
                tracking_number: "retry".into(),
                shipping_cost: None,
                shipped_by: None
            })
            .is_err()
    );
    assert!(ship(dec!(3)).1.is_err(), "over-shipment rolls back every ledger");
    assert_stock(dec!(8.5), dec!(2.5), dec!(2.5));
    ship(dec!(2.5)).1.unwrap();
    assert_stock(dec!(6), dec!(0), dec!(0));
    db.inventory().release_reservation(reservation.id).unwrap();
    assert_stock(dec!(6), dec!(0), dec!(0));
    assert!(ship(dec!(1)).1.is_err());
}

/// A serial reserved for one order cannot be picked or shipped for another:
/// custody used to rewrite `serial_numbers.status` without looking at the
/// serial's reservation, handing a unit promised to order A to order B and
/// silently consuming A's reservation at shipment. Unreserved serials and
/// serials reserved for the picking order keep working.
pub(crate) fn serial_reservation_owner(db: &dyn Database) {
    let sku = format!("SERIAL-CUSTODY-{}", Uuid::new_v4());
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let source = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: source.id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(2),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let serial = |suffix: &str| {
        db.serials()
            .create(CreateSerialNumber {
                serial: Some(format!("{sku}-{suffix}")),
                sku: sku.clone(),
                location_id: Some(source.id),
                ..Default::default()
            })
            .unwrap()
    };
    let promised = serial("PROMISED");
    let free = serial("FREE");
    let (order_a, order_b) = (OrderId::new(), OrderId::new());
    let reserve = |serial_id: Uuid, order: OrderId| {
        db.serials()
            .reserve(ReserveSerialNumber {
                serial_id,
                reference_type: "order".into(),
                reference_id: order.into_uuid(),
                ..Default::default()
            })
            .unwrap()
    };
    let reserved_for_a = reserve(promised.id, order_a);
    let pick = |order: OrderId, serial: &str| {
        let task = db
            .fulfillment()
            .create_pick(CreatePickTask {
                order_id: order,
                order_item_id: OrderItemId::new(),
                warehouse_id: warehouse.id,
                source_location_id: source.id,
                sku: sku.clone(),
                quantity_requested: dec!(1),
                lot_id: None,
                wave_id: None,
                product_name: None,
                serial_number: Some(serial.to_string()),
                priority: None,
                notes: None,
            })
            .unwrap();
        db.fulfillment().complete_pick(CompletePick {
            pick_id: task.id,
            quantity_picked: dec!(1),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
    };

    // Order B cannot pick the unit reserved for order A; nothing moves.
    let refused = pick(order_b, &promised.serial).expect_err("serial is reserved for order A");
    assert!(matches!(refused, CommerceError::Conflict(_)), "{refused:?}");
    let unit = db.serials().get(promised.id).unwrap().unwrap();
    assert_eq!(unit.status, SerialStatus::Reserved);
    assert_eq!(unit.current_location_id, Some(source.id), "the refused pick moved nothing");
    assert_eq!(
        db.warehouse().get_location_inventory(source.id).unwrap()[0].quantity_on_hand,
        dec!(2)
    );

    // An unreserved serial and the owner's own reservation still pick.
    pick(order_b, &free.serial).expect("unreserved serial picks for any order");
    pick(order_a, &promised.serial).expect("the reserving order picks its serial");

    // Ship: A's staged unit is re-promised to B (A releases, B reserves); A
    // can no longer ship it, and B's reservation survives the attempt.
    db.serials().release_reservation(reserved_for_a.id).unwrap();
    let reserved_for_b = reserve(promised.id, order_b);
    let ship = |order: OrderId, serial: &str| {
        let pack =
            db.fulfillment().create_pack(CreatePackTask { order_id: order, notes: None }).unwrap();
        let carton = db
            .fulfillment()
            .add_carton(AddCarton { pack_task_id: pack.id, ..Default::default() })
            .unwrap();
        db.fulfillment()
            .add_carton_item(AddCartonItem {
                carton_id: carton.id,
                sku: sku.clone(),
                quantity: dec!(1),
                lot_id: None,
                serial_number: Some(serial.to_string()),
            })
            .unwrap();
        let task = db
            .fulfillment()
            .create_ship(CreateShipTask {
                order_id: order,
                shipment_id: ShipmentId::new(),
                pack_task_id: pack.id,
                carrier: None,
                service_level: None,
                notes: None,
            })
            .unwrap();
        db.fulfillment().complete_ship(CompleteShip {
            ship_task_id: task.id,
            tracking_number: "serial-owner".into(),
            shipping_cost: None,
            shipped_by: None,
        })
    };
    let refused = ship(order_a, &promised.serial).expect_err("serial is now reserved for order B");
    assert!(matches!(refused, CommerceError::Conflict(_)), "{refused:?}");
    assert_eq!(db.serials().get(promised.id).unwrap().unwrap().status, SerialStatus::Reserved);
    let still_open = db.serials().get_reservation(reserved_for_b.id).unwrap().unwrap();
    assert!(still_open.released_at.is_none(), "B's reservation was not consumed");

    // The unreserved serial picked for B ships as before.
    ship(order_b, &free.serial).expect("unreserved serial ships");
    assert_eq!(db.serials().get(free.id).unwrap().unwrap().status, SerialStatus::Shipped);
}
