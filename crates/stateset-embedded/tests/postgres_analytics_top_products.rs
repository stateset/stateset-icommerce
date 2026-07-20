//! Postgres parity for `top_products` revenue aggregation.
//!
//! Postgres grouped top-products revenue by `(product_id, sku, name)`, so a SKU
//! sold under two names (a product renamed mid-window) had its revenue split
//! into two rows — different figures and top-N ranking than SQLite (which groups
//! by SKU). Both now group by SKU.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{AnalyticsQuery, CreateCustomer, CreateOrder, CreateOrderItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_top_products_aggregates_revenue_by_sku_across_name_changes() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("top-{unique}@example.com"),
            first_name: "Top".into(),
            last_name: "Products".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let product = commerce
        .products()
        .create(stateset_core::CreateProduct { name: format!("P {unique}"), ..Default::default() })
        .await
        .expect("create product");
    let sku = format!("TOP-DIV-{}", &unique[..8]);

    for name in ["Widget", "Widget Pro"] {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: product.id,
                    sku: sku.clone(),
                    name: name.into(),
                    quantity: 1,
                    unit_price: dec!(100.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .expect("create order");
    }

    let top =
        commerce.analytics().top_products(AnalyticsQuery::default()).await.expect("top products");
    let rows: Vec<_> = top.iter().filter(|p| p.sku == sku).collect();
    assert_eq!(
        rows.len(),
        1,
        "a SKU's revenue must aggregate into a single row, not fragment by name: {rows:?}"
    );
    assert_eq!(rows[0].revenue, dec!(200.00), "revenue must be the summed total for the SKU");
    assert_eq!(rows[0].units_sold, 2);
}
