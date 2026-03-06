//! Line-item pricing calculations.
//!
//! A [`LineItem`] represents a single product line in an order, carrying its
//! unit price, quantity, optional discount, and optional tax rate. All methods
//! are pure functions — no side effects, fully deterministic.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::PricingResult;
use crate::rounding::{RoundingPolicy, round};
use crate::validation::{
    validate_line_discount, validate_non_negative, validate_quantity, validate_tax_rate,
};

/// Discount applied to a single line item.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::LineDiscount;
/// use rust_decimal_macros::dec;
///
/// let pct = LineDiscount::Percentage(dec!(0.10)); // 10% off
/// let fixed = LineDiscount::FixedAmount(dec!(5.00)); // $5 off
/// let override_price = LineDiscount::FixedPrice(dec!(19.99)); // set to $19.99
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LineDiscount {
    /// A percentage discount (0.10 = 10%). Must be between 0 and 1.
    Percentage(Decimal),
    /// A fixed dollar amount off the subtotal.
    FixedAmount(Decimal),
    /// Override the per-unit price to this value.
    FixedPrice(Decimal),
}

/// A single line item in an order.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{LineItem, LineDiscount};
/// use rust_decimal_macros::dec;
///
/// let item = LineItem {
///     sku: "WIDGET-001".into(),
///     name: "Blue Widget".into(),
///     unit_price: dec!(25.00),
///     quantity: 3,
///     discount: Some(LineDiscount::Percentage(dec!(0.10))),
///     tax_rate: Some(dec!(0.08)),
/// };
///
/// assert_eq!(item.subtotal(), dec!(75.00));
/// assert_eq!(item.discount_amount(), dec!(7.50));
/// assert_eq!(item.taxable_amount(), dec!(67.50));
/// assert_eq!(item.tax_amount(), dec!(5.40));
/// assert_eq!(item.total(), dec!(72.90));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineItem {
    /// Stock-keeping unit identifier.
    pub sku: String,
    /// Human-readable name.
    pub name: String,
    /// Price per unit before discounts.
    pub unit_price: Decimal,
    /// Number of units.
    pub quantity: u32,
    /// Optional discount for this line.
    pub discount: Option<LineDiscount>,
    /// Optional tax rate (e.g. 0.08 = 8%).
    pub tax_rate: Option<Decimal>,
}

impl LineItem {
    /// Validate that the line item contains only supported pricing inputs.
    ///
    /// ```rust
    /// use stateset_pricing::LineItem;
    /// use rust_decimal_macros::dec;
    ///
    /// let item = LineItem {
    ///     sku: "A".into(), name: "A".into(),
    ///     unit_price: dec!(10.00), quantity: 5,
    ///     discount: None, tax_rate: None,
    /// };
    /// assert!(item.validate().is_ok());
    /// ```
    pub fn validate(&self) -> PricingResult<()> {
        validate_quantity(self.quantity)?;
        validate_non_negative("unit price", self.unit_price)?;

        if let Some(discount) = &self.discount {
            let subtotal = self.unit_price * Decimal::from(self.quantity);
            validate_line_discount(discount, subtotal, self.unit_price)?;
        }

        if let Some(rate) = self.tax_rate {
            validate_tax_rate(rate)?;
        }

        Ok(())
    }

    /// Compute the subtotal (unit price times quantity) before discounts.
    pub fn try_subtotal(&self) -> PricingResult<Decimal> {
        validate_quantity(self.quantity)?;
        validate_non_negative("unit price", self.unit_price)?;
        Ok(self.unit_price * Decimal::from(self.quantity))
    }

    /// Compute the subtotal and panic if the line item is invalid.
    #[must_use]
    pub fn subtotal(&self) -> Decimal {
        self.try_subtotal().expect("invalid line item pricing input")
    }

    /// Compute the discount amount for this line.
    ///
    /// Returns zero if no discount is set.
    pub fn try_discount_amount(&self) -> PricingResult<Decimal> {
        self.validate()?;
        let sub = self.unit_price * Decimal::from(self.quantity);
        let discount = match &self.discount {
            None => Decimal::ZERO,
            Some(LineDiscount::Percentage(pct)) => sub * *pct,
            Some(LineDiscount::FixedAmount(amt)) => *amt,
            Some(LineDiscount::FixedPrice(price)) => {
                let new_total = *price * Decimal::from(self.quantity);
                sub - new_total
            }
        };
        Ok(discount)
    }

    /// Compute the discount amount and panic if the line item is invalid.
    #[must_use]
    pub fn discount_amount(&self) -> Decimal {
        self.try_discount_amount().expect("invalid line item pricing input")
    }

    /// Compute the taxable amount (subtotal minus discount).
    pub fn try_taxable_amount(&self) -> PricingResult<Decimal> {
        Ok((self.try_subtotal()? - self.try_discount_amount()?).max(Decimal::ZERO))
    }

    /// Compute the taxable amount and panic if the line item is invalid.
    #[must_use]
    pub fn taxable_amount(&self) -> Decimal {
        self.try_taxable_amount().expect("invalid line item pricing input")
    }

    /// Compute the tax amount.
    pub fn try_tax_amount(&self) -> PricingResult<Decimal> {
        let taxable = self.try_taxable_amount()?;
        Ok(match self.tax_rate {
            Some(rate) => taxable * rate,
            None => Decimal::ZERO,
        })
    }

    /// Compute the tax amount and panic if the line item is invalid.
    #[must_use]
    pub fn tax_amount(&self) -> Decimal {
        self.try_tax_amount().expect("invalid line item pricing input")
    }

    /// Compute the line total (taxable amount + tax).
    pub fn try_total(&self) -> PricingResult<Decimal> {
        Ok(self.try_taxable_amount()? + self.try_tax_amount()?)
    }

    /// Compute the total and panic if the line item is invalid.
    #[must_use]
    pub fn total(&self) -> Decimal {
        self.try_total().expect("invalid line item pricing input")
    }

    /// Compute the line total with rounding applied to intermediate values.
    pub fn try_total_rounded(&self, policy: &RoundingPolicy) -> PricingResult<Decimal> {
        let taxable = round(self.try_taxable_amount()?, policy);
        let tax = round(taxable * self.tax_rate.unwrap_or(Decimal::ZERO), policy);
        Ok(taxable + tax)
    }

    /// Compute the rounded total and panic if the line item is invalid.
    #[must_use]
    pub fn total_rounded(&self, policy: &RoundingPolicy) -> Decimal {
        self.try_total_rounded(policy).expect("invalid line item pricing input")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn simple_item(price: Decimal, qty: u32) -> LineItem {
        LineItem {
            sku: "TEST".into(),
            name: "Test Item".into(),
            unit_price: price,
            quantity: qty,
            discount: None,
            tax_rate: None,
        }
    }

    // ---- subtotal ----

    #[test]
    fn subtotal_basic() {
        let item = simple_item(dec!(10.00), 3);
        assert_eq!(item.subtotal(), dec!(30.00));
    }

    #[test]
    fn subtotal_single_unit() {
        let item = simple_item(dec!(49.99), 1);
        assert_eq!(item.subtotal(), dec!(49.99));
    }

    #[test]
    fn subtotal_zero_quantity_is_rejected() {
        let item = simple_item(dec!(10.00), 0);
        assert_eq!(
            item.try_subtotal().unwrap_err(),
            crate::PricingError::InvalidQuantity { value: 0 }
        );
    }

    #[test]
    fn subtotal_large_quantity() {
        let item = simple_item(dec!(0.01), 1_000_000);
        assert_eq!(item.subtotal(), dec!(10000.00));
    }

    #[test]
    fn subtotal_zero_price() {
        let item = simple_item(Decimal::ZERO, 10);
        assert_eq!(item.subtotal(), Decimal::ZERO);
    }

    // ---- percentage discount ----

    #[test]
    fn discount_percentage_10_pct() {
        let mut item = simple_item(dec!(100.00), 2);
        item.discount = Some(LineDiscount::Percentage(dec!(0.10)));
        assert_eq!(item.discount_amount(), dec!(20.00));
        assert_eq!(item.taxable_amount(), dec!(180.00));
    }

    #[test]
    fn discount_percentage_100_pct() {
        let mut item = simple_item(dec!(50.00), 1);
        item.discount = Some(LineDiscount::Percentage(Decimal::ONE));
        assert_eq!(item.discount_amount(), dec!(50.00));
        assert_eq!(item.taxable_amount(), Decimal::ZERO);
    }

    #[test]
    fn discount_percentage_zero() {
        let mut item = simple_item(dec!(50.00), 1);
        item.discount = Some(LineDiscount::Percentage(Decimal::ZERO));
        assert_eq!(item.discount_amount(), Decimal::ZERO);
    }

    #[test]
    fn discount_percentage_above_one_is_rejected() {
        let mut item = simple_item(dec!(50.00), 1);
        item.discount = Some(LineDiscount::Percentage(dec!(1.5)));
        assert_eq!(
            item.try_discount_amount().unwrap_err(),
            crate::PricingError::invalid_discount(dec!(1.5))
        );
    }

    #[test]
    fn discount_percentage_below_zero_is_rejected() {
        let mut item = simple_item(dec!(50.00), 1);
        item.discount = Some(LineDiscount::Percentage(dec!(-0.10)));
        assert_eq!(
            item.try_discount_amount().unwrap_err(),
            crate::PricingError::invalid_discount(dec!(-0.10))
        );
    }

    // ---- fixed amount discount ----

    #[test]
    fn discount_fixed_amount() {
        let mut item = simple_item(dec!(30.00), 2);
        item.discount = Some(LineDiscount::FixedAmount(dec!(10.00)));
        assert_eq!(item.discount_amount(), dec!(10.00));
        assert_eq!(item.taxable_amount(), dec!(50.00));
    }

    #[test]
    fn discount_fixed_amount_exceeds_subtotal_is_rejected() {
        let mut item = simple_item(dec!(5.00), 1);
        item.discount = Some(LineDiscount::FixedAmount(dec!(10.00)));
        assert_eq!(
            item.try_discount_amount().unwrap_err(),
            crate::PricingError::amount_exceeds_max("discount amount", dec!(10.00), dec!(5.00))
        );
    }

    #[test]
    fn discount_fixed_amount_negative_is_rejected() {
        let mut item = simple_item(dec!(50.00), 1);
        item.discount = Some(LineDiscount::FixedAmount(dec!(-5.00)));
        assert_eq!(
            item.try_discount_amount().unwrap_err(),
            crate::PricingError::invalid_amount("discount amount", dec!(-5.00))
        );
    }

    // ---- fixed price discount ----

    #[test]
    fn discount_fixed_price() {
        let mut item = simple_item(dec!(30.00), 2);
        item.discount = Some(LineDiscount::FixedPrice(dec!(20.00)));
        // Old subtotal: 60.00, new total: 20*2 = 40.00, discount = 20.00
        assert_eq!(item.discount_amount(), dec!(20.00));
        assert_eq!(item.taxable_amount(), dec!(40.00));
    }

    #[test]
    fn discount_fixed_price_higher_than_unit_is_rejected() {
        let mut item = simple_item(dec!(10.00), 1);
        item.discount = Some(LineDiscount::FixedPrice(dec!(15.00)));
        assert_eq!(
            item.try_discount_amount().unwrap_err(),
            crate::PricingError::FixedPriceExceedsUnitPrice {
                price: dec!(15.00),
                unit_price: dec!(10.00),
            }
        );
    }

    #[test]
    fn discount_fixed_price_zero() {
        let mut item = simple_item(dec!(10.00), 3);
        item.discount = Some(LineDiscount::FixedPrice(Decimal::ZERO));
        assert_eq!(item.discount_amount(), dec!(30.00));
        assert_eq!(item.taxable_amount(), Decimal::ZERO);
    }

    // ---- no discount ----

    #[test]
    fn no_discount() {
        let item = simple_item(dec!(25.00), 4);
        assert_eq!(item.discount_amount(), Decimal::ZERO);
        assert_eq!(item.taxable_amount(), dec!(100.00));
    }

    // ---- tax ----

    #[test]
    fn tax_8_percent() {
        let mut item = simple_item(dec!(100.00), 1);
        item.tax_rate = Some(dec!(0.08));
        assert_eq!(item.tax_amount(), dec!(8.00));
        assert_eq!(item.total(), dec!(108.00));
    }

    #[test]
    fn tax_with_discount() {
        let mut item = simple_item(dec!(100.00), 1);
        item.discount = Some(LineDiscount::Percentage(dec!(0.20)));
        item.tax_rate = Some(dec!(0.10));
        // Taxable: 80.00, tax: 8.00, total: 88.00
        assert_eq!(item.taxable_amount(), dec!(80.00));
        assert_eq!(item.tax_amount(), dec!(8.00));
        assert_eq!(item.total(), dec!(88.00));
    }

    #[test]
    fn no_tax() {
        let item = simple_item(dec!(50.00), 2);
        assert_eq!(item.tax_amount(), Decimal::ZERO);
        assert_eq!(item.total(), dec!(100.00));
    }

    #[test]
    fn tax_zero_rate() {
        let mut item = simple_item(dec!(50.00), 1);
        item.tax_rate = Some(Decimal::ZERO);
        assert_eq!(item.tax_amount(), Decimal::ZERO);
    }

    #[test]
    fn tax_rate_above_one_is_rejected() {
        let mut item = simple_item(dec!(50.00), 1);
        item.tax_rate = Some(dec!(1.10));
        assert_eq!(
            item.try_tax_amount().unwrap_err(),
            crate::PricingError::invalid_tax_rate(dec!(1.10))
        );
    }

    #[test]
    fn negative_unit_price_is_rejected() {
        let item = simple_item(dec!(-1.00), 1);
        assert_eq!(
            item.try_total().unwrap_err(),
            crate::PricingError::invalid_amount("unit price", dec!(-1.00))
        );
    }

    // ---- total ----

    #[test]
    fn total_no_extras() {
        let item = simple_item(dec!(25.00), 4);
        assert_eq!(item.total(), dec!(100.00));
    }

    #[test]
    fn total_with_everything() {
        let item = LineItem {
            sku: "FULL".into(),
            name: "Full".into(),
            unit_price: dec!(50.00),
            quantity: 2,
            discount: Some(LineDiscount::FixedAmount(dec!(15.00))),
            tax_rate: Some(dec!(0.07)),
        };
        // Subtotal: 100, discount: 15, taxable: 85, tax: 5.95, total: 90.95
        assert_eq!(item.subtotal(), dec!(100.00));
        assert_eq!(item.discount_amount(), dec!(15.00));
        assert_eq!(item.taxable_amount(), dec!(85.00));
        assert_eq!(item.tax_amount(), dec!(5.95));
        assert_eq!(item.total(), dec!(90.95));
    }

    // ---- total_rounded ----

    #[test]
    fn total_rounded_usd() {
        let item = LineItem {
            sku: "R".into(),
            name: "R".into(),
            unit_price: dec!(33.33),
            quantity: 3,
            discount: Some(LineDiscount::Percentage(dec!(0.15))),
            tax_rate: Some(dec!(0.0825)),
        };
        let policy = RoundingPolicy::usd();
        let total = item.total_rounded(&policy);
        // Subtotal: 99.99, discount: 14.9985, taxable: 84.9915 -> rounded 84.99
        // tax: 84.99 * 0.0825 = 7.011675 -> rounded 7.01
        // total: 84.99 + 7.01 = 92.00
        assert_eq!(total, dec!(92.00));
    }

    // ---- serde ----

    #[test]
    fn serde_roundtrip() {
        let item = LineItem {
            sku: "SER".into(),
            name: "Serde".into(),
            unit_price: dec!(10.00),
            quantity: 1,
            discount: Some(LineDiscount::Percentage(dec!(0.05))),
            tax_rate: Some(dec!(0.08)),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: LineItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, parsed);
    }
}
