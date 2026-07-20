//! Postgres parity for the `complete_pick` idempotency + over-pick guard.
//!
//! Completing a pick folds into the wave's `completed_pick_count`. The Postgres
//! path historically ran the pick UPDATE and the counter increment as separate
//! non-transactional statements with no prior-status check and no
//! `quantity_picked <= quantity_requested` guard — so a duplicate completion
//! (a double-scan) double-incremented the wave counter, and a worker could pick
//! more than was requested. Both are now guarded inside one transaction
//! (`SELECT ... FOR UPDATE`), matching the SQLite backend.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CompletePick, CreateCustomer, CreateLocation, CreateOrder, CreateOrderItem,
    CreatePickTask, CreateProduct, CreateWarehouse, CreateWave, LocationType, OrderItemId,
    WarehouseType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_complete_pick_is_idempotent_and_rejects_over_pick() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &unique[..8].to_uppercase()),
            name: "Pick WH".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .await
        .expect("create warehouse");
    let location = commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            location_type: LocationType::Pick,
            zone: Some("A".into()),
            aisle: Some("01".into()),
            rack: Some("01".into()),
            bin: Some("01".into()),
            ..Default::default()
        })
        .await
        .expect("create location");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("pick-{unique}@example.com"),
            first_name: "Pick".into(),
            last_name: "Er".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let product = commerce
        .products()
        .create(CreateProduct { name: format!("Pickable {unique}"), ..Default::default() })
        .await
        .expect("create product");
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: "PICK-SKU".into(),
                name: "Pickable".into(),
                quantity: 5,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    let wave = commerce
        .fulfillment()
        .create_wave(CreateWave {
            warehouse_id: warehouse.id,
            order_ids: vec![order.id],
            priority: Some(1),
            notes: None,
            created_by: None,
        })
        .await
        .expect("create wave");
    let pick = commerce
        .fulfillment()
        .create_pick(CreatePickTask {
            wave_id: Some(wave.id),
            order_id: order.id,
            order_item_id: OrderItemId::new(),
            warehouse_id: warehouse.id,
            sku: "PICK-SKU".into(),
            product_name: Some("Pickable".into()),
            source_location_id: location.id,
            quantity_requested: dec!(5),
            lot_id: None,
            serial_number: None,
            priority: Some(1),
            notes: None,
        })
        .await
        .expect("create pick");

    let complete = |qty| CompletePick {
        pick_id: pick.id,
        quantity_picked: qty,
        quantity_short: None,
        short_reason: None,
        lot_id: None,
        serial_number: None,
        completed_by: None,
    };

    // Over-pick guard.
    let err = commerce
        .fulfillment()
        .complete_pick(complete(dec!(6)))
        .await
        .expect_err("over-pick must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Normal completion → wave counter 1.
    commerce.fulfillment().complete_pick(complete(dec!(5))).await.expect("complete pick");
    let wave_1 =
        commerce.fulfillment().get_wave(wave.id.into()).await.expect("get").expect("exists");
    assert_eq!(wave_1.completed_pick_count, 1);

    // Duplicate completion is idempotent — no double-increment.
    commerce.fulfillment().complete_pick(complete(dec!(5))).await.expect("idempotent re-complete");
    let wave_2 =
        commerce.fulfillment().get_wave(wave.id.into()).await.expect("get").expect("exists");
    assert_eq!(
        wave_2.completed_pick_count, 1,
        "duplicate complete_pick must not double-increment the wave counter"
    );
}
