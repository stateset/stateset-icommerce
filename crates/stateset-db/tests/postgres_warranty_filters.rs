//! Postgres parity for `warranties::list`/`count` filters.
//!
//! Both backends previously applied only `customer_id`/`status`/`active_only`,
//! silently dropping `order_id`, `product_id`, `sku`, `serial_number`, and
//! `warranty_type` from `WarrantyFilter`. This locks in that Postgres now applies
//! them and that a filtered `count_async` matches the filtered `list_async`.
//!
//! `order_id` is exercised by the SQLite unit test; here it is left unset to
//! avoid the `warranties.order_id -> orders(id)` FK, and its bind path is
//! identical to `product_id` (both `Uuid` via `into_uuid()`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateCustomer, CreateWarranty, ProductId, WarrantyFilter, WarrantyType};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_warranty_list_and_count_filter_by_line_item_fields() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping warranty filter test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    // warranties.customer_id has a NOT NULL FK to customers(id).
    let unique = uuid::Uuid::new_v4().to_string();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("warr-{unique}@example.com"),
            first_name: "Warr".into(),
            last_name: "Anty".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let warranties = db.warranties();
    let prod_a = ProductId::new();

    // Distinct sku/serial keep this test isolated on a shared database.
    let sku_a = format!("SKU-{}", unique.replace('-', ""));
    let serial_a = format!("SN-{}", unique.replace('-', ""));

    let target = warranties
        .create_async(CreateWarranty {
            customer_id: customer.id,
            order_id: None,
            order_item_id: None,
            product_id: Some(prod_a),
            sku: Some(sku_a.clone()),
            serial_number: Some(serial_a.clone()),
            warranty_type: Some(WarrantyType::Extended),
            provider: None,
            coverage_description: None,
            purchase_date: None,
            start_date: None,
            end_date: None,
            duration_months: Some(12),
            max_coverage_amount: None,
            deductible: None,
            max_claims: Some(2),
            terms: None,
            notes: None,
        })
        .await
        .expect("create target warranty");

    // A second warranty for the same customer with different line-item attrs.
    warranties
        .create_async(CreateWarranty {
            customer_id: customer.id,
            order_id: None,
            order_item_id: None,
            product_id: Some(ProductId::new()),
            sku: Some(format!("OTHER-{}", unique.replace('-', ""))),
            serial_number: Some(format!("OTHERSN-{}", unique.replace('-', ""))),
            warranty_type: Some(WarrantyType::Standard),
            provider: None,
            coverage_description: None,
            purchase_date: None,
            start_date: None,
            end_date: None,
            duration_months: Some(12),
            max_coverage_amount: None,
            deductible: None,
            max_claims: Some(2),
            terms: None,
            notes: None,
        })
        .await
        .expect("create other warranty");

    // Scope every filter to this customer so a shared DB stays deterministic.
    let cases: Vec<(&str, WarrantyFilter)> = vec![
        (
            "product_id",
            WarrantyFilter {
                customer_id: Some(customer.id),
                product_id: Some(prod_a),
                ..Default::default()
            },
        ),
        (
            "sku",
            WarrantyFilter {
                customer_id: Some(customer.id),
                sku: Some(sku_a.clone()),
                ..Default::default()
            },
        ),
        (
            "serial_number",
            WarrantyFilter {
                customer_id: Some(customer.id),
                serial_number: Some(serial_a.clone()),
                ..Default::default()
            },
        ),
        (
            "warranty_type",
            WarrantyFilter {
                customer_id: Some(customer.id),
                warranty_type: Some(WarrantyType::Extended),
                ..Default::default()
            },
        ),
    ];

    for (label, filter) in cases {
        let listed = warranties.list_async(filter.clone()).await.expect("list");
        assert_eq!(listed.len(), 1, "filter {label} must return exactly one row");
        assert_eq!(listed[0].id, target.id, "filter {label} returned the wrong warranty");
        assert_eq!(
            warranties.count_async(filter).await.expect("count"),
            1,
            "filter {label}: count must match the filtered list"
        );
    }
}
