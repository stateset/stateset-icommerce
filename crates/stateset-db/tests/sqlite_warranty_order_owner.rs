//! A warranty sold against an order belongs to that order's customer.
//!
//! `create` used to store whatever `customer_id` the input carried, and every
//! caller that only knew the order (the HTTP route takes `order_id` and never
//! a customer) stored the nil UUID. The customer could then never see their own
//! warranty: ownership checks and `customer_id` list filters matched nothing.

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreateWarranty, CustomerId,
    CustomerRepository, OrderId, OrderRepository, ProductId, WarrantyFilter, WarrantyRepository,
};
use stateset_db::SqliteDatabase;

fn customer(db: &SqliteDatabase) -> CustomerId {
    let unique = uuid::Uuid::new_v4().to_string();
    db.customers()
        .create(CreateCustomer {
            email: format!("owner-{}@example.com", &unique[..8]),
            first_name: "Olive".into(),
            last_name: "Owner".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

fn order(db: &SqliteDatabase, customer_id: CustomerId) -> OrderId {
    db.orders()
        .create(CreateOrder {
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
        .expect("create order")
        .id
}

#[test]
fn a_warranty_on_an_order_takes_the_order_customer() {
    let db = SqliteDatabase::in_memory().expect("db");
    let owner = customer(&db);
    let order_id = order(&db, owner);

    let warranty = db
        .warranties()
        .create(CreateWarranty {
            customer_id: CustomerId::nil(),
            order_id: Some(order_id),
            ..Default::default()
        })
        .expect("create warranty");

    assert_eq!(warranty.customer_id, owner);
    let listed = db
        .warranties()
        .list(WarrantyFilter { customer_id: Some(owner), ..Default::default() })
        .expect("list");
    assert_eq!(listed.iter().map(|w| w.id).collect::<Vec<_>>(), vec![warranty.id]);
}

#[test]
fn a_matching_explicit_customer_is_accepted() {
    let db = SqliteDatabase::in_memory().expect("db");
    let owner = customer(&db);
    let order_id = order(&db, owner);

    let warranty = db
        .warranties()
        .create(CreateWarranty {
            customer_id: owner,
            order_id: Some(order_id),
            ..Default::default()
        })
        .expect("create warranty");
    assert_eq!(warranty.customer_id, owner);
}

#[test]
fn a_different_customer_than_the_order_is_refused() {
    let db = SqliteDatabase::in_memory().expect("db");
    let owner = customer(&db);
    let other = customer(&db);
    let order_id = order(&db, owner);

    let err = db
        .warranties()
        .create(CreateWarranty {
            customer_id: other,
            order_id: Some(order_id),
            ..Default::default()
        })
        .expect_err("customer mismatch must be refused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let stored = db.warranties().list(WarrantyFilter::default()).expect("list");
    assert!(stored.is_empty(), "a refused create writes nothing");
}

#[test]
fn an_unknown_order_is_not_found() {
    let db = SqliteDatabase::in_memory().expect("db");
    let err = db
        .warranties()
        .create(CreateWarranty {
            customer_id: CustomerId::nil(),
            order_id: Some(OrderId::new()),
            ..Default::default()
        })
        .expect_err("unknown order");
    assert!(matches!(err, CommerceError::OrderNotFound(_)), "{err:?}");
}

#[test]
fn a_warranty_without_an_order_keeps_its_customer() {
    let db = SqliteDatabase::in_memory().expect("db");
    let owner = customer(&db);
    let warranty = db
        .warranties()
        .create(CreateWarranty { customer_id: owner, ..Default::default() })
        .expect("create warranty");
    assert_eq!(warranty.customer_id, owner);
}

#[test]
fn the_atomic_batch_resolves_each_owner_and_is_all_or_nothing() {
    let db = SqliteDatabase::in_memory().expect("db");
    let first = customer(&db);
    let second = customer(&db);
    let first_order = order(&db, first);
    let second_order = order(&db, second);

    let created = db
        .warranties()
        .create_batch_atomic(vec![
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
        .expect("batch");
    assert_eq!(created.iter().map(|w| w.customer_id).collect::<Vec<_>>(), vec![first, second]);

    let err = db
        .warranties()
        .create_batch_atomic(vec![
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
        .expect_err("second input names the wrong customer");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let stored = db.warranties().list(WarrantyFilter::default()).expect("list");
    assert_eq!(stored.len(), 2, "the refused batch wrote nothing");
}
