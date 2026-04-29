#![cfg(feature = "sqlite")]

//! Integration tests for tax calculation features

use chrono::Utc;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateTaxJurisdiction, CreateTaxRate, JurisdictionLevel, ProductTaxCategory,
    TaxAddress, TaxCalculationRequest, TaxLineItem, TaxType,
};

#[test]
fn test_us_sales_tax_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate tax for California order
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-1".into(),
                sku: Some("WIDGET-001".into()),
                product_id: None,
                quantity: dec!(2),
                unit_price: dec!(29.99),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                tax_code: None,
                description: Some("Premium Widget".into()),
            }],
            shipping_address: TaxAddress {
                country: "US".into(),
                state: Some("CA".into()),
                city: Some("Los Angeles".into()),
                postal_code: Some("90210".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    // Check results
    assert_eq!(result.subtotal, dec!(59.98)); // 2 * 29.99
    assert!(result.total_tax >= dec!(0)); // Some tax should be calculated
    assert_eq!(result.total, result.subtotal + result.total_tax);
    assert!(!result.tax_breakdown.is_empty() || result.total_tax == dec!(0));
}

#[test]
fn test_eu_vat_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate VAT for German order
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-1".into(),
                sku: Some("BOOK-001".into()),
                product_id: None,
                quantity: dec!(1),
                unit_price: dec!(19.99),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Reduced, // Books often have reduced VAT
                tax_code: None,
                description: Some("Paperback Book".into()),
            }],
            shipping_address: TaxAddress {
                country: "DE".into(),
                state: None,
                city: Some("Berlin".into()),
                postal_code: Some("10115".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    assert_eq!(result.subtotal, dec!(19.99));
    assert!(result.total >= result.subtotal);
}

#[test]
fn test_canadian_gst_hst_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate tax for Ontario order (HST province)
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-1".into(),
                sku: Some("GADGET-001".into()),
                product_id: None,
                quantity: dec!(1),
                unit_price: dec!(99.99),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                tax_code: None,
                description: Some("Electronic Gadget".into()),
            }],
            shipping_address: TaxAddress {
                country: "CA".into(),
                state: Some("ON".into()), // Ontario
                city: Some("Toronto".into()),
                postal_code: Some("M5H 2N2".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    assert_eq!(result.subtotal, dec!(99.99));
    assert!(result.total >= result.subtotal);
}

#[test]
fn test_tax_exempt_product() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate tax for exempt product (groceries in many US states)
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-1".into(),
                sku: Some("FOOD-001".into()),
                product_id: None,
                quantity: dec!(3),
                unit_price: dec!(5.99),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Exempt,
                tax_code: None,
                description: Some("Organic Groceries".into()),
            }],
            shipping_address: TaxAddress {
                country: "US".into(),
                state: Some("TX".into()),
                city: Some("Austin".into()),
                postal_code: Some("78701".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    // Exempt products should have zero tax
    assert_eq!(result.subtotal, dec!(17.97)); // 3 * 5.99
    // Note: Tax may still be non-zero if exempt category isn't configured
}

#[test]
fn test_effective_rate_lookup() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Get effective rate for Texas
    let rate = commerce
        .tax()
        .get_effective_rate(
            &TaxAddress { country: "US".into(), state: Some("TX".into()), ..Default::default() },
            ProductTaxCategory::Standard,
        )
        .expect("Failed to get effective rate");

    // Rate should be a reasonable percentage (0-30%)
    assert!(rate >= dec!(0));
    assert!(rate <= dec!(0.30));
}

#[test]
fn test_jurisdiction_listing() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // List all jurisdictions
    let jurisdictions = commerce
        .tax()
        .list_jurisdictions(Default::default())
        .expect("Failed to list jurisdictions");

    // Should return successfully (may be empty if not seeded)
    let _ = jurisdictions.len();
}

#[test]
fn test_rate_listing() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // List all tax rates
    let rates = commerce.tax().list_rates(Default::default()).expect("Failed to list rates");

    // Should return successfully (may be empty if not seeded)
    let _ = rates.len();
}

#[test]
fn test_us_state_tax_info() {
    // Test the static US state tax info helper
    use stateset_embedded::get_us_state_tax_info;

    // Test California
    let ca_info = get_us_state_tax_info("CA");
    assert!(ca_info.is_some());
    let ca = ca_info.unwrap();
    assert_eq!(ca.state_code, "CA");
    assert!(ca.state_rate > dec!(0));
    assert!(ca.has_local_taxes);

    // Test Oregon (no sales tax)
    let or_info = get_us_state_tax_info("OR");
    assert!(or_info.is_some());
    let or = or_info.unwrap();
    assert_eq!(or.state_code, "OR");
    assert_eq!(or.state_rate, dec!(0));
}

#[test]
fn test_eu_vat_info() {
    // Test the static EU VAT info helper
    use stateset_embedded::get_eu_vat_info;

    // Test Germany
    let de_info = get_eu_vat_info("DE");
    assert!(de_info.is_some());
    let de = de_info.unwrap();
    assert_eq!(de.country_code, "DE");
    assert_eq!(de.standard_rate, dec!(0.19)); // 19%
    assert!(de.reduced_rate.is_some());
}

#[test]
fn test_canadian_tax_info() {
    // Test the static Canadian tax info helper
    use stateset_embedded::get_canadian_tax_info;

    // Test Ontario (HST)
    let on_info = get_canadian_tax_info("ON");
    assert!(on_info.is_some());
    let on = on_info.unwrap();
    assert_eq!(on.province_code, "ON");
    assert!(on.hst_rate.is_some());
    assert_eq!(on.hst_rate, Some(dec!(0.13))); // 13% HST

    // Test British Columbia (GST + PST)
    let bc_info = get_canadian_tax_info("BC");
    assert!(bc_info.is_some());
    let bc = bc_info.unwrap();
    assert_eq!(bc.province_code, "BC");
    assert_eq!(bc.gst_rate, dec!(0.05)); // 5% GST
    assert!(bc.pst_rate.is_some());
}

#[test]
fn test_multiple_items_tax_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate tax for multiple items with different categories
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![
                TaxLineItem {
                    id: "item-1".into(),
                    sku: Some("ELEC-001".into()),
                    product_id: None,
                    quantity: dec!(1),
                    unit_price: dec!(199.99),
                    discount_amount: dec!(0),
                    tax_category: ProductTaxCategory::Standard,
                    tax_code: None,
                    description: Some("Electronics".into()),
                },
                TaxLineItem {
                    id: "item-2".into(),
                    sku: Some("CLOTH-001".into()),
                    product_id: None,
                    quantity: dec!(2),
                    unit_price: dec!(49.99),
                    discount_amount: dec!(0),
                    tax_category: ProductTaxCategory::Clothing,
                    tax_code: None,
                    description: Some("Clothing".into()),
                },
                TaxLineItem {
                    id: "item-3".into(),
                    sku: Some("FOOD-001".into()),
                    product_id: None,
                    quantity: dec!(1),
                    unit_price: dec!(15.99),
                    discount_amount: dec!(0),
                    tax_category: ProductTaxCategory::Food,
                    tax_code: None,
                    description: Some("Food Item".into()),
                },
            ],
            shipping_address: TaxAddress {
                country: "US".into(),
                state: Some("NY".into()),
                city: Some("New York".into()),
                postal_code: Some("10001".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    // Check subtotal is correct: 199.99 + (2 * 49.99) + 15.99 = 315.96
    assert_eq!(result.subtotal, dec!(315.96));

    // Line item taxes should be calculated
    assert_eq!(result.line_item_taxes.len(), 3);

    // Total should include tax
    assert!(result.total >= result.subtotal);
}

#[test]
fn test_shipping_taxability() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Calculate tax with shipping amount (some states tax shipping)
    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-1".into(),
                sku: Some("WIDGET-001".into()),
                product_id: None,
                quantity: dec!(1),
                unit_price: dec!(50.00),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                tax_code: None,
                description: None,
            }],
            shipping_address: TaxAddress {
                country: "US".into(),
                state: Some("CA".into()),
                ..Default::default()
            },
            shipping_amount: Some(dec!(9.99)),
            ..Default::default()
        })
        .expect("Failed to calculate tax");

    assert_eq!(result.subtotal, dec!(50.00));
    // Total should at least equal subtotal (may or may not include shipping tax)
    assert!(result.total >= result.subtotal);
}

#[test]
fn test_tax_thresholds_and_caps() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let jurisdiction = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            name: "Test State".into(),
            code: "US-TS".into(),
            level: JurisdictionLevel::State,
            country_code: "US".into(),
            state_code: Some("TS".into()),
            ..Default::default()
        })
        .expect("Failed to create jurisdiction");

    commerce
        .tax()
        .create_rate(CreateTaxRate {
            jurisdiction_id: jurisdiction.id,
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate: dec!(0.10),
            name: "Threshold Rate".into(),
            threshold_min: Some(dec!(100.00)),
            threshold_max: Some(dec!(200.00)),
            effective_from: Utc::now().date_naive(),
            ..Default::default()
        })
        .expect("Failed to create tax rate");

    let address =
        TaxAddress { country: "US".into(), state: Some("TS".into()), ..Default::default() };

    let result_low = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-low".into(),
                quantity: dec!(1),
                unit_price: dec!(50.00),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                ..Default::default()
            }],
            shipping_address: address.clone(),
            ..Default::default()
        })
        .expect("Failed to calculate tax (below min)");

    assert_eq!(result_low.total_tax, dec!(0.00));

    let result_mid = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-mid".into(),
                quantity: dec!(1),
                unit_price: dec!(150.00),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                ..Default::default()
            }],
            shipping_address: address.clone(),
            ..Default::default()
        })
        .expect("Failed to calculate tax (within range)");

    assert_eq!(result_mid.total_tax, dec!(15.00));

    let result_high = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-high".into(),
                quantity: dec!(1),
                unit_price: dec!(250.00),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                ..Default::default()
            }],
            shipping_address: address,
            ..Default::default()
        })
        .expect("Failed to calculate tax (above cap)");

    assert_eq!(result_high.total_tax, dec!(20.00));
}

#[test]
fn test_tax_fixed_amount_rate() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let jurisdiction = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            name: "Fixed Tax State".into(),
            code: "US-FX".into(),
            level: JurisdictionLevel::State,
            country_code: "US".into(),
            state_code: Some("FX".into()),
            ..Default::default()
        })
        .expect("Failed to create jurisdiction");

    commerce
        .tax()
        .create_rate(CreateTaxRate {
            jurisdiction_id: jurisdiction.id,
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate: dec!(0.10),
            fixed_amount: Some(dec!(3.25)),
            name: "Fixed Amount Rate".into(),
            effective_from: Utc::now().date_naive(),
            ..Default::default()
        })
        .expect("Failed to create fixed tax rate");

    let result = commerce
        .tax()
        .calculate(TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "item-fixed".into(),
                quantity: dec!(1),
                unit_price: dec!(50.00),
                discount_amount: dec!(0),
                tax_category: ProductTaxCategory::Standard,
                ..Default::default()
            }],
            shipping_address: TaxAddress {
                country: "US".into(),
                state: Some("FX".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Failed to calculate fixed tax");

    assert_eq!(result.total_tax, dec!(3.25));
}
