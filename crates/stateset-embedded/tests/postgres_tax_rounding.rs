//! Postgres parity for the configurable tax rounding mode (SQLite covered by
//! the unit test in sqlite/tax.rs). The `TaxSettings.rounding_mode` must
//! actually change how computed tax is rounded, on both backends.

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
async fn postgres_calculate_tax_honors_configured_rounding_mode() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping tax rounding test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    // Unique jurisdiction AND country per run: other parity binaries create
    // country-level "ZZ" rates on the shared database (see
    // postgres_tax_jurisdiction_order), which would stack onto this state rate
    // and double the tax. A per-run country keeps exactly one rate applicable.
    let unique = uuid::Uuid::new_v4().to_string();
    let state = format!("R{}", &unique[..6]).to_uppercase();
    let country: String = unique
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(2)
        .map(|c| char::from(b'G' + (c.to_digit(16).unwrap() as u8)))
        .collect();
    let jur = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: format!("Test State {state}"),
            code: format!("{country}-{state}"),
            level: JurisdictionLevel::State,
            country_code: country.clone(),
            state_code: Some(state.clone()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .await
        .expect("create jurisdiction");

    commerce
        .tax()
        .create_rate(CreateTaxRate {
            jurisdiction_id: jur.id,
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate: dec!(0.05),
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

    // $2.50 at 5% = exactly $0.125 of tax — a rounding midpoint.
    let request = TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "line-1".into(),
            sku: None,
            product_id: None,
            quantity: dec!(1),
            unit_price: dec!(2.50),
            discount_amount: rust_decimal::Decimal::ZERO,
            tax_category: ProductTaxCategory::Standard,
            tax_code: None,
            description: None,
        }],
        shipping_address: TaxAddress {
            line1: None,
            line2: None,
            city: None,
            state: Some(state),
            postal_code: None,
            country: country.clone(),
        },
        billing_address: None,
        customer_id: None,
        shipping_amount: None,
        currency: CurrencyCode::USD,
        transaction_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
        prices_include_tax: false,
    };

    let mut settings = commerce.tax().get_settings().await.expect("settings");
    settings.rounding_mode = "half_even".into();
    commerce.tax().update_settings(settings).await.expect("update settings");
    let even = commerce.tax().calculate_tax(request.clone()).await.expect("calc");
    assert_eq!(
        even.total_tax,
        dec!(0.12),
        "half_even must round $0.125 down to 0.12 (retained digit is even): {even:?}"
    );

    let mut settings = commerce.tax().get_settings().await.expect("settings");
    settings.rounding_mode = "half_up".into();
    commerce.tax().update_settings(settings).await.expect("update settings");
    let up = commerce.tax().calculate_tax(request).await.expect("calc");
    assert_eq!(up.total_tax, dec!(0.13), "half_up must round $0.125 up to 0.13: {up:?}");
}
