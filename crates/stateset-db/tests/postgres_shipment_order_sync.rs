#![cfg(feature = "postgres")]
//! PostgreSQL twin of `sqlite_shipment_order_sync.rs`: shipments, orders and
//! stock agree in both directions. Skipped without `POSTGRES_URL` /
//! `DATABASE_URL`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreateShipment, CreateShipmentItem, FulfillmentStatus, Order, OrderStatus, ProductId,
    ReservationStatus, ShipOrder, ShipmentLineInput, ShipmentStatus, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

macro_rules! require_db {
    () => {
        match connect().await {
            Some(db) => db,
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

struct Fixture {
    db: PostgresDatabase,
    sku_a: String,
    sku_b: String,
}

/// Two tracked SKUs with 10 on hand each, and a `Confirmed` order for
/// 3 × A and 2 × B (both fully reserved).
async fn fixture(db: PostgresDatabase) -> (Fixture, Order) {
    let tag = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let (sku_a, sku_b) = (format!("PG-SYNC-A-{tag}"), format!("PG-SYNC-B-{tag}"));
    for sku in [&sku_a, &sku_b] {
        db.inventory()
            .create_item_async(CreateInventoryItem {
                sku: sku.clone(),
                name: sku.clone(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .await
            .expect("inventory item");
    }
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("pg-sync-{tag}@example.com"),
            first_name: "Grace".into(),
            last_name: "Hopper".into(),
            ..Default::default()
        })
        .await
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
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![line(&sku_a, 3), line(&sku_b, 2)],
            ..Default::default()
        })
        .await
        .expect("order");
    db.orders()
        .update_async(
            order.id.into_uuid(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("confirm");
    let order = canonical(order, &sku_a);
    (Fixture { db, sku_a, sku_b }, order)
}

/// Line A first: Postgres does not promise the order of lines written in one
/// transaction, and the tests address lines by position.
fn canonical(mut order: Order, sku_a: &str) -> Order {
    order.items.sort_by_key(|item| item.sku != sku_a);
    order
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

fn cancel() -> UpdateOrder {
    UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() }
}

impl Fixture {
    async fn stock(&self, sku: &str) -> (Decimal, Decimal) {
        let level = self.db.inventory().get_stock_async(sku).await.expect("stock").expect("level");
        (level.total_on_hand, level.total_allocated)
    }

    async fn order(&self, order: &Order) -> Order {
        let order =
            self.db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order");
        canonical(order, &self.sku_a)
    }

    async fn shipment(&self, order: &Order, items: &[(usize, i32)]) -> Uuid {
        self.db
            .shipments()
            .create_async(CreateShipment {
                order_id: order.id,
                recipient_name: "Grace Hopper".into(),
                shipping_address: "1 Harbor Way".into(),
                items: Some(items.iter().map(|(l, q)| item(order, *l, *q)).collect()),
                ..Default::default()
            })
            .await
            .expect("shipment")
            .id
            .into_uuid()
    }

    async fn walk_to(&self, id: Uuid, target: ShipmentStatus) {
        let s = self.db.shipments();
        let steps = [
            ShipmentStatus::Processing,
            ShipmentStatus::ReadyToShip,
            ShipmentStatus::Shipped,
            ShipmentStatus::InTransit,
            ShipmentStatus::OutForDelivery,
            ShipmentStatus::Delivered,
        ];
        let current = s.get_async(id).await.expect("get").expect("shipment").status;
        let from = steps.iter().position(|step| *step == current).map_or(0, |i| i + 1);
        for step in steps.into_iter().skip(from) {
            match step {
                ShipmentStatus::Processing => s.mark_processing_async(id).await,
                ShipmentStatus::ReadyToShip => s.mark_ready_async(id).await,
                ShipmentStatus::Shipped => s.ship_async(id, Some("1Z-PG-SYNC".into())).await,
                ShipmentStatus::InTransit => s.mark_in_transit_async(id).await,
                ShipmentStatus::OutForDelivery => s.mark_out_for_delivery_async(id).await,
                _ => s.mark_delivered_async(id).await,
            }
            .expect("advance shipment");
            if step == target {
                return;
            }
        }
    }

    async fn facts(&self, aggregate_id: &str) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT event_type FROM kernel_outbox WHERE aggregate_id = $1 ORDER BY created_at, id",
        )
        .bind(aggregate_id)
        .fetch_all(self.db.pool())
        .await
        .expect("facts")
    }
}

#[tokio::test]
async fn postgres_order_ship_consumes_reservations_and_on_hand() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    assert_eq!(f.stock(&f.sku_a).await, (dec!(10), dec!(3)));
    let a = order.items[0].id;
    f.db.orders()
        .ship_async(
            order.id.into_uuid(),
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a, quantity: 2 }]),
            },
        )
        .await
        .expect("partial ship");
    assert_eq!(f.stock(&f.sku_a).await, (dec!(8), dec!(1)));
    assert_eq!(f.stock(&f.sku_b).await, (dec!(10), dec!(2)));

    let shipped =
        f.db.orders().ship_async(order.id.into_uuid(), ShipOrder::default()).await.expect("ship");
    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(f.stock(&f.sku_a).await, (dec!(7), dec!(0)));
    assert_eq!(f.stock(&f.sku_b).await, (dec!(8), dec!(0)));
    let reservations =
        f.db.inventory()
            .list_reservations_by_reference_async("order", &order.id.to_string())
            .await
            .expect("reservations");
    assert!(reservations.iter().all(|r| r.status == ReservationStatus::Fulfilled));
    for r in &reservations {
        assert!(
            f.facts(&r.id.to_string())
                .await
                .iter()
                .any(|t| t == "inventory.reservation_fulfilled.v1"),
            "reservation {} has no fulfilled fact",
            r.id
        );
    }
    let item_id = f.db.inventory().get_item_by_sku_async(&f.sku_a).await.unwrap().unwrap().id;
    let movements: Decimal =
        f.db.inventory()
            .get_transactions_async(item_id, 100)
            .await
            .unwrap()
            .iter()
            .map(|t| t.quantity)
            .sum();
    assert_eq!(movements, dec!(7));
}

#[tokio::test]
async fn postgres_shipment_ship_and_delivery_move_the_order_without_double_counting() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let first = f.shipment(&order, &[(0, 3)]).await;
    let second = f.shipment(&order, &[(1, 2)]).await;

    f.walk_to(first, ShipmentStatus::Shipped).await;
    let o = f.order(&order).await;
    assert_eq!(o.status, OrderStatus::PartiallyShipped, "confirmed walks through processing");
    assert_eq!((o.items[0].shipped_quantity, o.items[1].shipped_quantity), (3, 0));
    assert_eq!(f.stock(&f.sku_a).await, (dec!(7), dec!(0)));
    assert!(f.facts(&order.id.to_string()).await.iter().any(|t| t == "orders.updated.v1"));

    f.walk_to(first, ShipmentStatus::Delivered).await;
    assert_eq!(f.order(&order).await.status, OrderStatus::PartiallyShipped);

    f.walk_to(second, ShipmentStatus::Shipped).await;
    let o = f.order(&order).await;
    assert_eq!(o.status, OrderStatus::Shipped);
    assert_eq!(o.fulfillment_status, FulfillmentStatus::Shipped);
    assert_eq!(f.stock(&f.sku_b).await, (dec!(8), dec!(0)));

    let again =
        f.db.orders().ship_async(order.id.into_uuid(), ShipOrder::default()).await.expect("ship");
    let again = canonical(again, &f.sku_a);
    assert_eq!((again.items[0].shipped_quantity, again.items[1].shipped_quantity), (3, 2));
    assert_eq!(f.stock(&f.sku_a).await, (dec!(7), dec!(0)));

    f.walk_to(second, ShipmentStatus::Delivered).await;
    let o = f.order(&order).await;
    assert_eq!(o.status, OrderStatus::Delivered);
    assert_eq!(o.fulfillment_status, FulfillmentStatus::Delivered);
}

#[tokio::test]
async fn postgres_shipment_ships_only_what_the_order_line_still_has_open() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let s = f.shipment(&order, &[(0, 3)]).await;
    let a = order.items[0].id;
    f.db.orders()
        .ship_async(
            order.id.into_uuid(),
            ShipOrder {
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: a, quantity: 2 }]),
            },
        )
        .await
        .expect("partial order ship");
    f.walk_to(s, ShipmentStatus::Shipped).await;
    let o = f.order(&order).await;
    assert_eq!(o.items[0].shipped_quantity, 3, "capped at the line's open units");
    assert_eq!(o.status, OrderStatus::PartiallyShipped);
    assert_eq!(f.stock(&f.sku_a).await, (dec!(7), dec!(0)));
}

#[tokio::test]
async fn postgres_itemless_shipment_carries_the_unpromised_remainder() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let itemized = f.shipment(&order, &[(1, 2)]).await;
    let whole =
        f.db.shipments()
            .create_async(CreateShipment {
                order_id: order.id,
                recipient_name: "Grace Hopper".into(),
                shipping_address: "1 Harbor Way".into(),
                ..Default::default()
            })
            .await
            .expect("itemless shipment")
            .id
            .into_uuid();
    f.walk_to(whole, ShipmentStatus::Shipped).await;
    let o = f.order(&order).await;
    assert_eq!((o.items[0].shipped_quantity, o.items[1].shipped_quantity), (3, 0));
    assert_eq!(o.status, OrderStatus::PartiallyShipped);
    f.walk_to(itemized, ShipmentStatus::Shipped).await;
    assert_eq!(f.order(&order).await.status, OrderStatus::Shipped);
}

#[tokio::test]
async fn postgres_hold_blocks_completing_ship_and_closed_orders_refuse_shipments() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let held = f.shipment(&order, &[(0, 1)]).await;
    f.db.shipments().hold_async(held).await.expect("hold");
    let before = f.order(&order).await;
    let err =
        f.db.orders()
            .ship_async(order.id.into_uuid(), ShipOrder::default())
            .await
            .expect_err("held");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let after = f.order(&order).await;
    assert_eq!((after.status, after.version), (before.status, before.version));
    assert_eq!(f.stock(&f.sku_a).await, (dec!(10), dec!(3)), "a refused ship moves no stock");

    f.db.shipments().cancel_async(held).await.expect("cancel hold");
    f.db.orders().ship_async(order.id.into_uuid(), ShipOrder::default()).await.expect("ship");
    let err =
        f.db.shipments()
            .create_async(CreateShipment {
                order_id: order.id,
                recipient_name: "Late".into(),
                shipping_address: "1 Harbor Way".into(),
                ..Default::default()
            })
            .await
            .expect_err("shipped order takes no shipment");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[tokio::test]
async fn postgres_cancel_cancels_open_shipments_and_is_refused_once_one_left() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let pending = f.shipment(&order, &[(0, 1)]).await;
    let held = f.shipment(&order, &[(1, 1)]).await;
    f.db.shipments().hold_async(held).await.expect("hold");
    f.db.orders().update_async(order.id.into_uuid(), cancel()).await.expect("cancel");
    for id in [pending, held] {
        let s = f.db.shipments().get_async(id).await.unwrap().unwrap();
        assert_eq!(s.status, ShipmentStatus::Cancelled);
        assert!(f.facts(&id.to_string()).await.iter().any(|t| t == "shipment.status_changed"));
    }
    assert_eq!(f.stock(&f.sku_a).await, (dec!(10), dec!(0)));
    let err = f.db.shipments().mark_processing_async(pending).await.expect_err("cancelled");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let db = require_db!();
    let (g, order) = fixture(db).await;
    let s = g.shipment(&order, &[(0, 3), (1, 2)]).await;
    g.walk_to(s, ShipmentStatus::Shipped).await;
    let err = g.db.orders().update_async(order.id.into_uuid(), cancel()).await.expect_err("left");
    assert!(
        matches!(err, CommerceError::Conflict(_) | CommerceError::OrderCannotBeCancelled(_)),
        "{err:?}"
    );
    assert_eq!(g.order(&order).await.status, OrderStatus::Shipped);
}

#[tokio::test]
async fn postgres_cancel_is_refused_when_a_shipment_left_without_shipping_order_units() {
    let db = require_db!();
    let (f, order) = fixture(db).await;
    let s = f.shipment(&order, &[(0, 1)]).await;
    f.walk_to(s, ShipmentStatus::ReadyToShip).await;
    sqlx::query("UPDATE shipments SET status = 'in_transit' WHERE id = $1")
        .bind(s)
        .execute(f.db.pool())
        .await
        .expect("legacy row");
    let err =
        f.db.orders().update_async(order.id.into_uuid(), cancel()).await.expect_err("in transit");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(f.order(&order).await.status, OrderStatus::Confirmed);
}
