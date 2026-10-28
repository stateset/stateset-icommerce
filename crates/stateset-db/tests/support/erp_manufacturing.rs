#![allow(dead_code)]
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn input(db: &dyn Database) -> (CreateWorkOrder, String) {
    let tag = Uuid::new_v4().to_string();
    let component_sku = format!("RAW-{tag}");
    let output_sku = format!("FINISHED-{tag}");
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: tag.clone(),
            name: tag.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let output = db
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some("OUTPUT".into()),
            ..Default::default()
        })
        .unwrap();
    let mut product_ids = Vec::new();
    for sku in [&component_sku, &output_sku] {
        let product = db
            .products()
            .create(CreateProduct {
                name: sku.clone(),
                variants: Some(vec![CreateProductVariant {
                    sku: sku.clone(),
                    price: dec!(1),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();
        product_ids.push(product.id);
    }
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location.id,
            sku: component_sku.clone(),
            quantity: dec!(20),
            reason: "manufacturing fixture".into(),
            lot_id: None,
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let bom = db
        .bom()
        .create(CreateBom {
            product_id: product_ids[1],
            name: tag,
            components: Some(vec![CreateBomComponent {
                component_product_id: Some(product_ids[0]),
                component_sku: Some(component_sku.clone()),
                name: "Raw material".into(),
                quantity: dec!(2),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    db.bom().activate(bom.id).unwrap();
    (
        CreateWorkOrder {
            product_id: product_ids[1],
            bom_id: Some(bom.id),
            quantity_to_build: dec!(3),
            stock: Some(WorkOrderStock {
                source_location_id: location.id,
                output_location_id: output.id,
                output_sku,
            }),
            ..Default::default()
        },
        component_sku,
    )
}

pub(crate) fn lifecycle(db: &dyn Database) {
    let (input, component) = input(db);
    let routing = input.stock.clone().unwrap();
    let repo = db.work_orders();
    let order = repo.create(input.clone()).unwrap();
    assert_eq!(order.materials.len(), 1);
    assert_eq!(order.materials[0].reserved_quantity, dec!(6));
    assert!(order.materials[0].inventory_reservation_id.is_none());
    assert_eq!(order.stock, input.stock);
    assert!(repo.complete(order.id, dec!(1)).is_err());
    assert!(repo.hold(order.id).is_err());
    assert!(
        repo.update(
            order.id,
            UpdateWorkOrder { status: Some(WorkOrderStatus::Completed), ..Default::default() }
        )
        .is_err()
    );
    let started = repo.start(order.id).unwrap();
    assert!(started.materials[0].inventory_reservation_id.is_some());
    assert_eq!(db.inventory().get_stock(&component).unwrap().unwrap().total_allocated, dec!(6));
    assert_eq!(
        db.warehouse().get_location_inventory(routing.source_location_id).unwrap()[0]
            .quantity_reserved,
        dec!(6)
    );
    assert!(repo.start(order.id).is_err());
    assert!(
        repo.add_material(
            order.id,
            AddWorkOrderMaterial {
                component_sku: component.clone(),
                quantity: dec!(1),
                ..Default::default()
            }
        )
        .is_err()
    );
    repo.consume_material(order.materials[0].id, dec!(1)).unwrap();
    assert_eq!(db.inventory().get_stock(&component).unwrap().unwrap().total_on_hand, dec!(19));
    let partial = repo.complete(order.id, dec!(1.5)).unwrap();
    assert_eq!(partial.status, WorkOrderStatus::PartiallyCompleted);
    assert_eq!(partial.materials[0].consumed_quantity, dec!(3));
    assert_eq!(db.inventory().get_stock(&component).unwrap().unwrap().total_on_hand, dec!(17));
    assert_eq!(
        db.inventory().get_stock(&routing.output_sku).unwrap().unwrap().total_on_hand,
        dec!(1.5)
    );
    let edited = repo
        .update(
            order.id,
            UpdateWorkOrder { notes: Some("QC checked".into()), ..Default::default() },
        )
        .unwrap();
    assert_eq!(edited.status, WorkOrderStatus::PartiallyCompleted);
    assert_eq!(edited.quantity_completed, dec!(1.5));
    repo.resume(order.id).unwrap();
    repo.hold(order.id).unwrap();
    assert!(repo.complete(order.id, dec!(1)).is_err());
    assert!(repo.consume_material(order.materials[0].id, dec!(1)).is_err());
    repo.resume(order.id).unwrap();
    let finished = repo.complete(order.id, dec!(1.5)).unwrap();
    assert_eq!(finished.status, WorkOrderStatus::Completed);
    assert_eq!(finished.materials[0].consumed_quantity, dec!(6));
    assert_eq!(db.inventory().get_stock(&component).unwrap().unwrap().total_on_hand, dec!(14));
    assert_eq!(
        db.inventory().get_stock(&component).unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );
    assert_eq!(
        db.inventory().get_stock(&routing.output_sku).unwrap().unwrap().total_on_hand,
        dec!(3)
    );
    assert_eq!(
        db.warehouse().get_location_inventory(routing.source_location_id).unwrap()[0]
            .quantity_reserved,
        Decimal::ZERO
    );
    assert!(repo.complete(order.id, dec!(1)).is_err());
    assert!(repo.start(order.id).is_err());
    assert!(repo.cancel(order.id).is_err());
    assert!(repo.delete(order.id).is_err());
    let moves = db
        .warehouse()
        .get_movements(MovementFilter {
            sku: Some(routing.output_sku.clone()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        moves
            .iter()
            .filter(|m| m.reference_type.as_deref() == Some("work_order_output"))
            .map(|m| m.quantity)
            .sum::<Decimal>(),
        dec!(3)
    );

    let cancel = repo.create(input.clone()).unwrap();
    repo.start(cancel.id).unwrap();
    repo.consume_material(cancel.materials[0].id, dec!(1)).unwrap();
    repo.cancel(cancel.id).unwrap();
    assert_eq!(db.inventory().get_stock(&component).unwrap().unwrap().total_on_hand, dec!(13));
    assert_eq!(
        db.inventory().get_stock(&component).unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );
    assert_eq!(
        db.warehouse().get_location_inventory(routing.source_location_id).unwrap()[0]
            .quantity_reserved,
        Decimal::ZERO
    );

    let too_large =
        repo.create(CreateWorkOrder { quantity_to_build: dec!(100), ..input.clone() }).unwrap();
    assert!(repo.start(too_large.id).is_err());
    assert_eq!(repo.get(too_large.id).unwrap().unwrap().status, WorkOrderStatus::Planned);
    assert_eq!(
        db.inventory().get_stock(&component).unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );

    let rollback = repo.create(input.clone()).unwrap();
    repo.add_material(
        rollback.id,
        AddWorkOrderMaterial {
            component_sku: "ZZ-UNSTOCKED".into(),
            component_name: "Missing".into(),
            quantity: dec!(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(repo.start(rollback.id).is_err());
    assert!(
        repo.get(rollback.id)
            .unwrap()
            .unwrap()
            .materials
            .iter()
            .all(|m| m.inventory_reservation_id.is_none())
    );
    assert_eq!(
        db.inventory().get_stock(&component).unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );
    assert_eq!(
        db.warehouse().get_location_inventory(routing.source_location_id).unwrap()[0]
            .quantity_reserved,
        Decimal::ZERO
    );

    let unmapped = repo.create(CreateWorkOrder { stock: None, ..input.clone() }).unwrap();
    assert!(repo.start(unmapped.id).is_err());
    repo.update(unmapped.id, UpdateWorkOrder { stock: input.stock.clone(), ..Default::default() })
        .unwrap();
    repo.start(unmapped.id).unwrap();
    repo.cancel(unmapped.id).unwrap();

    let draft_bom = db
        .bom()
        .create(CreateBom {
            product_id: input.product_id,
            name: "Draft".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(repo.create(CreateWorkOrder { bom_id: Some(draft_bom.id), ..input.clone() }).is_err());
    let other = db
        .products()
        .create(CreateProduct {
            name: format!("Wrong product {}", uuid::Uuid::new_v4()),
            ..Default::default()
        })
        .unwrap();
    assert!(repo.create(CreateWorkOrder { product_id: other.id, ..input.clone() }).is_err());

    let count = repo
        .count(WorkOrderFilter { product_id: Some(input.product_id), ..Default::default() })
        .unwrap();
    let invalid = CreateWorkOrder { product_id: ProductId::new(), ..input.clone() };
    assert!(repo.create_batch_atomic(vec![input.clone(), invalid]).is_err());
    assert_eq!(
        repo.count(WorkOrderFilter { product_id: Some(input.product_id), ..Default::default() })
            .unwrap(),
        count
    );
    let mut bad_task = input.clone();
    bad_task.tasks =
        Some(vec![CreateWorkOrderTask { task_name: String::new(), ..Default::default() }]);
    assert!(repo.create(bad_task).is_err());
    assert_eq!(
        repo.count(WorkOrderFilter { product_id: Some(input.product_id), ..Default::default() })
            .unwrap(),
        count
    );
}
