//! Regression for `top_products` revenue aggregation (SQLite).
//!
//! `order_items.name` is a per-line snapshot, so the same SKU can carry
//! different names across orders (e.g. a product renamed mid-window). The
//! "top products by revenue" report must aggregate revenue by SKU into one row,
//! not fragment it across name variations. (Postgres previously grouped by
//! `(product_id, sku, name)`, splitting a renamed product's revenue into
//! multiple rows and changing the top-N ranking; both backends now group by SKU.)

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_embedded::{AnalyticsQuery, Commerce, CreateCustomer, CreateOrder, CreateOrderItem};
use uuid::Uuid;

#[test]
fn top_products_aggregates_revenue_by_sku_across_name_changes() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("top-{}@example.com", Uuid::new_v4()),
            first_name: "Top".into(),
            last_name: "Products".into(),
            ..Default::default()
        })
        .expect("create customer");

    let product_id = stateset_core::ProductId::new();
    // Two orders for the same SKU, but with different line-item names (the
    // product was renamed between them).
    for name in ["Widget", "Widget Pro"] {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id,
                    sku: "TOP-DIV".into(),
                    name: name.into(),
                    quantity: 1,
                    unit_price: dec!(100.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create order");
    }

    let top = commerce.analytics().top_products(AnalyticsQuery::default()).expect("top products");
    let rows: Vec<_> = top.iter().filter(|p| p.sku == "TOP-DIV").collect();
    assert_eq!(
        rows.len(),
        1,
        "a SKU's revenue must aggregate into a single row, not fragment by name: {rows:?}"
    );
    assert_eq!(rows[0].revenue, dec!(200.00), "revenue must be the summed total for the SKU");
    assert_eq!(rows[0].units_sold, 2);
}
