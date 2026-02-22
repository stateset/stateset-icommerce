//! Tax calculation with multi-jurisdiction and compound tax support.
//!
//! All functions are pure — no side effects, fully deterministic.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::rounding::{RoundingPolicy, round};

/// What a tax rule applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TaxAppliesTo {
    /// Applies to all items.
    AllItems,
    /// Only applies to items in specific categories.
    SpecificCategories(Vec<String>),
    /// Only applies to shipping.
    ShippingOnly,
}

/// A single tax rule for a jurisdiction.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{TaxRule, TaxAppliesTo};
/// use rust_decimal_macros::dec;
///
/// let rule = TaxRule {
///     jurisdiction: "CA".into(),
///     rate: dec!(0.0725),
///     applies_to: TaxAppliesTo::AllItems,
///     compound: false,
/// };
/// assert_eq!(rule.jurisdiction, "CA");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxRule {
    /// Jurisdiction name (e.g. "CA", "NY", "VAT-EU").
    pub jurisdiction: String,
    /// Tax rate (e.g. 0.0725 = 7.25%).
    pub rate: Decimal,
    /// What this tax applies to.
    pub applies_to: TaxAppliesTo,
    /// If true, this tax is compounded on top of previously computed taxes.
    pub compound: bool,
}

/// An item that can be taxed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxableItem {
    /// Taxable amount.
    pub amount: Decimal,
    /// Product category for category-based tax rules.
    pub category: Option<String>,
    /// If true, this item is tax-exempt.
    pub exempt: bool,
}

/// Context for tax calculation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxContext {
    /// Taxable items.
    pub items: Vec<TaxableItem>,
    /// Shipping amount (may be taxed separately).
    pub shipping: Decimal,
}

/// A single computed tax line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxLine {
    /// The jurisdiction this tax comes from.
    pub jurisdiction: String,
    /// The tax rate.
    pub rate: Decimal,
    /// The amount that was taxed.
    pub taxable_amount: Decimal,
    /// The computed tax.
    pub tax_amount: Decimal,
}

/// Result of a tax calculation.
///
/// ```rust
/// use stateset_pricing::TaxResult;
/// use rust_decimal::Decimal;
///
/// let r = TaxResult {
///     tax_lines: vec![],
///     total_tax: Decimal::ZERO,
/// };
/// assert!(r.tax_lines.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxResult {
    /// Individual tax lines per jurisdiction.
    pub tax_lines: Vec<TaxLine>,
    /// Sum of all tax amounts.
    pub total_tax: Decimal,
}

/// Calculate taxes on items and shipping for a set of tax rules.
///
/// Tax rules are applied in order. Non-compound taxes are calculated on the
/// original taxable base. Compound taxes are calculated on the base plus
/// all previously computed taxes.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{
///     TaxRule, TaxAppliesTo, TaxableItem, TaxContext, TaxResult,
///     RoundingPolicy, calculate_tax,
/// };
/// use rust_decimal_macros::dec;
///
/// let rules = vec![TaxRule {
///     jurisdiction: "CA".into(),
///     rate: dec!(0.0725),
///     applies_to: TaxAppliesTo::AllItems,
///     compound: false,
/// }];
///
/// let context = TaxContext {
///     items: vec![TaxableItem {
///         amount: dec!(100.00),
///         category: None,
///         exempt: false,
///     }],
///     shipping: dec!(0),
/// };
///
/// let result = calculate_tax(&rules, &context, &RoundingPolicy::usd());
/// assert_eq!(result.total_tax, dec!(7.25));
/// ```
#[must_use]
pub fn calculate_tax(
    rules: &[TaxRule],
    context: &TaxContext,
    rounding: &RoundingPolicy,
) -> TaxResult {
    let mut tax_lines = Vec::new();
    let mut cumulative_tax = Decimal::ZERO;

    for rule in rules {
        let taxable_base = compute_taxable_base(rule, context);

        if taxable_base.is_zero() {
            continue;
        }

        // For compound taxes, add previously computed taxes to the base
        let effective_base = if rule.compound {
            taxable_base + cumulative_tax
        } else {
            taxable_base
        };

        let tax_amount = round(effective_base * rule.rate, rounding);

        if !tax_amount.is_zero() {
            cumulative_tax += tax_amount;
            tax_lines.push(TaxLine {
                jurisdiction: rule.jurisdiction.clone(),
                rate: rule.rate,
                taxable_amount: effective_base,
                tax_amount,
            });
        }
    }

    let total_tax = tax_lines.iter().map(|tl| tl.tax_amount).sum();

    TaxResult { tax_lines, total_tax }
}

/// Compute the taxable base amount for a given rule.
fn compute_taxable_base(rule: &TaxRule, context: &TaxContext) -> Decimal {
    match &rule.applies_to {
        TaxAppliesTo::AllItems => {
            context
                .items
                .iter()
                .filter(|item| !item.exempt)
                .map(|item| item.amount)
                .sum()
        }
        TaxAppliesTo::SpecificCategories(cats) => {
            context
                .items
                .iter()
                .filter(|item| {
                    !item.exempt
                        && item
                            .category
                            .as_ref()
                            .is_some_and(|c| cats.contains(c))
                })
                .map(|item| item.amount)
                .sum()
        }
        TaxAppliesTo::ShippingOnly => context.shipping,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn simple_context(amount: Decimal) -> TaxContext {
        TaxContext {
            items: vec![TaxableItem {
                amount,
                category: None,
                exempt: false,
            }],
            shipping: dec!(0),
        }
    }

    fn usd() -> RoundingPolicy {
        RoundingPolicy::usd()
    }

    // ---- single jurisdiction ----

    #[test]
    fn single_tax_rule() {
        let rules = vec![TaxRule {
            jurisdiction: "CA".into(),
            rate: dec!(0.0725),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &simple_context(dec!(100.00)), &usd());
        assert_eq!(result.tax_lines.len(), 1);
        assert_eq!(result.tax_lines[0].jurisdiction, "CA");
        assert_eq!(result.tax_lines[0].tax_amount, dec!(7.25));
        assert_eq!(result.total_tax, dec!(7.25));
    }

    #[test]
    fn single_tax_high_amount() {
        let rules = vec![TaxRule {
            jurisdiction: "NY".into(),
            rate: dec!(0.08875),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &simple_context(dec!(1599.99)), &usd());
        // 1599.99 * 0.08875 = 141.999...  -> 142.00
        assert_eq!(result.total_tax, dec!(142.00));
    }

    // ---- multiple jurisdictions (non-compound) ----

    #[test]
    fn multiple_non_compound() {
        let rules = vec![
            TaxRule {
                jurisdiction: "State".into(),
                rate: dec!(0.06),
                applies_to: TaxAppliesTo::AllItems,
                compound: false,
            },
            TaxRule {
                jurisdiction: "County".into(),
                rate: dec!(0.01),
                applies_to: TaxAppliesTo::AllItems,
                compound: false,
            },
        ];
        let result = calculate_tax(&rules, &simple_context(dec!(100.00)), &usd());
        assert_eq!(result.tax_lines.len(), 2);
        assert_eq!(result.tax_lines[0].tax_amount, dec!(6.00));
        assert_eq!(result.tax_lines[1].tax_amount, dec!(1.00));
        assert_eq!(result.total_tax, dec!(7.00));
    }

    // ---- compound taxes ----

    #[test]
    fn compound_tax() {
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
        let result = calculate_tax(&rules, &simple_context(dec!(100.00)), &usd());
        // GST: 100 * 0.05 = 5.00
        // QST (compound): (100 + 5) * 0.09975 = 10.47375 -> 10.47
        assert_eq!(result.tax_lines[0].tax_amount, dec!(5.00));
        assert_eq!(result.tax_lines[1].tax_amount, dec!(10.47));
        assert_eq!(result.total_tax, dec!(15.47));
    }

    #[test]
    fn compound_on_compound() {
        let rules = vec![
            TaxRule {
                jurisdiction: "A".into(),
                rate: dec!(0.10),
                applies_to: TaxAppliesTo::AllItems,
                compound: false,
            },
            TaxRule {
                jurisdiction: "B".into(),
                rate: dec!(0.05),
                applies_to: TaxAppliesTo::AllItems,
                compound: true,
            },
            TaxRule {
                jurisdiction: "C".into(),
                rate: dec!(0.02),
                applies_to: TaxAppliesTo::AllItems,
                compound: true,
            },
        ];
        let result = calculate_tax(&rules, &simple_context(dec!(100.00)), &usd());
        // A: 100 * 0.10 = 10.00
        // B: (100 + 10) * 0.05 = 5.50
        // C: (100 + 10 + 5.50) * 0.02 = 2.31
        assert_eq!(result.tax_lines[0].tax_amount, dec!(10.00));
        assert_eq!(result.tax_lines[1].tax_amount, dec!(5.50));
        assert_eq!(result.tax_lines[2].tax_amount, dec!(2.31));
        assert_eq!(result.total_tax, dec!(17.81));
    }

    // ---- exempt items ----

    #[test]
    fn exempt_items_excluded() {
        let ctx = TaxContext {
            items: vec![
                TaxableItem { amount: dec!(50.00), category: None, exempt: false },
                TaxableItem { amount: dec!(30.00), category: None, exempt: true },
            ],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "TX".into(),
            rate: dec!(0.0625),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        // Only $50 is taxable
        assert_eq!(result.tax_lines[0].taxable_amount, dec!(50.00));
        assert_eq!(result.total_tax, dec!(3.13));
    }

    #[test]
    fn all_items_exempt() {
        let ctx = TaxContext {
            items: vec![
                TaxableItem { amount: dec!(50.00), category: None, exempt: true },
            ],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "TX".into(),
            rate: dec!(0.10),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
        assert_eq!(result.total_tax, Decimal::ZERO);
    }

    // ---- category filtering ----

    #[test]
    fn specific_categories() {
        let ctx = TaxContext {
            items: vec![
                TaxableItem {
                    amount: dec!(100.00),
                    category: Some("electronics".into()),
                    exempt: false,
                },
                TaxableItem {
                    amount: dec!(50.00),
                    category: Some("clothing".into()),
                    exempt: false,
                },
                TaxableItem {
                    amount: dec!(25.00),
                    category: Some("food".into()),
                    exempt: false,
                },
            ],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "NY".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::SpecificCategories(vec![
                "electronics".into(),
                "clothing".into(),
            ]),
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        // Only electronics (100) + clothing (50) = 150 taxable
        assert_eq!(result.tax_lines[0].taxable_amount, dec!(150.00));
        assert_eq!(result.total_tax, dec!(12.00));
    }

    #[test]
    fn category_no_match() {
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(100.00),
                category: Some("food".into()),
                exempt: false,
            }],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "CA".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into()]),
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
    }

    #[test]
    fn category_none_does_not_match() {
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(100.00),
                category: None,
                exempt: false,
            }],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "CA".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into()]),
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
    }

    // ---- shipping tax ----

    #[test]
    fn shipping_only_tax() {
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(100.00),
                category: None,
                exempt: false,
            }],
            shipping: dec!(10.00),
        };
        let rules = vec![TaxRule {
            jurisdiction: "FL".into(),
            rate: dec!(0.06),
            applies_to: TaxAppliesTo::ShippingOnly,
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert_eq!(result.tax_lines[0].taxable_amount, dec!(10.00));
        assert_eq!(result.total_tax, dec!(0.60));
    }

    #[test]
    fn shipping_zero() {
        let ctx = TaxContext {
            items: vec![],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "FL".into(),
            rate: dec!(0.06),
            applies_to: TaxAppliesTo::ShippingOnly,
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
    }

    // ---- mixed rules: items + shipping ----

    #[test]
    fn items_and_shipping_tax() {
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(200.00),
                category: None,
                exempt: false,
            }],
            shipping: dec!(15.00),
        };
        let rules = vec![
            TaxRule {
                jurisdiction: "State-Items".into(),
                rate: dec!(0.07),
                applies_to: TaxAppliesTo::AllItems,
                compound: false,
            },
            TaxRule {
                jurisdiction: "State-Shipping".into(),
                rate: dec!(0.07),
                applies_to: TaxAppliesTo::ShippingOnly,
                compound: false,
            },
        ];
        let result = calculate_tax(&rules, &ctx, &usd());
        // Items: 200 * 0.07 = 14.00
        // Shipping: 15 * 0.07 = 1.05
        assert_eq!(result.tax_lines[0].tax_amount, dec!(14.00));
        assert_eq!(result.tax_lines[1].tax_amount, dec!(1.05));
        assert_eq!(result.total_tax, dec!(15.05));
    }

    // ---- no rules ----

    #[test]
    fn no_rules() {
        let result = calculate_tax(&[], &simple_context(dec!(100.00)), &usd());
        assert!(result.tax_lines.is_empty());
        assert_eq!(result.total_tax, Decimal::ZERO);
    }

    // ---- no items ----

    #[test]
    fn no_items() {
        let ctx = TaxContext { items: vec![], shipping: Decimal::ZERO };
        let rules = vec![TaxRule {
            jurisdiction: "CA".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
    }

    // ---- zero rate ----

    #[test]
    fn zero_tax_rate() {
        let rules = vec![TaxRule {
            jurisdiction: "OR".into(),
            rate: Decimal::ZERO,
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let result = calculate_tax(&rules, &simple_context(dec!(100.00)), &usd());
        assert!(result.tax_lines.is_empty());
        assert_eq!(result.total_tax, Decimal::ZERO);
    }

    // ---- rounding ----

    #[test]
    fn tax_rounding_jpy() {
        let rules = vec![TaxRule {
            jurisdiction: "JP".into(),
            rate: dec!(0.10),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let ctx = simple_context(dec!(999));
        let result = calculate_tax(&rules, &ctx, &RoundingPolicy::jpy());
        // 999 * 0.10 = 99.9 -> rounds to 100
        assert_eq!(result.total_tax, dec!(100));
    }

    #[test]
    fn tax_rounding_bhd() {
        let rules = vec![TaxRule {
            jurisdiction: "BH".into(),
            rate: dec!(0.05),
            applies_to: TaxAppliesTo::AllItems,
            compound: false,
        }];
        let ctx = simple_context(dec!(100.000));
        let result = calculate_tax(&rules, &ctx, &RoundingPolicy::bhd());
        assert_eq!(result.total_tax, dec!(5.000));
    }

    // ---- serde ----

    #[test]
    fn tax_result_serde() {
        let result = TaxResult {
            tax_lines: vec![TaxLine {
                jurisdiction: "CA".into(),
                rate: dec!(0.08),
                taxable_amount: dec!(100.00),
                tax_amount: dec!(8.00),
            }],
            total_tax: dec!(8.00),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: TaxResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    // ---- exempt + category combined ----

    #[test]
    fn exempt_overrides_category() {
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(100.00),
                category: Some("electronics".into()),
                exempt: true,
            }],
            shipping: dec!(0),
        };
        let rules = vec![TaxRule {
            jurisdiction: "CA".into(),
            rate: dec!(0.08),
            applies_to: TaxAppliesTo::SpecificCategories(vec!["electronics".into()]),
            compound: false,
        }];
        let result = calculate_tax(&rules, &ctx, &usd());
        assert!(result.tax_lines.is_empty());
    }

    // ---- compound with shipping ----

    #[test]
    fn compound_does_not_apply_to_shipping_base() {
        // Compound tax should compound on previously computed tax amounts,
        // not cross between items and shipping
        let ctx = TaxContext {
            items: vec![TaxableItem {
                amount: dec!(100.00),
                category: None,
                exempt: false,
            }],
            shipping: dec!(10.00),
        };
        let rules = vec![
            TaxRule {
                jurisdiction: "Shipping".into(),
                rate: dec!(0.05),
                applies_to: TaxAppliesTo::ShippingOnly,
                compound: false,
            },
            TaxRule {
                jurisdiction: "Items".into(),
                rate: dec!(0.10),
                applies_to: TaxAppliesTo::AllItems,
                compound: true,
            },
        ];
        let result = calculate_tax(&rules, &ctx, &usd());
        // Shipping: 10 * 0.05 = 0.50
        // Items (compound): (100 + 0.50) * 0.10 = 10.05
        assert_eq!(result.tax_lines[0].tax_amount, dec!(0.50));
        assert_eq!(result.tax_lines[1].tax_amount, dec!(10.05));
        assert_eq!(result.total_tax, dec!(10.55));
    }
}
