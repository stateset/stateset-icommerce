use rust_decimal::Decimal;
use stateset_pricing::*;

fn item(price: Decimal, quantity: u32) -> LineItem {
    LineItem {
        sku: "overflow".into(),
        name: "overflow".into(),
        unit_price: price,
        quantity,
        discount: None,
        tax_rate: None,
    }
}

const fn order(items: Vec<LineItem>) -> OrderTotalInput {
    OrderTotalInput {
        items,
        shipping_cost: Decimal::ZERO,
        shipping_tax_rate: None,
        order_discount: None,
        fees: vec![],
        rounding: RoundingPolicy::usd(),
    }
}

#[test]
fn large_line_and_order_values_return_errors() {
    assert!(matches!(
        item(Decimal::MAX, 2).try_subtotal(),
        Err(PricingError::OverflowError { .. })
    ));
    assert!(
        try_compute_order_total(&order(vec![item(Decimal::MAX, 1), item(Decimal::ONE, 1)]))
            .is_err()
    );
    let mut with_fees = order(vec![item(Decimal::MAX, 1)]);
    with_fees.fees.push(Fee { name: "fee".into(), amount: Decimal::ONE });
    assert!(try_compute_order_total(&with_fees).is_err());
    let mut taxed = item(Decimal::MAX, 1);
    taxed.tax_rate = Some(Decimal::ONE);
    assert!(taxed.try_total().is_err());
    assert!(taxed.try_total_rounded(&RoundingPolicy::usd()).is_err());
}

#[test]
fn large_conversion_and_compound_tax_return_errors() {
    let mut converter = CurrencyConverter::new();
    converter.add_rate(ExchangeRate {
        from: "USD".into(),
        to: "EUR".into(),
        rate: Decimal::from(2),
        as_of: chrono::Utc::now(),
    });
    assert!(converter.convert(Decimal::MAX, "USD", "EUR").is_err());
    let rules = vec![
        TaxRule {
            jurisdiction: "first".into(),
            rate: Decimal::ONE,
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        },
        TaxRule {
            jurisdiction: "second".into(),
            rate: Decimal::ONE,
            applies_to: TaxAppliesTo::AllItems,
            compound: true,
        },
    ];
    let context = TaxContext {
        items: vec![TaxableItem { amount: Decimal::MAX, category: None, exempt: false }],
        shipping: Decimal::ZERO,
    };
    assert!(try_calculate_tax(&rules, &context, &RoundingPolicy::usd()).is_err());
}

#[test]
fn pricing_and_money_share_currency_precision() {
    for code in ["USD", "BHD", "KWD", "CLP", "JPY", "BTC", "ETH"] {
        let currency: stateset_primitives::CurrencyCode = code.parse().unwrap();
        let scale = minor_units_for_currency(code);
        assert_eq!(scale, u32::from(currency.decimal_places()));
        assert!(stateset_primitives::Money::try_new(Decimal::new(1, scale), currency).is_ok());
        assert!(stateset_primitives::Money::try_new(Decimal::new(1, scale + 1), currency).is_err());
    }
}
