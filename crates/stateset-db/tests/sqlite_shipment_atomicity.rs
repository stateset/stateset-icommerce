#![cfg(feature = "sqlite")]
#[path = "support/shipment_atomicity.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::*;
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn snapshot(db: &SqliteDatabase) -> Vec<(String, Vec<Vec<String>>)> {
    let conn = db.pool().get().unwrap();
    [
        "orders",
        "order_items",
        "inventory_balances",
        "inventory_reservations",
        "inventory_transactions",
        "kernel_outbox",
        "kernel_receipts",
    ]
    .iter()
    .map(|table| {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY 1")).unwrap();
        let columns = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                (0..columns).map(|i| row.get_ref(i).map(|v| format!("{v:?}"))).collect()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        (table.to_string(), rows)
    })
    .collect()
}

#[test]
fn shipment_faults_rollback_every_effect_and_allow_same_key_retry() {
    let db = SqliteDatabase::in_memory().unwrap();
    for fault in support::FAULTS {
        for preview in [false, true] {
            for partial in [false, true] {
                let suffix = Uuid::new_v4();
                let skus = [format!("ATOMIC-FIRST-{suffix}"), format!("ATOMIC-SECOND-{suffix}")];
                let items: Vec<_> = skus
                    .iter()
                    .map(|sku| db.inventory().create_item(support::stock(sku)).unwrap())
                    .collect();
                let customer = db.customers().create(support::customer()).unwrap();
                let order = db.orders().create(support::order(customer.id, &skus)).unwrap();
                for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
                    db.orders()
                        .update(
                            order.id,
                            UpdateOrder { status: Some(status), ..Default::default() },
                        )
                        .unwrap();
                }
                let payment = db.payments().create(support::payment(&order)).unwrap();
                db.payments().mark_completed(payment.id).unwrap();
                let order = db.orders().get(order.id).unwrap().unwrap();
                let kernel = db.kernel_executor(support::policy());
                let mut command = support::command(&order, partial);
                if preview {
                    assert_eq!(
                        kernel.execute_ship_reserved_order(&command).unwrap().status,
                        ExecutionStatus::Previewed
                    );
                    command.command_id = Uuid::new_v4();
                }
                command.mode = ExecutionMode::Apply;
                let before = snapshot(&db);
                let conn = db.pool().get().unwrap();
                let second: String = conn.query_row(
                    "SELECT id FROM inventory_reservations WHERE item_id = ? AND reference_id = ?",
                    rusqlite::params![items[1].id, order.id.to_string()], |r| r.get(0),
                ).unwrap();
                let (table, operation, condition) = support::fault(
                    fault,
                    items[1].id,
                    second.parse().unwrap(),
                    order.id,
                    &command.idempotency_key,
                );
                let first_item = items[0].id;
                conn.execute_batch(&format!("CREATE TRIGGER shipment_fault BEFORE {operation} ON {table}
                    WHEN {condition} BEGIN
                    SELECT CASE WHEN (SELECT COUNT(*) FROM inventory_transactions WHERE item_id = {first_item} AND transaction_type = 'shipment') <> 1
                        THEN RAISE(ABORT, 'earlier shipment write missing') END;
                    SELECT RAISE(ABORT, 'injected shipment atomicity failure'); END;")).unwrap();
                drop(conn);
                let failed = kernel.execute_ship_reserved_order(&command);
                db.pool().get().unwrap().execute_batch("DROP TRIGGER shipment_fault").unwrap();
                let error = failed.expect_err("the selected fault must be reached").to_string();
                assert!(
                    error.contains("injected shipment atomicity failure"),
                    "{fault} preview={preview} partial={partial}: {error}"
                );
                assert_eq!(
                    snapshot(&db),
                    before,
                    "{fault} preview={preview} partial={partial}: all durable rows must roll back"
                );

                let receipt = kernel.execute_ship_reserved_order(&command).unwrap();
                support::check_receipt(&receipt, &order, partial);
                for (index, sku) in skus.iter().enumerate() {
                    let stock = db.inventory().get_stock(sku).unwrap().unwrap();
                    let reserved = Decimal::from(index as i32 + 2);
                    let shipped = if partial { Decimal::ONE } else { reserved };
                    assert_eq!(
                        (stock.total_on_hand, stock.total_allocated, stock.total_available),
                        (
                            Decimal::from(10) - shipped,
                            reserved - shipped,
                            Decimal::from(10) - reserved
                        )
                    );
                }
                let conn = db.pool().get().unwrap();
                let linked: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
                        [command.command_id.to_string()],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(linked, 3);
                let shipped: i64 = conn.query_row("SELECT COUNT(*) FROM inventory_transactions WHERE transaction_type = 'shipment' AND item_id IN (?, ?)", [items[0].id, items[1].id], |r| r.get(0)).unwrap();
                assert_eq!(shipped, 2);
                drop(conn);
                let committed = snapshot(&db);
                command.command_id = Uuid::new_v4();
                let replay = kernel.execute_ship_reserved_order(&command).unwrap();
                assert_eq!(
                    serde_json::to_value(replay).unwrap(),
                    serde_json::to_value(receipt).unwrap()
                );
                assert_eq!(snapshot(&db), committed, "replay must not write again");
            }
        }
    }
}

#[test]
fn line_owned_fractional_splits_preserve_same_sku_sibling() {
    let db = SqliteDatabase::in_memory().unwrap();
    let sku = format!("OWNERSHIP-{}", Uuid::new_v4());
    db.inventory().create_item(support::stock(&sku)).unwrap();
    let customer = db.customers().create(support::customer()).unwrap();
    let order =
        db.orders().create(support::order(customer.id, &[sku.clone(), sku.clone()])).unwrap();
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .unwrap();
    }
    let payment = db.payments().create(support::payment(&order)).unwrap();
    db.payments().mark_completed(payment.id).unwrap();
    let order = db.orders().get(order.id).unwrap().unwrap();
    let target = &order.items[0];
    let sibling = &order.items[1];
    let micro = Decimal::new(1, 6);
    let mut conn = db.pool().get().unwrap();
    let source: String = conn
        .query_row(
            "SELECT id FROM inventory_reservations WHERE order_item_id = ?",
            [target.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let source: Uuid = source.parse().unwrap();
    let child = Uuid::from_u128(source.as_u128() + 1);
    let tx = conn.transaction().unwrap();
    tx.execute("INSERT INTO inventory_reservations (id,item_id,location_id,quantity,status,reference_type,reference_id,expires_at,created_at,order_item_id)
        SELECT ?,item_id,location_id,?,status,reference_type,reference_id,expires_at,created_at,order_item_id FROM inventory_reservations WHERE id = ?",
        rusqlite::params![child.to_string(), (Decimal::from(target.quantity) - micro).to_string(), source.to_string()]).unwrap();
    tx.execute(
        "UPDATE inventory_reservations SET quantity = ? WHERE id = ?",
        rusqlite::params![micro.to_string(), source.to_string()],
    )
    .unwrap();
    tx.commit().unwrap();
    drop(conn);
    let mut command = support::command(&order, false);
    command.mode = ExecutionMode::Apply;
    command.payload.lines = Some(vec![ShipmentLineInput { order_item_id: target.id, quantity: 1 }]);
    let kernel = db.kernel_executor(support::policy());
    let receipt = kernel.execute_ship_reserved_order(&command).unwrap();
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{:?}", receipt.error_message);
    assert_eq!(receipt.event_ids.len(), 3);
    let shipped = receipt.result.as_ref().unwrap();
    assert_eq!(shipped.items.iter().find(|i| i.id == target.id).unwrap().shipped_quantity, 1);
    assert_eq!(shipped.items.iter().find(|i| i.id == sibling.id).unwrap().shipped_quantity, 0);
    let conn = db.pool().get().unwrap();
    let original: (String, String) = conn
        .query_row(
            "SELECT status, quantity FROM inventory_reservations WHERE id = ?",
            [source.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(original, ("fulfilled".into(), micro.to_string()));
    let remainder: String = conn
        .query_row(
            "SELECT quantity FROM inventory_reservations WHERE id = ?",
            [child.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remainder.parse::<Decimal>().unwrap(), Decimal::from(target.quantity - 1));
    let sibling_row: (String, String) = conn
        .query_row(
            "SELECT status, quantity FROM inventory_reservations WHERE order_item_id = ?",
            [sibling.id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(sibling_row, ("pending".into(), sibling.quantity.to_string()));
    drop(conn);
    let stock = db.inventory().get_stock(&sku).unwrap().unwrap();
    assert_eq!(
        (stock.total_on_hand, stock.total_allocated, stock.total_available),
        (Decimal::from(9), Decimal::from(4), Decimal::from(5))
    );
    command.command_id = Uuid::new_v4();
    assert_eq!(
        serde_json::to_value(kernel.execute_ship_reserved_order(&command).unwrap()).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );

    // Preserve SKU totals while moving one micro-unit to the wrong line.
    let mut conn = db.pool().get().unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute(
        "UPDATE inventory_reservations SET quantity = ? WHERE id = ?",
        rusqlite::params![
            (Decimal::from(target.quantity - 1) - micro).to_string(),
            child.to_string()
        ],
    )
    .unwrap();
    tx.execute(
        "UPDATE inventory_reservations SET quantity = ? WHERE order_item_id = ?",
        rusqlite::params![
            (Decimal::from(sibling.quantity) + micro).to_string(),
            sibling.id.to_string()
        ],
    )
    .unwrap();
    tx.commit().unwrap();
    drop(conn);
    let mut next = support::command(shipped, false);
    next.mode = ExecutionMode::Apply;
    next.payload.lines = command.payload.lines;
    let refused = kernel.execute_ship_reserved_order(&next).unwrap();
    assert_eq!(refused.status, ExecutionStatus::Rejected);
    assert_eq!(refused.error_code.as_deref(), Some("commerce.shipment_inventory_invalid"));
    assert!(refused.event_ids.is_empty());
    assert_eq!(db.orders().get(order.id).unwrap().unwrap(), *shipped);
    assert_eq!(
        serde_json::to_value(db.inventory().get_stock(&sku).unwrap().unwrap()).unwrap(),
        serde_json::to_value(stock).unwrap()
    );
}
