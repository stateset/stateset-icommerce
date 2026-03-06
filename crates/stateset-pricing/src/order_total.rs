//! Order-level total computation.
//!
//! Aggregates line-item totals, applies order-level discounts, shipping,
//! shipping tax, and fees into a single [`OrderTotal`].

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::PricingResult;
use crate::line_item::{LineDiscount, LineItem};
use crate::rounding::{RoundingPolicy, round};
use crate::validation::{validate_base_discount, validate_non_negative, validate_tax_rate};

/// A fee added to the order (e.g. handling, gift wrap, restocking).
///
/// # Example
///
/// ```rust
/// use stateset_pricing::Fee;
/// use rust_decimal_macros::dec;
///
/// let fee = Fee { name: "Gift Wrap".into(), amount: dec!(4.99) };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fee {
    /// Human-readable fee name.
    pub name: String,
    /// Fee amount.
    pub amount: Decimal,
}

/// Input to [`compute_order_total`].
///
/// Collects all the data needed to compute the final order total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderTotalInput {
    /// Line items in the order.
    pub items: Vec<LineItem>,
    /// Base shipping cost before tax.
    pub shipping_cost: Decimal,
    /// Optional tax rate applied to shipping.
    pub shipping_tax_rate: Option<Decimal>,
    /// Optional order-level discount applied after line-item discounts.
    pub order_discount: Option<LineDiscount>,
    /// Additional fees.
    pub fees: Vec<Fee>,
    /// Rounding policy for intermediate calculations.
    pub rounding: RoundingPolicy,
}

/// The computed order total, broken down by component.
///
/// # Invariant
///
/// `grand_total = subtotal - total_discount + total_tax + shipping + shipping_tax + fees`
///
/// (within rounding tolerance)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderTotal {
    /// Sum of all line-item subtotals (before any discounts).
    pub subtotal: Decimal,
    /// Total discount (line-level + order-level).
    pub total_discount: Decimal,
    /// Total tax on items.
    pub total_tax: Decimal,
    /// Shipping cost (before shipping tax).
    pub shipping: Decimal,
    /// Tax on shipping.
    pub shipping_tax: Decimal,
    /// Sum of all fees.
    pub fees: Decimal,
    /// Final amount due.
    pub grand_total: Decimal,
}

/// Compute the order total from the given input.
///
/// Processing order:
/// 1. Sum line-item subtotals, discounts, and taxes.
/// 2. Apply order-level discount to the (subtotal - line discounts).
/// 3. Add shipping and shipping tax.
/// 4. Add fees.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{
///     LineItem, LineDiscount, Fee, OrderTotalInput, OrderTotal,
///     RoundingPolicy, compute_order_total,
/// };
/// use rust_decimal_macros::dec;
///
/// let input = OrderTotalInput {
///     items: vec![
///         LineItem {
///             sku: "A".into(), name: "Widget".into(),
///             unit_price: dec!(25.00), quantity: 2,
///             discount: None, tax_rate: Some(dec!(0.08)),
///         },
///     ],
///     shipping_cost: dec!(5.99),
///     shipping_tax_rate: Some(dec!(0.08)),
///     order_discount: None,
///     fees: vec![],
///     rounding: RoundingPolicy::usd(),
/// };
///
/// let total = compute_order_total(&input);
/// assert_eq!(total.subtotal, dec!(50.00));
/// assert_eq!(total.shipping, dec!(5.99));
/// ```
/// Compute the order total while validating all monetary inputs first.
pub fn try_compute_order_total(input: &OrderTotalInput) -> PricingResult<OrderTotal> {
    for item in &input.items {
        item.validate()?;
    }

    validate_non_negative("shipping cost", input.shipping_cost)?;
    if let Some(rate) = input.shipping_tax_rate {
        validate_tax_rate(rate)?;
    }
    for fee in &input.fees {
        validate_non_negative("fee amount", fee.amount)?;
    }

    let r = &input.rounding;

    // Step 1: Line-item aggregation
    let mut subtotal_raw = Decimal::ZERO;
    let mut line_discount_raw = Decimal::ZERO;
    let mut line_taxable = Decimal::ZERO;
    let mut line_tax_raw = Decimal::ZERO;
    for item in &input.items {
        subtotal_raw += item.try_subtotal()?;
        line_discount_raw += item.try_discount_amount()?;
        line_taxable += item.try_taxable_amount()?;
        line_tax_raw += item.try_tax_amount()?;
    }

    let subtotal = round(subtotal_raw, r);
    let line_discount = round(line_discount_raw, r);
    let line_tax = round(line_tax_raw, r);

    // Step 2: Order-level discount
    let order_discount_amount = match &input.order_discount {
        None => Decimal::ZERO,
        Some(discount) => {
            validate_base_discount(discount, line_taxable)?;
            match discount {
                LineDiscount::Percentage(pct) => round(line_taxable * *pct, r),
                LineDiscount::FixedAmount(amt) => round(*amt, r),
                LineDiscount::FixedPrice(price) => round(line_taxable - *price, r),
            }
        }
    };

    let total_discount = round(line_discount + order_discount_amount, r);

    // Recalculate tax if order discount changes the taxable base
    let effective_taxable = round((line_taxable - order_discount_amount).max(Decimal::ZERO), r);
    let total_tax = if line_taxable.is_zero() {
        Decimal::ZERO
    } else {
        round(line_tax * effective_taxable / line_taxable, r)
    };

    // Step 3: Shipping
    let shipping = round(input.shipping_cost, r);
    let shipping_tax = round(shipping * input.shipping_tax_rate.unwrap_or(Decimal::ZERO), r);

    // Step 4: Fees
    let fees: Decimal = input.fees.iter().map(|f| f.amount).sum();
    let fees = round(fees, r);

    let grand_total = round(effective_taxable + total_tax + shipping + shipping_tax + fees, r);

    Ok(OrderTotal {
        subtotal,
        total_discount,
        total_tax,
        shipping,
        shipping_tax,
        fees,
        grand_total,
    })
}

#[must_use]
/// Compute the order total and panic if the input contains invalid pricing data.
pub fn compute_order_total(input: &OrderTotalInput) -> OrderTotal {
    try_compute_order_total(input).expect("invalid order pricing input")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_item(price: Decimal, qty: u32, tax: Option<Decimal>) -> LineItem {
        LineItem {
            sku: "T".into(),
            name: "Test".into(),
            unit_price: price,
            quantity: qty,
            discount: None,
            tax_rate: tax,
        }
    }

    fn default_input(items: Vec<LineItem>) -> OrderTotalInput {
        OrderTotalInput {
            items,
            shipping_cost: Decimal::ZERO,
            shipping_tax_rate: None,
            order_discount: None,
            fees: vec![],
            rounding: RoundingPolicy::usd(),
        }
    }

    // ---- single item ----

    #[test]
    fn single_item_no_extras() {
        let input = default_input(vec![make_item(dec!(25.00), 2, None)]);
        let total = compute_order_total(&input);
        assert_eq!(total.subtotal, dec!(50.00));
        assert_eq!(total.total_discount, Decimal::ZERO);
        assert_eq!(total.total_tax, Decimal::ZERO);
        assert_eq!(total.grand_total, dec!(50.00));
    }

    #[test]
    fn single_item_with_tax() {
        let input = default_input(vec![make_item(dec!(100.00), 1, Some(dec!(0.08)))]);
        let total = compute_order_total(&input);
        assert_eq!(total.subtotal, dec!(100.00));
        assert_eq!(total.total_tax, dec!(8.00));
        assert_eq!(total.grand_total, dec!(108.00));
    }

    // ---- multiple items ----

    #[test]
    fn multiple_items() {
        let input = default_input(vec![
            make_item(dec!(10.00), 3, Some(dec!(0.05))),
            make_item(dec!(20.00), 1, Some(dec!(0.10))),
        ]);
        let total = compute_order_total(&input);
        // Subtotal: 30 + 20 = 50
        assert_eq!(total.subtotal, dec!(50.00));
        // Tax: 30*0.05 + 20*0.10 = 1.50 + 2.00 = 3.50
        assert_eq!(total.total_tax, dec!(3.50));
        assert_eq!(total.grand_total, dec!(53.50));
    }

    // ---- shipping ----

    #[test]
    fn with_shipping() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.shipping_cost = dec!(9.99);
        let total = compute_order_total(&input);
        assert_eq!(total.shipping, dec!(9.99));
        assert_eq!(total.shipping_tax, Decimal::ZERO);
        assert_eq!(total.grand_total, dec!(59.99));
    }

    #[test]
    fn with_shipping_tax() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.shipping_cost = dec!(10.00);
        input.shipping_tax_rate = Some(dec!(0.08));
        let total = compute_order_total(&input);
        assert_eq!(total.shipping, dec!(10.00));
        assert_eq!(total.shipping_tax, dec!(0.80));
        assert_eq!(total.grand_total, dec!(60.80));
    }

    // ---- fees ----

    #[test]
    fn with_fees() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.fees = vec![
            Fee { name: "Handling".into(), amount: dec!(3.00) },
            Fee { name: "Gift Wrap".into(), amount: dec!(4.99) },
        ];
        let total = compute_order_total(&input);
        assert_eq!(total.fees, dec!(7.99));
        assert_eq!(total.grand_total, dec!(57.99));
    }

    // ---- order-level discount ----

    #[test]
    fn order_discount_percentage() {
        let mut input = default_input(vec![make_item(dec!(100.00), 1, Some(dec!(0.10)))]);
        input.order_discount = Some(LineDiscount::Percentage(dec!(0.20)));
        let total = compute_order_total(&input);
        // Subtotal: 100, line discount: 0, taxable: 100
        // Order discount: 100 * 0.20 = 20
        // Total discount: 20
        // Effective taxable: 80
        // Tax: 10 * 80/100 = 8.00
        assert_eq!(total.total_discount, dec!(20.00));
        assert_eq!(total.total_tax, dec!(8.00));
        assert_eq!(total.grand_total, dec!(88.00));
    }

    #[test]
    fn order_discount_fixed_amount() {
        let mut input = default_input(vec![make_item(dec!(100.00), 1, None)]);
        input.order_discount = Some(LineDiscount::FixedAmount(dec!(15.00)));
        let total = compute_order_total(&input);
        assert_eq!(total.total_discount, dec!(15.00));
        assert_eq!(total.grand_total, dec!(85.00));
    }

    #[test]
    fn order_discount_fixed_price() {
        let mut input = default_input(vec![make_item(dec!(100.00), 1, None)]);
        input.order_discount = Some(LineDiscount::FixedPrice(dec!(79.99)));
        let total = compute_order_total(&input);
        assert_eq!(total.total_discount, dec!(20.01));
        assert_eq!(total.grand_total, dec!(79.99));
    }

    // ---- combined line + order discount ----

    #[test]
    fn combined_discounts() {
        let items = vec![LineItem {
            sku: "X".into(),
            name: "X".into(),
            unit_price: dec!(100.00),
            quantity: 1,
            discount: Some(LineDiscount::Percentage(dec!(0.10))),
            tax_rate: Some(dec!(0.08)),
        }];
        let mut input = default_input(items);
        input.order_discount = Some(LineDiscount::FixedAmount(dec!(10.00)));
        let total = compute_order_total(&input);
        // Subtotal: 100, line discount: 10, line taxable: 90
        // Order discount: 10 (fixed, applied to 90)
        // Total discount: 10 + 10 = 20
        // Effective taxable: 90 - 10 = 80
        // Line tax was 90*0.08 = 7.20, adjusted: 7.20 * 80/90 = 6.40
        assert_eq!(total.subtotal, dec!(100.00));
        assert_eq!(total.total_discount, dec!(20.00));
        assert_eq!(total.total_tax, dec!(6.40));
        assert_eq!(total.grand_total, dec!(86.40));
    }

    // ---- empty order ----

    #[test]
    fn empty_order() {
        let input = default_input(vec![]);
        let total = compute_order_total(&input);
        assert_eq!(total.subtotal, Decimal::ZERO);
        assert_eq!(total.grand_total, Decimal::ZERO);
    }

    // ---- all components ----

    #[test]
    fn kitchen_sink() {
        let items = vec![
            LineItem {
                sku: "A".into(),
                name: "A".into(),
                unit_price: dec!(25.00),
                quantity: 2,
                discount: Some(LineDiscount::Percentage(dec!(0.10))),
                tax_rate: Some(dec!(0.08)),
            },
            LineItem {
                sku: "B".into(),
                name: "B".into(),
                unit_price: dec!(15.00),
                quantity: 1,
                discount: Some(LineDiscount::FixedAmount(dec!(3.00))),
                tax_rate: Some(dec!(0.08)),
            },
        ];
        let input = OrderTotalInput {
            items,
            shipping_cost: dec!(7.50),
            shipping_tax_rate: Some(dec!(0.08)),
            order_discount: Some(LineDiscount::FixedAmount(dec!(5.00))),
            fees: vec![Fee { name: "Handle".into(), amount: dec!(2.00) }],
            rounding: RoundingPolicy::usd(),
        };
        let total = compute_order_total(&input);
        // Line A: subtotal=50, discount=5, taxable=45, tax=3.60
        // Line B: subtotal=15, discount=3, taxable=12, tax=0.96
        // Combined: subtotal=65, line_discount=8, line_taxable=57, line_tax=4.56
        // Order discount: 5 off 57 = 5
        // Total discount: 8+5=13
        // Effective taxable: 57-5=52
        // Tax: 4.56 * 52/57 = 4.16
        // Shipping: 7.50, shipping_tax: 0.60
        // Fees: 2.00
        // Grand: 52 + 4.16 + 7.50 + 0.60 + 2.00 = 66.26
        assert_eq!(total.subtotal, dec!(65.00));
        assert_eq!(total.total_discount, dec!(13.00));
        assert_eq!(total.total_tax, dec!(4.16));
        assert_eq!(total.shipping, dec!(7.50));
        assert_eq!(total.shipping_tax, dec!(0.60));
        assert_eq!(total.fees, dec!(2.00));
        assert_eq!(total.grand_total, dec!(66.26));
    }

    // ---- rounding ----

    #[test]
    fn jpy_rounding() {
        let input = OrderTotalInput {
            items: vec![make_item(dec!(333), 3, Some(dec!(0.10)))],
            shipping_cost: Decimal::ZERO,
            shipping_tax_rate: None,
            order_discount: None,
            fees: vec![],
            rounding: RoundingPolicy::jpy(),
        };
        let total = compute_order_total(&input);
        // subtotal: 999, tax: 99.9 -> rounded 100
        assert_eq!(total.subtotal, dec!(999));
        assert_eq!(total.total_tax, dec!(100));
        assert_eq!(total.grand_total, dec!(1099));
    }

    // ---- serde roundtrip ----

    #[test]
    fn order_total_serde() {
        let total = OrderTotal {
            subtotal: dec!(100.00),
            total_discount: dec!(10.00),
            total_tax: dec!(7.20),
            shipping: dec!(5.00),
            shipping_tax: dec!(0.40),
            fees: dec!(2.00),
            grand_total: dec!(104.60),
        };
        let json = serde_json::to_string(&total).unwrap();
        let parsed: OrderTotal = serde_json::from_str(&json).unwrap();
        assert_eq!(total, parsed);
    }

    // ---- order discount exceeds subtotal ----

    #[test]
    fn order_discount_exceeds_subtotal_is_rejected() {
        let mut input = default_input(vec![make_item(dec!(10.00), 1, None)]);
        input.order_discount = Some(LineDiscount::FixedAmount(dec!(50.00)));
        assert_eq!(
            try_compute_order_total(&input).unwrap_err(),
            crate::PricingError::amount_exceeds_max("discount amount", dec!(50.00), dec!(10.00))
        );
    }

    // ---- zero shipping with tax rate ----

    #[test]
    fn zero_shipping_with_tax_rate() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.shipping_cost = Decimal::ZERO;
        input.shipping_tax_rate = Some(dec!(0.10));
        let total = compute_order_total(&input);
        assert_eq!(total.shipping_tax, Decimal::ZERO);
    }

    #[test]
    fn negative_shipping_cost_is_rejected() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.shipping_cost = dec!(-1.00);
        assert_eq!(
            try_compute_order_total(&input).unwrap_err(),
            crate::PricingError::invalid_amount("shipping cost", dec!(-1.00))
        );
    }

    #[test]
    fn invalid_shipping_tax_rate_is_rejected() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.shipping_tax_rate = Some(dec!(1.25));
        assert_eq!(
            try_compute_order_total(&input).unwrap_err(),
            crate::PricingError::invalid_tax_rate(dec!(1.25))
        );
    }

    #[test]
    fn negative_fee_is_rejected() {
        let mut input = default_input(vec![make_item(dec!(50.00), 1, None)]);
        input.fees = vec![Fee { name: "Handling".into(), amount: dec!(-2.00) }];
        assert_eq!(
            try_compute_order_total(&input).unwrap_err(),
            crate::PricingError::invalid_amount("fee amount", dec!(-2.00))
        );
    }
}
