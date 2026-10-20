#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CustomerRepository,
    InventoryRepository, OrderRepository, OrderStatus, ProductId, ReservationStatus, UpdateOrder,
};
use stateset_db::SqliteDatabase;

fn fixture() -> (SqliteDatabase, stateset_core::Order) {
    let db = SqliteDatabase::in_memory().unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "stock-posting@example.test".into(),
            first_name: "Stock".into(),
            last_name: "Posting".into(),
            ..Default::default()
        })
        .unwrap();
    for sku in ["POST-A", "POST-B"] {
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: sku.into(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .unwrap();
    }
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: ["POST-A", "POST-B"]
                .into_iter()
                .map(|sku| CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: sku.into(),
                    name: sku.into(),
                    quantity: 2,
                    unit_price: dec!(1.25),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
        .unwrap();
    (db, order)
}
fn ship(db: &SqliteDatabase, order: &stateset_core::Order) {
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .unwrap();
    }
}
#[test]
fn physical_posting_requires_full_shipment_and_retries_do_not_consume_twice() {
    let (db, order) = fixture();
    assert!(db.inventory().fulfill_order_stock(order.id.into_uuid()).is_err());
    ship(&db, &order);
    let stock = db.inventory().get_stock("POST-A").unwrap().unwrap();
    assert_eq!(stock.total_on_hand, dec!(10));
    assert_eq!(stock.total_allocated, dec!(2));
    for _ in 0..2 {
        db.inventory().fulfill_order_stock(order.id.into_uuid()).unwrap();
    }
    for sku in ["POST-A", "POST-B"] {
        let stock = db.inventory().get_stock(sku).unwrap().unwrap();
        assert_eq!(
            (stock.total_on_hand, stock.total_allocated, stock.total_available),
            (dec!(8), dec!(0), dec!(8))
        );
    }
    let holds =
        db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
    assert!(holds.iter().all(|hold| hold.status == ReservationStatus::Fulfilled));
    for hold in holds {
        db.inventory().release_reservation(hold.id).unwrap();
    }
    assert_eq!(db.inventory().get_stock("POST-A").unwrap().unwrap().total_available, dec!(8));
}
#[test]
fn stock_posting_rolls_back_all_lines_when_required_fact_is_suppressed() {
    let (db, order) = fixture();
    ship(&db, &order);
    let connection = db.pool().get().unwrap();
    let second_hold: String = connection.query_row("SELECT id FROM inventory_reservations WHERE reference_id = ? ORDER BY item_id DESC LIMIT 1", [order.id.to_string()], |row| row.get(0)).unwrap();
    connection.execute_batch(&format!("CREATE TRIGGER suppress_fulfillment_fact BEFORE INSERT ON kernel_outbox WHEN NEW.event_type = 'inventory.reservation_fulfilled.v1' AND NEW.aggregate_id = '{second_hold}' BEGIN SELECT RAISE(IGNORE); END;")).unwrap();
    assert!(db.inventory().fulfill_order_stock(order.id.into_uuid()).is_err());
    for sku in ["POST-A", "POST-B"] {
        let stock = db.inventory().get_stock(sku).unwrap().unwrap();
        assert_eq!((stock.total_on_hand, stock.total_allocated), (dec!(10), dec!(2)));
    }
    assert!(
        db.inventory()
            .list_reservations_by_reference("order", &order.id.to_string())
            .unwrap()
            .iter()
            .all(|hold| hold.status == ReservationStatus::Confirmed)
    );
    connection.execute_batch("DROP TRIGGER suppress_fulfillment_fact").unwrap();
    db.inventory().fulfill_order_stock(order.id.into_uuid()).unwrap();
}

#[test]
fn stock_posting_refuses_suppressed_or_rewritten_inventory_writes() {
    for trigger in [
        "CREATE TRIGGER interfere BEFORE UPDATE ON inventory_reservations WHEN NEW.status = 'fulfilled' BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER interfere BEFORE INSERT ON inventory_transactions WHEN NEW.transaction_type = 'shipment' BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER interfere AFTER UPDATE ON inventory_reservations WHEN NEW.status = 'fulfilled' BEGIN UPDATE inventory_reservations SET status = 'confirmed' WHERE id = NEW.id; END;",
        "CREATE TRIGGER interfere AFTER UPDATE ON inventory_balances WHEN NEW.quantity_on_hand != '10' BEGIN UPDATE inventory_balances SET quantity_on_hand = '99' WHERE item_id = NEW.item_id AND location_id = NEW.location_id; END;",
        "CREATE TRIGGER interfere AFTER INSERT ON inventory_transactions WHEN NEW.transaction_type = 'shipment' BEGIN UPDATE inventory_transactions SET quantity = '0' WHERE id = NEW.id; END;",
        "CREATE TRIGGER interfere AFTER INSERT ON kernel_outbox WHEN NEW.event_type = 'inventory.reservation_fulfilled.v1' BEGIN UPDATE inventory_balances SET quantity_on_hand = '99'; END;",
    ] {
        let (db, order) = fixture();
        ship(&db, &order);
        let connection = db.pool().get().unwrap();
        let facts_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0))
            .unwrap();
        connection.execute_batch(trigger).unwrap();
        let result = db.inventory().fulfill_order_stock(order.id.into_uuid());
        assert!(result.is_err(), "must refuse silently changed stock posting: {trigger}");
        for sku in ["POST-A", "POST-B"] {
            let stock = db.inventory().get_stock(sku).unwrap().unwrap();
            assert_eq!(
                (stock.total_on_hand, stock.total_allocated, stock.total_available),
                (dec!(10), dec!(2), dec!(8)),
                "{trigger}"
            );
        }
        let holds =
            db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
        assert!(holds.iter().all(|hold| hold.status == ReservationStatus::Confirmed && hold.quantity == dec!(2)));
        let shipments: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM inventory_transactions WHERE transaction_type = 'shipment'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(shipments, 0, "{trigger}");
        let facts_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(facts_after, facts_before, "{trigger}");
        connection.execute_batch("DROP TRIGGER interfere").unwrap();
        for _ in 0..2 {
            db.inventory().fulfill_order_stock(order.id.into_uuid()).unwrap();
        }
        assert_eq!(db.inventory().get_stock("POST-A").unwrap().unwrap().total_on_hand, dec!(8));
    }
}
