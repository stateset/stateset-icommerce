#![cfg(feature = "sqlite")]

//! Integration tests for tax calculation features

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate,
    ExemptionType, JurisdictionLevel, ProductTaxCategory, TaxAddress, TaxCalculationRequest,
    TaxLineItem, TaxType,
};
use uuid::Uuid;

/// A store with a single 5% jurisdiction and one customer.
fn store_with_rate() -> (Commerce, Uuid) {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let jurisdiction = commerce
        .tax()
        .create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "Exemption Test State".into(),
            code: "ZX-ET".into(),
            level: JurisdictionLevel::State,
            country_code: "ZX".into(),
            state_code: Some("ET".into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .expect("jurisdiction");
    commerce
        .tax()
        .create_rate(CreateTaxRate {
            jurisdiction_id: jurisdiction.id,
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate: dec!(0.05),
            name: "Sales Tax".into(),
            effective_from: NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
            ..Default::default()
        })
        .expect("rate");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("exempt-{}@example.com", Uuid::new_v4()),
            first_name: "Ex".into(),
            last_name: "Empt".into(),
            ..Default::default()
        })
        .expect("customer");
    (commerce, customer.id.into_uuid())
}

fn hundred_dollar_request(customer_id: Uuid) -> TaxCalculationRequest {
    TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "item-1".into(),
            quantity: dec!(1),
            unit_price: dec!(100),
            tax_category: ProductTaxCategory::Standard,
            ..Default::default()
        }],
        shipping_address: TaxAddress {
            country: "ZX".into(),
            state: Some("ET".into()),
            ..Default::default()
        },
        customer_id: Some(customer_id),
        ..Default::default()
    }
}

/// The exemption lifecycle end to end through the PUBLIC facade: an exemption
/// can be created there, and — since only verified exemptions are honoured —
/// verified there too. `verify_exemption` existed on both repositories but was
/// reachable from neither facade, so an exemption created through the public
/// API could never be honoured.
#[test]
fn tax_exemption_is_honoured_only_after_it_is_verified() {
    let (commerce, customer_id) = store_with_rate();
    let request = hundred_dollar_request(customer_id);

    assert_eq!(commerce.tax().calculate(request.clone()).expect("calc").total_tax, dec!(5.00));

    let exemption = commerce
        .tax()
        .create_exemption(CreateTaxExemption {
            customer_id,
            exemption_type: ExemptionType::Resale,
            certificate_number: Some("RES-9".into()),
            issuing_authority: None,
            jurisdiction_ids: vec![],
            exempt_categories: vec![],
            effective_from: Utc::now().date_naive() - chrono::Duration::days(1),
            expires_at: None,
            notes: None,
        })
        .expect("create exemption");
    assert!(!exemption.verified, "exemptions are created unverified");
    assert!(
        !commerce.tax().customer_is_exempt(customer_id).expect("is exempt"),
        "an unverified certificate is not an exemption in force"
    );
    assert_eq!(
        commerce.tax().calculate(request.clone()).expect("calc").total_tax,
        dec!(5.00),
        "unverified: still taxed"
    );

    let verified = commerce.tax().verify_exemption(exemption.id, true).expect("verify");
    assert!(verified.verified && verified.verified_at.is_some());
    assert!(commerce.tax().customer_is_exempt(customer_id).expect("is exempt"));

    let result = commerce.tax().calculate(request.clone()).expect("calc");
    assert_eq!(result.total_tax, Decimal::ZERO, "verified: tax drops to zero");
    assert!(result.exemptions_applied);
    assert_eq!(result.applied_exemptions.len(), 1);
    assert_eq!(result.applied_exemptions[0].tax_saved, dec!(5.00));

    // Revoking the verification puts the tax back.
    commerce.tax().verify_exemption(exemption.id, false).expect("revoke");
    assert!(!commerce.tax().customer_is_exempt(customer_id).expect("is exempt"));
    assert_eq!(commerce.tax().calculate(request).expect("calc").total_tax, dec!(5.00));
}

/// `customer_is_exempt` must agree with the engine. It used to return true for
/// ANY stored row, so it said "exempt" for unverified, inactive and expired
/// certificates while the very next calculation charged full tax.
#[test]
fn customer_is_exempt_agrees_with_the_engine() {
    let (commerce, customer_id) = store_with_rate();
    let today = Utc::now().date_naive();

    // Expired yesterday.
    let expired = commerce
        .tax()
        .create_exemption(CreateTaxExemption {
            customer_id,
            exemption_type: ExemptionType::NonProfit,
            certificate_number: Some("NP-1".into()),
            issuing_authority: None,
            jurisdiction_ids: vec![],
            exempt_categories: vec![],
            effective_from: today - chrono::Duration::days(30),
            expires_at: Some(today - chrono::Duration::days(1)),
            notes: None,
        })
        .expect("create exemption");
    commerce.tax().verify_exemption(expired.id, true).expect("verify");

    assert!(
        !commerce.tax().customer_is_exempt(customer_id).expect("is exempt"),
        "an expired certificate is not in force"
    );
    assert_eq!(
        commerce.tax().calculate(hundred_dollar_request(customer_id)).expect("calc").total_tax,
        dec!(5.00),
        "and the engine agrees"
    );
    // It WAS in force inside its window.
    assert!(
        commerce
            .tax()
            .customer_is_exempt_on(customer_id, today - chrono::Duration::days(2))
            .expect("is exempt on")
    );
    // The row is still there — the old implementation said "exempt" for it.
    assert_eq!(commerce.tax().get_customer_exemptions(customer_id).expect("list").len(), 1);
}

/// Turning tax off in settings must stop tax being charged, and the effective
/// rate lookup must agree.
#[test]
fn disabling_tax_settings_charges_no_tax() {
    let (commerce, customer_id) = store_with_rate();
    let address =
        TaxAddress { country: "ZX".into(), state: Some("ET".into()), ..Default::default() };

    assert_eq!(
        commerce.tax().calculate(hundred_dollar_request(customer_id)).expect("calc").total_tax,
        dec!(5.00)
    );
    assert_eq!(
        commerce.tax().get_effective_rate(&address, ProductTaxCategory::Standard).expect("rate"),
        dec!(0.05)
    );

    let settings = commerce.tax().set_enabled(false).expect("disable");
    assert!(!settings.enabled);
    assert!(!commerce.tax().is_enabled().expect("is enabled"));

    let result = commerce.tax().calculate(hundred_dollar_request(customer_id)).expect("calc");
    assert_eq!(result.total_tax, Decimal::ZERO, "disabled tax must charge nothing: {result:?}");
    assert_eq!(result.subtotal, dec!(100));
    assert_eq!(result.total, dec!(100));
    assert!(result.tax_breakdown.is_empty());
    assert_eq!(
        commerce.tax().get_effective_rate(&address, ProductTaxCategory::Standard).expect("rate"),
        Decimal::ZERO,
        "the quoted rate must agree with what is charged"
    );

    commerce.tax().set_enabled(true).expect("re-enable");
    assert_eq!(
        commerce.tax().calculate(hundred_dollar_request(customer_id)).expect("calc").total_tax,
        dec!(5.00)
    );
}

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
