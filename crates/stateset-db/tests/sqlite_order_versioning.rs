#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CustomerRepository, OrderRepository,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

#[test]
fn sqlite_order_item_changes_increment_version_and_total() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");

    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "alice@example.com".to_string(),
            first_name: "Alice".to_string(),
            last_name: "Smith".to_string(),
            ..Default::default()
        })
        .expect("create customer");

    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-1".to_string(),
                name: "Item 1".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");

    let initial_version = order.version;

    let added_item = db
        .orders()
        .add_item(
            order.id,
            CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-2".to_string(),
                name: "Item 2".to_string(),
                quantity: 2,
                unit_price: dec!(5.00),
                ..Default::default()
            },
        )
        .expect("add item");

    let after_add = db
        .orders()
        .get(order.id)
        .expect("get order")
        .expect("order exists");
    assert_eq!(after_add.version, initial_version + 1);
    assert_eq!(after_add.total_amount, after_add.calculate_total());

    db.orders()
        .remove_item(order.id, added_item.id)
        .expect("remove item");

    let after_remove = db
        .orders()
        .get(order.id)
        .expect("get order")
        .expect("order exists");
    assert_eq!(after_remove.version, initial_version + 2);
    assert_eq!(after_remove.total_amount, after_remove.calculate_total());
}
