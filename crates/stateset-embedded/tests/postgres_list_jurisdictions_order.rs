//! Postgres side of the `list_jurisdictions` ordering parity guard.
//!
//! `list_jurisdictions` must return a deterministic order that matches SQLite.
//! Postgres used to order only by `(level, name)`; it now orders by
//! `(country_code, COALESCE(state_code, ''), level, name)` like SQLite. This
//! asserts the country-first order (see
//! `sqlite/tax.rs::list_jurisdictions_orders_by_country_then_state`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateTaxJurisdiction, JurisdictionLevel, TaxJurisdictionFilter};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_list_jurisdictions_orders_by_country_then_state() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping list_jurisdictions order test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let tax = commerce.tax();

    // Country-order and name-order disagree: XA/"Zebra" vs XB/"Apple".
    tax.create_jurisdiction(CreateTaxJurisdiction {
        parent_id: None,
        name: "Zebra".into(),
        code: "XA-1".into(),
        level: JurisdictionLevel::State,
        country_code: "XA".into(),
        state_code: Some("X1".into()),
        county: None,
        city: None,
        postal_codes: vec![],
    })
    .await
    .expect("xa");
    tax.create_jurisdiction(CreateTaxJurisdiction {
        parent_id: None,
        name: "Apple".into(),
        code: "XB-1".into(),
        level: JurisdictionLevel::State,
        country_code: "XB".into(),
        state_code: Some("X2".into()),
        county: None,
        city: None,
        postal_codes: vec![],
    })
    .await
    .expect("xb");

    let all = tax.list_jurisdictions(TaxJurisdictionFilter::default()).await.expect("list");
    let mine: Vec<String> = all
        .iter()
        .filter(|j| j.country_code == "XA" || j.country_code == "XB")
        .map(|j| j.country_code.clone())
        .collect();
    assert_eq!(mine, vec!["XA", "XB"], "must order by country_code, not by name");
}
