use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn lifecycle(db: &dyn Database) {
    let sku = format!("LOT-STOCK-{}", Uuid::new_v4());
    let warehouse = |suffix: &str| {
        db.warehouse()
            .create_warehouse(CreateWarehouse {
                code: format!("{sku}-{suffix}"),
                name: suffix.into(),
                ..Default::default()
            })
            .unwrap()
            .id
    };
    let source = warehouse("source");
    let destination = warehouse("destination");
    let lot = db
        .lots()
        .create(CreateLot {
            sku: sku.clone(),
            quantity: dec!(10),
            initial_location_id: Some(source),
            ..Default::default()
        })
        .unwrap();
    let check = |on_hand: Decimal, reserved: Decimal| {
        let stock = db.inventory().get_stock(&sku).unwrap().unwrap();
        assert_eq!((stock.total_on_hand, stock.total_allocated), (on_hand, reserved));
        let mut physical = Vec::new();
        let mut bins = Vec::new();
        for warehouse in [source, destination] {
            physical.extend(db.warehouse().get_inventory_for_sku(warehouse, &sku).unwrap());
            bins.extend(db.bins().get_bin_levels_for_sku(warehouse, &sku).unwrap());
        }
        assert_eq!(physical.iter().map(|r| r.quantity_on_hand).sum::<Decimal>(), on_hand);
        assert_eq!(physical.iter().map(|r| r.quantity_reserved).sum::<Decimal>(), reserved);
        assert_eq!(bins.iter().map(|r| r.quantity_on_hand).sum::<Decimal>(), on_hand);
        assert_eq!(bins.iter().map(|r| r.quantity_allocated).sum::<Decimal>(), reserved);
    };
    check(dec!(10), dec!(0));
    let generic = db
        .inventory()
        .reserve(ReserveInventory {
            sku: sku.clone(),
            location_id: Some(source),
            quantity: dec!(2),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4().to_string(),
            expires_in_seconds: None,
        })
        .unwrap();
    let reserve = |lot_id, quantity| {
        db.lots()
            .reserve(ReserveLot {
                lot_id,
                quantity,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: None,
            })
            .unwrap()
    };
    let consume = |lot_id, quantity| {
        db.lots().consume(ConsumeLot {
            lot_id,
            quantity,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
    };
    let own = reserve(lot.id, dec!(3));
    check(dec!(10), dec!(5));
    consume(lot.id, dec!(5)).unwrap();
    assert!(consume(lot.id, dec!(1)).is_err());
    check(dec!(5), dec!(5));
    db.lots().confirm_reservation(own).unwrap();
    check(dec!(2), dec!(2));
    db.inventory().release_reservation(generic.id).unwrap();
    check(dec!(2), dec!(0));
    db.lots()
        .adjust(AdjustLot {
            lot_id: lot.id,
            quantity_change: dec!(1),
            location_id: Some(source),
            reason: "count correction".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let child = db
        .lots()
        .split(SplitLot { lot_id: lot.id, quantity: dec!(1), new_lot_number: None, reason: None })
        .unwrap();
    check(dec!(3), dec!(0));
    let merged = db
        .lots()
        .merge(MergeLots {
            source_lot_ids: vec![lot.id, child.id],
            target_lot_number: None,
            reason: None,
        })
        .unwrap();
    check(dec!(3), dec!(0));
    db.lots()
        .transfer(TransferLot {
            lot_id: merged.id,
            from_location_id: source,
            to_location_id: destination,
            quantity: dec!(2),
            reason: None,
            performed_by: None,
        })
        .unwrap();
    assert_eq!(db.lots().get_quantity_at_location(merged.id, source).unwrap(), Some(dec!(1)));
    assert_eq!(db.lots().get_quantity_at_location(merged.id, destination).unwrap(), Some(dec!(2)));
    check(dec!(3), dec!(0));
    let own = reserve(merged.id, dec!(1));
    db.lots().quarantine(merged.id, "inspection").unwrap();
    db.lots().release_reservation(own).unwrap();
    check(dec!(3), dec!(3));
    assert!(consume(merged.id, dec!(1)).is_err());
    db.lots().release_quarantine(merged.id).unwrap();
    check(dec!(3), dec!(0));
    consume(merged.id, dec!(3)).unwrap();
    check(dec!(0), dec!(0));
    let unreceived =
        db.lots().create(CreateLot { sku, quantity: dec!(1), ..Default::default() }).unwrap();
    assert!(consume(unreceived.id, dec!(1)).is_err());
    assert_eq!(db.lots().get(unreceived.id).unwrap().unwrap().quantity_remaining, dec!(1));
}

pub(crate) fn mixed_bins_and_order_warehouses(db: &dyn Database) {
    let sku = format!("MULTI-STOCK-{}", Uuid::new_v4());
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let other = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("{sku}-2"),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    for quantity in [dec!(2), dec!(3)] {
        db.lots()
            .create(CreateLot {
                sku: sku.clone(),
                quantity,
                initial_location_id: Some(warehouse.id),
                ..Default::default()
            })
            .unwrap();
    }
    let source = db.bins().get_bin_levels_for_sku(warehouse.id, &sku).unwrap()[0].bin_id;
    let destination = db
        .bins()
        .create_bin(CreateWarehouseBin {
            warehouse_id: warehouse.id,
            code: "destination".into(),
            ..Default::default()
        })
        .unwrap();
    let movement = db
        .bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: source,
            to_bin_id: destination.id,
            sku: sku.clone(),
            quantity: dec!(4),
            reason: None,
            performed_by: None,
        })
        .unwrap();
    assert_eq!(movement.quantity, dec!(4));
    db.bins()
        .adjust_bin_level(AdjustBinLevel {
            bin_id: destination.id,
            sku: sku.clone(),
            quantity: dec!(-2),
            reason: "damage".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(3));
    assert_eq!(
        db.lots()
            .list(LotFilter { sku: Some(sku.clone()), ..Default::default() })
            .unwrap()
            .iter()
            .map(|lot| lot.quantity_remaining)
            .sum::<Decimal>(),
        dec!(3)
    );
    db.lots()
        .create(CreateLot {
            sku: sku.clone(),
            quantity: dec!(2),
            initial_location_id: Some(other.id),
            ..Default::default()
        })
        .unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{sku}@example.test"),
            first_name: "Stock".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    let product = db
        .products()
        .create(CreateProduct {
            name: sku.clone(),
            slug: Some(sku.to_lowercase()),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(1),
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    db.products()
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .unwrap();
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            stock_policy: StockPolicy::RejectIfInsufficient,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 5,
                unit_price: dec!(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let reservations =
        db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
    assert_eq!(reservations.len(), 2);
    assert_eq!(
        reservations.iter().find(|r| r.location_id == warehouse.id).unwrap().quantity,
        dec!(3)
    );
    assert_eq!(reservations.iter().find(|r| r.location_id == other.id).unwrap().quantity, dec!(2));
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_allocated, dec!(5));
}
