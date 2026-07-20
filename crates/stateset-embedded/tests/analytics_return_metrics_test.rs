//! Regression for `return_metrics` top-returned-products aggregation (SQLite).
//!
//! Like top-products, `return_items.name` is a per-line snapshot, so the same
//! SKU can appear under different names. The top-returned-products list must
//! aggregate returned units by SKU into one row, not fragment across names.
//! (Postgres previously grouped by `(sku, name)`.)

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_embedded::{
    AnalyticsQuery, Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreateReturn,
    CreateReturnItem, ItemCondition, Order, OrderStatus, ReturnReason,
};
use uuid::Uuid;

fn delivered_order(
    commerce: &Commerce,
    customer_id: stateset_core::CustomerId,
    name: &str,
) -> Order {
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "RET-DIV".into(),
                name: name.into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");
    for status in [
        OrderStatus::Confirmed,
        OrderStatus::Processing,
        OrderStatus::Shipped,
        OrderStatus::Delivered,
    ] {
        commerce.orders().update_status(order.id, status).expect("advance order");
    }
    commerce.orders().get(order.id).expect("get").expect("order")
}

#[test]
fn return_metrics_aggregates_units_by_sku_across_name_changes() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("ret-{}@example.com", Uuid::new_v4()),
            first_name: "Ret".into(),
            last_name: "Metrics".into(),
            ..Default::default()
        })
        .expect("create customer");

    // Two delivered orders for the same SKU under different names.
    let o1 = delivered_order(&commerce, customer.id, "Widget");
    let o2 = delivered_order(&commerce, customer.id, "Widget Pro");

    for order in [&o1, &o2] {
        commerce
            .returns()
            .create(CreateReturn {
                order_id: order.id,
                reason: ReturnReason::Defective,
                items: vec![CreateReturnItem {
                    order_item_id: order.items[0].id,
                    quantity: 1,
                    condition: Some(ItemCondition::Defective),
                }],
                ..Default::default()
            })
            .expect("create return");
    }

    let metrics =
        commerce.analytics().return_metrics(AnalyticsQuery::default()).expect("return metrics");
    let rows: Vec<_> =
        metrics.top_returned_products.iter().filter(|p| p.sku == "RET-DIV").collect();
    assert_eq!(
        rows.len(),
        1,
        "returned units for a SKU must aggregate into one row, not fragment by name: {rows:?}"
    );
    assert_eq!(rows[0].units_returned, 2);
}
