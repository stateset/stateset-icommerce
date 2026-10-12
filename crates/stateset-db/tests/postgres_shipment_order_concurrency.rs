#![cfg(feature = "postgres")]
//! Concurrency regression: shipment status updates that sync the order must not
//! deadlock with an order cancel that cascades to shipments. Both paths lock
//! the ORDER row before any SHIPMENT rows; opposing batches preserve that order.
//!
//! Requires `POSTGRES_URL` / `DATABASE_URL`; skipped otherwise.

use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateShipment,
    FulfillmentStatus, OrderStatus, ProductId, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use rust_decimal_macros::dec;
use std::env;
use std::sync::Arc;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())?;
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

#[tokio::test]
async fn postgres_ship_vs_cancel_do_not_deadlock() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(db);

    // Seed minimal catalog and order (two units, fully reserved).
    let sku = format!("PG-DEADLOCK-{}", Uuid::new_v4().simple());
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .await
        .expect("inventory");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("deadlock-{}@example.com", Uuid::new_v4().simple()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("customer");
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 2,
                unit_price: dec!(1),
                ..Default::default()
            }],
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
    assert_eq!(
        db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap().fulfillment_status,
        FulfillmentStatus::Unfulfilled
    );

    // Create one shipment covering both units.
    let shipment = db
        .shipments()
        .create_async(CreateShipment {
            order_id: order.id,
            recipient_name: "Concurrent".into(),
            shipping_address: "1 Test Way".into(),
            items: None, // itemless: carries remainder
            ..Default::default()
        })
        .await
        .expect("shipment")
        .id
        .into_uuid();

    // Race: ship the shipment (which ships the order) vs cancel the order (which cancels shipments).
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let db_ship = Arc::clone(&db);
    let db_cancel = Arc::clone(&db);
    let order_id = order.id.into_uuid();
    let ship_task = {
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            db_ship.shipments().ship_async(shipment, Some("1Z-DEADLOCK".into())).await
        })
    };
    let cancel_task = {
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            db_cancel
                .orders()
                .update_async(order_id, UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() })
                .await
        })
    };

    // Both complete without hanging; exactly one should succeed.
    let ship_res = ship_task.await.expect("join ship task");
    let cancel_res = cancel_task.await.expect("join cancel task");

    // Accept either outcome deterministically: if ship wins, cancel is refused; if cancel
    // wins, ship is refused (by order status guard). No deadlock means we got here.
    match (ship_res, cancel_res) {
        (Ok(_shipment), Err(CommerceError::Conflict(_) | CommerceError::OrderCannotBeCancelled(_))) => {}
        (Err(CommerceError::ValidationError(_) | CommerceError::Conflict(_)), Ok(_order)) => {}
        other => panic!("unexpected outcomes: {:?}", other),
    }
}

