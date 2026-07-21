//! Units of measure, unit classes, and conversion rules
//!
//! A `UnitClass` groups compatible units (e.g. Length, Weight, Volume). Each
//! `UnitOfMeasure` belongs to a class and is convertible to other UOMs in the
//! same class via a factor relative to the class's base unit. `UnitConversionRule`
//! captures explicit conversions, either globally (`System`) or per-product (`Sku`).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{ProductId, UnitClassId, UnitConversionRuleId, UnitOfMeasureId};
use strum::{Display, EnumString};

/// A class of mutually-convertible units (e.g. Length, Weight, Volume).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitClass {
    /// Unique unit class ID.
    pub id: UnitClassId,
    /// Class name (e.g. "Weight").
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The base UOM all conversion factors in this class are expressed against.
    pub base_uom_id: Option<UnitOfMeasureId>,
    /// When the class was created.
    pub created_at: DateTime<Utc>,
    /// When the class was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A unit of measure within a unit class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitOfMeasure {
    /// Unique UOM ID.
    pub id: UnitOfMeasureId,
    /// Parent unit class.
    pub unit_class_id: UnitClassId,
    /// Display name (e.g. "Kilogram").
    pub name: String,
    /// Abbreviation / symbol (e.g. "kg").
    pub abbreviation: String,
    /// Conversion factor relative to the class base unit (base = factor × this).
    /// For the base unit itself this is `1`.
    pub factor: Decimal,
    /// Whether this UOM is the base unit for its class.
    pub is_base: bool,
    /// When the UOM was created.
    pub created_at: DateTime<Utc>,
    /// When the UOM was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Filter for listing units of measure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnitOfMeasureFilter {
    /// Restrict to a single unit class.
    pub class_id: Option<UnitClassId>,
    /// Maximum number of rows to return (server default/cap applies when unset)
    pub limit: Option<u32>,
    /// Number of rows to skip
    pub offset: Option<u32>,
}

/// The scope of a conversion rule.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ConversionRuleType {
    /// A global, per-class conversion rule.
    #[default]
    System,
    /// A product-specific conversion override.
    Sku,
}

/// An explicit conversion rule between two UOMs.
///
/// `Sku` rules win over `System` rules when resolving a conversion for a
/// specific product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitConversionRule {
    /// Unique rule ID.
    pub id: UnitConversionRuleId,
    /// Scope of the rule.
    pub rule_type: ConversionRuleType,
    /// Product the rule applies to. Required for `Sku`, must be absent for `System`.
    pub product_id: Option<ProductId>,
    /// The UOM being converted from.
    pub from_uom_id: UnitOfMeasureId,
    /// The UOM being converted to.
    pub to_uom_id: UnitOfMeasureId,
    /// Multiplier: `to_qty = from_qty × factor`.
    pub factor: Decimal,
    /// When the rule was created.
    pub created_at: DateTime<Utc>,
    /// When the rule was last updated.
    pub updated_at: DateTime<Utc>,
}

impl UnitConversionRule {
    /// Validate the invariant tying `rule_type` to `product_id`.
    ///
    /// `Sku` rules must carry a `product_id`; `System` rules must not.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        match self.rule_type {
            ConversionRuleType::Sku => self.product_id.is_some(),
            ConversionRuleType::System => self.product_id.is_none(),
        }
    }

    /// Apply this rule to a quantity expressed in the `from` UOM.
    #[must_use]
    pub fn convert(&self, from_qty: Decimal) -> Decimal {
        from_qty * self.factor
    }
}

/// Input for creating a unit class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUnitClass {
    /// Class name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Input for creating a unit of measure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUnitOfMeasure {
    /// Parent unit class.
    pub unit_class_id: UnitClassId,
    /// Display name.
    pub name: String,
    /// Abbreviation.
    pub abbreviation: String,
    /// Factor relative to base unit.
    pub factor: Decimal,
}

/// Input for creating a conversion rule. `product_id` is required when
/// `rule_type` is `Sku` and must be omitted when it is `System`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUnitConversionRule {
    /// Rule scope.
    pub rule_type: ConversionRuleType,
    /// Product (for `Sku` rules only).
    pub product_id: Option<ProductId>,
    /// From UOM.
    pub from_uom_id: UnitOfMeasureId,
    /// To UOM.
    pub to_uom_id: UnitOfMeasureId,
    /// Conversion factor.
    pub factor: Decimal,
}

/// Resolve the effective conversion factor between two UOMs in the same class
/// using their base-relative factors. Returns `None` if `to`'s factor is zero.
#[must_use]
pub fn factor_between(from: &UnitOfMeasure, to: &UnitOfMeasure) -> Option<Decimal> {
    if to.factor.is_zero() {
        return None;
    }
    // qty_in_base = from_qty × from.factor ; to_qty = qty_in_base / to.factor
    Some(from.factor / to.factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_uom(class: UnitClassId, factor: Decimal, is_base: bool) -> UnitOfMeasure {
        UnitOfMeasure {
            id: UnitOfMeasureId::new(),
            unit_class_id: class,
            name: "Unit".to_string(),
            abbreviation: "u".to_string(),
            factor,
            is_base,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_rule(rule_type: ConversionRuleType, product: Option<ProductId>) -> UnitConversionRule {
        UnitConversionRule {
            id: UnitConversionRuleId::new(),
            rule_type,
            product_id: product,
            from_uom_id: UnitOfMeasureId::new(),
            to_uom_id: UnitOfMeasureId::new(),
            factor: dec!(2),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn system_rule_valid_without_product() {
        assert!(make_rule(ConversionRuleType::System, None).is_valid());
        assert!(!make_rule(ConversionRuleType::System, Some(ProductId::new())).is_valid());
    }

    #[test]
    fn sku_rule_valid_with_product() {
        assert!(make_rule(ConversionRuleType::Sku, Some(ProductId::new())).is_valid());
        assert!(!make_rule(ConversionRuleType::Sku, None).is_valid());
    }

    #[test]
    fn convert_applies_factor() {
        let rule = make_rule(ConversionRuleType::System, None);
        assert_eq!(rule.convert(dec!(5)), dec!(10));
    }

    #[test]
    fn factor_between_base_relative() {
        let class = UnitClassId::new();
        // gram is base (factor 1), kilogram factor 1000 (1 kg = 1000 g base)
        let gram = make_uom(class, dec!(1), true);
        let kilo = make_uom(class, dec!(1000), false);
        // 1 kg expressed in grams = factor 1000
        assert_eq!(factor_between(&kilo, &gram), Some(dec!(1000)));
        // 1 g expressed in kg = 1/1000
        assert_eq!(factor_between(&gram, &kilo), Some(dec!(0.001)));
    }

    #[test]
    fn factor_between_zero_target_is_none() {
        let class = UnitClassId::new();
        let from = make_uom(class, dec!(1), true);
        let zero = make_uom(class, dec!(0), false);
        assert_eq!(factor_between(&from, &zero), None);
    }

    #[test]
    fn rule_type_roundtrip_uppercase() {
        for t in [ConversionRuleType::System, ConversionRuleType::Sku] {
            let s = t.to_string();
            let parsed: ConversionRuleType = s.parse().unwrap();
            assert_eq!(parsed, t, "round-trip failed for {s}");
        }
        // serde uses uppercase wire form
        assert_eq!(serde_json::to_string(&ConversionRuleType::Sku).unwrap(), "\"SKU\"");
    }
}
