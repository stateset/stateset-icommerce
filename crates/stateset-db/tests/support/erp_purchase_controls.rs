use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn approved_lines_are_immutable(db: &dyn Database) {
    let name = format!("PO-EDIT-{}", Uuid::new_v4());
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: name.clone(), ..Default::default() })
        .unwrap();
    let line = |cost| CreatePurchaseOrderItem {
        sku: name.clone(),
        name: name.clone(),
        quantity: dec!(2),
        unit_cost: cost,
        ..Default::default()
    };
    let po = db
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![line(dec!(10))],
            ..Default::default()
        })
        .unwrap();
    let initial = po.items[0].id;
    let added = db.purchase_orders().add_item(po.id, line(dec!(5))).unwrap();
    db.purchase_orders().update_item(initial, line(dec!(12))).unwrap();
    assert_eq!(db.purchase_orders().get(po.id).unwrap().unwrap().total, dec!(34));
    db.purchase_orders().submit_for_approval(po.id).unwrap();
    db.purchase_orders().remove_item(added.id).unwrap();
    assert_eq!(db.purchase_orders().get(po.id).unwrap().unwrap().total, dec!(24));
    db.purchase_orders().approve(po.id, "approver").unwrap();
    for sent in [false, true] {
        if sent {
            db.purchase_orders().send(po.id).unwrap();
        }
        assert!(matches!(
            db.purchase_orders().add_item(po.id, line(dec!(50))),
            Err(CommerceError::Conflict(_))
        ));
        assert!(matches!(
            db.purchase_orders().update_item(initial, line(dec!(50))),
            Err(CommerceError::Conflict(_))
        ));
        assert!(matches!(
            db.purchase_orders().remove_item(initial),
            Err(CommerceError::Conflict(_))
        ));
        let po = db.purchase_orders().get(po.id).unwrap().unwrap();
        assert_eq!(po.total, dec!(24));
        assert_eq!(po.items.len(), 1);
        assert_eq!(po.items[0].unit_cost, dec!(12));
    }
}

pub(crate) fn receipt_serials_and_line_statuses(db: &dyn Database) {
    let sku = format!("RECEIPT-SERIAL-{}", Uuid::new_v4());
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let receipt = db
        .receiving()
        .create_receipt(CreateReceipt {
            warehouse_id: warehouse.id,
            items: vec![
                CreateReceiptItem {
                    sku: sku.clone(),
                    expected_quantity: dec!(3),
                    ..Default::default()
                },
                CreateReceiptItem {
                    sku: format!("{sku}-EMPTY"),
                    expected_quantity: dec!(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
    let items = db.receiving().get_receipt_items(receipt.id).unwrap();
    let line = |quantity, serials| ReceiveItemLine {
        receipt_item_id: items[0].id,
        quantity_received: quantity,
        quantity_rejected: None,
        rejection_reason: None,
        lot_number: None,
        serial_numbers: serials,
        expiration_date: None,
        notes: None,
    };
    let receive = |lines| {
        db.receiving().receive_items(ReceiveItems {
            receipt_id: receipt.id,
            items: lines,
            receiving_location_id: Some(location.id),
            received_by: Some("receiver".into()),
        })
    };
    let a = format!("{sku}-A");
    let b = format!("{sku}-B");
    assert!(receive(vec![line(dec!(2), Some(vec![a.clone()]))]).is_err());
    assert!(receive(vec![line(dec!(0.5), Some(vec![a.clone()]))]).is_err());
    assert!(receive(vec![line(dec!(2), Some(vec![a.clone(), a.clone()]))]).is_err());
    assert_eq!(db.receiving().get_receipt(receipt.id).unwrap().unwrap().received_quantity, dec!(0));
    receive(vec![line(dec!(1), Some(vec![a.clone()]))]).unwrap();
    // Missing serials and a duplicate on the second line roll back all quantities and registrations.
    assert!(receive(vec![line(dec!(1), None)]).is_err());
    assert!(
        receive(vec![line(dec!(1), Some(vec![b.clone()])), line(dec!(1), Some(vec![a.clone()]))])
            .is_err()
    );
    assert!(db.serials().get_by_serial(&b).unwrap().is_none());
    assert_eq!(db.receiving().get_receipt(receipt.id).unwrap().unwrap().received_quantity, dec!(1));
    receive(vec![line(dec!(1), Some(vec![b.clone()]))]).unwrap();
    let serial = db.serials().get_by_serial(&a).unwrap().unwrap();
    assert_eq!(serial.sku, sku);
    assert!(
        db.serials()
            .reserve(ReserveSerialNumber {
                serial_id: serial.id,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                reserved_by: None,
                expires_in_seconds: None
            })
            .is_err()
    );
    db.receiving().complete_receiving(receipt.id).unwrap();
    let lines = db.receiving().get_receipt_items(receipt.id).unwrap();
    assert_eq!(lines[0].status, ReceiptItemStatus::PartiallyReceived);
    assert_eq!(lines[1].status, ReceiptItemStatus::Pending);
    assert_eq!(lines[0].serial_numbers.as_deref(), Some(format!("{a},{b}").as_str()));
    for _ in 0..2 {
        let put = db
            .receiving()
            .create_put_away(CreatePutAway {
                receipt_id: receipt.id,
                receipt_item_id: items[0].id,
                sku: sku.clone(),
                from_location_id: None,
                to_location_id: location.id,
                quantity: dec!(1),
                lot_id: None,
                assigned_to: None,
                notes: None,
            })
            .unwrap();
        db.receiving()
            .complete_put_away(CompletePutAway {
                put_away_id: put.id,
                actual_location_id: None,
                completed_by: None,
                notes: None,
            })
            .unwrap();
    }
    for serial in [&a, &b] {
        let registered = db.serials().get_by_serial(serial).unwrap().unwrap();
        assert_eq!(registered.current_location_id, Some(location.id));
        let reservation = db
            .serials()
            .reserve(ReserveSerialNumber {
                serial_id: registered.id,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                reserved_by: None,
                expires_in_seconds: None,
            })
            .unwrap();
        db.serials().release_reservation(reservation.id).unwrap();
    }
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(2));
}

pub(crate) fn po_backorders_consume_received_stock(db: &dyn Database) {
    let sku = format!("PO-BACKORDER-{}", Uuid::new_v4());
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: sku.clone(), ..Default::default() })
        .unwrap();
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let po = db
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: sku.clone(),
                name: sku.clone(),
                quantity: dec!(5),
                unit_cost: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let create = |quantity| {
        db.backorder()
            .create_backorder(CreateBackorder {
                order_id: Uuid::new_v4(),
                order_line_id: None,
                customer_id: Uuid::new_v4(),
                sku: sku.clone(),
                quantity,
                priority: None,
                expected_date: None,
                promised_date: None,
                source_location_id: Some(warehouse.id),
                notes: None,
            })
            .unwrap()
    };
    let first = create(dec!(3));
    let fulfill = |id, source, quantity| {
        db.backorder().fulfill_backorder(FulfillBackorder {
            backorder_id: id,
            quantity,
            source_type: FulfillmentSourceType::PurchaseOrder,
            source_id: source,
            notes: None,
            fulfilled_by: None,
        })
    };
    assert!(fulfill(first.id, None, dec!(3)).is_err());
    assert!(fulfill(first.id, Some(Uuid::new_v4()), dec!(3)).is_err());
    assert!(fulfill(first.id, Some(po.id.into()), dec!(3)).is_err());
    db.purchase_orders().submit_for_approval(po.id).unwrap();
    db.purchase_orders().approve(po.id, "approver").unwrap();
    db.purchase_orders().send(po.id).unwrap();
    db.purchase_orders()
        .receive(
            po.id,
            ReceivePurchaseOrderItems {
                items: vec![ReceivePurchaseOrderItem {
                    item_id: po.items[0].id,
                    quantity_received: dec!(5),
                    notes: None,
                }],
                warehouse_id: warehouse.id,
                receiving_location_id: location.id,
                notes: None,
            },
        )
        .unwrap();
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(5));
    // Existing backorder allocations are released and replaced by the selected receipt stock atomically.
    db.backorder()
        .allocate_backorder(AllocateBackorder {
            backorder_id: first.id,
            quantity: dec!(3),
            location_id: Some(warehouse.id),
            lot_id: None,
            expires_at: None,
        })
        .unwrap();
    fulfill(first.id, Some(po.id.into()), dec!(3)).unwrap();
    let stock = db.inventory().get_stock(&sku).unwrap().unwrap();
    assert_eq!((stock.total_on_hand, stock.total_allocated), (dec!(2), dec!(0)));
    let second = create(dec!(3));
    assert!(fulfill(second.id, Some(po.id.into()), dec!(3)).is_err());
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(2));
    assert_eq!(
        db.backorder().get_backorder(second.id).unwrap().unwrap().quantity_fulfilled,
        dec!(0)
    );
    fulfill(second.id, Some(po.id.into()), dec!(2)).unwrap();
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(0));
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location.id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(5),
            reason: "unrelated stock".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    // Replenishment at the same shelf cannot reuse a fully attributed PO receipt.
    assert!(fulfill(second.id, Some(po.id.into()), dec!(1)).is_err());
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_on_hand, dec!(5));
}
