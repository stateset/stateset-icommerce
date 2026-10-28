use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

fn receive(db: &dyn Database, warehouse: i32, location: i32, sku: &str, quantity: Decimal) {
    let receiving = db.receiving();
    let receipt = receiving
        .create_receipt(CreateReceipt {
            warehouse_id: warehouse,
            items: vec![CreateReceiptItem {
                sku: sku.into(),
                expected_quantity: quantity,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let item = receiving.get_receipt_items(receipt.id).unwrap()[0].id;
    receiving.start_receiving(receipt.id).unwrap();
    receiving
        .receive_items(ReceiveItems {
            receipt_id: receipt.id,
            receiving_location_id: Some(location),
            received_by: None,
            items: vec![ReceiveItemLine {
                receipt_item_id: item,
                quantity_received: quantity,
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
        })
        .unwrap();
    receiving.complete_receiving(receipt.id).unwrap();
    let put_away = receiving
        .create_put_away(CreatePutAway {
            receipt_id: receipt.id,
            receipt_item_id: item,
            sku: sku.into(),
            from_location_id: None,
            to_location_id: location,
            quantity,
            lot_id: None,
            assigned_to: None,
            notes: None,
        })
        .unwrap();
    receiving
        .complete_put_away(CompletePutAway {
            put_away_id: put_away.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .unwrap();
}

pub(crate) fn cycle_counts(db: &dyn Database) {
    let sku = format!("COUNT-{}", uuid::Uuid::new_v4());
    let w = db.warehouse();
    let warehouse = w
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = w
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(sku.clone()),
            is_receivable: Some(true),
            ..Default::default()
        })
        .unwrap();
    receive(db, warehouse.id, location.id, &sku, dec!(40));
    let create = || {
        w.create_cycle_count(CreateCycleCount {
            warehouse_id: warehouse.id,
            location_id: Some(location.id),
            lines: vec![CreateCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                expected_quantity: dec!(50),
            }],
            ..Default::default()
        })
        .unwrap()
    };
    let record = |id, quantity| {
        w.start_cycle_count(id).unwrap();
        w.record_cycle_counts(
            id,
            vec![RecordCycleCountLine {
                sku: sku.clone(),
                lot_id: None,
                counted_quantity: quantity,
            }],
        )
        .unwrap();
    };
    let assert_stock = |quantity| {
        let levels = w.get_location_inventory(location.id).unwrap();
        assert_eq!(levels.iter().find(|row| row.sku == sku).unwrap().quantity_on_hand, quantity);
        let item = db.inventory().get_item_by_sku(&sku).unwrap().unwrap();
        assert_eq!(
            db.inventory().get_balance(item.id, warehouse.id).unwrap().unwrap().quantity_on_hand,
            quantity
        );
    };
    let count = create();
    assert_eq!(count.lines[0].expected_quantity, dec!(40), "caller supplied 50 must be ignored");
    record(count.id, dec!(40));
    assert_eq!(w.complete_cycle_count(count.id).unwrap().lines[0].variance, Some(dec!(0)));
    assert_stock(dec!(40));
    assert!(w.complete_cycle_count(count.id).is_err(), "completion applies exactly once");
    let count = create();
    record(count.id, dec!(40));
    receive(db, warehouse.id, location.id, &sku, dec!(5));
    assert_eq!(w.complete_cycle_count(count.id).unwrap().lines[0].variance, Some(dec!(-5)));
    assert_stock(dec!(40));
    let _reservation = db
        .inventory()
        .reserve(ReserveInventory {
            sku: sku.clone(),
            location_id: Some(warehouse.id),
            quantity: dec!(35),
            reference_type: "test".into(),
            reference_id: uuid::Uuid::new_v4().to_string(),
            expires_in_seconds: None,
        })
        .unwrap();
    let count = create();
    record(count.id, dec!(30));
    assert!(w.complete_cycle_count(count.id).is_err(), "count cannot consume reserved stock");
    assert_stock(dec!(40));
    assert_eq!(w.get_cycle_count(count.id).unwrap().unwrap().status, CycleCountStatus::InProgress);
}

pub(crate) fn put_away_identity(db: &dyn Database) {
    let sku = format!("IDENTITY-{}", uuid::Uuid::new_v4());
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
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(sku.clone()),
            is_receivable: Some(true),
            ..Default::default()
        })
        .unwrap();
    let lot = db
        .lots()
        .create(CreateLot { sku: sku.clone(), quantity: dec!(10), ..Default::default() })
        .unwrap();
    let receiving = db.receiving();
    let receipt = receiving
        .create_receipt(CreateReceipt {
            warehouse_id: warehouse.id,
            items: vec![CreateReceiptItem {
                sku: sku.clone(),
                expected_quantity: dec!(10),
                lot_number: Some(lot.lot_number.clone()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let item = receiving.get_receipt_items(receipt.id).unwrap()[0].id;
    receiving.start_receiving(receipt.id).unwrap();
    receiving
        .receive_items(ReceiveItems {
            receipt_id: receipt.id,
            receiving_location_id: Some(location.id),
            received_by: None,
            items: vec![ReceiveItemLine {
                receipt_item_id: item,
                quantity_received: dec!(10),
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
        })
        .unwrap();
    let input = CreatePutAway {
        receipt_id: receipt.id,
        receipt_item_id: item,
        sku,
        from_location_id: None,
        to_location_id: location.id,
        quantity: dec!(10),
        lot_id: None,
        assigned_to: None,
        notes: None,
    };
    assert!(
        receiving
            .create_put_away(CreatePutAway { sku: "EXPENSIVE-SKU".into(), ..input.clone() })
            .is_err()
    );
    assert!(
        receiving
            .create_put_away(CreatePutAway { lot_id: Some(uuid::Uuid::new_v4()), ..input.clone() })
            .is_err()
    );
    let task = receiving.create_put_away(input.clone()).unwrap();
    assert_eq!(task.lot_id, Some(lot.id), "lot must be derived from the receipt");
    receiving.cancel_receipt(receipt.id).unwrap();
    assert!(receiving.create_put_away(input).is_err());
    assert!(
        receiving
            .complete_put_away(CompletePutAway {
                put_away_id: task.id,
                actual_location_id: None,
                completed_by: None,
                notes: None
            })
            .is_err()
    );
    assert!(db.warehouse().get_location_inventory(location.id).unwrap().is_empty());
    assert_ne!(receiving.get_put_away(task.id).unwrap().unwrap().status, PutAwayStatus::Completed);
}

pub(crate) fn purchase_receiving(db: &dyn Database) {
    let code = format!("PO-{}", uuid::Uuid::new_v4());
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: code.clone(),
            name: code.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let repo = db.purchase_orders();
    let supplier =
        repo.create_supplier(CreateSupplier { name: code.clone(), ..Default::default() }).unwrap();
    let po = repo
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: code.clone(),
                name: code.clone(),
                quantity: dec!(10),
                unit_cost: dec!(0.125),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let receiving = db.receiving();
    assert!(
        receiving.create_receipt_from_po(po.id.into(), warehouse.id).is_err(),
        "draft PO must not accept receipt documents"
    );
    repo.submit_for_approval(po.id).unwrap();
    repo.approve(po.id, "test-approver").unwrap();
    repo.send(po.id).unwrap();
    let input = |quantity, destination| ReceivePurchaseOrderItems {
        warehouse_id: warehouse.id,
        receiving_location_id: destination,
        items: vec![ReceivePurchaseOrderItem {
            item_id: po.items[0].id,
            quantity_received: quantity,
            notes: None,
        }],
        notes: None,
    };
    assert!(repo.receive(po.id, input(dec!(1), 0)).is_err(), "destination must be explicit");
    assert!(
        repo.receive(po.id, input(dec!(1), i32::MAX)).is_err(),
        "invalid destination rolls back"
    );
    assert_eq!(repo.get(po.id).unwrap().unwrap().items[0].quantity_received, Decimal::ZERO);
    assert!(
        receiving
            .list_receipts(ReceiptFilter { reference_id: Some(po.id.into()), ..Default::default() })
            .unwrap()
            .is_empty()
    );
    let partial = repo.receive(po.id, input(dec!(4), location.id)).unwrap();
    assert_eq!(partial.status, PurchaseOrderStatus::PartiallyReceived);
    let receipts = receiving
        .list_receipts(ReceiptFilter { reference_id: Some(po.id.into()), ..Default::default() })
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].put_away_quantity, dec!(4));
    let lines = receiving.get_receipt_items(receipts[0].id).unwrap();
    assert_eq!(lines[0].po_line_id, Some(po.items[0].id));
    assert_eq!(lines[0].received_quantity, dec!(4));
    assert_eq!(db.inventory().get_stock(&code).unwrap().unwrap().total_on_hand, dec!(4));
    let pending = receiving.create_receipt_from_po(po.id.into(), warehouse.id).unwrap();
    let competing = receiving.create_receipt_from_po(po.id.into(), warehouse.id).unwrap();
    assert_eq!(pending.expected_quantity, dec!(6));
    assert_eq!(competing.expected_quantity, dec!(6));
    let line = receiving.get_receipt_items(pending.id).unwrap().remove(0);
    assert_eq!(line.po_line_id, Some(po.items[0].id));
    let receive = |id, item, qty| ReceiveItems {
        receipt_id: id,
        receiving_location_id: Some(location.id),
        received_by: None,
        items: vec![ReceiveItemLine {
            receipt_item_id: item,
            quantity_received: qty,
            quantity_rejected: None,
            rejection_reason: None,
            lot_number: None,
            serial_numbers: None,
            expiration_date: None,
            notes: None,
        }],
    };
    receiving.receive_items(receive(pending.id, line.id, dec!(2))).unwrap();
    assert_eq!(repo.get(po.id).unwrap().unwrap().items[0].quantity_received, dec!(6));
    assert!(
        receiving.cancel_receipt(pending.id).is_err(),
        "received PO quantities cannot disappear through cancellation"
    );
    receiving.complete_receiving(pending.id).unwrap();
    assert_eq!(
        receiving.get_receipt_items(pending.id).unwrap()[0].status,
        ReceiptItemStatus::PartiallyReceived
    );
    let competing_line = receiving.get_receipt_items(competing.id).unwrap().remove(0);
    assert!(
        receiving.receive_items(receive(competing.id, competing_line.id, dec!(6))).is_err(),
        "other receipts must share the PO cap"
    );
    assert_eq!(
        receiving.get_receipt(competing.id).unwrap().unwrap().received_quantity,
        Decimal::ZERO
    );
    assert_eq!(repo.get(po.id).unwrap().unwrap().items[0].quantity_received, dec!(6));
    let completed = repo.receive(po.id, input(dec!(4), location.id)).unwrap();
    assert_eq!(completed.status, PurchaseOrderStatus::Received);
    assert!(repo.receive(po.id, input(dec!(1), location.id)).is_err());
    // Two received units remain on the closed partial receipt awaiting put-away.
    assert_eq!(db.inventory().get_stock(&code).unwrap().unwrap().total_on_hand, dec!(8));
    let task = receiving
        .create_put_away(CreatePutAway {
            receipt_id: pending.id,
            receipt_item_id: line.id,
            sku: code.clone(),
            from_location_id: None,
            to_location_id: location.id,
            quantity: dec!(2),
            lot_id: None,
            assigned_to: None,
            notes: None,
        })
        .unwrap();
    receiving
        .complete_put_away(CompletePutAway {
            put_away_id: task.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .unwrap();
    assert_eq!(db.inventory().get_stock(&code).unwrap().unwrap().total_on_hand, dec!(10));
}

pub(crate) fn quality_guards(db: &dyn Database) {
    let sku = format!("HOLD-{}", uuid::Uuid::new_v4());
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
    let destination = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{sku}@example.test"),
            first_name: "Quality".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    let product = db
        .products()
        .create(CreateProduct {
            name: sku.clone(),
            slug: Some(sku.to_lowercase()),
            description: None,
            attributes: None,
            product_type: None,
            seo: None,
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(1),
                is_default: Some(true),
                ..Default::default()
            }]),
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
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 1,
                unit_price: dec!(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    receive(db, warehouse.id, location.id, &sku, dec!(10));
    let pick = |lot_id| CreatePickTask {
        wave_id: None,
        order_id: order.id,
        order_item_id: order.items[0].id,
        warehouse_id: warehouse.id,
        sku: sku.clone(),
        product_name: None,
        source_location_id: location.id,
        quantity_requested: dec!(1),
        lot_id,
        serial_number: None,
        priority: None,
        notes: None,
    };
    let complete = |pick_id, lot_id| CompletePick {
        pick_id,
        quantity_picked: dec!(1),
        quantity_short: None,
        short_reason: None,
        lot_id,
        serial_number: None,
        completed_by: None,
    };
    let planned = db.fulfillment().create_pick(pick(None)).unwrap();
    let hold = db
        .quality()
        .create_hold(CreateQualityHold {
            sku: sku.clone(),
            location_id: Some(location.id),
            quantity: dec!(10),
            reason: "Awaiting inspection".into(),
            placed_by: "quality-test".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(db.fulfillment().create_pick(pick(None)).is_err());
    assert!(db.fulfillment().complete_pick(complete(planned.id, None)).is_err());
    assert!(
        db.inventory()
            .reserve(ReserveInventory {
                sku: sku.clone(),
                location_id: Some(warehouse.id),
                quantity: dec!(1),
                reference_type: "test".into(),
                reference_id: order.id.to_string(),
                expires_in_seconds: None
            })
            .is_err()
    );
    assert!(
        db.warehouse()
            .move_inventory(MoveInventory {
                sku: sku.clone(),
                lot_id: None,
                from_location_id: location.id,
                to_location_id: destination.id,
                quantity: dec!(1),
                reason: None,
                performed_by: None
            })
            .is_err()
    );
    assert_eq!(
        db.warehouse().get_location_inventory(location.id).unwrap()[0].quantity_on_hand,
        dec!(10)
    );
    db.quality()
        .release_hold(
            hold.id,
            ReleaseQualityHold {
                released_by: "quality-test".into(),
                release_notes: Some("inspection passed".into()),
            },
        )
        .unwrap();
    let blocked = db
        .lots()
        .create(CreateLot {
            lot_number: Some(format!("BLOCKED-{sku}")),
            sku: sku.clone(),
            quantity: dec!(5),
            initial_location_id: Some(warehouse.id),
            ..Default::default()
        })
        .unwrap();
    db.lots().quarantine(blocked.id, "quarantine test").unwrap();
    assert!(db.fulfillment().create_pick(pick(Some(blocked.id))).is_err());
    assert!(db.fulfillment().complete_pick(complete(planned.id, Some(blocked.id))).is_err());
    db.lots().release_quarantine(blocked.id).unwrap();
    db.lots()
        .consume(ConsumeLot {
            lot_id: blocked.id,
            quantity: dec!(5),
            reference_type: "quality_test".into(),
            reference_id: blocked.id,
            ..Default::default()
        })
        .unwrap();
    let expired = db
        .lots()
        .create(CreateLot {
            sku: sku.clone(),
            quantity: dec!(5),
            lot_number: Some(format!("EXPIRED-{sku}")),
            expiration_date: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            ..Default::default()
        })
        .unwrap();
    assert!(db.fulfillment().create_pick(pick(Some(expired.id))).is_err());
    let foreign = db
        .lots()
        .create(CreateLot { sku: format!("OTHER-{sku}"), quantity: dec!(5), ..Default::default() })
        .unwrap();
    assert!(db.fulfillment().complete_pick(complete(planned.id, Some(foreign.id))).is_err());
    let serial = db
        .serials()
        .create(CreateSerialNumber {
            serial: Some(format!("SERIAL-{sku}")),
            sku: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let serial_hold = db
        .quality()
        .create_hold(CreateQualityHold {
            sku: sku.clone(),
            serial_number: Some(serial.serial.clone()),
            quantity: dec!(1),
            reason: "Serial inspection".into(),
            placed_by: "quality-test".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        db.serials()
            .reserve(ReserveSerialNumber {
                serial_id: serial.id,
                reference_type: "test".into(),
                reference_id: order.id.into(),
                ..Default::default()
            })
            .is_err()
    );
    db.quality()
        .release_hold(
            serial_hold.id,
            ReleaseQualityHold { released_by: "quality-test".into(), release_notes: None },
        )
        .unwrap();
    db.fulfillment().complete_pick(complete(planned.id, None)).unwrap();
    assert_eq!(
        db.warehouse().get_location_inventory(location.id).unwrap()[0].quantity_on_hand,
        dec!(9)
    );
    assert_eq!(db.inventory().get_stock(&sku).unwrap().unwrap().total_allocated, dec!(1));
}
