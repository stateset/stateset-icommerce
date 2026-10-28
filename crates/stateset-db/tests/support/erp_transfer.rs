#![allow(dead_code)]
use rust_decimal::Decimal;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn input(db: &dyn Database, quantities: &[Decimal]) -> CreateTransferOrder {
    let tag = uuid::Uuid::new_v4().to_string();
    let mut locations = Vec::new();
    for prefix in ["SRC", "DST"] {
        let warehouse = db
            .warehouse()
            .create_warehouse(CreateWarehouse {
                code: format!("{prefix}-{tag}"),
                name: prefix.into(),
                ..Default::default()
            })
            .unwrap();
        let location = db
            .warehouse()
            .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
            .unwrap();
        locations.push(location.id);
    }
    let mut items = Vec::new();
    for (index, quantity) in quantities.iter().enumerate() {
        let sku = format!("TRANSFER-{tag}-{index}");
        let product = db
            .products()
            .create(CreateProduct {
                name: sku.clone(),
                slug: None,
                description: None,
                product_type: None,
                attributes: None,
                seo: None,
                variants: Some(vec![CreateProductVariant {
                    sku: sku.clone(),
                    price: Decimal::ONE,
                    ..Default::default()
                }]),
            })
            .unwrap();
        db.warehouse()
            .adjust_inventory(AdjustLocationInventory {
                location_id: locations[0],
                sku: sku.clone(),
                lot_id: None,
                quantity: *quantity,
                reason: "transfer fixture".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .unwrap();
        items.push(CreateTransferOrderItem {
            product_id: product.id,
            sku,
            lot_id: None,
            quantity: *quantity,
        });
    }
    CreateTransferOrder {
        source_warehouse_id: WarehouseId::new(),
        destination_warehouse_id: WarehouseId::new(),
        source_location_id: locations[0],
        destination_location_id: locations[1],
        items,
        expected_at: None,
        notes: None,
    }
}

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_input(
    db: &stateset_db::PostgresDatabase,
    quantities: &[Decimal],
) -> CreateTransferOrder {
    let tag = uuid::Uuid::new_v4().to_string();
    let mut locations = Vec::new();
    for prefix in ["SRC", "DST"] {
        let warehouse = db
            .warehouse()
            .create_warehouse_async(CreateWarehouse {
                code: format!("{prefix}-{tag}"),
                name: prefix.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let location = db
            .warehouse()
            .create_location_async(CreateLocation {
                warehouse_id: warehouse.id,
                ..Default::default()
            })
            .await
            .unwrap();
        locations.push(location.id);
    }
    let mut items = Vec::new();
    for (index, quantity) in quantities.iter().enumerate() {
        let sku = format!("TRANSFER-{tag}-{index}");
        let product = db
            .products()
            .create_async(CreateProduct {
                name: sku.clone(),
                slug: None,
                description: None,
                product_type: None,
                attributes: None,
                seo: None,
                variants: Some(vec![CreateProductVariant {
                    sku: sku.clone(),
                    price: Decimal::ONE,
                    ..Default::default()
                }]),
            })
            .await
            .unwrap();
        db.warehouse()
            .adjust_inventory_async(AdjustLocationInventory {
                location_id: locations[0],
                sku: sku.clone(),
                lot_id: None,
                quantity: *quantity,
                reason: "transfer fixture".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .await
            .unwrap();
        items.push(CreateTransferOrderItem {
            product_id: product.id,
            sku,
            lot_id: None,
            quantity: *quantity,
        });
    }
    CreateTransferOrder {
        source_warehouse_id: WarehouseId::new(),
        destination_warehouse_id: WarehouseId::new(),
        source_location_id: locations[0],
        destination_location_id: locations[1],
        items,
        expected_at: None,
        notes: None,
    }
}

pub(crate) fn lifecycle(db: &dyn Database) {
    use rust_decimal_macros::dec;
    let input = input(db, &[dec!(10)]);
    let sku = input.items[0].sku.clone();
    let source = input.source_location_id;
    let destination = input.destination_location_id;
    let repo = db.transfer_orders();
    let order = repo.create(input).unwrap();
    let hold = db
        .quality()
        .create_hold(CreateQualityHold {
            sku: sku.clone(),
            location_id: Some(source),
            quantity: dec!(10),
            reason: "Inspection".into(),
            placed_by: "test".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(repo.ship(order.id).is_err());
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(10));
    db.quality()
        .release_hold(
            hold.id,
            ReleaseQualityHold { released_by: "test".into(), release_notes: None },
        )
        .unwrap();
    let shipped = repo.ship(order.id).unwrap();
    assert_eq!(shipped.status, TransferOrderStatus::InTransit);
    assert_eq!(shipped.items[0].quantity_outstanding(), dec!(10));
    assert_eq!(
        db.warehouse()
            .get_location_inventory(source)
            .unwrap()
            .iter()
            .map(|row| row.quantity_on_hand)
            .sum::<Decimal>(),
        Decimal::ZERO
    );
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, Decimal::ZERO);
    assert!(repo.ship(order.id).is_err());
    assert!(repo.cancel(order.id).is_err(), "in-transit stock cannot disappear by cancellation");
    repo.receive_line(order.id, order.items[0].id, dec!(6)).unwrap();
    let partial = repo.receive_line(order.id, order.items[0].id, dec!(3)).unwrap();
    assert_eq!(partial.status, TransferOrderStatus::PartiallyReceived);
    assert_eq!(partial.items[0].quantity_outstanding(), dec!(1));
    assert!(repo.receive_line(order.id, order.items[0].id, dec!(2)).is_err());
    assert_eq!(
        db.warehouse().get_location_inventory(destination).unwrap()[0].quantity_on_hand,
        dec!(9)
    );
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(9));
    let closed = repo.complete_receiving(order.id).unwrap();
    assert_eq!(closed.status, TransferOrderStatus::Received);
    assert_eq!(closed.items[0].quantity_variance, dec!(1));
    assert_eq!(closed.items[0].quantity_outstanding(), Decimal::ZERO);
    assert!(repo.complete_receiving(order.id).is_err());
    assert!(repo.receive_line(order.id, order.items[0].id, dec!(1)).is_err());
    let moves = db
        .warehouse()
        .get_movements(MovementFilter { sku: Some(sku), ..Default::default() })
        .unwrap();
    assert!(moves.iter().any(
        |m| m.reference_type.as_deref() == Some("transfer_shortage") && m.quantity == dec!(-1)
    ));
}
