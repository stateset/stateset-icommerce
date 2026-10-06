//! Postgres twin of `sqlite_warranty_order_owner`: a warranty sold against an
//! order belongs to that order's customer, so a caller that only knows the
//! order (the HTTP route) no longer stores a nil-UUID owner.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreateWarranty, CustomerId,
    OrderId, ProductId, WarrantyFilter,
};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
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
    let unique = uuid::Uuid::new_v4().to_string();
    db.customers()
        .create_async(CreateCustomer {
            email: format!("owner-{}@example.com", &unique[..12]),
            first_name: "Olive".into(),
            last_name: "Owner".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
        .id
}

async fn order(db: &PostgresDatabase, customer_id: CustomerId) -> OrderId {
    db.orders()
        .create_async(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SKU-WARRANTY".to_string(),
                name: "Kettle".to_string(),
                quantity: 1,
                unit_price: dec!(40.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
        .id
}

async fn owned_by(db: &PostgresDatabase, customer_id: CustomerId) -> usize {
    db.warranties()
        .list_async(WarrantyFilter { customer_id: Some(customer_id), ..Default::default() })
        .await
        .expect("list")
        .len()
}

#[tokio::test]
async fn postgres_a_warranty_on_an_order_takes_the_order_customer() {
    let db = require_db!();
    let owner = customer(&db).await;
    let order_id = order(&db, owner).await;

    let warranty = db
        .warranties()
        .create_async(CreateWarranty {
            customer_id: CustomerId::nil(),
            order_id: Some(order_id),
            ..Default::default()
        })
        .await
        .expect("create warranty");

    assert_eq!(warranty.customer_id, owner);
    assert_eq!(owned_by(&db, owner).await, 1);
}

#[tokio::test]
async fn postgres_a_different_customer_than_the_order_is_refused() {
    let db = require_db!();
    let owner = customer(&db).await;
    let other = customer(&db).await;
    let order_id = order(&db, owner).await;

    let err = db
        .warranties()
        .create_async(CreateWarranty {
            customer_id: other,
            order_id: Some(order_id),
            ..Default::default()
        })
        .await
        .expect_err("customer mismatch must be refused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(owned_by(&db, other).await, 0);
    assert_eq!(owned_by(&db, owner).await, 0);
}

#[tokio::test]
async fn postgres_an_unknown_order_is_not_found() {
    let db = require_db!();
    let err = db
        .warranties()
        .create_async(CreateWarranty {
            customer_id: CustomerId::nil(),
            order_id: Some(OrderId::new()),
            ..Default::default()
        })
        .await
        .expect_err("unknown order");
    assert!(matches!(err, CommerceError::OrderNotFound(_)), "{err:?}");
}

#[tokio::test]
async fn postgres_a_warranty_without_an_order_keeps_its_customer() {
    let db = require_db!();
    let owner = customer(&db).await;
    let warranty = db
        .warranties()
        .create_async(CreateWarranty { customer_id: owner, ..Default::default() })
        .await
        .expect("create warranty");
    assert_eq!(warranty.customer_id, owner);
}

#[tokio::test]
async fn postgres_the_atomic_batch_resolves_each_owner_and_is_all_or_nothing() {
    let db = require_db!();
    let first = customer(&db).await;
    let second = customer(&db).await;
    let first_order = order(&db, first).await;
    let second_order = order(&db, second).await;

    let created = db
        .warranties()
        .create_batch_atomic_async(vec![
            CreateWarranty {
                customer_id: CustomerId::nil(),
                order_id: Some(first_order),
                ..Default::default()
            },
            CreateWarranty {
                customer_id: CustomerId::nil(),
                order_id: Some(second_order),
                ..Default::default()
            },
        ])
        .await
        .expect("batch");
    assert_eq!(created.iter().map(|w| w.customer_id).collect::<Vec<_>>(), vec![first, second]);

    let err = db
        .warranties()
        .create_batch_atomic_async(vec![
            CreateWarranty {
                customer_id: CustomerId::nil(),
                order_id: Some(first_order),
                ..Default::default()
            },
            CreateWarranty {
                customer_id: first,
                order_id: Some(second_order),
                ..Default::default()
            },
        ])
        .await
        .expect_err("second input names the wrong customer");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(owned_by(&db, first).await, 1, "the refused batch wrote nothing");
    assert_eq!(owned_by(&db, second).await, 1);
}
