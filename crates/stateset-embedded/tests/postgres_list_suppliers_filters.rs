//! Postgres side of the `list_suppliers` filter + pagination parity guard.
//!
//! Postgres previously applied only `active_only`, silently ignoring the `name`
//! and `country` filters, and had no `OFFSET`. It now honors name/country and
//! offset (matching SQLite). This asserts that behavior against a live database.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateSupplier, SupplierFilter};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn supplier(commerce: &AsyncCommerce, name: &str, country: &str) {
    commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: name.to_string(),
            supplier_code: None,
            contact_name: None,
            email: None,
            phone: None,
            website: None,
            address: None,
            city: None,
            state: None,
            postal_code: None,
            country: Some(country.to_string()),
            tax_id: None,
            payment_terms: None,
            currency: None,
            lead_time_days: Some(7),
            minimum_order: None,
            notes: None,
        })
        .await
        .expect("create supplier");
}

#[tokio::test]
async fn postgres_list_suppliers_honors_name_country_and_offset() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping list_suppliers filter test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let pos = commerce.purchase_orders();

    supplier(&commerce, "Acme Corp", "US").await;
    supplier(&commerce, "Acme Subsidiary", "CA").await;
    supplier(&commerce, "Globex", "US").await;

    // name filter (case-insensitive substring) — previously ignored on Postgres.
    let acmes = pos
        .list_suppliers(SupplierFilter { name: Some("acme".into()), ..Default::default() })
        .await
        .expect("by name");
    assert_eq!(acmes.len(), 2, "name filter must select the two Acme suppliers: {acmes:?}");

    // country filter — previously ignored on Postgres.
    let us = pos
        .list_suppliers(SupplierFilter { country: Some("US".into()), ..Default::default() })
        .await
        .expect("by country");
    assert_eq!(us.len(), 2, "country filter must select the two US suppliers: {us:?}");

    // offset — previously not applied on Postgres.
    let all = pos.list_suppliers(SupplierFilter::default()).await.expect("all");
    assert_eq!(all.len(), 3);
    let offset1 = pos
        .list_suppliers(SupplierFilter { offset: Some(1), ..Default::default() })
        .await
        .expect("offset");
    assert_eq!(offset1.len(), 2, "offset must skip rows");
}
