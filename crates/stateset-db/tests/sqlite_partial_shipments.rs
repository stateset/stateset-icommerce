#![cfg(feature = "sqlite")]
//! Per-line shipped quantities and partial shipments (SQLite).

use rusqlite::params;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateReturn,
    CreateReturnItem, CustomerId, CustomerRepository, InventoryRepository, Order, OrderId,
    OrderRepository, OrderStatus, ProductId, ReturnReason, ReturnRepository, ShipOrder,
    ShipmentLineInput, UpdateOrder,
};
use stateset_db::SqliteDatabase;
use stateset_db::migrations::run_migrations;

fn create_customer(db: &SqliteDatabase, email: &str) -> CustomerId {
    db.customers()
        .create(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

fn stock(db: &SqliteDatabase, sku: &str, qty: Decimal) {
    match db.inventory().create_item(CreateInventoryItem {
        sku: sku.to_string(),
        name: sku.to_string(),
        initial_quantity: Some(qty),
        ..Default::default()
    }) {
        Ok(_) | Err(CommerceError::DuplicateSku(_)) => {}
        Err(e) => panic!("create inventory item: {e:?}"),
    }
}

/// Two-line order: SKU-A x5 and SKU-B x2, both fully reserved, in `Processing`.
fn processing_order(db: &SqliteDatabase, email: &str) -> Order {
    let customer_id = create_customer(db, email);
    stock(db, "PS-SKU-A", dec!(10));
    stock(db, "PS-SKU-B", dec!(10));
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "PS-SKU-A".to_string(),
                    name: "A".to_string(),
                    quantity: 5,
                    unit_price: dec!(10.00),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "PS-SKU-B".to_string(),
                    name: "B".to_string(),
                    quantity: 2,
                    unit_price: dec!(4.00),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("create order");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance status");
    }
    order
}

fn line(order: &Order, sku: &str) -> stateset_core::OrderItem {
    order.items.iter().find(|i| i.sku == sku).cloned().expect("line present")
}

fn reservations(db: &SqliteDatabase, order_id: OrderId, sku: &str) -> Vec<(String, Decimal)> {
    let conn = db.conn().expect("conn");
    let mut stmt = conn
        .prepare(
            "SELECT r.status, r.quantity FROM inventory_reservations r
             JOIN inventory_items i ON i.id = r.item_id
             WHERE r.reference_type = 'order' AND r.reference_id = ? AND i.sku = ?
             ORDER BY r.status",
        )
        .expect("prepare");
    stmt.query_map(params![order_id.to_string(), sku], |row| {
        let status: String = row.get(0)?;
        let qty: String = row.get(1)?;
        Ok((status, qty.parse::<Decimal>().expect("decimal")))
    })
    .expect("query")
    .collect::<Result<Vec<_>, _>>()
    .expect("collect")
}

fn allocated(db: &SqliteDatabase, sku: &str) -> Decimal {
    let conn = db.conn().expect("conn");
    let qty: String = conn
        .query_row(
            "SELECT b.quantity_allocated FROM inventory_balances b
             JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = ?",
            [sku],
            |row| row.get(0),
        )
        .expect("balance");
    qty.parse().expect("decimal")
}

#[test]
fn partial_ship_sets_partially_shipped_and_per_line_quantities() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "partial@example.com");
    let a = line(&order, "PS-SKU-A");

    let shipped = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: Some("TRK-1".into()),
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 3 }]),
            },
        )
        .expect("partial ship");

    assert_eq!(shipped.status, OrderStatus::PartiallyShipped);
    assert_eq!(shipped.tracking_number.as_deref(), Some("TRK-1"));
    assert_eq!(line(&shipped, "PS-SKU-A").shipped_quantity, 3);
    assert_eq!(line(&shipped, "PS-SKU-A").remaining_to_ship(), 2);
    assert_eq!(line(&shipped, "PS-SKU-B").shipped_quantity, 0);

    // Reservation for SKU-A was split: 3 confirmed + 2 still pending; SKU-B untouched.
    let a_res = reservations(&db, order.id, "PS-SKU-A");
    assert_eq!(a_res, vec![("confirmed".to_string(), dec!(3)), ("pending".to_string(), dec!(2))]);
    assert_eq!(reservations(&db, order.id, "PS-SKU-B"), vec![("pending".to_string(), dec!(2))]);
    // Allocation is unchanged by confirmation (it was already allocated on reserve).
    assert_eq!(allocated(&db, "PS-SKU-A"), dec!(5));
}

#[test]
fn second_ship_completes_order() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "complete@example.com");
    let a = line(&order, "PS-SKU-A");
    let b = line(&order, "PS-SKU-B");

    db.orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 3 }]),
            },
        )
        .expect("first ship");

    let shipped = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![
                    ShipmentLineInput { order_item_id: a.id, quantity: 2 },
                    ShipmentLineInput { order_item_id: b.id, quantity: 2 },
                ]),
            },
        )
        .expect("second ship");

    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(line(&shipped, "PS-SKU-A").shipped_quantity, 5);
    assert_eq!(line(&shipped, "PS-SKU-B").shipped_quantity, 2);
    assert!(reservations(&db, order.id, "PS-SKU-A").iter().all(|(s, _)| s == "confirmed"));
    assert!(reservations(&db, order.id, "PS-SKU-B").iter().all(|(s, _)| s == "confirmed"));
}

#[test]
fn overship_is_rejected_and_rolled_back() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "overship@example.com");
    let a = line(&order, "PS-SKU-A");
    let b = line(&order, "PS-SKU-B");

    // Line B is valid, line A overships: nothing (not even B) may persist.
    let err = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: Some("TRK-BAD".into()),
                lines: Some(vec![
                    ShipmentLineInput { order_item_id: b.id, quantity: 2 },
                    ShipmentLineInput { order_item_id: a.id, quantity: 6 },
                ]),
            },
        )
        .expect_err("overship must fail");
    match err {
        CommerceError::ShipmentExceedsOrdered { order_item_id, requested, remaining } => {
            assert_eq!(order_item_id, a.id.into_uuid());
            assert_eq!(requested, 6);
            assert_eq!(remaining, 5);
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let after = db.orders().get(order.id).expect("get").expect("exists");
    assert_eq!(after.status, OrderStatus::Processing);
    assert_eq!(after.tracking_number, None);
    assert!(after.items.iter().all(|i| i.shipped_quantity == 0));
    assert_eq!(reservations(&db, order.id, "PS-SKU-B"), vec![("pending".to_string(), dec!(2))]);

    // Cumulative overship across calls is also rejected.
    db.orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 4 }]),
            },
        )
        .expect("ship 4");
    let err = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 2 }]),
            },
        )
        .expect_err("cumulative overship must fail");
    assert!(matches!(
        err,
        CommerceError::ShipmentExceedsOrdered { requested: 2, remaining: 1, .. }
    ));
}

#[test]
fn ship_rejects_foreign_line_and_non_positive_quantity() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "foreign@example.com");
    let other = processing_order(&db, "foreign-other@example.com");
    let a = line(&order, "PS-SKU-A");

    let err = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput {
                    order_item_id: line(&other, "PS-SKU-A").id,
                    quantity: 1,
                }]),
            },
        )
        .expect_err("foreign line");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let err = db
        .orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 0 }]),
            },
        )
        .expect_err("zero quantity");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[test]
fn legacy_ship_without_lines_ships_everything() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "legacy@example.com");

    let shipped = db.orders().ship(order.id, ShipOrder::default()).expect("ship all");
    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert!(shipped.items.iter().all(|i| i.shipped_quantity == i.quantity));
    assert!(reservations(&db, order.id, "PS-SKU-A").iter().all(|(s, _)| s == "confirmed"));
}

#[test]
fn legacy_status_update_to_shipped_ships_everything() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "legacy-status@example.com");

    let shipped = db
        .orders()
        .update(order.id, UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() })
        .expect("status flip");
    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert!(shipped.items.iter().all(|i| i.shipped_quantity == i.quantity));

    // After a partial ship, a plain status flip ships the remainder.
    let order2 = processing_order(&db, "legacy-status-2@example.com");
    let a = line(&order2, "PS-SKU-A");
    db.orders()
        .ship(
            order2.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 1 }]),
            },
        )
        .expect("partial");
    let done = db
        .orders()
        .update(order2.id, UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() })
        .expect("flip remainder");
    assert_eq!(done.status, OrderStatus::Shipped);
    assert!(done.items.iter().all(|i| i.shipped_quantity == i.quantity));
}

#[test]
fn partially_shipped_cannot_be_set_directly() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "direct@example.com");
    let err = db
        .orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::PartiallyShipped), ..Default::default() },
        )
        .expect_err("must reject");
    assert!(matches!(err, CommerceError::ValidationError(_)));
}

#[test]
fn returns_validate_against_shipped_quantity() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = processing_order(&db, "returns@example.com");
    let a = line(&order, "PS-SKU-A");
    let b = line(&order, "PS-SKU-B");

    db.orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a.id, quantity: 3 }]),
            },
        )
        .expect("partial ship");

    // 4 > 3 shipped: rejected even though 5 were ordered.
    let err = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: a.id,
                quantity: 4,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect_err("over shipped qty");
    // Typed invariant error carrying the stable code an agent branches on.
    assert!(
        matches!(
            err,
            CommerceError::ReturnExceedsReturnable {
                basis: "shipped",
                returnable: 3,
                already_returned: 0,
                requested: 4,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(err.invariant_code(), Some("commerce.return.exceeds_shipped"));
    assert!(err.to_string().contains("3 shipped"), "{err}");

    // Unshipped line: nothing returnable.
    let err = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: b.id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect_err("unshipped line");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "{err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.return.exceeds_shipped"));

    // 3 <= 3 shipped: accepted.
    db.returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: a.id,
                quantity: 3,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("return within shipped qty");

    // Cumulative: nothing left.
    let err = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: a.id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect_err("cumulative");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "{err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.return.exceeds_shipped"));
}

#[test]
fn migration_backfills_shipped_quantity_for_shipped_orders() {
    // Simulate a pre-067 database: a minimal `orders`/`order_items` schema with
    // legacy rows, and every migration before 067 marked as already applied so
    // the runner executes only 067 (ALTER + backfill) against it.
    let mut conn = rusqlite::Connection::open_in_memory().expect("sqlite");
    conn.execute_batch("PRAGMA foreign_keys = OFF").expect("pragma");
    conn.execute_batch(
        "CREATE TABLE customers (id TEXT PRIMARY KEY, email TEXT, first_name TEXT, last_name TEXT);
         CREATE TABLE orders (id TEXT PRIMARY KEY, order_number TEXT, customer_id TEXT, status TEXT NOT NULL);
         CREATE TABLE order_items (id TEXT PRIMARY KEY, order_id TEXT NOT NULL, quantity INTEGER NOT NULL);",
    )
    .expect("scratch schema");
    // Mark EVERY migration except 067 as already applied, so the runner executes
    // exactly 067 against this minimal schema. Filtering (rather than taking the
    // prefix before 067) keeps the test isolated when later migrations are added:
    // those touch tables this fixture deliberately does not create.
    conn.execute_batch(
        "CREATE TABLE _migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, checksum TEXT, applied_at TEXT NOT NULL DEFAULT (datetime('now')));",
    )
    .expect("migrations table");
    let names = stateset_db::migrations::known_migration_names();
    for name in names.iter().filter(|n| **n != "067_order_item_shipped_quantity") {
        conn.execute("INSERT INTO _migrations (name, checksum) VALUES (?, '')", [name])
            .expect("mark applied");
    }
    conn.execute_batch(
        "INSERT INTO orders (id, order_number, customer_id, status) VALUES
            ('o-shipped', 'ORD-1', 'c', 'shipped'),
            ('o-delivered', 'ORD-2', 'c', 'delivered'),
            ('o-processing', 'ORD-3', 'c', 'processing');
         INSERT INTO order_items (id, order_id, quantity) VALUES
            ('i1', 'o-shipped', 4), ('i2', 'o-delivered', 2), ('i3', 'o-processing', 7);",
    )
    .expect("legacy rows");

    run_migrations(&mut conn).expect("run migrations");

    let shipped: Vec<(String, i64)> = conn
        .prepare("SELECT id, shipped_quantity FROM order_items ORDER BY id")
        .expect("prepare")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("collect");
    assert_eq!(shipped, vec![("i1".to_string(), 4), ("i2".to_string(), 2), ("i3".to_string(), 0)]);
}
