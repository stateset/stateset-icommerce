use rust_decimal::Decimal;

use crate::error::{PricingError, PricingResult};
use crate::line_item::LineDiscount;

pub(crate) fn validate_non_negative(field: &'static str, value: Decimal) -> PricingResult<()> {
    if value < Decimal::ZERO { Err(PricingError::InvalidAmount { field, value }) } else { Ok(()) }
}

pub(crate) const fn validate_quantity(quantity: u32) -> PricingResult<()> {
    if quantity == 0 { Err(PricingError::InvalidQuantity { value: quantity }) } else { Ok(()) }
}

pub(crate) fn validate_discount_rate(rate: Decimal) -> PricingResult<()> {
    if (Decimal::ZERO..=Decimal::ONE).contains(&rate) {
        Ok(())
    } else {
        Err(PricingError::InvalidDiscount { value: rate })
    }
}

pub(crate) fn validate_tax_rate(rate: Decimal) -> PricingResult<()> {
    if (Decimal::ZERO..=Decimal::ONE).contains(&rate) {
        Ok(())
    } else {
        Err(PricingError::InvalidTaxRate { value: rate })
    }
}

pub(crate) fn validate_maximum(
    field: &'static str,
    value: Decimal,
    max: Decimal,
) -> PricingResult<()> {
    if value > max { Err(PricingError::AmountExceedsMaximum { field, value, max }) } else { Ok(()) }
}

pub(crate) fn validate_line_discount(
    discount: &LineDiscount,
    subtotal: Decimal,
    unit_price: Decimal,
) -> PricingResult<()> {
    match discount {
        LineDiscount::Percentage(rate) => validate_discount_rate(*rate),
        LineDiscount::FixedAmount(amount) => {
            validate_non_negative("discount amount", *amount)?;
            validate_maximum("discount amount", *amount, subtotal)
        }
        LineDiscount::FixedPrice(price) => {
            validate_non_negative("fixed price", *price)?;
            if *price > unit_price {
                Err(PricingError::FixedPriceExceedsUnitPrice { price: *price, unit_price })
            } else {
                Ok(())
            }
        }
    }
}

pub(crate) fn validate_base_discount(discount: &LineDiscount, base: Decimal) -> PricingResult<()> {
    match discount {
        LineDiscount::Percentage(rate) => validate_discount_rate(*rate),
        LineDiscount::FixedAmount(amount) => {
            validate_non_negative("discount amount", *amount)?;
            validate_maximum("discount amount", *amount, base)
        }
        LineDiscount::FixedPrice(price) => {
            validate_non_negative("fixed price", *price)?;
            validate_maximum("fixed price", *price, base)
        }
    }
}
