//! Backend-agnostic cart money rules.
//!
//! Both cart repositories call these, so the SQLite and Postgres backends
//! apply one definition of discount capping, tax carry-over, totals rounding
//! and line-input validation instead of two hand-synchronised copies.

use rust_decimal::Decimal;
use stateset_core::{
    AddCartItem, Cart, CommerceError, CurrencyCode, Result, UpdateCartItem, validate_money_scale,
    validate_price,
};

/// Round a cart amount to `currency`'s minor unit: whole units for JPY,
/// cents for USD, mils for BHD. Both backends store cart money with enough
/// scale for every ISO 4217 minor unit (Postgres since migration 107).
pub(crate) fn round_cart_money(amount: Decimal, currency: CurrencyCode) -> Decimal {
    amount.round_dp(u32::from(currency.decimal_places()))
}

/// Outcome of a backend's `derive_discount`.
#[derive(Debug, Clone)]
pub(crate) struct DerivedDiscount {
    /// Discount the cart carries, capped at what the cart can cover.
    pub(crate) amount: Decimal,
    /// `discount_description` to store (`None` keeps the stored one).
    pub(crate) description: Option<String>,
    /// Why the cart's coupon no longer qualifies, if it does not.
    pub(crate) coupon_error: Option<String>,
}

impl DerivedDiscount {
    pub(crate) fn capped(cart: &Cart, amount: Decimal, description: Option<String>) -> Self {
        Self {
            amount: cap_discount(cart, amount),
            description: description.or_else(|| cart.discount_description.clone()),
            coupon_error: None,
        }
    }

    /// `subtotal + tax + shipping - amount`, which the cap keeps non-negative.
    pub(crate) fn grand_total(&self, cart: &Cart) -> Decimal {
        round_cart_money(
            cart.subtotal + cart.tax_amount + cart.shipping_amount - self.amount,
            cart.currency,
        )
        .max(Decimal::ZERO)
    }
}

/// Clamp a discount into `[0, subtotal + tax + shipping]` so the order minted
/// from the cart can never have a negative total.
pub(crate) fn cap_discount(cart: &Cart, amount: Decimal) -> Decimal {
    let coverable = (cart.subtotal + cart.tax_amount + cart.shipping_amount).max(Decimal::ZERO);
    amount.max(Decimal::ZERO).min(coverable)
}

/// Order-level discount a coupon-activated promotion grants on `cart`,
/// rounded to the cart currency's precision so the cart's money math stays on
/// minor-unit boundaries (percentage-off with an optional cap, or a fixed
/// amount never exceeding the subtotal).
pub(crate) fn coupon_discount_amount(promotion: &stateset_core::Promotion, cart: &Cart) -> Decimal {
    let request = stateset_core::ApplyPromotionsRequest::from_cart(
        cart,
        cart.coupon_code.as_deref().unwrap_or_default(),
    );
    let raw = promotion.calculate_discount(&request, Decimal::ZERO);
    round_cart_money(cap_discount(cart, raw), cart.currency)
}

/// Carry a cart's tax across a change of its lines.
///
/// The cart stores only a tax *amount* (set by `set_tax`, normally from the
/// tax engine), not the rate or jurisdiction it came from, so the storage
/// layer cannot re-run the engine. It keeps the effective rate instead: the
/// stored tax is scaled by `new_subtotal / previous_subtotal`, which is exact
/// for a single-rate cart and a best-effort estimate for mixed-rate lines —
/// the embedded cart accessor re-runs the tax engine after each mutation when
/// it can and overwrites this estimate. Tax that was never set (zero) stays
/// zero, and a cart emptied of lines carries no tax.
pub(crate) fn rescale_tax(
    previous_tax: Decimal,
    previous_subtotal: Decimal,
    new_subtotal: Decimal,
    currency: CurrencyCode,
) -> Decimal {
    if previous_tax <= Decimal::ZERO || previous_subtotal <= Decimal::ZERO {
        return previous_tax.max(Decimal::ZERO);
    }
    if new_subtotal == previous_subtotal {
        return previous_tax;
    }
    if new_subtotal <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    round_cart_money(previous_tax * new_subtotal / previous_subtotal, currency)
}

/// Invariant M1 (`commerce.money.scale_exceeds_currency`) for a line being
/// added: no money input may carry more decimals than the cart currency's
/// minor unit, because the line total and cart subtotal are rounded to it
/// and a sub-minor-unit price would silently lose money.
pub(crate) fn validate_add_item_money(currency: CurrencyCode, item: &AddCartItem) -> Result<()> {
    validate_money_scale(currency, item.unit_price)?;
    if let Some(original) = item.original_price {
        validate_money_scale(currency, original)?;
    }
    Ok(())
}

/// Invariant M1 for a line update (see [`validate_add_item_money`]).
pub(crate) fn validate_update_cart_item_money(
    currency: CurrencyCode,
    input: &UpdateCartItem,
) -> Result<()> {
    if let Some(unit_price) = input.unit_price {
        validate_money_scale(currency, unit_price)?;
    }
    if let Some(discount) = input.discount_amount {
        validate_money_scale(currency, discount)?;
    }
    Ok(())
}

/// Reject a cart-item update that would store a non-positive quantity, a
/// negative price or a negative line discount. Quantity 0 is NOT a silent
/// remove: callers must use `remove_item`.
pub(crate) fn validate_update_cart_item(input: &UpdateCartItem) -> Result<()> {
    if let Some(quantity) = input.quantity
        && quantity < 1
    {
        return Err(CommerceError::ValidationError(format!(
            "Item quantity must be positive, got {quantity}; remove the item instead"
        )));
    }
    if let Some(unit_price) = input.unit_price {
        validate_price(unit_price)?;
    }
    if let Some(discount) = input.discount_amount
        && discount < Decimal::ZERO
    {
        return Err(CommerceError::ValidationError(format!(
            "Item discount must not be negative, got {discount}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn cart_money_rounds_to_the_currency_minor_unit() {
        assert_eq!(round_cart_money(dec!(33.3333), CurrencyCode::USD), dec!(33.33));
        assert_eq!(round_cart_money(dec!(33.3333), CurrencyCode::JPY), dec!(33));
        let bhd: CurrencyCode = "BHD".parse().expect("BHD");
        assert_eq!(round_cart_money(dec!(1.23456), bhd), dec!(1.235));
    }

    #[test]
    fn rescaled_tax_never_carries_fractional_yen() {
        let tax = rescale_tax(dec!(10), dec!(100), dec!(333), CurrencyCode::JPY);
        assert_eq!(tax, dec!(33));
        assert_eq!(rescale_tax(dec!(10), dec!(100), dec!(333), CurrencyCode::USD), dec!(33.30));
    }
}
