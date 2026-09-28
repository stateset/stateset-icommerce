#![cfg(feature = "sqlite")]

//! Rows the SQLite migrations seed must be reachable by the ids the API hands out.
//!
//! The seeds wrote ids as `lower(hex(randomblob(16)))`: 32 bare hex digits.
//! Every row the engine writes itself stores the hyphenated form, and every
//! read parses the stored id into a `Uuid` and hands out `to_string()` -- the
//! hyphenated form. So each seeded row came back under an id that matches
//! nothing in its own table: `get_plan(plan.id)` on a listed plan was `None`,
//! and a rate's `jurisdiction_id` never matched its seeded jurisdiction, so
//! tax on a fresh store was zero for every address, with tax enabled and a
//! 7.25% California rate seeded.

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, ProductTaxCategory, SubscriptionPlanFilter, TaxAddress, TaxCalculationRequest,
    TaxLineItem, TaxRateFilter,
};

fn california_sale() -> TaxCalculationRequest {
    TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "item-1".into(),
            quantity: dec!(1),
            unit_price: dec!(100),
            tax_category: ProductTaxCategory::Standard,
            ..Default::default()
        }],
        shipping_address: TaxAddress {
            country: "US".into(),
            state: Some("CA".into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn a_fresh_store_taxes_a_california_sale_at_the_seeded_rate() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let result = commerce.tax().calculate(california_sale()).expect("calculate");
    assert_eq!(result.total_tax, dec!(7.25), "the seeded 7.25% California rate: {result:?}");
}

#[test]
fn a_seeded_jurisdiction_is_found_by_the_id_it_is_listed_under() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let california = commerce
        .tax()
        .get_jurisdiction_by_code("US-CA")
        .expect("lookup")
        .expect("California is seeded");
    let by_id = commerce.tax().get_jurisdiction(california.id).expect("lookup");
    assert_eq!(by_id.map(|j| j.code), Some("US-CA".to_string()));

    let rates = commerce
        .tax()
        .list_rates(TaxRateFilter { jurisdiction_id: Some(california.id), ..Default::default() })
        .expect("list rates");
    assert_eq!(rates.len(), 1, "California's seeded rate belongs to California");
    assert_eq!(rates[0].rate, dec!(0.0725));

    let united_states =
        commerce.tax().get_jurisdiction_by_code("US").expect("lookup").expect("US is seeded");
    assert_eq!(california.parent_id, Some(united_states.id), "the state hangs off its country");
}

#[test]
fn a_seeded_subscription_plan_is_found_by_the_id_it_is_listed_under() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let plans =
        commerce.subscriptions().list_plans(SubscriptionPlanFilter::default()).expect("list");
    assert!(!plans.is_empty(), "the migrations seed example plans");
    for plan in plans {
        let found = commerce.subscriptions().get_plan(plan.id).expect("get plan");
        assert_eq!(found.map(|p| p.code), Some(plan.code.clone()), "plan {}", plan.code);
    }
}
