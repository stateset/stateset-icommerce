//! Expanded pricing tests covering edge cases, multi-currency, promotions,
//! tax jurisdictions, currency conversion, and rounding.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_pricing::*;

// ---------------------------------------------------------------------------
// 1. Line item with discounts and tax
// ---------------------------------------------------------------------------

#[test]
fn line_item_percentage_discount_with_tax() {
    let item = LineItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        unit_price: dec!(50.00),
        quantity: 4,
        discount: Some(LineDiscount::Percentage(dec!(0.25))),
        tax_rate: Some(dec!(0.0975)),
    };
    // subtotal = 200, discount = 50, taxable = 150, tax = 14.625, total = 164.625
    assert_eq!(item.subtotal(), dec!(200.00));
    assert_eq!(item.discount_amount(), dec!(50.00));
    assert_eq!(item.taxable_amount(), dec!(150.00));
    assert_eq!(item.tax_amount(), dec!(14.625));
    assert_eq!(item.total(), dec!(164.625));
}

#[test]
fn line_item_fixed_amount_discount_with_tax() {
    let item = LineItem {
        sku: "SKU-002".into(),
        name: "Gadget".into(),
        unit_price: dec!(75.00),
        quantity: 2,
        discount: Some(LineDiscount::FixedAmount(dec!(25.00))),
        tax_rate: Some(dec!(0.06)),
    };
    // subtotal = 150, discount = 25, taxable = 125, tax = 7.50, total = 132.50
    assert_eq!(item.subtotal(), dec!(150.00));
    assert_eq!(item.discount_amount(), dec!(25.00));
    assert_eq!(item.taxable_amount(), dec!(125.00));
    assert_eq!(item.tax_amount(), dec!(7.50));
    assert_eq!(item.total(), dec!(132.50));
}

#[test]
fn line_item_fixed_price_override_with_tax() {
    let item = LineItem {
        sku: "SKU-003".into(),
        name: "Doohickey".into(),
        unit_price: dec!(100.00),
        quantity: 3,
        discount: Some(LineDiscount::FixedPrice(dec!(79.99))),
        tax_rate: Some(dec!(0.08)),
    };
    // subtotal = 300, new total = 79.99*3 = 239.97, discount = 60.03
    // taxable = 239.97, tax = 19.1976, total = 259.1676
    assert_eq!(item.subtotal(), dec!(300.00));
    assert_eq!(item.discount_amount(), dec!(60.03));
    assert_eq!(item.taxable_amount(), dec!(239.97));
}

#[test]
fn line_item_zero_unit_price_is_zero_total() {
    let item = LineItem {
        sku: "FREE".into(),
        name: "Free Sample".into(),
        unit_price: Decimal::ZERO,
        quantity: 10,
        discount: None,
        tax_rate: Some(dec!(0.10)),
    };
    assert_eq!(item.subtotal(), Decimal::ZERO);
    assert_eq!(item.tax_amount(), Decimal::ZERO);
    assert_eq!(item.total(), Decimal::ZERO);
}

#[test]
fn line_item_100_percent_discount_results_in_zero() {
    let item = LineItem {
        sku: "PROMO".into(),
        name: "Promo Item".into(),
        unit_price: dec!(99.99),
        quantity: 1,
        discount: Some(LineDiscount::Percentage(Decimal::ONE)),
        tax_rate: Some(dec!(0.08)),
    };
    assert_eq!(item.taxable_amount(), Decimal::ZERO);
    assert_eq!(item.tax_amount(), Decimal::ZERO);
    assert_eq!(item.total(), Decimal::ZERO);
}

#[test]
fn line_item_validate_rejects_negative_price() {
    let item = LineItem {
        sku: "NEG".into(),
        name: "Negative".into(),
        unit_price: dec!(-10.00),
        quantity: 1,
        discount: None,
        tax_rate: None,
    };
    assert!(item.validate().is_err());
}

#[test]
fn line_item_validate_rejects_zero_quantity() {
    let item = LineItem {
        sku: "ZERO".into(),
        name: "Zero Qty".into(),
        unit_price: dec!(10.00),
        quantity: 0,
        discount: None,
        tax_rate: None,
    };
    assert!(item.validate().is_err());
}

// ---------------------------------------------------------------------------
// 2. Multi-currency order total with exchange rates
// ---------------------------------------------------------------------------

#[test]
fn order_total_multi_item_different_tax_rates() {
    let input = OrderTotalInput {
        items: vec![
            LineItem {
                sku: "A".into(),
                name: "Item A".into(),
                unit_price: dec!(100.00),
                quantity: 1,
                discount: None,
                tax_rate: Some(dec!(0.05)),
            },
            LineItem {
                sku: "B".into(),
                name: "Item B".into(),
                unit_price: dec!(200.00),
                quantity: 2,
                discount: Some(LineDiscount::Percentage(dec!(0.10))),
                tax_rate: Some(dec!(0.10)),
            },
        ],
        shipping_cost: dec!(12.50),
        shipping_tax_rate: Some(dec!(0.05)),
        order_discount: None,
        fees: vec![Fee { name: "Handling".into(), amount: dec!(3.00) }],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);
    // A: sub=100, tax=5.00
    // B: sub=400, disc=40, taxable=360, tax=36.00
    // Combined: sub=500, disc=40, taxable=460, tax=41.00
    assert_eq!(total.subtotal, dec!(500.00));
}

#[test]
fn order_total_with_jpy_rounding() {
    let input = OrderTotalInput {
        items: vec![LineItem {
            sku: "JP".into(),
            name: "Japanese Item".into(),
            unit_price: dec!(1500),
            quantity: 3,
            discount: Some(LineDiscount::Percentage(dec!(0.15))),
            tax_rate: Some(dec!(0.10)),
        }],
        shipping_cost: dec!(500),
        shipping_tax_rate: Some(dec!(0.10)),
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::jpy(),
    };
    let total = compute_order_total(&input);
    // sub=4500, disc=675, taxable=3825, tax=383 (rounded)
    assert_eq!(total.subtotal, dec!(4500));
    assert_eq!(total.total_discount, dec!(675));
}

#[test]
fn currency_conversion_usd_to_eur_and_back() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(ExchangeRate {
        from: "USD".into(),
        to: "EUR".into(),
        rate: dec!(0.92),
        as_of: Utc::now(),
    });
    let eur = converter.convert(dec!(100.00), "USD", "EUR").unwrap();
    assert_eq!(eur.amount, dec!(92.00));

    // Convert back
    let usd = converter.convert(eur.amount, "EUR", "USD").unwrap();
    // 92 / 0.92 = 100
    assert_eq!(usd.amount, dec!(100.00));
}

#[test]
fn currency_conversion_triangulation_gbp_to_jpy() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(ExchangeRate {
        from: "USD".into(),
        to: "GBP".into(),
        rate: dec!(0.79),
        as_of: Utc::now(),
    });
    converter.add_rate(ExchangeRate {
        from: "USD".into(),
        to: "JPY".into(),
        rate: dec!(150.25),
        as_of: Utc::now(),
    });
    // GBP -> JPY via USD
    let result = converter.convert(dec!(100.00), "GBP", "JPY").unwrap();
    // GBP -> USD: 100 / 0.79 = ~126.58, USD -> JPY: 126.58 * 150.25 = ~19019
    assert!(result.amount > dec!(19000.00));
    assert!(result.amount < dec!(19100.00));
}

#[test]
fn currency_conversion_same_currency_identity() {
    let converter = CurrencyConverter::new();
    let result = converter.convert(dec!(42.50), "USD", "usd").unwrap();
    assert_eq!(result.amount, dec!(42.50));
    assert_eq!(result.rate, Decimal::ONE);
}

#[test]
fn currency_conversion_missing_rate_fails() {
    let converter = CurrencyConverter::new();
    let result = converter.convert(dec!(100.00), "USD", "XYZ");
    assert!(result.is_err());
    assert!(matches!(result, Err(PricingError::NoExchangeRate { .. })));
}

#[test]
fn currency_conversion_zero_amount() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(ExchangeRate {
        from: "USD".into(),
        to: "EUR".into(),
        rate: dec!(0.92),
        as_of: Utc::now(),
    });
    let result = converter.convert(Decimal::ZERO, "USD", "EUR").unwrap();
    assert_eq!(result.amount, Decimal::ZERO);
}

// ---------------------------------------------------------------------------
// 3. Promotion evaluation
// ---------------------------------------------------------------------------

fn promo_context() -> PromotionContext {
    PromotionContext {
        order_total: dec!(200.00),
        item_count: 5,
        skus: vec!["WIDGET-A".into(), "GADGET-B".into()],
        now: Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap(),
        customer_group: Some("VIP".into()),
        is_first_order: false,
    }
}

#[test]
fn promotion_percentage_off() {
    let promos = vec![Promotion {
        code: "SAVE20".into(),
        discount: LineDiscount::Percentage(dec!(0.20)),
        rules: vec![PromotionRule::MinimumOrderTotal(dec!(100.00))],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.total_discount, dec!(40.00));
}

#[test]
fn promotion_fixed_amount_off() {
    let promos = vec![Promotion {
        code: "FLAT15".into(),
        discount: LineDiscount::FixedAmount(dec!(15.00)),
        rules: vec![],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.total_discount, dec!(15.00));
}

#[test]
fn promotion_buy_x_get_y_via_fixed_price() {
    // Simulate "buy X get Y" by setting a fixed price
    let promos = vec![Promotion {
        code: "BOGO".into(),
        discount: LineDiscount::FixedPrice(dec!(150.00)),
        rules: vec![PromotionRule::MinimumQuantity(3)],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.total_discount, dec!(50.00)); // 200 - 150
}

#[test]
fn promotion_sku_specific_match() {
    let promos = vec![Promotion {
        code: "WIDGET10".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![PromotionRule::SpecificSkus(vec!["WIDGET-A".into()])],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 1);
}

#[test]
fn promotion_sku_specific_no_match() {
    let promos = vec![Promotion {
        code: "ZETA10".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![PromotionRule::SpecificSkus(vec!["ZETA-99".into()])],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 0);
    assert_eq!(result.rejected.len(), 1);
}

#[test]
fn promotion_date_range_active() {
    let promos = vec![Promotion {
        code: "SUMMER".into(),
        discount: LineDiscount::Percentage(dec!(0.15)),
        rules: vec![PromotionRule::DateRange {
            start: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 8, 31, 23, 59, 59).unwrap(),
        }],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 1);
}

#[test]
fn promotion_date_range_expired() {
    let promos = vec![Promotion {
        code: "WINTER".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![PromotionRule::DateRange {
            start: Utc.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 2, 28, 23, 59, 59).unwrap(),
        }],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 0);
}

#[test]
fn promotion_first_order_with_vip() {
    let mut ctx = promo_context();
    ctx.is_first_order = true;
    let promos = vec![Promotion {
        code: "WELCOME".into(),
        discount: LineDiscount::FixedAmount(dec!(25.00)),
        rules: vec![PromotionRule::FirstOrder, PromotionRule::CustomerGroup("VIP".into())],
        stackable: false,
        max_uses: None,
        current_uses: 0,
    }];
    let result = evaluate_promotions(&promos, &ctx);
    assert_eq!(result.applied.len(), 1);
}

#[test]
fn promotion_non_stackable_best_wins() {
    let promos = vec![
        Promotion {
            code: "SMALL".into(),
            discount: LineDiscount::FixedAmount(dec!(10.00)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "BIG".into(),
            discount: LineDiscount::FixedAmount(dec!(50.00)),
            rules: vec![],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.applied[0].code, "BIG");
    assert_eq!(result.total_discount, dec!(50.00));
    // SMALL should be superseded
    assert!(result.rejected.iter().any(|r| r.code == "SMALL"));
}

#[test]
fn promotion_stackable_cumulative_capped() {
    let promos = vec![
        Promotion {
            code: "A".into(),
            discount: LineDiscount::FixedAmount(dec!(120.00)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
        Promotion {
            code: "B".into(),
            discount: LineDiscount::FixedAmount(dec!(120.00)),
            rules: vec![],
            stackable: true,
            max_uses: None,
            current_uses: 0,
        },
    ];
    let result = evaluate_promotions(&promos, &promo_context());
    // Total discount capped at order total (200)
    assert_eq!(result.total_discount, dec!(200.00));
    assert_eq!(result.applied[0].discount_amount, dec!(120.00));
    assert_eq!(result.applied[1].discount_amount, dec!(80.00));
}

#[test]
fn promotion_max_uses_at_limit() {
    let promos = vec![Promotion {
        code: "LIMITED".into(),
        discount: LineDiscount::Percentage(dec!(0.10)),
        rules: vec![],
        stackable: false,
        max_uses: Some(50),
        current_uses: 50,
    }];
    let result = evaluate_promotions(&promos, &promo_context());
    assert_eq!(result.applied.len(), 0);
    assert!(matches!(result.rejected[0].reason, RejectionReason::MaxUsesExceeded));
}

// ---------------------------------------------------------------------------
// 4. Tax calculation with multiple jurisdictions
// ---------------------------------------------------------------------------

#[test]
fn tax_state_plus_county_non_compound() {
    let rules = vec![
        TaxRule {
            jurisdiction: "State".into(),
            rate: dec!(0.0625),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "County".into(),
            rate: dec!(0.02),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
    ];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    assert_eq!(result.tax_lines.len(), 2);
    assert_eq!(result.tax_lines[0].tax_amount, dec!(6.25));
    assert_eq!(result.tax_lines[1].tax_amount, dec!(2.00));
    assert_eq!(result.total_tax, dec!(8.25));
}

#[test]
fn tax_canadian_gst_qst_compound() {
    let rules = vec![
        TaxRule {
            jurisdiction: "GST".into(),
            rate: dec!(0.05),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "QST".into(),
            rate: dec!(0.09975),
            applies_to: TaxAppliesTo::AllItems,
            compound: true,
        },
    ];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(200.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    // GST: 200 * 0.05 = 10.00
    // QST: (200 + 10) * 0.09975 = 20.9475 -> 20.95
    assert_eq!(result.tax_lines[0].tax_amount, dec!(10.00));
    assert_eq!(result.tax_lines[1].tax_amount, dec!(20.95));
    assert_eq!(result.total_tax, dec!(30.95));
}

#[test]
fn tax_category_electronics_only() {
    let rules = vec![TaxRule {
        jurisdiction: "Electronics Tax".into(),
        rate: dec!(0.12),
        applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into()]),
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![
            TaxableItem {
                amount: dec!(500.00),
                category: Some("electronics".into()),
                exempt: false,
            },
            TaxableItem { amount: dec!(100.00), category: Some("clothing".into()), exempt: false },
        ],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    assert_eq!(result.tax_lines.len(), 1);
    assert_eq!(result.tax_lines[0].taxable_amount, dec!(500.00));
    assert_eq!(result.total_tax, dec!(60.00));
}

#[test]
fn tax_shipping_only_rule() {
    let rules = vec![
        TaxRule {
            jurisdiction: "Item Tax".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "Shipping Tax".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::ShippingOnly,
            compound: false,
        },
    ];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(50.00), category: None, exempt: false }],
        shipping: dec!(10.00),
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    assert_eq!(result.tax_lines[0].tax_amount, dec!(4.00));
    assert_eq!(result.tax_lines[1].tax_amount, dec!(0.80));
    assert_eq!(result.total_tax, dec!(4.80));
}

#[test]
fn tax_exempt_item_not_taxed() {
    let rules = vec![TaxRule {
        jurisdiction: "CA".into(),
        rate: dec!(0.0725),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![
            TaxableItem { amount: dec!(100.00), category: None, exempt: false },
            TaxableItem { amount: dec!(50.00), category: None, exempt: true },
        ],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    assert_eq!(result.tax_lines[0].taxable_amount, dec!(100.00));
    assert_eq!(result.total_tax, dec!(7.25));
}

#[test]
fn tax_all_exempt_no_tax() {
    let rules = vec![TaxRule {
        jurisdiction: "CA".into(),
        rate: dec!(0.10),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.00), category: None, exempt: true }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::usd());
    assert!(result.tax_lines.is_empty());
    assert_eq!(result.total_tax, Decimal::ZERO);
}

#[test]
fn tax_bhd_three_decimal_rounding() {
    let rules = vec![TaxRule {
        jurisdiction: "BH".into(),
        rate: dec!(0.05),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(123.456), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&rules, &ctx, &RoundingPolicy::bhd());
    // 123.456 * 0.05 = 6.1728 -> 6.173 (3 decimal places, half-up)
    assert_eq!(result.total_tax, dec!(6.173));
}

// ---------------------------------------------------------------------------
// 5. Currency conversion and rounding
// ---------------------------------------------------------------------------

#[test]
fn rounding_half_up_standard() {
    let policy = RoundingPolicy::usd();
    assert_eq!(round(dec!(1.235), &policy), dec!(1.24));
    assert_eq!(round(dec!(1.234), &policy), dec!(1.23));
    assert_eq!(round(dec!(1.245), &policy), dec!(1.25));
}

#[test]
fn rounding_bankers_half_even() {
    let policy = RoundingPolicy::new(RoundingMode::HalfEven, 2);
    assert_eq!(round(dec!(2.345), &policy), dec!(2.34)); // 4 is even
    assert_eq!(round(dec!(2.355), &policy), dec!(2.36)); // 6 is even
    assert_eq!(round(dec!(2.365), &policy), dec!(2.36)); // 6 is even
    assert_eq!(round(dec!(2.375), &policy), dec!(2.38)); // 8 is even
}

#[test]
fn rounding_truncate_down() {
    let policy = RoundingPolicy::new(RoundingMode::Down, 2);
    assert_eq!(round(dec!(1.999), &policy), dec!(1.99));
    assert_eq!(round(dec!(-1.999), &policy), dec!(-1.99));
}

#[test]
fn rounding_ceiling_up() {
    let policy = RoundingPolicy::new(RoundingMode::Up, 2);
    assert_eq!(round(dec!(1.001), &policy), dec!(1.01));
    assert_eq!(round(dec!(-1.001), &policy), dec!(-1.01));
    // Exact value: no change
    assert_eq!(round(dec!(1.50), &policy), dec!(1.50));
}

#[test]
fn rounding_jpy_zero_decimals() {
    let policy = RoundingPolicy::jpy();
    assert_eq!(round(dec!(100.4), &policy), dec!(100));
    assert_eq!(round(dec!(100.5), &policy), dec!(101));
    assert_eq!(round(dec!(100.99), &policy), dec!(101));
}

#[test]
fn minor_units_lookup() {
    assert_eq!(minor_units_for_currency("USD"), 2);
    assert_eq!(minor_units_for_currency("EUR"), 2);
    assert_eq!(minor_units_for_currency("JPY"), 0);
    assert_eq!(minor_units_for_currency("KRW"), 0);
    assert_eq!(minor_units_for_currency("VND"), 0);
    assert_eq!(minor_units_for_currency("BHD"), 3);
    assert_eq!(minor_units_for_currency("KWD"), 3);
    assert_eq!(minor_units_for_currency("OMR"), 3);
    assert_eq!(minor_units_for_currency("TND"), 3);
    // Unknown defaults to 2
    assert_eq!(minor_units_for_currency("XYZ"), 2);
}

// ---------------------------------------------------------------------------
// 6. Edge cases: zero amounts, overflow
// ---------------------------------------------------------------------------

#[test]
fn order_total_empty_no_items() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);
    assert_eq!(total.subtotal, Decimal::ZERO);
    assert_eq!(total.grand_total, Decimal::ZERO);
}

#[test]
fn order_total_only_shipping() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: dec!(9.99),
        shipping_tax_rate: Some(dec!(0.08)),
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);
    assert_eq!(total.shipping, dec!(9.99));
    assert_eq!(total.shipping_tax, dec!(0.80));
    assert_eq!(total.grand_total, dec!(10.79));
}

#[test]
fn order_total_only_fees() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![
            Fee { name: "A".into(), amount: dec!(1.50) },
            Fee { name: "B".into(), amount: dec!(2.50) },
        ],
        rounding: RoundingPolicy::usd(),
    };
    let total = compute_order_total(&input);
    assert_eq!(total.fees, dec!(4.00));
    assert_eq!(total.grand_total, dec!(4.00));
}

#[test]
fn try_compute_rejects_negative_fee() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![Fee { name: "Bad".into(), amount: dec!(-1.00) }],
        rounding: RoundingPolicy::usd(),
    };
    assert!(try_compute_order_total(&input).is_err());
}

#[test]
fn try_compute_rejects_negative_shipping() {
    let input = OrderTotalInput {
        items: vec![],
        shipping_cost: dec!(-5.00),
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::usd(),
    };
    assert!(try_compute_order_total(&input).is_err());
}

#[test]
fn promotion_empty_promos_list() {
    let result = evaluate_promotions(&[], &promo_context());
    assert_eq!(result.applied.len(), 0);
    assert_eq!(result.rejected.len(), 0);
    assert_eq!(result.total_discount, Decimal::ZERO);
}

#[test]
fn tax_no_rules_no_tax() {
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    let result = calculate_tax(&[], &ctx, &RoundingPolicy::usd());
    assert!(result.tax_lines.is_empty());
    assert_eq!(result.total_tax, Decimal::ZERO);
}

#[test]
fn tax_rejects_invalid_rate() {
    let rules = vec![TaxRule {
        jurisdiction: "BAD".into(),
        rate: dec!(1.50),
        applies_to: TaxAppliesTo::AllItems,
        compound: false,
    }];
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(100.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    assert!(try_calculate_tax(&rules, &ctx, &RoundingPolicy::usd()).is_err());
}

#[test]
fn tax_rejects_negative_amount() {
    let ctx = TaxContext {
        items: vec![TaxableItem { amount: dec!(-10.00), category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    assert!(try_calculate_tax(&[], &ctx, &RoundingPolicy::usd()).is_err());
}

#[test]
fn error_display_messages() {
    let e1 = PricingError::invalid_discount(dec!(1.5));
    assert!(e1.to_string().contains("1.5"));

    let e2 = PricingError::no_exchange_rate("USD", "XYZ");
    assert!(e2.to_string().contains("USD"));
    assert!(e2.to_string().contains("XYZ"));

    let e3 = PricingError::InvalidQuantity { value: 0 };
    assert!(e3.to_string().contains("0"));
}
