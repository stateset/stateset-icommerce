#![cfg(feature = "sqlite")]
//! Shipments, orders and stock agree in both directions (SQLite).
//!
//! - An order ship consumes the shipped units' reservations: on-hand and
//!   allocated both drop, a `shipment` movement is written, and a restocked
//!   return puts back exactly what left.
//! - Shipping a shipment ships its lines on the order; delivering every
//!   shipment delivers it. Units are never counted twice.
//! - A hold blocks the ship that would complete the order; a closed order
//!   takes no new shipment or shipment line.
//! - Cancelling an order cancels its shipments that never left, and is
//!   refused once one has.
//!
//! Postgres twin: `postgres_shipment_order_sync.rs`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreateShipment, CreateShipmentItem, CustomerRepository, InventoryRepository, Order,
    OrderRepository, OrderStatus, ProductId, ShipOrder, ShipmentLineInput, ShipmentRepository,
    ShipmentStatus, UpdateOrder,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

struct Fixture {
    db: SqliteDatabase,
    sku_a: String,
    sku_b: String,
}

/// Two tracked SKUs with 10 on hand each, and a `Confirmed` order for
/// 3 × A and 2 × B (both fully reserved).
fn fixture() -> (Fixture, Order) {
    let db = SqliteDatabase::in_memory().expect("db");
    let tag = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let (sku_a, sku_b) = (format!("SYNC-A-{tag}"), format!("SYNC-B-{tag}"));
    for sku in [&sku_a, &sku_b] {
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.clone(),
                name: sku.clone(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .expect("inventory item");
    }
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("sync-{tag}@example.com"),
            first_name: "Grace".into(),
            last_name: "Hopper".into(),
            ..Default::default()
        })
        .expect("customer");
    let line = |sku: &str, quantity| CreateOrderItem {
        product_id: ProductId::new(),
        sku: sku.into(),
        name: sku.into(),
        quantity,
        unit_price: dec!(5.00),
        ..Default::default()
    };
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![line(&sku_a, 3), line(&sku_b, 2)],
            ..Default::default()
        })
        .expect("order");
    db.orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .expect("confirm");
    (Fixture { db, sku_a, sku_b }, order)
}

impl Fixture {
    fn stock(&self, sku: &str) -> (Decimal, Decimal) {
        let level = self.db.inventory().get_stock(sku).expect("stock").expect("level");
        (level.total_on_hand, level.total_allocated)
    }

    fn order(&self, order: &Order) -> Order {
        self.db.orders().get(order.id).expect("get").expect("order")
    }

    fn shipment(&self, order: &Order, items: &[(usize, i32)]) -> stateset_core::Shipment {
        self.db
            .shipments()
            .create(CreateShipment {
                order_id: order.id,
                recipient_name: "Grace Hopper".into(),
                shipping_address: "1 Harbor Way".into(),
                items: Some(items.iter().map(|(l, q)| item(order, *l, *q)).collect()),
                ..Default::default()
            })
            .expect("shipment")
    }

    fn walk_to(&self, id: stateset_core::ShipmentId, target: ShipmentStatus) {
        let s = self.db.shipments();
        let steps = [
            ShipmentStatus::Processing,
            ShipmentStatus::ReadyToShip,
            ShipmentStatus::Shipped,
            ShipmentStatus::InTransit,
            ShipmentStatus::OutForDelivery,
            ShipmentStatus::Delivered,
        ];
        let current = s.get(id).expect("get").expect("shipment").status;
        let from = steps.iter().position(|step| *step == current).map_or(0, |i| i + 1);
        for step in steps.into_iter().skip(from) {
            match step {
                ShipmentStatus::Processing => s.mark_processing(id),
                ShipmentStatus::ReadyToShip => s.mark_ready(id),
                ShipmentStatus::Shipped => s.ship(id, Some("1Z-SYNC".into())),
                ShipmentStatus::InTransit => s.mark_in_transit(id),
                ShipmentStatus::OutForDelivery => s.mark_out_for_delivery(id),
                _ => s.mark_delivered(id),
            }
            .expect("advance shipment");
            if step == target {
                return;
            }
        }
    }

    /// Outbox event types recorded against one aggregate, oldest first.
    fn facts(&self, aggregate_id: &str) -> Vec<String> {
        let conn = self.db.conn().expect("conn");
        let mut stmt = conn
            .prepare("SELECT event_type FROM kernel_outbox WHERE aggregate_id = ? ORDER BY rowid")
            .expect("prepare");
        stmt.query_map([aggregate_id], |row| row.get(0))
            .expect("query")
            .collect::<rusqlite::Result<Vec<String>>>()
            .expect("collect")
    }

    fn order_reservations(&self, order: &Order) -> Vec<stateset_core::InventoryReservation> {
        self.db
            .inventory()
            .list_reservations_by_reference("order", &order.id.to_string())
            .expect("reservations")
    }
}

fn cancel() -> UpdateOrder {
    UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() }
}

fn item(order: &Order, line: usize, quantity: i32) -> CreateShipmentItem {
    let line = &order.items[line];
    CreateShipmentItem {
        order_item_id: Some(line.id.into_uuid()),
        product_id: None,
        sku: line.sku.clone(),
        name: line.name.clone(),
        quantity,
    }
}

#[test]
fn order_ship_consumes_reservations_and_on_hand() {
    let (f, order) = fixture();
    assert_eq!(f.stock(&f.sku_a), (dec!(10), dec!(3)));
    let a = order.items[0].id;
    f.db.orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a, quantity: 2 }]),
            },
        )
        .expect("partial ship");
    // Two units left the building; one is still held.
    assert_eq!(f.stock(&f.sku_a), (dec!(8), dec!(1)));
    assert_eq!(f.stock(&f.sku_b), (dec!(10), dec!(2)));

    let shipped = f.db.orders().ship(order.id, ShipOrder::default()).expect("ship remainder");
    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(f.stock(&f.sku_a), (dec!(7), dec!(0)));
    assert_eq!(f.stock(&f.sku_b), (dec!(8), dec!(0)));
    let reservations = f.order_reservations(&order);
    assert!(
        reservations.iter().all(|r| r.status == stateset_core::ReservationStatus::Fulfilled),
        "{reservations:?}"
    );
    for r in &reservations {
        assert!(
            f.facts(&r.id.to_string()).iter().any(|t| t == "inventory.reservation_fulfilled.v1"),
            "reservation {} has no fulfilled fact",
            r.id
        );
    }
    // The ledger explains on-hand: 10 received − 3 shipped.
    let item_id = f.db.inventory().get_item_by_sku(&f.sku_a).unwrap().unwrap().id;
    let movements: Decimal =
        f.db.inventory().get_transactions(item_id, 100).unwrap().iter().map(|t| t.quantity).sum();
    assert_eq!(movements, dec!(7));
}

#[test]
fn shipment_ship_and_delivery_move_the_order_without_double_counting() {
    let (f, order) = fixture();
    let first = f.shipment(&order, &[(0, 3)]);
    let second = f.shipment(&order, &[(1, 2)]);

    f.walk_to(first.id, ShipmentStatus::Shipped);
    let o = f.order(&order);
    assert_eq!(o.status, OrderStatus::PartiallyShipped, "confirmed walks through processing");
    assert_eq!((o.items[0].shipped_quantity, o.items[1].shipped_quantity), (3, 0));
    assert_eq!(f.stock(&f.sku_a), (dec!(7), dec!(0)));
    assert!(f.facts(&order.id.to_string()).iter().any(|t| t == "orders.updated.v1"));

    // Delivering one of two shipments does not deliver the order.
    f.walk_to(first.id, ShipmentStatus::Delivered);
    assert_eq!(f.order(&order).status, OrderStatus::PartiallyShipped);

    f.walk_to(second.id, ShipmentStatus::Shipped);
    let o = f.order(&order);
    assert_eq!(o.status, OrderStatus::Shipped);
    assert_eq!(o.fulfillment_status, stateset_core::FulfillmentStatus::Shipped);
    assert_eq!(f.stock(&f.sku_b), (dec!(8), dec!(0)));

    // Re-shipping through the order moves nothing more.
    let again = f.db.orders().ship(order.id, ShipOrder::default()).expect("idempotent ship");
    assert_eq!((again.items[0].shipped_quantity, again.items[1].shipped_quantity), (3, 2));
    assert_eq!(f.stock(&f.sku_a), (dec!(7), dec!(0)));

    f.walk_to(second.id, ShipmentStatus::Delivered);
    let o = f.order(&order);
    assert_eq!(o.status, OrderStatus::Delivered);
    assert_eq!(o.fulfillment_status, stateset_core::FulfillmentStatus::Delivered);
}

#[test]
fn shipment_ships_only_what_the_order_line_still_has_open() {
    let (f, order) = fixture();
    let s = f.shipment(&order, &[(0, 3)]);
    let a = order.items[0].id;
    f.db.orders()
        .ship(
            order.id,
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a, quantity: 2 }]),
            },
        )
        .expect("partial order ship");
    f.walk_to(s.id, ShipmentStatus::Shipped);
    let o = f.order(&order);
    assert_eq!(o.items[0].shipped_quantity, 3, "capped at the line's open units");
    assert_eq!(o.status, OrderStatus::PartiallyShipped);
    assert_eq!(f.stock(&f.sku_a), (dec!(7), dec!(0)));
}

#[test]
fn itemless_shipment_carries_the_unpromised_remainder() {
    let (f, order) = fixture();
    let itemized = f.shipment(&order, &[(1, 2)]);
    let whole =
        f.db.shipments()
            .create(CreateShipment {
                order_id: order.id,
                recipient_name: "Grace Hopper".into(),
                shipping_address: "1 Harbor Way".into(),
                ..Default::default()
            })
            .expect("itemless shipment");
    f.walk_to(whole.id, ShipmentStatus::Shipped);
    let o = f.order(&order);
    // Line B is promised to the itemized shipment.
    assert_eq!((o.items[0].shipped_quantity, o.items[1].shipped_quantity), (3, 0));
    assert_eq!(o.status, OrderStatus::PartiallyShipped);
    f.walk_to(itemized.id, ShipmentStatus::Shipped);
    assert_eq!(f.order(&order).status, OrderStatus::Shipped);
}

#[test]
fn hold_blocks_completing_ship_and_closed_orders_refuse_shipments() {
    let (f, order) = fixture();
    let held = f.shipment(&order, &[(0, 1)]);
    f.db.shipments().hold(held.id).expect("hold");
    let before = f.order(&order);
    let err = f.db.orders().ship(order.id, ShipOrder::default()).expect_err("held");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let after = f.order(&order);
    assert_eq!((after.status, after.version), (before.status, before.version));
    assert_eq!(f.stock(&f.sku_a), (dec!(10), dec!(3)), "a refused ship moves no stock");

    f.db.shipments().cancel(held.id).expect("cancel hold");
    f.db.orders().ship(order.id, ShipOrder::default()).expect("ship");

    let err =
        f.db.shipments()
            .create(CreateShipment {
                order_id: order.id,
                recipient_name: "Late".into(),
                shipping_address: "1 Harbor Way".into(),
                ..Default::default()
            })
            .expect_err("shipped order takes no shipment");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let err = f.db.shipments().add_item(held.id, item(&order, 0, 1)).expect_err("closed order");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[test]
fn cancel_cancels_open_shipments_and_is_refused_once_one_left() {
    let (f, order) = fixture();
    let pending = f.shipment(&order, &[(0, 1)]);
    let held = f.shipment(&order, &[(1, 1)]);
    f.db.shipments().hold(held.id).expect("hold");
    f.db.orders().update(order.id, cancel()).expect("cancel");
    for id in [pending.id, held.id] {
        let s = f.db.shipments().get(id).unwrap().unwrap();
        assert_eq!(s.status, ShipmentStatus::Cancelled);
        assert!(f.facts(&id.to_string()).iter().any(|t| t == "shipment.status_changed"));
    }
    assert_eq!(f.stock(&f.sku_a), (dec!(10), dec!(0)));
    let err = f.db.shipments().mark_processing(pending.id).expect_err("cancelled");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    // A second order whose shipment has left cannot be cancelled.
    let (g, order) = fixture();
    let s = g.shipment(&order, &[(0, 3), (1, 2)]);
    g.walk_to(s.id, ShipmentStatus::Shipped);
    let err = g.db.orders().update(order.id, cancel()).expect_err("left");
    assert!(
        matches!(err, CommerceError::Conflict(_) | CommerceError::OrderCannotBeCancelled(_)),
        "{err:?}"
    );
    assert_eq!(g.order(&order).status, OrderStatus::Shipped);
}

#[test]
fn cancel_is_refused_when_a_shipment_left_without_shipping_order_units() {
    // A shipment of an order this store does not track lines for can leave
    // without moving the order (legacy data); the cancel guard still holds.
    let (f, order) = fixture();
    let s = f.shipment(&order, &[(0, 1)]);
    f.walk_to(s.id, ShipmentStatus::ReadyToShip);
    let conn = f.db.conn().expect("conn");
    conn.execute("UPDATE shipments SET status = 'in_transit' WHERE id = ?", [s.id.to_string()])
        .expect("legacy row");
    drop(conn);
    let err = f.db.orders().update(order.id, cancel()).expect_err("in transit");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(f.order(&order).status, OrderStatus::Confirmed);
    assert_eq!(f.db.shipments().get(s.id).unwrap().unwrap().status, ShipmentStatus::InTransit);
}
