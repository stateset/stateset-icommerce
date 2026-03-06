//! Error types for the pricing engine.

use rust_decimal::Decimal;
use std::fmt;

/// Errors that can occur during pricing calculations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PricingError {
    /// An amount must not be negative.
    #[error("invalid {field} {value}: must be non-negative")]
    InvalidAmount {
        /// The field that failed validation.
        field: &'static str,
        /// The invalid value.
        value: Decimal,
    },

    /// A discount percentage must be between 0 and 1 (inclusive).
    #[error("invalid discount percentage {value}: must be between 0 and 1")]
    InvalidDiscount {
        /// The invalid value.
        value: Decimal,
    },

    /// No exchange rate found for the requested currency pair.
    #[error("no exchange rate found from {from} to {to}")]
    NoExchangeRate {
        /// Source currency code.
        from: String,
        /// Target currency code.
        to: String,
    },

    /// A currency code was not recognized.
    #[error("currency not found: {code}")]
    CurrencyNotFound {
        /// The unrecognized currency code.
        code: String,
    },

    /// A tax rate is outside the valid range (0..=1).
    #[error("invalid tax rate {value}: must be between 0 and 1")]
    InvalidTaxRate {
        /// The invalid rate.
        value: Decimal,
    },

    /// An amount exceeded the maximum allowed for the current calculation.
    #[error("invalid {field} {value}: exceeds maximum {max}")]
    AmountExceedsMaximum {
        /// The field that failed validation.
        field: &'static str,
        /// The invalid value.
        value: Decimal,
        /// The maximum allowed value.
        max: Decimal,
    },

    /// An arithmetic overflow occurred during calculation.
    #[error("arithmetic overflow: {context}")]
    OverflowError {
        /// Description of what was being calculated.
        context: String,
    },

    /// A quantity must be greater than zero.
    #[error("invalid quantity {value}: must be greater than zero")]
    InvalidQuantity {
        /// The invalid quantity.
        value: u32,
    },

    /// A promotion has exceeded its maximum uses.
    #[error("promotion {code} has exceeded its maximum uses ({max})")]
    PromotionExhausted {
        /// The promotion code.
        code: String,
        /// The maximum number of uses.
        max: u32,
    },

    /// A fixed price discount exceeds the unit price.
    #[error("fixed price {price} exceeds unit price {unit_price}")]
    FixedPriceExceedsUnitPrice {
        /// The fixed-price discount value.
        price: Decimal,
        /// The item's unit price.
        unit_price: Decimal,
    },
}

/// A convenience alias for results from pricing operations.
pub type PricingResult<T> = Result<T, PricingError>;

impl PricingError {
    /// Create an [`InvalidDiscount`](PricingError::InvalidDiscount) error.
    #[must_use]
    pub const fn invalid_discount(value: Decimal) -> Self {
        Self::InvalidDiscount { value }
    }

    /// Create an [`InvalidAmount`](PricingError::InvalidAmount) error.
    #[must_use]
    pub const fn invalid_amount(field: &'static str, value: Decimal) -> Self {
        Self::InvalidAmount { field, value }
    }

    /// Create a [`NoExchangeRate`](PricingError::NoExchangeRate) error.
    #[must_use]
    pub fn no_exchange_rate(from: impl fmt::Display, to: impl fmt::Display) -> Self {
        Self::NoExchangeRate { from: from.to_string(), to: to.to_string() }
    }

    /// Create a [`CurrencyNotFound`](PricingError::CurrencyNotFound) error.
    #[must_use]
    pub fn currency_not_found(code: impl fmt::Display) -> Self {
        Self::CurrencyNotFound { code: code.to_string() }
    }

    /// Create an [`InvalidTaxRate`](PricingError::InvalidTaxRate) error.
    #[must_use]
    pub const fn invalid_tax_rate(value: Decimal) -> Self {
        Self::InvalidTaxRate { value }
    }

    /// Create an [`AmountExceedsMaximum`](PricingError::AmountExceedsMaximum) error.
    #[must_use]
    pub const fn amount_exceeds_max(field: &'static str, value: Decimal, max: Decimal) -> Self {
        Self::AmountExceedsMaximum { field, value, max }
    }

    /// Create an [`OverflowError`](PricingError::OverflowError) error.
    #[must_use]
    pub fn overflow(context: impl fmt::Display) -> Self {
        Self::OverflowError { context: context.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn display_invalid_discount() {
        let err = PricingError::invalid_discount(dec!(1.5));
        assert_eq!(err.to_string(), "invalid discount percentage 1.5: must be between 0 and 1");
    }

    #[test]
    fn display_invalid_amount() {
        let err = PricingError::invalid_amount("shipping cost", dec!(-1.0));
        assert_eq!(err.to_string(), "invalid shipping cost -1.0: must be non-negative");
    }

    #[test]
    fn display_no_exchange_rate() {
        let err = PricingError::no_exchange_rate("USD", "XYZ");
        assert_eq!(err.to_string(), "no exchange rate found from USD to XYZ");
    }

    #[test]
    fn display_currency_not_found() {
        let err = PricingError::currency_not_found("XYZ");
        assert_eq!(err.to_string(), "currency not found: XYZ");
    }

    #[test]
    fn display_invalid_tax_rate() {
        let err = PricingError::invalid_tax_rate(dec!(2.0));
        assert_eq!(err.to_string(), "invalid tax rate 2.0: must be between 0 and 1");
    }

    #[test]
    fn display_amount_exceeds_max() {
        let err = PricingError::amount_exceeds_max("discount amount", dec!(50.0), dec!(10.0));
        assert_eq!(err.to_string(), "invalid discount amount 50.0: exceeds maximum 10.0");
    }

    #[test]
    fn display_overflow() {
        let err = PricingError::overflow("subtotal computation");
        assert_eq!(err.to_string(), "arithmetic overflow: subtotal computation");
    }

    #[test]
    fn display_invalid_quantity() {
        let err = PricingError::InvalidQuantity { value: 0 };
        assert_eq!(err.to_string(), "invalid quantity 0: must be greater than zero");
    }

    #[test]
    fn display_promotion_exhausted() {
        let err = PricingError::PromotionExhausted { code: "SAVE10".into(), max: 100 };
        assert_eq!(err.to_string(), "promotion SAVE10 has exceeded its maximum uses (100)");
    }

    #[test]
    fn errors_are_eq() {
        let a = PricingError::invalid_discount(dec!(1.5));
        let b = PricingError::invalid_discount(dec!(1.5));
        assert_eq!(a, b);
    }
}
