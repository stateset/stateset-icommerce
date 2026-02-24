#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CustomerId, OrderStatus, ProductId, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
async fn setup_db() -> Option<PostgresDatabase> {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres versioning test");
            return None;
        }
    };

    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

#[cfg(feature = "postgres")]
async fn create_customer(db: &PostgresDatabase, email: &str) -> stateset_core::Customer {
    db.customers()
        .create_async(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
}

#[cfg(feature = "postgres")]
async fn create_order(db: &PostgresDatabase, customer_id: CustomerId) -> stateset_core::Order {
    db.orders()
        .create_async(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "VER-SKU-001".to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_order_item_changes_increment_version_and_total() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("version-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id).await;
    let initial_version = order.version;

    let added_item = db
        .orders()
        .add_item_async(
            order.id,
            CreateOrderItem {
                product_id: ProductId::new(),
                sku: "VER-SKU-002".to_string(),
                name: "Widget".to_string(),
                quantity: 2,
                unit_price: dec!(5.00),
                ..Default::default()
            },
        )
        .await
        .expect("add order item");
    let after_add =
        db.orders().get_async(order.id).await.expect("get order").expect("order exists");

    assert_eq!(after_add.version, initial_version + 1);
    assert_eq!(after_add.total_amount, after_add.calculate_total());

    db.orders()
        .remove_item_async(order.id, added_item.id.into_uuid())
        .await
        .expect("remove order item");

    let after_remove = db
        .orders()
        .get_async(order.id)
        .await
        .expect("get order after removal")
        .expect("order exists");

    assert_eq!(after_remove.version, initial_version + 2);
    assert_eq!(after_remove.total_amount, after_remove.calculate_total());
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_order_status_update_increments_version() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer =
        create_customer(&db, &format!("status-version-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id).await;
    let initial_version = order.version;

    let updated = db
        .orders()
        .update_async(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order status");

    assert_eq!(updated.version, initial_version + 1);
    assert_eq!(updated.status, OrderStatus::Confirmed);
}
