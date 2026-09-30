#![cfg(feature = "sqlite")]

//! Canadian sales tax on a fresh store, from the seeded jurisdictions.
//!
//! GST is federal (5%). The harmonized provinces charge HST *instead of* GST
//! -- HST includes the federal part -- and the rest charge GST plus any
//! provincial tax. The seeds put GST on the country jurisdiction, which every
//! Canadian address matches, and HST on the harmonized provinces, so the two
//! stacked: Ontario charged 5% + 13% = 18%. Nova Scotia's HST fell from 15% to
//! 14% on 2025-04-01, and Newfoundland and Labrador, Prince Edward Island and
//! the three territories were not seeded at all.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, ProductTaxCategory, TaxAddress, TaxCalculationRequest, TaxLineItem,
};

fn sale_in(province: &str, on: NaiveDate) -> TaxCalculationRequest {
    TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "item-1".into(),
            quantity: dec!(1),
            unit_price: dec!(100),
            tax_category: ProductTaxCategory::Standard,
            ..Default::default()
        }],
        shipping_address: TaxAddress {
            country: "CA".into(),
            state: Some(province.into()),
            ..Default::default()
        },
        transaction_date: Some(on),
        ..Default::default()
    }
}

fn tax_on_100(commerce: &Commerce, province: &str, on: NaiveDate) -> Decimal {
    commerce.tax().calculate(sale_in(province, on)).expect("calculate").total_tax
}

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

#[test]
fn harmonized_provinces_charge_hst_instead_of_gst() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    assert_eq!(tax_on_100(&commerce, "ON", today()), dec!(13.00), "Ontario HST, not GST + HST");
    assert_eq!(tax_on_100(&commerce, "NB", today()), dec!(15.00), "New Brunswick HST");
    assert_eq!(tax_on_100(&commerce, "NL", today()), dec!(15.00), "Newfoundland and Labrador HST");
    assert_eq!(tax_on_100(&commerce, "PE", today()), dec!(15.00), "Prince Edward Island HST");
}

#[test]
fn nova_scotia_hst_is_fourteen_percent_from_april_2025() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    assert_eq!(tax_on_100(&commerce, "NS", today()), dec!(14.00));
}

#[test]
fn gst_provinces_and_territories_charge_gst_plus_provincial_tax() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    assert_eq!(tax_on_100(&commerce, "AB", today()), dec!(5.00), "Alberta: GST only");
    for territory in ["NT", "NU", "YT"] {
        assert_eq!(tax_on_100(&commerce, territory, today()), dec!(5.00), "{territory}: GST only");
    }
    assert_eq!(tax_on_100(&commerce, "BC", today()), dec!(12.00), "BC: GST 5% + PST 7%");
    assert_eq!(tax_on_100(&commerce, "SK", today()), dec!(11.00), "SK: GST 5% + PST 6%");
    assert_eq!(tax_on_100(&commerce, "MB", today()), dec!(12.00), "MB: GST 5% + RST 7%");
    assert_eq!(tax_on_100(&commerce, "QC", today()), dec!(14.98), "QC: GST 5% + QST 9.975%");
}
