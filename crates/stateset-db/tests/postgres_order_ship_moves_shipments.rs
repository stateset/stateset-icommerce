#![cfg(feature = "postgres")]
//! PostgreSQL twin of the SQLite rule in `sqlite_partial_shipments.rs`: an
//! order that ships in full carries its open shipment records (`pending`,
//! `processing`, `ready_to_ship`) to `shipped`, a shipment without a tracking
//! number adopts the order's, one with its own keeps it, a hold blocks
//! the completing ship, cancellations are left alone, and a partial shipment moves nothing.

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CreateShipment, CustomerId, Order, OrderStatus,
    ProductId, ShipOrder, ShipmentLineInput, ShipmentStatus, ShippingCarrier, UpdateOrder,
    UpdateShipment,
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

async fn customer(db: &PostgresDatabase) -> CustomerId {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("pg-follow-{}@example.com", Uuid::new_v4()),
            first_name: "Pg".into(),
            last_name: "Follow".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
        .id
}

/// A two-unit, single-line order advanced to `Processing`.
async fn processing_order(db: &PostgresDatabase) -> Order {
    let customer_id = customer(db).await;
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PG-FOLLOW-{}", Uuid::new_v4()),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance status");
    }
    order
}

async fn open_shipment(db: &PostgresDatabase, order: &Order, name: &str) -> Uuid {
    db.shipments()
        .create_async(CreateShipment {
            order_id: order.id,
            recipient_name: name.into(),
            shipping_address: "1 Main St".into(),
            carrier: Some(ShippingCarrier::Ups),
            ..Default::default()
        })
        .await
        .expect("create shipment")
        .id
        .into_uuid()
}

async fn load(db: &PostgresDatabase, id: Uuid) -> stateset_core::Shipment {
    db.shipments().get_async(id).await.expect("load").expect("shipment")
}

async fn facts(db: &PostgresDatabase, id: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT payload->>'status' FROM kernel_outbox
         WHERE event_type = 'shipment.status_changed' AND aggregate_id = $1
         ORDER BY created_at, id",
    )
    .bind(id.to_string())
    .fetch_all(db.pool())
    .await
    .expect("shipment facts")
}

#[tokio::test]
async fn postgres_full_order_ship_carries_open_shipments_but_partial_does_not() {
    let db = require_db!();
    let order = processing_order(&db).await;
    let pending = open_shipment(&db, &order, "Pending").await;
    let processing = open_shipment(&db, &order, "Processing").await;
    db.shipments().mark_processing_async(processing).await.expect("processing");
    let ready = open_shipment(&db, &order, "Ready").await;
    db.shipments().mark_processing_async(ready).await.expect("processing");
    db.shipments().mark_ready_async(ready).await.expect("ready");
    let held = open_shipment(&db, &order, "Held").await;
    db.shipments().hold_async(held).await.expect("hold");
    let cancelled = open_shipment(&db, &order, "Cancelled").await;
    db.shipments().cancel_async(cancelled).await.expect("cancel");
    let labelled = open_shipment(&db, &order, "Labelled").await;
    db.shipments()
        .update_async(
            labelled,
            UpdateShipment { tracking_number: Some("OWN-LABEL".into()), ..Default::default() },
        )
        .await
        .expect("label");

    // A partial shipment cannot say which package moved: nothing follows.
    let line = order.items[0].id;
    let partial = db
        .orders()
        .ship_async(
            order.id.into_uuid(),
            ShipOrder {
                tracking_number: Some("1Z-PG-PARTIAL".into()),
                lines: Some(vec![ShipmentLineInput { order_item_id: line, quantity: 1 }]),
            },
        )
        .await
        .expect("partial ship");
    assert_eq!(partial.status, OrderStatus::PartiallyShipped);
    for id in [pending, processing, ready, labelled] {
        assert_ne!(
            load(&db, id).await.status,
            ShipmentStatus::Shipped,
            "a partial ship moved an open shipment"
        );
        assert!(facts(&db, id).await.is_empty());
    }
    assert_eq!(load(&db, pending).await.tracking_number, None);

    // A hold blocks the ship that would complete the order: nothing moves.
    let err = db
        .orders()
        .ship_async(order.id.into_uuid(), ShipOrder { tracking_number: None, lines: None })
        .await
        .expect_err("held shipment blocks completing the order");
    assert!(matches!(err, stateset_core::CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(load(&db, pending).await.status, ShipmentStatus::Pending);
    db.shipments().cancel_async(held).await.expect("cancel the hold");

    let shipped = db
        .orders()
        .ship_async(
            order.id.into_uuid(),
            ShipOrder { tracking_number: Some("1Z-PG-FOLLOW".into()), lines: None },
        )
        .await
        .expect("ship remainder");
    assert_eq!(shipped.status, OrderStatus::Shipped);

    for id in [pending, processing, ready] {
        let moved = load(&db, id).await;
        assert_eq!(
            moved.status,
            ShipmentStatus::Shipped,
            "an open shipment should follow the order"
        );
        assert_eq!(moved.tracking_number.as_deref(), Some("1Z-PG-FOLLOW"));
        assert!(moved.tracking_url.as_deref().is_some_and(|u| u.contains("1Z-PG-FOLLOW")));
        assert!(moved.shipped_at.is_some());
        assert_eq!(facts(&db, id).await, vec!["shipped".to_string()]);
    }
    assert_eq!(load(&db, held).await.status, ShipmentStatus::Cancelled);
    assert!(facts(&db, held).await.is_empty());
    assert_eq!(load(&db, cancelled).await.status, ShipmentStatus::Cancelled);
    assert!(facts(&db, cancelled).await.is_empty());
    let own = load(&db, labelled).await;
    assert_eq!(own.status, ShipmentStatus::Shipped);
    assert_eq!(own.tracking_number.as_deref(), Some("OWN-LABEL"));
}

#[tokio::test]
async fn postgres_status_update_to_shipped_carries_open_shipments() {
    let db = require_db!();
    let order = processing_order(&db).await;
    let pending = open_shipment(&db, &order, "Pending").await;
    db.orders()
        .update_async(
            order.id.into_uuid(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await
        .expect("ship via status update");
    let moved = load(&db, pending).await;
    assert_eq!(moved.status, ShipmentStatus::Shipped);
    assert!(moved.shipped_at.is_some());
}
