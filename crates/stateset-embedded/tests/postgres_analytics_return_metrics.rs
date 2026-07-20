//! Postgres parity for `return_metrics` top-returned-products aggregation.
//!
//! Postgres grouped top-returned-products by `(sku, name)`, so a SKU returned
//! under two names (product renamed) fragmented into two rows — different unit
//! counts and ranking than SQLite (which groups by sku). Both now group by sku.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AnalyticsQuery, CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct, CreateReturn,
    CreateReturnItem, ItemCondition, Order, OrderStatus, ProductId, ReturnReason,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn delivered_order(
    commerce: &AsyncCommerce,
    customer_id: stateset_core::CustomerId,
    product_id: ProductId,
    sku: &str,
    name: &str,
) -> Order {
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id,
                sku: sku.into(),
                name: name.into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    for status in [
        OrderStatus::Confirmed,
        OrderStatus::Processing,
        OrderStatus::Shipped,
        OrderStatus::Delivered,
    ] {
        commerce.orders().update_status(order.id.into_uuid(), status).await.expect("advance");
    }
    commerce.orders().get(order.id.into_uuid()).await.expect("get").expect("order")
}

#[tokio::test]
async fn postgres_return_metrics_aggregates_units_by_sku_across_name_changes() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("ret-{unique}@example.com"),
            first_name: "Ret".into(),
            last_name: "Metrics".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let product = commerce
        .products()
        .create(CreateProduct { name: format!("P {unique}"), ..Default::default() })
        .await
        .expect("create product");
    let sku = format!("RET-DIV-{}", &unique[..8]);

    let o1 = delivered_order(&commerce, customer.id, product.id, &sku, "Widget").await;
    let o2 = delivered_order(&commerce, customer.id, product.id, &sku, "Widget Pro").await;

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
            .await
            .expect("create return");
    }

    let metrics = commerce
        .analytics()
        .return_metrics(AnalyticsQuery::default())
        .await
        .expect("return metrics");
    let rows: Vec<_> = metrics.top_returned_products.iter().filter(|p| p.sku == sku).collect();
    assert_eq!(
        rows.len(),
        1,
        "returned units for a SKU must aggregate into one row, not fragment by name: {rows:?}"
    );
    assert_eq!(rows[0].units_returned, 2);
}
