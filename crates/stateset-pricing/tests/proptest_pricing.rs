//! Property-based tests for the stateset-pricing crate.
//!
//! Uses `proptest` to verify invariants over randomised inputs.

use proptest::prelude::*;
use rust_decimal::Decimal;
use stateset_pricing::*;

// ---------------------------------------------------------------------------
// Helpers & strategies
// ---------------------------------------------------------------------------

/// Generate a non-negative price as Decimal (cents divided by 100).
fn arb_price() -> impl Strategy<Value = Decimal> {
    (0i64..1_000_000i64).prop_map(|cents| Decimal::new(cents, 2))
}

/// Generate a reasonable quantity in [1, 100].
fn arb_qty() -> impl Strategy<Value = u32> {
    1u32..100
}

/// Generate a rate in [0, 1] (percentage expressed as decimal fraction).
fn arb_rate() -> impl Strategy<Value = Decimal> {
    (0i64..100i64).prop_map(|pct| Decimal::new(pct, 2))
}

/// Generate a `LineDiscount` variant.
fn arb_line_discount() -> impl Strategy<Value = LineDiscount> {
    prop_oneof![
        // Percentage discount: 0..100 %
        (0i64..=100i64).prop_map(|p| LineDiscount::Percentage(Decimal::new(p, 2))),
        // Fixed amount: 0..max_subtotal (clamped to positive cents)
        (0i64..1_000_000i64).prop_map(|c| LineDiscount::FixedAmount(Decimal::new(c, 2))),
        // Fixed price: 0..max_subtotal (per-unit override)
        (0i64..1_000_000i64).prop_map(|c| LineDiscount::FixedPrice(Decimal::new(c, 2))),
    ]
}

/// Generate a `LineDiscount` suitable for order-level usage.
fn arb_order_discount() -> impl Strategy<Value = Option<LineDiscount>> {
    prop_oneof![
        2 => Just(None),
        1 => (0i64..=100i64)
            .prop_map(|p| Some(LineDiscount::Percentage(Decimal::new(p, 2)))),
        1 => (0i64..1_000_000i64)
            .prop_map(|c| Some(LineDiscount::FixedAmount(Decimal::new(c, 2)))),
    ]
}

/// Pick a `RoundingPolicy` from the built-in presets plus a custom one.
fn arb_rounding_policy() -> impl Strategy<Value = RoundingPolicy> {
    prop_oneof![
        Just(RoundingPolicy::usd()),
        Just(RoundingPolicy::jpy()),
        Just(RoundingPolicy::bhd()),
        Just(RoundingPolicy::eur()),
        Just(RoundingPolicy::gbp()),
        (0u32..=4u32).prop_map(|dp| { RoundingPolicy::new(RoundingMode::HalfEven, dp) }),
        (0u32..=4u32).prop_map(|dp| { RoundingPolicy::new(RoundingMode::Down, dp) }),
        (0u32..=4u32).prop_map(|dp| { RoundingPolicy::new(RoundingMode::Up, dp) }),
    ]
}

/// Build a `LineItem` from arbitrary pieces.
fn arb_line_item() -> impl Strategy<Value = LineItem> {
    (arb_price(), arb_qty(), prop::option::of(arb_line_discount()), prop::option::of(arb_rate()))
        .prop_map(|(price, qty, discount, tax_rate)| LineItem {
            sku: "PROP".into(),
            name: "PropTest Item".into(),
            unit_price: price,
            quantity: qty,
            discount,
            tax_rate,
        })
}

/// Build a full `OrderTotalInput`.
fn arb_order_total_input() -> impl Strategy<Value = OrderTotalInput> {
    (
        prop::collection::vec(arb_line_item(), 0..6),
        arb_price(),                  // shipping_cost
        prop::option::of(arb_rate()), // shipping_tax_rate
        arb_order_discount(),
        prop::collection::vec(arb_price().prop_map(|a| Fee { name: "F".into(), amount: a }), 0..4),
        arb_rounding_policy(),
    )
        .prop_map(
            |(items, shipping_cost, shipping_tax_rate, order_discount, fees, rounding)| {
                OrderTotalInput {
                    items,
                    shipping_cost,
                    shipping_tax_rate,
                    order_discount,
                    fees,
                    rounding,
                }
            },
        )
}

// ---------------------------------------------------------------------------
// 1. Non-negative line total for non-negative inputs
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn line_total_non_negative(item in arb_line_item()) {
        let total = item.total();
        prop_assert!(
            total >= Decimal::ZERO,
            "line total was negative: {} for item {:?}",
            total,
            item,
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Non-negative grand_total for non-negative inputs
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn grand_total_non_negative(input in arb_order_total_input()) {
        let total = compute_order_total(&input);
        prop_assert!(
            total.grand_total >= Decimal::ZERO,
            "grand_total was negative: {} for input {:?}",
            total.grand_total,
            input,
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Rounding idempotency: round(round(x)) == round(x)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn rounding_is_idempotent(
        cents in -1_000_000_000i64..1_000_000_000i64,
        policy in arb_rounding_policy(),
    ) {
        let amount = Decimal::new(cents, 4); // fine-grained fractional value
        let once = round(amount, &policy);
        let twice = round(once, &policy);
        prop_assert_eq!(
            once, twice,
            "round was not idempotent for amount={}, policy={:?}",
            amount, policy,
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Tax calculation monotonicity
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn tax_monotonicity(
        a_cents in 0i64..500_000,
        delta_cents in 1i64..500_000,
        rate_pct in 1i64..50,
    ) {
        let amount_a = Decimal::new(a_cents, 2);
        let amount_b = Decimal::new(a_cents + delta_cents, 2);
        let rate = Decimal::new(rate_pct, 2);

        let rule = vec![TaxRule {
            jurisdiction: "TEST".into(),
            rate,
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let rounding = RoundingPolicy::usd();

        let ctx_a = TaxContext {
            items: vec![TaxableItem { amount: amount_a, category: None, exempt: false }],
            shipping: Decimal::ZERO,
        };
        let ctx_b = TaxContext {
            items: vec![TaxableItem { amount: amount_b, category: None, exempt: false }],
            shipping: Decimal::ZERO,
        };

        let tax_a = calculate_tax(&rule, &ctx_a, &rounding);
        let tax_b = calculate_tax(&rule, &ctx_b, &rounding);

        prop_assert!(
            tax_a.total_tax <= tax_b.total_tax,
            "tax({amount_a})={} > tax({amount_b})={}",
            tax_a.total_tax,
            tax_b.total_tax,
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Discount monotonicity: higher percentage discount => lower or equal total
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn discount_monotonicity(
        price_cents in 1i64..10_000,
        qty in 1u32..20,
        pct_a_raw in 0i64..100,
        delta in 1i64..100,
        tax_rate_raw in 0i64..50,
    ) {
        let unit_price = Decimal::new(price_cents, 2);
        let pct_a = Decimal::new(pct_a_raw.min(99), 2);
        let pct_b = Decimal::new((pct_a_raw + delta).min(100), 2);
        let tax_rate = Decimal::new(tax_rate_raw, 2);

        // pct_a <= pct_b, so the item with pct_b should have <= total
        let item_a = LineItem {
            sku: "M".into(),
            name: "M".into(),
            unit_price,
            quantity: qty,
            discount: Some(LineDiscount::Percentage(pct_a)),
            tax_rate: Some(tax_rate),
        };
        let item_b = LineItem {
            sku: "M".into(),
            name: "M".into(),
            unit_price,
            quantity: qty,
            discount: Some(LineDiscount::Percentage(pct_b)),
            tax_rate: Some(tax_rate),
        };

        prop_assert!(
            item_b.total() <= item_a.total(),
            "higher discount {}% should yield <= total than {}%: {} vs {}",
            pct_b, pct_a, item_b.total(), item_a.total(),
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Line item subtotal = unit_price * quantity
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn line_item_subtotal_is_price_times_qty(
        price in arb_price(),
        qty in arb_qty(),
    ) {
        let item = LineItem {
            sku: "S".into(),
            name: "S".into(),
            unit_price: price,
            quantity: qty,
            discount: None,
            tax_rate: None,
        };
        let expected = price * Decimal::from(qty);
        prop_assert_eq!(
            item.subtotal(), expected,
            "subtotal mismatch for price={}, qty={}",
            price, qty,
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Discount is capped at subtotal
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn discount_never_exceeds_subtotal(item in arb_line_item()) {
        let sub = item.subtotal();
        let disc = item.discount_amount();
        prop_assert!(
            disc <= sub,
            "discount {disc} > subtotal {sub} for item {:?}",
            item,
        );
        prop_assert!(
            disc >= Decimal::ZERO,
            "discount was negative: {disc}",
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Currency conversion identity: convert(amount, X, X) == amount
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn currency_conversion_identity(
        amount_cents in 0i64..1_000_000,
    ) {
        let amount = Decimal::new(amount_cents, 2);
        let converter = CurrencyConverter::new();

        // Same currency should always return the same amount with rate=1
        let result = converter.convert(amount, "USD", "USD")
            .expect("same-currency conversion should always succeed");
        prop_assert_eq!(
            result.amount, amount,
            "convert({}, USD, USD) returned {} instead of identity",
            amount, result.amount,
        );
        prop_assert_eq!(result.rate, Decimal::ONE);
    }
}

// ---------------------------------------------------------------------------
// 9. Serde round-trip: serialize then deserialize LineItem preserves fields
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn line_item_serde_roundtrip(item in arb_line_item()) {
        let json = serde_json::to_string(&item)
            .expect("serialization should succeed");
        let parsed: LineItem = serde_json::from_str(&json)
            .expect("deserialization should succeed");
        prop_assert_eq!(
            &item, &parsed,
            "serde round-trip did not preserve LineItem",
        );
    }
}

// ---------------------------------------------------------------------------
// 10. Order total component sum
//
//    The implementation computes:
//      effective_taxable = (line_taxable - order_discount_amount).max(0)
//      grand_total = effective_taxable + total_tax + shipping + shipping_tax + fees
//
//    Which, within rounding tolerance, equals:
//      subtotal - total_discount + total_tax + shipping + shipping_tax + fees
//
//    We verify the documented invariant within 1 minor-unit of tolerance.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn order_total_components_sum_within_tolerance(input in arb_order_total_input()) {
        let t = compute_order_total(&input);

        // The documented invariant:
        let expected = t.subtotal - t.total_discount + t.total_tax
            + t.shipping + t.shipping_tax + t.fees;

        // Compute the rounding tolerance: 1 minor unit
        let tolerance = Decimal::new(1, input.rounding.minor_units);

        let diff = (t.grand_total - expected).abs();
        prop_assert!(
            diff <= tolerance,
            "component sum off by {diff} (tolerance {tolerance}): \
             grand_total={}, expected={}, total={:?}",
            t.grand_total,
            expected,
            t,
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Rounded amounts never exceed one minor unit of error
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn rounding_error_is_bounded_by_single_minor_unit(
        raw in -1_000_000_000i64..1_000_000_000i64,
        scale in 0u32..8,
        policy in arb_rounding_policy(),
    ) {
        let amount = Decimal::new(raw, scale);
        let rounded = round(amount, &policy);
        let unit = Decimal::new(1, policy.minor_units);
        let error = (rounded - amount).abs();
        prop_assert!(
            error <= unit,
            "rounding error {} exceeded one minor unit {} (amount={}, rounded={}, policy={:?})",
            error,
            unit,
            amount,
            rounded,
            policy,
        );
    }
}

// ---------------------------------------------------------------------------
// 12. Rounding output is quantized to configured minor units
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn rounding_quantizes_to_minor_units(
        raw in -1_000_000_000i64..1_000_000_000i64,
        scale in 0u32..8,
        policy in arb_rounding_policy(),
    ) {
        let amount = Decimal::new(raw, scale);
        let rounded = round(amount, &policy);
        let normalized_scale = rounded.normalize().scale();
        prop_assert!(
            normalized_scale <= policy.minor_units,
            "rounded scale {} exceeded minor_units {} for amount {} with policy {:?}",
            normalized_scale,
            policy.minor_units,
            amount,
            policy,
        );
    }
}

// ---------------------------------------------------------------------------
// 13. Rounding monotonicity: if a <= b then round(a) <= round(b)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn rounding_monotonicity(
        a_raw in -1_000_000_000i64..1_000_000_000i64,
        b_raw in -1_000_000_000i64..1_000_000_000i64,
        scale in 0u32..8,
        policy in arb_rounding_policy(),
    ) {
        let a = Decimal::new(a_raw.min(b_raw), scale);
        let b = Decimal::new(a_raw.max(b_raw), scale);
        let rounded_a = round(a, &policy);
        let rounded_b = round(b, &policy);
        prop_assert!(
            rounded_a <= rounded_b,
            "rounding was not monotonic: round({})={} > round({})={} for policy {:?}",
            a,
            rounded_a,
            b,
            rounded_b,
            policy,
        );
    }
}
