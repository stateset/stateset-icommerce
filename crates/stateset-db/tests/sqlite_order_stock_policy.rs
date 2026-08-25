#![cfg(feature = "sqlite")]

//! `CreateOrder::stock_policy` on SQLite: `AllowBackorder` (default) keeps the
//! historical partial-reserve + backorder behaviour; `RejectIfInsufficient`
//! fails the whole order transaction with a typed `InsufficientStock` error and
//! leaves no trace (no order row, no reservation, unchanged inventory).

use rust_decimal_macros::dec;
use stateset_core::{
    BackorderFilter, BackorderRepository, CommerceError, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CustomerId, CustomerRepository, InventoryRepository, OrderFilter,
    OrderRepository, ProductId, StockPolicy,
};
use stateset_db::SqliteDatabase;

const SKU: &str = "POLICY-SKU-001";

fn setup(initial_quantity: rust_decimal::Decimal) -> (SqliteDatabase, CustomerId) {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "stock-policy@example.com".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            ..Default::default()
        })
        .expect("create customer");
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: SKU.to_string(),
            name: "Policy Widget".to_string(),
            initial_quantity: Some(initial_quantity),
            ..Default::default()
        })
        .expect("create inventory item");
    (db, customer.id)
}

fn order_input(customer_id: CustomerId, quantity: i32, stock_policy: StockPolicy) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: vec![CreateOrderItem {
            product_id: ProductId::new(),
            sku: SKU.to_string(),
            name: "Policy Widget".to_string(),
            quantity,
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        stock_policy,
        ..Default::default()
    }
}

#[test]
fn stock_policy_defaults_to_allow_backorder() {
    assert_eq!(StockPolicy::default(), StockPolicy::AllowBackorder);
    assert_eq!(CreateOrder::default().stock_policy, StockPolicy::AllowBackorder);
    // Omitting the field on the wire must deserialize to the default.
    let parsed: CreateOrder = serde_json::from_value(serde_json::json!({
        "customer_id": CustomerId::new(),
        "items": [],
    }))
    .expect("deserialize without stock_policy");
    assert_eq!(parsed.stock_policy, StockPolicy::AllowBackorder);
    let parsed: CreateOrder = serde_json::from_value(serde_json::json!({
        "customer_id": CustomerId::new(),
        "items": [],
        "stock_policy": "reject_if_insufficient",
    }))
    .expect("deserialize with stock_policy");
    assert_eq!(parsed.stock_policy, StockPolicy::RejectIfInsufficient);
}

#[test]
fn allow_backorder_reserves_available_and_backorders_remainder() {
    let (db, customer_id) = setup(dec!(2));

    let order = db
        .orders()
        .create(order_input(customer_id, 5, StockPolicy::AllowBackorder))
        .expect("order is created under AllowBackorder");

    let reservations =
        db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
    assert_eq!(reservations.len(), 1, "the available quantity is reserved");
    assert_eq!(reservations[0].quantity, dec!(2));

    let backorders = db
        .backorder()
        .list_backorders(BackorderFilter {
            order_id: Some(order.id.into_uuid()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(backorders.len(), 1, "the shortfall becomes a backorder");
    assert_eq!(backorders[0].quantity_ordered, dec!(3));

    let stock = db.inventory().get_stock(SKU).unwrap().expect("stock level");
    assert_eq!(stock.total_available, dec!(0));
}

#[test]
fn reject_if_insufficient_fails_whole_order_and_rolls_back() {
    let (db, customer_id) = setup(dec!(2));

    let err = db
        .orders()
        .create(order_input(customer_id, 5, StockPolicy::RejectIfInsufficient))
        .expect_err("order must be rejected when stock is short");

    match err {
        CommerceError::InsufficientStock { sku, requested, available } => {
            assert_eq!(sku, SKU);
            assert_eq!(requested, "5");
            assert_eq!(available, "2");
        }
        other => panic!("expected InsufficientStock, got {other:?}"),
    }

    // Nothing persisted: no order row ...
    let orders = db.orders().list(OrderFilter::default()).unwrap();
    assert!(orders.is_empty(), "no order row must survive the rollback");
    // ... no backorder ...
    let backorders = db.backorder().list_backorders(BackorderFilter::default()).unwrap();
    assert!(backorders.is_empty(), "no backorder must survive the rollback");
    // ... and inventory is untouched.
    let stock = db.inventory().get_stock(SKU).unwrap().expect("stock level");
    assert_eq!(stock.total_available, dec!(2));
    assert_eq!(stock.total_allocated, dec!(0));
}

#[test]
fn reject_if_insufficient_creates_order_when_fully_in_stock() {
    let (db, customer_id) = setup(dec!(5));

    let order = db
        .orders()
        .create(order_input(customer_id, 5, StockPolicy::RejectIfInsufficient))
        .expect("order is created when stock covers the request");

    let reservations =
        db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
    assert_eq!(reservations.len(), 1);
    assert_eq!(reservations[0].quantity, dec!(5));

    let backorders = db
        .backorder()
        .list_backorders(BackorderFilter {
            order_id: Some(order.id.into_uuid()),
            ..Default::default()
        })
        .unwrap();
    assert!(backorders.is_empty(), "fully reserved orders never backorder");
}

#[test]
fn reject_if_insufficient_multi_line_rolls_back_earlier_reservations() {
    let (db, customer_id) = setup(dec!(5));
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "POLICY-SKU-002".to_string(),
            name: "Scarce Widget".to_string(),
            initial_quantity: Some(dec!(0)),
            ..Default::default()
        })
        .expect("create scarce item");

    let input = CreateOrder {
        customer_id,
        items: vec![
            CreateOrderItem {
                product_id: ProductId::new(),
                sku: SKU.to_string(),
                name: "Policy Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            },
            CreateOrderItem {
                product_id: ProductId::new(),
                sku: "POLICY-SKU-002".to_string(),
                name: "Scarce Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        ],
        stock_policy: StockPolicy::RejectIfInsufficient,
        ..Default::default()
    };

    let err = db.orders().create(input).expect_err("second line is short");
    assert!(
        matches!(err, CommerceError::InsufficientStock { ref sku, .. } if sku == "POLICY-SKU-002")
    );

    // The first line's reservation was rolled back with the rest of the tx.
    let stock = db.inventory().get_stock(SKU).unwrap().expect("stock level");
    assert_eq!(stock.total_available, dec!(5));
    assert!(db.orders().list(OrderFilter::default()).unwrap().is_empty());
}
