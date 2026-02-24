//! Comprehensive integration tests for the `stateset-pricing` crate.
//!
//! These tests exercise the public API across all modules (line items, order
//! totals, promotions, tax, rounding, and currency conversion) through
//! realistic multi-component scenarios.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use stateset_pricing::{
    // Promotions
    AppliedPromotion,
    // Currency
    CurrencyConverter,
    ExchangeRate,
    // Order totals
    Fee,
    // Line items
    LineDiscount,
    LineItem,
    OrderTotal,
    OrderTotalInput,
    // Errors
    PricingError,
    Promotion,
    PromotionContext,
    PromotionResult,
    PromotionRule,
    RejectedPromotion,
    RejectionReason,
    // Rounding
    RoundingMode,
    RoundingPolicy,
    // Tax
    TaxAppliesTo,
    TaxContext,
    TaxLine,
    TaxResult,
    TaxRule,
    TaxableItem,
    calculate_tax,
    compute_order_total,
    evaluate_promotions,
    round,
};

// =========================================================================
// Helpers
// =========================================================================

fn item(
    sku: &str,
    price: Decimal,
    qty: u32,
    discount: Option<LineDiscount>,
    tax: Option<Decimal>,
) -> LineItem {
    LineItem {
        sku: sku.into(),
        name: format!("Item {sku}"),
        unit_price: price,
        quantity: qty,
        discount,
        tax_rate: tax,
    }
}

const fn order_input(items: Vec<LineItem>) -> OrderTotalInput {
    OrderTotalInput {
        items,
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::usd(),
    }
}

fn base_promo_context() -> PromotionContext {
    PromotionContext {
        order_total: dec!(200.00),
        item_count: 5,
        skus: vec!["WIDGET-A".into(), "WIDGET-B".into(), "GADGET-1".into()],
        now: Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap(),
        customer_group: Some("VIP".into()),
        is_first_order: false,
    }
}

fn make_rate(from: &str, to: &str, rate: Decimal) -> ExchangeRate {
    ExchangeRate { from: from.into(), to: to.into(), rate, as_of: Utc::now() }
}

// =========================================================================
// 1. Multi-item Order Totals with Mixed Discounts
// =========================================================================

#[test]
fn order_mixed_discount_types() {
    // Three items: Percentage, FixedAmount, FixedPrice discounts
    let items = vec![
        item("A", dec!(50.00), 2, Some(LineDiscount::Percentage(dec!(0.10))), Some(dec!(0.08))),
        item("B", dec!(30.00), 3, Some(LineDiscount::FixedAmount(dec!(15.00))), Some(dec!(0.08))),
        item("C", dec!(40.00), 1, Some(LineDiscount::FixedPrice(dec!(25.00))), Some(dec!(0.08))),
    ];
    let input = order_input(items);
    let total = compute_order_total(&input);

    // A: subtotal=100, discount=10, taxable=90, tax=7.20
    // B: subtotal=90, discount=15, taxable=75, tax=6.00
    // C: subtotal=40, discount=15 (40 - 25*1), taxable=25, tax=2.00
    assert_eq!(total.subtotal, dec!(230.00));
    assert_eq!(total.total_discount, dec!(40.00));
    // taxable total = 90+75+25 = 190, line_tax = 7.20+6.00+2.00 = 15.20
    assert_eq!(total.total_tax, dec!(15.20));
    // grand = 190 + 15.20 = 205.20
    assert_eq!(total.grand_total, dec!(205.20));
}

#[test]
fn order_no_discounts_baseline() {
    let items = vec![
        item("X", dec!(25.00), 4, None, Some(dec!(0.07))),
        item("Y", dec!(10.00), 10, None, Some(dec!(0.07))),
    ];
    let input = order_input(items);
    let total = compute_order_total(&input);

    // X: 100, Y: 100, subtotal=200, tax=14.00
    assert_eq!(total.subtotal, dec!(200.00));
    assert_eq!(total.total_discount, Decimal::ZERO);
    assert_eq!(total.total_tax, dec!(14.00));
    assert_eq!(total.grand_total, dec!(214.00));
}

#[test]
fn order_100_percent_discount_only_shipping_and_fees() {
    let items = vec![item(
        "FREE",
        dec!(75.00),
        2,
        Some(LineDiscount::Percentage(Decimal::ONE)),
        Some(dec!(0.10)),
    )];
    let mut input = order_input(items);
    input.shipping_cost = dec!(9.99);
    input.shipping_tax_rate = Some(dec!(0.08));
    input.fees = vec![Fee { name: "Handling".into(), amount: dec!(3.00) }];

    let total = compute_order_total(&input);

    // 100% discount: taxable = 0, tax = 0
    assert_eq!(total.subtotal, dec!(150.00));
    assert_eq!(total.total_discount, dec!(150.00));
    assert_eq!(total.total_tax, Decimal::ZERO);
    // grand = 0 + 0 + 9.99 + 0.80 + 3.00 = 13.79
    assert_eq!(total.shipping, dec!(9.99));
    assert_eq!(total.shipping_tax, dec!(0.80));
    assert_eq!(total.fees, dec!(3.00));
    assert_eq!(total.grand_total, dec!(13.79));
}

#[test]
fn order_multi_item_with_order_level_discount() {
    let items = vec![
        item("A", dec!(100.00), 1, Some(LineDiscount::Percentage(dec!(0.10))), Some(dec!(0.08))),
        item("B", dec!(50.00), 2, Some(LineDiscount::FixedAmount(dec!(5.00))), Some(dec!(0.06))),
    ];
    let mut input = order_input(items);
    input.order_discount = Some(LineDiscount::Percentage(dec!(0.05)));
    input.shipping_cost = dec!(12.00);
    input.shipping_tax_rate = Some(dec!(0.08));
    input.fees = vec![Fee { name: "Insurance".into(), amount: dec!(4.50) }];

    let total = compute_order_total(&input);

    // A: subtotal=100, discount=10, taxable=90, tax=7.20
    // B: subtotal=100, discount=5, taxable=95, tax=5.70
    // Line totals: subtotal=200, line_discount=15, line_taxable=185, line_tax=12.90
    // Order discount: 185 * 0.05 = 9.25
    // Total discount: 15 + 9.25 = 24.25
    // Effective taxable: 185 - 9.25 = 175.75
    // Tax adjustment: 12.90 * 175.75/185 = 12.255... => 12.26 (rounded half-up to 2dp)
    assert_eq!(total.subtotal, dec!(200.00));
    assert_eq!(total.total_discount, dec!(24.25));
    // Shipping: 12.00, shipping_tax: 0.96
    assert_eq!(total.shipping, dec!(12.00));
    assert_eq!(total.shipping_tax, dec!(0.96));
    assert_eq!(total.fees, dec!(4.50));
}

// =========================================================================
// 2. Tax Calculation Across Jurisdictions
// =========================================================================

#[test]
fn tax_multi_jurisdiction_non_compound() {
    let rules = vec![
        TaxRule {
            jurisdiction: "State".into(),
            rate: dec!(0.06),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "County".into(),
            rate: dec!(0.01),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "City".into(),
            rate: dec!(0.005),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
    ];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(200.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    assert_eq!(result.tax_lines.len(), 3);
    assert_eq!(result.tax_lines[0].tax_amount, dec!(12.00)); // 200 * 0.06
    assert_eq!(result.tax_lines[1].tax_amount, dec!(2.00)); // 200 * 0.01
    assert_eq!(result.tax_lines[2].tax_amount, dec!(1.00)); // 200 * 0.005
    assert_eq!(result.total_tax, dec!(15.00));
}

#[test]
fn tax_compound_state_then_county() {
    // State (non-compound) then county (compound on state+base)
    let rules = vec![
        TaxRule {
            jurisdiction: "State".into(),
            rate: dec!(0.05),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "County".into(),
            rate: dec!(0.02),
            applies_to: TaxAppliesTo::AllItems,
            compound: true,
        },
    ];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    // State: 100 * 0.05 = 5.00
    // County (compound): (100 + 5) * 0.02 = 2.10
    assert_eq!(result.tax_lines[0].tax_amount, dec!(5.00));
    assert_eq!(result.tax_lines[1].tax_amount, dec!(2.10));
    assert_eq!(result.total_tax, dec!(7.10));
}

#[test]
fn tax_specific_categories_some_exempt() {
    let ctx = TaxContext {
        items: vec![
            TaxableItem {
                amount: dec!(100.00),
                category: Some("electronics".into()),
                exempt: false,
            },
            TaxableItem { amount: dec!(50.00), category: Some("food".into()), exempt: false },
            TaxableItem { amount: dec!(25.00), category: Some("clothing".into()), exempt: false },
        ],
        shipping: Decimal::ZERO,
    };
    let rules = vec![TaxRule {
        jurisdiction: "Luxury".into(),
        rate: dec!(0.12),
        applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into()]),
        compound: false,
    }];
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    // Only electronics ($100) is taxable
    assert_eq!(result.tax_lines.len(), 1);
    assert_eq!(result.tax_lines[0].taxable_amount, dec!(100.00));
    assert_eq!(result.total_tax, dec!(12.00));
}

#[test]
fn tax_on_shipping_only() {
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(500.00), category: None, exempt: false }],
        shipping: dec!(25.00),
    };
    let rules = vec![TaxRule {
        jurisdiction: "Shipping-Tax".into(),
        rate: dec!(0.08),
        applies_to: TaxAppliesTo::ShippingOnly,
        compound: false,
    }];
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    assert_eq!(result.tax_lines.len(), 1);
    assert_eq!(result.tax_lines[0].taxable_amount, dec!(25.00));
    assert_eq!(result.total_tax, dec!(2.00));
}

#[test]
fn tax_zero_rate_produces_no_lines() {
    let rules = vec![TaxRule {
        jurisdiction: "Oregon".into(),
        rate: Decimal::ZERO,
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(999.99), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    assert!(result.tax_lines.is_empty());
    assert_eq!(result.total_tax, Decimal::ZERO);
}

#[test]
fn tax_exempt_items_skipped() {
    let ctx = TaxContext {
        items: vec![
            TaxableItem { amount: dec!(80.00), category: None, exempt: false },
            TaxableItem { amount: dec!(120.00), category: None, exempt: true },
        ],
        shipping: Decimal::ZERO,
    };
    let rules = vec![TaxRule {
        jurisdiction: "CA".into(),
        rate: dec!(0.0725),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    // Only $80 taxable
    assert_eq!(result.tax_lines[0].taxable_amount, dec!(80.00));
    assert_eq!(result.total_tax, dec!(5.80));
}

#[test]
fn tax_mixed_items_shipping_compound() {
    // A realistic scenario: state item tax, county item tax (compound), shipping tax
    let ctx = TaxContext {
        items: vec![
            TaxableItem {
                amount: dec!(150.00),
                category: Some("electronics".into()),
                exempt: false,
            },
            TaxableItem { amount: dec!(50.00), category: Some("food".into()), exempt: true },
        ],
        shipping: dec!(10.00),
    };
    let rules = vec![
        TaxRule {
            jurisdiction: "State".into(),
            rate: dec!(0.06),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "County".into(),
            rate: dec!(0.015),
            applies_to: TaxAppliesTo::AllItems,
            compound: true,
        },
        TaxRule {
            jurisdiction: "Ship-Tax".into(),
            rate: dec!(0.06),
            applies_to: TaxAppliesTo::ShippingOnly,
            compound: false,
        },
    ];
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());

    // State: only $150 non-exempt, 150 * 0.06 = 9.00
    // County (compound): (150 + 9.00) * 0.015 = 2.385 -> 2.39
    // Ship-Tax: 10 * 0.06 = 0.60
    assert_eq!(result.tax_lines[0].tax_amount, dec!(9.00));
    assert_eq!(result.tax_lines[1].tax_amount, dec!(2.39));
    assert_eq!(result.tax_lines[2].tax_amount, dec!(0.60));
    assert_eq!(result.total_tax, dec!(11.99));
}

// =========================================================================
// 3. Promotion Stacking and Exclusion
// =========================================================================

#[test]
fn promo_multiple_stackable_all_applied() {
    let promos = vec![
        Promotion {
            code: "STACK-A".into(),
            discount: LineDiscount::FixedAmount(dec!(10.00)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "STACK-B".into(),
            discount: LineDiscount::Percentage(dec!(0.05)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "STACK-C".into(),
            discount: LineDiscount::FixedAmount(dec!(3.00)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert_eq!(result.applied.len(), 3);
    // 10 + (200 * 0.05 = 10) + 3 = 23
    assert_eq!(result.total_discount, dec!(23.00));
    assert!(result.rejected.is_empty());
}

#[test]
fn promo_non_stackable_best_deal_wins() {
    let promos = vec![
        Promotion {
            code: "NS-SMALL".into(),
            discount: LineDiscount::Percentage(dec!(0.05)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "NS-BIG".into(),
            discount: LineDiscount::FixedAmount(dec!(30.00)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "NS-MED".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &base_promo_context());

    // NS-SMALL: 200*0.05=10, NS-BIG: 30, NS-MED: 200*0.10=20 => NS-BIG wins
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.applied[0].code, "NS-BIG");
    assert_eq!(result.applied[0].discount_amount, dec!(30.00));
    assert_eq!(result.rejected.len(), 2);
    for r in &result.rejected {
        assert!(
            matches!(&r.reason, RejectionReason::SupersededByBetterDeal { winner_code } if winner_code == "NS-BIG")
        );
    }
}

#[test]
fn promo_mix_stackable_and_non_stackable() {
    let promos = vec![
        Promotion {
            code: "NS-A".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "NS-B".into(),
            discount: LineDiscount::Percentage(dec!(0.15)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "S-FLAT".into(),
            discount: LineDiscount::FixedAmount(dec!(5.00)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &base_promo_context());

    // NS-B wins (30 > 20), S-FLAT stacks
    assert_eq!(result.applied.len(), 2);
    let codes: Vec<&str> = result.applied.iter().map(|a| a.code.as_str()).collect();
    assert!(codes.contains(&"NS-B"));
    assert!(codes.contains(&"S-FLAT"));
    // 200*0.15 + 5 = 35
    assert_eq!(result.total_discount, dec!(35.00));
}

#[test]
fn promo_rejected_minimum_order_total_not_met() {
    let promos = vec![Promotion {
        code: "BIG-SPEND".into(),
        discount: LineDiscount::Percentage(dec!(0.20)),
        rules: vec![PromotionRule::MinimumOrderTotal(dec!(500.00))],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &base_promo_context()); // order_total = 200

    assert!(result.applied.is_empty());
    assert_eq!(result.rejected.len(), 1);
    assert!(
        matches!(&result.rejected[0].reason, RejectionReason::RulesNotMet(reasons) if !reasons.is_empty())
    );
}

#[test]
fn promo_rejected_max_uses_exceeded() {
    let promos = vec![Promotion {
        code: "EXHAUSTED".into(),
        discount: LineDiscount::FixedAmount(dec!(10.00)),
        rules: vec![],
        stackable: false,
        max_uses: Some(100),
        current_uses: 100,
    }];
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert!(result.applied.is_empty());
    assert_eq!(result.rejected.len(), 1);
    assert!(matches!(result.rejected[0].reason, RejectionReason::MaxUsesExceeded));
}

#[test]
fn promo_specific_skus_matching() {
    let promos = vec![Promotion {
        code: "WIDGET-DEAL".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![PromotionRule::SpecificSkus(vec!["WIDGET-A".into(), "WIDGET-C".into()])],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &base_promo_context());

    // WIDGET-A is in the context's skus
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.total_discount, dec!(20.00));
}

#[test]
fn promo_specific_skus_non_matching() {
    let promos = vec![Promotion {
        code: "NOPE".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![PromotionRule::SpecificSkus(vec!["SKU-ZZZ".into()])],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert!(result.applied.is_empty());
    assert!(matches!(&result.rejected[0].reason, RejectionReason::RulesNotMet(_)));
}

#[test]
fn promo_date_range_expired() {
    let promos = vec![Promotion {
        code: "EXPIRED".into(),
        discount: LineDiscount::Percentage(dec!(0.25)),
        rules: vec![PromotionRule::DateRange {
            start: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap(),
        }],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    // Context's now = 2026-06-15
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert!(result.applied.is_empty());
}

#[test]
fn promo_date_range_active() {
    let promos = vec![Promotion {
        code: "SUMMER26".into(),
        discount: LineDiscount::Percentage(dec!(0.15)),
        rules: vec![PromotionRule::DateRange {
            start: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 8, 31, 23, 59, 59).unwrap(),
        }],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.applied[0].code, "SUMMER26");
}

#[test]
fn promo_superseded_by_better_deal_reason() {
    let promos = vec![
        Promotion {
            code: "LOSER".into(),
            discount: LineDiscount::FixedAmount(dec!(5.00)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "WINNER".into(),
            discount: LineDiscount::FixedAmount(dec!(50.00)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &base_promo_context());

    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.applied[0].code, "WINNER");
    assert_eq!(result.rejected.len(), 1);
    match &result.rejected[0].reason {
        RejectionReason::SupersededByBetterDeal { winner_code } => {
            assert_eq!(winner_code, "WINNER");
        }
        other => panic!("expected SupersededByBetterDeal, got {other:?}"),
    }
}

// =========================================================================
// 4. Rounding Edge Cases
// =========================================================================

#[test]
fn rounding_half_up_at_midpoint() {
    let policy = RoundingPolicy::new(RoundingMode::HalfUp, 2);
    assert_eq!(round(dec!(2.345), &policy), dec!(2.35));
    assert_eq!(round(dec!(2.335), &policy), dec!(2.34));
    assert_eq!(round(dec!(2.355), &policy), dec!(2.36));
}

#[test]
fn rounding_half_even_at_midpoint() {
    let policy = RoundingPolicy::new(RoundingMode::HalfEven, 2);
    // 2.345 -> 2.34 (4 is even, round down)
    assert_eq!(round(dec!(2.345), &policy), dec!(2.34));
    // 2.355 -> 2.36 (5 is odd, round up to 6)
    assert_eq!(round(dec!(2.355), &policy), dec!(2.36));
    // 2.365 -> 2.36 (6 is even, round down)
    assert_eq!(round(dec!(2.365), &policy), dec!(2.36));
    // 2.375 -> 2.38 (7 is odd, round up to 8)
    assert_eq!(round(dec!(2.375), &policy), dec!(2.38));
}

#[test]
fn rounding_zero_quantity_line_item() {
    let li = item("ZERO-QTY", dec!(99.99), 0, None, Some(dec!(0.10)));
    assert_eq!(li.subtotal(), Decimal::ZERO);
    assert_eq!(li.discount_amount(), Decimal::ZERO);
    assert_eq!(li.taxable_amount(), Decimal::ZERO);
    assert_eq!(li.tax_amount(), Decimal::ZERO);
    assert_eq!(li.total(), Decimal::ZERO);
}

#[test]
fn rounding_very_large_amount() {
    let li = item("BIG", dec!(999_999_999.99), 1, None, Some(dec!(0.08)));
    let expected_tax = dec!(999_999_999.99) * dec!(0.08);
    assert_eq!(li.tax_amount(), expected_tax);
    assert_eq!(li.total(), dec!(999_999_999.99) + expected_tax);
}

#[test]
fn rounding_very_small_amount() {
    let li = item("TINY", dec!(0.001), 1, None, Some(dec!(0.10)));
    assert_eq!(li.subtotal(), dec!(0.001));
    assert_eq!(li.tax_amount(), dec!(0.0001));
    assert_eq!(li.total(), dec!(0.0011));

    // With USD rounding
    let policy = RoundingPolicy::usd();
    let rounded_total = li.total_rounded(&policy);
    // taxable = round(0.001, 2dp) = 0.00, tax = round(0.00 * 0.10, 2dp) = 0.00
    assert_eq!(rounded_total, dec!(0.00));
}

#[test]
fn rounding_jpy_zero_decimal_places() {
    let policy = RoundingPolicy::jpy();
    assert_eq!(round(dec!(100.5), &policy), dec!(101));
    assert_eq!(round(dec!(100.4), &policy), dec!(100));
    assert_eq!(round(dec!(99.999), &policy), dec!(100));

    // In an order
    let items = vec![item("JPY-ITEM", dec!(1999), 1, None, Some(dec!(0.10)))];
    let input = OrderTotalInput {
        items,
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::jpy(),
    };
    let total = compute_order_total(&input);
    // tax = 1999 * 0.10 = 199.9 -> rounded to 200
    assert_eq!(total.total_tax, dec!(200));
    assert_eq!(total.grand_total, dec!(2199));
}

#[test]
fn rounding_bhd_three_decimal_places() {
    let policy = RoundingPolicy::bhd();
    assert_eq!(round(dec!(1.23456), &policy), dec!(1.235));
    assert_eq!(round(dec!(1.23449), &policy), dec!(1.234));

    let rules = vec![TaxRule {
        jurisdiction: "BH".into(),
        rate: dec!(0.05),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.000), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::bhd());
    assert_eq!(result.total_tax, dec!(5.000));
}

// =========================================================================
// 5. Currency Conversion
// =========================================================================

#[test]
fn currency_direct_conversion_usd_to_eur() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(make_rate("USD", "EUR", dec!(0.920000)));

    let result = converter.convert(dec!(100.00), "USD", "EUR").unwrap();
    assert_eq!(result.amount, dec!(92.000000));
    assert_eq!(result.rate, dec!(0.920000));
    assert_eq!(result.from, "USD");
    assert_eq!(result.to, "EUR");
}

#[test]
fn currency_inverse_conversion_eur_to_usd() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(make_rate("USD", "EUR", dec!(0.920000)));

    // EUR -> USD should use inverted rate: 1/0.92 = 1.08695652173913...
    let result = converter.convert(dec!(92.00), "EUR", "USD").unwrap();
    // 92 / 0.92 = 100
    assert_eq!(result.amount, dec!(100.00));
    // Verify inverse rate precision
    assert!(result.rate > dec!(1.086));
    assert!(result.rate < dec!(1.087));
}

#[test]
fn currency_triangulation_eur_to_gbp_via_usd() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(make_rate("USD", "EUR", dec!(0.920000)));
    converter.add_rate(make_rate("USD", "GBP", dec!(0.790000)));

    // EUR -> GBP: EUR -> USD (1/0.92) then USD -> GBP (0.79)
    let result = converter.convert(dec!(100.00), "EUR", "GBP").unwrap();
    // 100 * (1/0.92) * 0.79 = 100 * 1.08695... * 0.79 = 85.869...
    assert!(result.amount > dec!(85.80));
    assert!(result.amount < dec!(85.90));
}

#[test]
fn currency_same_currency_identity() {
    let converter = CurrencyConverter::new();
    let result = converter.convert(dec!(42.50), "USD", "USD").unwrap();
    assert_eq!(result.amount, dec!(42.50));
    assert_eq!(result.rate, Decimal::ONE);
}

#[test]
fn currency_missing_rate_returns_error() {
    let converter = CurrencyConverter::new();
    let result = converter.convert(dec!(100.00), "USD", "XYZ");
    assert!(result.is_err());
    match result {
        Err(PricingError::NoExchangeRate { from, to }) => {
            assert_eq!(from, "USD");
            assert_eq!(to, "XYZ");
        }
        other => panic!("expected NoExchangeRate, got {other:?}"),
    }
}

#[test]
fn currency_rate_precision_six_plus_decimals() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(make_rate("USD", "BTC", dec!(0.000015)));

    let result = converter.convert(dec!(50000.00), "USD", "BTC").unwrap();
    // 50000 * 0.000015 = 0.75
    assert_eq!(result.amount, dec!(0.750000));
    assert_eq!(result.rate, dec!(0.000015));
}

#[test]
fn currency_inverse_precision() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(make_rate("USD", "EUR", dec!(0.923456)));

    let result = converter.convert(dec!(1.00), "EUR", "USD").unwrap();
    // 1 / 0.923456 = 1.08288...
    assert!(result.rate > dec!(1.0828));
    assert!(result.rate < dec!(1.0829));
}

// =========================================================================
// 6. Serde Round-trip
// =========================================================================

#[test]
fn serde_roundtrip_line_item() {
    let li = LineItem {
        sku: "SERDE-001".into(),
        name: "Serde Widget".into(),
        unit_price: dec!(49.99),
        quantity: 3,
        discount: Some(LineDiscount::Percentage(dec!(0.15))),
        tax_rate: Some(dec!(0.0825)),
    };
    let json = serde_json::to_string(&li).unwrap();
    let parsed: LineItem = serde_json::from_str(&json).unwrap();
    assert_eq!(li, parsed);
}

#[test]
fn serde_roundtrip_line_item_fixed_price_discount() {
    let li = LineItem {
        sku: "FP-001".into(),
        name: "Fixed Price Item".into(),
        unit_price: dec!(100.00),
        quantity: 1,
        discount: Some(LineDiscount::FixedPrice(dec!(79.99))),
        tax_rate: None,
    };
    let json = serde_json::to_string(&li).unwrap();
    let parsed: LineItem = serde_json::from_str(&json).unwrap();
    assert_eq!(li, parsed);
}

#[test]
fn serde_roundtrip_order_total() {
    let total = OrderTotal {
        subtotal: dec!(250.00),
        total_discount: dec!(25.00),
        total_tax: dec!(18.00),
        shipping: dec!(9.99),
        shipping_tax: dec!(0.80),
        fees: dec!(5.00),
        grand_total: dec!(258.79),
    };
    let json = serde_json::to_string(&total).unwrap();
    let parsed: OrderTotal = serde_json::from_str(&json).unwrap();
    assert_eq!(total, parsed);
}

#[test]
fn serde_roundtrip_promotion() {
    let promo = Promotion {
        code: "SUMMER26".into(),
        discount: LineDiscount::Percentage(dec!(0.15)),
        rules: vec![
            PromotionRule::MinimumOrderTotal(dec!(50.00)),
            PromotionRule::SpecificSkus(vec!["SKU-A".into(), "SKU-B".into()]),
            PromotionRule::DateRange {
                start: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2026, 8, 31, 23, 59, 59).unwrap(),
            },
            PromotionRule::CustomerGroup("VIP".into()),
        ],
        stackable: true,
        max_uses: Some(1000),
        current_uses: 42,
    };
    let json = serde_json::to_string(&promo).unwrap();
    let parsed: Promotion = serde_json::from_str(&json).unwrap();
    assert_eq!(promo, parsed);
}

#[test]
fn serde_roundtrip_tax_rule() {
    let rule = TaxRule {
        jurisdiction: "CA-STATE".into(),
        rate: dec!(0.0725),
        applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into(), "clothing".into()]),
        compound: true,
    };
    let json = serde_json::to_string(&rule).unwrap();
    let parsed: TaxRule = serde_json::from_str(&json).unwrap();
    assert_eq!(rule, parsed);
}

#[test]
fn serde_roundtrip_promotion_result_with_rejections() {
    let result = PromotionResult {
        applied: vec![AppliedPromotion {
            code: "WINNER".into(),
            discount_amount: dec!(30.00),
            discount: LineDiscount::FixedAmount(dec!(30.00)),
        }],
        rejected: vec![RejectedPromotion {
            code: "LOSER".into(),
            reason: RejectionReason::SupersededByBetterDeal { winner_code: "WINNER".into() },
        }],
        total_discount: dec!(30.00),
    };
    let json = serde_json::to_string(&result).unwrap();
    let parsed: PromotionResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result, parsed);
}

#[test]
fn serde_roundtrip_tax_result() {
    let result = TaxResult {
        tax_lines: vec![
            TaxLine {
                jurisdiction: "State".into(),
                rate: dec!(0.06),
                taxable_amount: dec!(100.00),
                tax_amount: dec!(6.00),
            },
            TaxLine {
                jurisdiction: "County".into(),
                rate: dec!(0.02),
                taxable_amount: dec!(106.00),
                tax_amount: dec!(2.12),
            },
        ],
        total_tax: dec!(8.12),
    };
    let json = serde_json::to_string(&result).unwrap();
    let parsed: TaxResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result, parsed);
}

#[test]
fn serde_roundtrip_exchange_rate() {
    let rate = ExchangeRate {
        from: "USD".into(),
        to: "JPY".into(),
        rate: dec!(149.123456),
        as_of: Utc.with_ymd_and_hms(2026, 2, 23, 10, 30, 0).unwrap(),
    };
    let json = serde_json::to_string(&rate).unwrap();
    let parsed: ExchangeRate = serde_json::from_str(&json).unwrap();
    assert_eq!(rate, parsed);
}

// =========================================================================
// Additional integration scenarios (cross-module)
// =========================================================================

#[test]
fn end_to_end_realistic_order() {
    // Simulate a realistic e-commerce order with promotions, tax, and currency
    // Step 1: Evaluate promotions
    let promos = vec![
        Promotion {
            code: "VIP10".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![
                PromotionRule::CustomerGroup("VIP".into()),
                PromotionRule::MinimumOrderTotal(dec!(100.00)),
            ],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "SUMMER5".into(),
            discount: LineDiscount::FixedAmount(dec!(5.00)),
            rules: vec![PromotionRule::DateRange {
                start: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap(),
            }],
            stackable: true,
            max_uses: Some(10000),
            current_uses: 500,
        },
    ];
    let promo_ctx = base_promo_context(); // order_total=200, VIP, now=2026-06-15
    let promo_result = evaluate_promotions(&promos, &promo_ctx);

    assert_eq!(promo_result.applied.len(), 2);
    // VIP10: 200 * 0.10 = 20, SUMMER5: 5
    assert_eq!(promo_result.total_discount, dec!(25.00));

    // Step 2: Build order with the promo discount as an order-level discount
    let items = vec![
        item("LAPTOP", dec!(150.00), 1, None, Some(dec!(0.08))),
        item("MOUSE", dec!(25.00), 2, None, Some(dec!(0.08))),
    ];
    let mut input = order_input(items);
    input.order_discount = Some(LineDiscount::FixedAmount(dec!(25.00)));
    input.shipping_cost = dec!(15.00);
    input.shipping_tax_rate = Some(dec!(0.08));

    let total = compute_order_total(&input);

    // Subtotal: 150 + 50 = 200, line_taxable = 200
    // Order discount: 25
    // Effective taxable: 175
    // Line tax raw: 200*0.08 = 16, adjusted: 16 * 175/200 = 14.00
    assert_eq!(total.subtotal, dec!(200.00));
    assert_eq!(total.total_discount, dec!(25.00));
    assert_eq!(total.total_tax, dec!(14.00));
    assert_eq!(total.shipping, dec!(15.00));
    assert_eq!(total.shipping_tax, dec!(1.20));
    // grand = 175 + 14 + 15 + 1.20 = 205.20
    assert_eq!(total.grand_total, dec!(205.20));
}

#[test]
fn order_with_all_discount_types_simultaneously() {
    // Item-level discounts of each type + order-level discount + shipping + fees
    let items = vec![
        item("PCT", dec!(80.00), 1, Some(LineDiscount::Percentage(dec!(0.25))), Some(dec!(0.10))),
        item("FIX", dec!(60.00), 2, Some(LineDiscount::FixedAmount(dec!(20.00))), Some(dec!(0.10))),
        item("PRC", dec!(45.00), 3, Some(LineDiscount::FixedPrice(dec!(30.00))), Some(dec!(0.10))),
        item("NON", dec!(20.00), 5, None, Some(dec!(0.10))),
    ];
    let input = OrderTotalInput {
        items,
        shipping_cost: dec!(8.50),
        shipping_tax_rate: Some(dec!(0.10)),
        order_discount: Some(LineDiscount::Percentage(dec!(0.05))),
        fees: vec![
            Fee { name: "Gift Wrap".into(), amount: dec!(3.99) },
            Fee { name: "Priority".into(), amount: dec!(2.00) },
        ],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);

    // PCT: sub=80, disc=20, taxable=60
    // FIX: sub=120, disc=20, taxable=100
    // PRC: sub=135, disc=45 (135 - 30*3 = 135-90=45), taxable=90
    // NON: sub=100, disc=0, taxable=100
    // Line totals: subtotal=435, line_disc=85, line_taxable=350
    assert_eq!(total.subtotal, dec!(435.00));

    // line_tax: 60*0.10 + 100*0.10 + 90*0.10 + 100*0.10 = 6+10+9+10 = 35.00
    // Order discount: 350 * 0.05 = 17.50
    // Total discount: 85 + 17.50 = 102.50
    assert_eq!(total.total_discount, dec!(102.50));

    // Effective taxable: 350 - 17.50 = 332.50
    // Adjusted tax: 35.00 * 332.50/350 = 33.25
    assert_eq!(total.total_tax, dec!(33.25));

    // Shipping: 8.50, shipping_tax: 0.85
    // Fees: 3.99 + 2.00 = 5.99
    assert_eq!(total.shipping, dec!(8.50));
    assert_eq!(total.shipping_tax, dec!(0.85));
    assert_eq!(total.fees, dec!(5.99));

    // Grand: 332.50 + 33.25 + 8.50 + 0.85 + 5.99 = 381.09
    assert_eq!(total.grand_total, dec!(381.09));
}

#[test]
fn empty_order_with_only_fees() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![Fee { name: "Account Setup".into(), amount: dec!(25.00) }],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);

    assert_eq!(total.subtotal, Decimal::ZERO);
    assert_eq!(total.total_discount, Decimal::ZERO);
    assert_eq!(total.total_tax, Decimal::ZERO);
    assert_eq!(total.fees, dec!(25.00));
    assert_eq!(total.grand_total, dec!(25.00));
}
