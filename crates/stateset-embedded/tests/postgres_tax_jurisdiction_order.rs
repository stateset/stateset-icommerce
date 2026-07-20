//! Postgres parity for the deterministic tax-jurisdiction ordering (SQLite
//! covered by `calculate_tax_returns_jurisdictions_in_stable_order`). Both
//! backends must emit the jurisdictions summary in the same stable order (by
//! code) rather than an internal `HashMap` order, so the same input produces an
//! identical result regardless of backend.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateTaxJurisdiction, CreateTaxRate, CurrencyCode, JurisdictionLevel, ProductTaxCategory,
    TaxAddress, TaxCalculationRequest, TaxLineItem, TaxType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_calculate_tax_returns_jurisdictions_in_stable_order() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping tax jurisdiction order test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    // Country-level ZZ plus state-level ZZ-CA, each taxing the item. No US-*
    // pre-seeds collide with the ZZ country.
    let country = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "ZZ Country".into(),
            code: "ZZ".into(),
            level: JurisdictionLevel::Country,
            country_code: "ZZ".into(),
            state_code: None,
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .await
        .expect("create country");
    let state = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "ZZ California".into(),
            code: "ZZ-CA".into(),
            level: JurisdictionLevel::State,
            country_code: "ZZ".into(),
            state_code: Some("CA".into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .await
        .expect("create state");

    for (jur_id, rate) in [(country.id, dec!(0.05)), (state.id, dec!(0.03))] {
        commerce
            .tax()
            .create_rate(CreateTaxRate {
                jurisdiction_id: jur_id,
                tax_type: TaxType::SalesTax,
                product_category: ProductTaxCategory::Standard,
                rate,
                name: "Sales Tax".into(),
                description: None,
                is_compound: false,
                priority: 1,
                threshold_min: None,
                threshold_max: None,
                fixed_amount: None,
                effective_from: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
                effective_to: None,
            })
            .await
            .expect("create rate");
    }

    let request = TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "line-1".into(),
            sku: None,
            product_id: None,
            quantity: dec!(1),
            unit_price: dec!(100.00),
            discount_amount: rust_decimal::Decimal::ZERO,
            tax_category: ProductTaxCategory::Standard,
            tax_code: None,
            description: None,
        }],
        shipping_address: TaxAddress {
            line1: None,
            line2: None,
            city: None,
            state: Some("CA".into()),
            postal_code: None,
            country: "ZZ".into(),
        },
        billing_address: None,
        customer_id: None,
        shipping_amount: None,
        currency: CurrencyCode::USD,
        transaction_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
        prices_include_tax: false,
    };

    let result = commerce.tax().calculate_tax(request).await.expect("calc");
    let codes: Vec<&str> = result.jurisdictions.iter().map(|j| j.code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["ZZ", "ZZ-CA"],
        "Postgres must return jurisdictions in stable code order: {codes:?}"
    );
}
