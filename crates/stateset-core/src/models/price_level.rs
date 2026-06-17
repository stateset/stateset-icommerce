//! Price level domain models
//!
//! A price level is a named B2B pricing tier (e.g. "Wholesale", "VIP") that
//! adjusts a product's base price — either by a percentage discount/markup
//! applied across the catalog, or by an explicit per-product fixed price
//! (a price-level entry). Per-product entries win over the tier default.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, PriceLevelId, ProductId};
use strum::{Display, EnumString};

/// How a price level adjusts the base price by default.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PriceAdjustmentType {
    /// No catalog-wide adjustment; only explicit entries apply.
    #[default]
    None,
    /// Reduce base price by `adjustment_value` percent.
    PercentageDiscount,
    /// Increase base price by `adjustment_value` percent.
    PercentageMarkup,
}

/// A named B2B pricing tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Unique price level ID.
    pub id: PriceLevelId,
    /// Display name (e.g. "Wholesale").
    pub name: String,
    /// Short code (e.g. "WHOLESALE").
    pub code: String,
    /// Optional description.
    pub description: Option<String>,
    /// Catalog-wide adjustment kind.
    pub adjustment_type: PriceAdjustmentType,
    /// Percentage value for the adjustment (e.g. `10` for 10%).
    pub adjustment_value: Decimal,
    /// Currency for fixed-price entries.
    pub currency: CurrencyCode,
    /// Whether the level is active.
    pub is_active: bool,
    /// When the level was created.
    pub created_at: DateTime<Utc>,
    /// When the level was last updated.
    pub updated_at: DateTime<Utc>,
}

impl PriceLevel {
    /// Apply this level's catalog-wide adjustment to a base price.
    ///
    /// Does not consider per-product entries (use [`resolve_price`] for that).
    /// The result is clamped at zero.
    #[must_use]
    pub fn adjust(&self, base: Decimal) -> Decimal {
        let hundred = Decimal::from(100);
        let adjusted = match self.adjustment_type {
            PriceAdjustmentType::None => base,
            PriceAdjustmentType::PercentageDiscount => {
                base - (base * self.adjustment_value / hundred)
            }
            PriceAdjustmentType::PercentageMarkup => {
                base + (base * self.adjustment_value / hundred)
            }
        };
        adjusted.max(Decimal::ZERO)
    }
}

/// An explicit fixed price for a product within a price level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevelEntry {
    /// Owning price level.
    pub price_level_id: PriceLevelId,
    /// Product the override applies to.
    pub product_id: ProductId,
    /// Fixed price for this product at this level.
    pub price: Decimal,
    /// When the entry was created.
    pub created_at: DateTime<Utc>,
    /// When the entry was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Resolve the effective price for a product at a level, given its base price.
///
/// An explicit `entry` fixed price wins; otherwise the level's catalog-wide
/// adjustment is applied to `base`.
#[must_use]
pub fn resolve_price(
    level: &PriceLevel,
    entry: Option<&PriceLevelEntry>,
    base: Decimal,
) -> Decimal {
    match entry {
        Some(e) => e.price,
        None => level.adjust(base),
    }
}

/// Input for creating a price level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePriceLevel {
    /// Display name.
    pub name: String,
    /// Short code.
    pub code: String,
    /// Optional description.
    pub description: Option<String>,
    /// Adjustment kind (defaults to `None`).
    #[serde(default)]
    pub adjustment_type: PriceAdjustmentType,
    /// Percentage value for the adjustment.
    #[serde(default)]
    pub adjustment_value: Decimal,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
}

/// Input for updating a price level (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePriceLevel {
    /// Updated name.
    pub name: Option<String>,
    /// Updated description.
    pub description: Option<String>,
    /// Updated adjustment kind.
    pub adjustment_type: Option<PriceAdjustmentType>,
    /// Updated adjustment value.
    pub adjustment_value: Option<Decimal>,
    /// Updated active state.
    pub is_active: Option<bool>,
}

/// Filter for listing price levels.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PriceLevelFilter {
    /// Filter by active state.
    pub is_active: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make(adjustment_type: PriceAdjustmentType, value: Decimal) -> PriceLevel {
        PriceLevel {
            id: PriceLevelId::new(),
            name: "Wholesale".into(),
            code: "WHOLESALE".into(),
            description: None,
            adjustment_type,
            adjustment_value: value,
            currency: CurrencyCode::USD,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn percentage_discount() {
        let level = make(PriceAdjustmentType::PercentageDiscount, dec!(10));
        assert_eq!(level.adjust(dec!(100)), dec!(90));
    }

    #[test]
    fn percentage_markup() {
        let level = make(PriceAdjustmentType::PercentageMarkup, dec!(20));
        assert_eq!(level.adjust(dec!(100)), dec!(120));
    }

    #[test]
    fn none_returns_base() {
        let level = make(PriceAdjustmentType::None, dec!(50));
        assert_eq!(level.adjust(dec!(100)), dec!(100));
    }

    #[test]
    fn discount_clamped_at_zero() {
        let level = make(PriceAdjustmentType::PercentageDiscount, dec!(150));
        assert_eq!(level.adjust(dec!(100)), dec!(0));
    }

    #[test]
    fn resolve_prefers_entry_fixed_price() {
        let level = make(PriceAdjustmentType::PercentageDiscount, dec!(10));
        let entry = PriceLevelEntry {
            price_level_id: level.id,
            product_id: ProductId::new(),
            price: dec!(42),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(resolve_price(&level, Some(&entry), dec!(100)), dec!(42));
        assert_eq!(resolve_price(&level, None, dec!(100)), dec!(90));
    }

    #[test]
    fn adjustment_type_roundtrip() {
        for t in [
            PriceAdjustmentType::None,
            PriceAdjustmentType::PercentageDiscount,
            PriceAdjustmentType::PercentageMarkup,
        ] {
            assert_eq!(t.to_string().parse::<PriceAdjustmentType>().unwrap(), t);
        }
    }
}
