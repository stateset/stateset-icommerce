//! Promotion evaluation engine.
//!
//! Evaluates a set of promotions against an order context, handling stackable
//! vs. non-stackable promotions, max-use enforcement, and rule matching.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::line_item::LineDiscount;

/// A single rule that must be satisfied for a promotion to apply.
///
/// All rules on a promotion must pass (AND logic).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PromotionRule {
    /// Order total must be at least this amount (pre-discount).
    MinimumOrderTotal(Decimal),
    /// Total quantity of items must be at least this value.
    MinimumQuantity(u32),
    /// At least one of these SKUs must be in the cart.
    SpecificSkus(Vec<String>),
    /// Current time must be within this date range.
    DateRange {
        /// Start of the promotion window (inclusive).
        start: DateTime<Utc>,
        /// End of the promotion window (exclusive).
        end: DateTime<Utc>,
    },
    /// Customer must belong to this group.
    CustomerGroup(String),
    /// Must be the customer's first order.
    FirstOrder,
}

/// A promotion that can be applied to an order.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{Promotion, PromotionRule, LineDiscount};
/// use rust_decimal_macros::dec;
///
/// let promo = Promotion {
///     code: "SAVE10".into(),
///     discount: LineDiscount::Percentage(dec!(0.10)),
///     rules: vec![PromotionRule::MinimumOrderTotal(dec!(50.00))],
///     stackable: false,
///     max_uses: Some(100),
///     current_uses: 42,
/// };
/// assert_eq!(promo.code, "SAVE10");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    /// Unique promotion code.
    pub code: String,
    /// The discount to apply.
    pub discount: LineDiscount,
    /// All rules must pass (AND logic).
    pub rules: Vec<PromotionRule>,
    /// Whether this promotion can stack with others.
    pub stackable: bool,
    /// Maximum number of times this promotion can be used.
    pub max_uses: Option<u32>,
    /// How many times this promotion has already been used.
    pub current_uses: u32,
}

/// Context provided by the caller for promotion evaluation.
///
/// Contains all the information needed to check promotion rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionContext {
    /// Current order total (pre-discount).
    pub order_total: Decimal,
    /// Total item count.
    pub item_count: u32,
    /// SKUs present in the cart.
    pub skus: Vec<String>,
    /// Current timestamp.
    pub now: DateTime<Utc>,
    /// Customer's group (if any).
    pub customer_group: Option<String>,
    /// Whether this is the customer's first order.
    pub is_first_order: bool,
}

/// A promotion that was successfully applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedPromotion {
    /// The promotion code.
    pub code: String,
    /// The computed discount amount.
    pub discount_amount: Decimal,
    /// The discount definition.
    pub discount: LineDiscount,
}

/// A promotion that was rejected, with a reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedPromotion {
    /// The promotion code.
    pub code: String,
    /// Why this promotion was rejected.
    pub reason: RejectionReason,
}

/// Reason a promotion was not applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RejectionReason {
    /// One or more rules were not satisfied.
    RulesNotMet(Vec<String>),
    /// The promotion has been used too many times.
    MaxUsesExceeded,
    /// A non-stackable promotion with a higher discount was chosen instead.
    SupersededByBetterDeal {
        /// The code of the promotion that superseded this one.
        winner_code: String,
    },
}

/// Result of evaluating promotions against an order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionResult {
    /// Promotions that were applied.
    pub applied: Vec<AppliedPromotion>,
    /// Promotions that were rejected.
    pub rejected: Vec<RejectedPromotion>,
    /// Total discount from all applied promotions.
    pub total_discount: Decimal,
}

/// Evaluate a set of promotions against an order context.
///
/// **Algorithm:**
/// 1. Check each promotion's rules and max-uses.
/// 2. Compute the discount amount for eligible promotions.
/// 3. Among non-stackable promotions, pick the one with the highest discount.
/// 4. All stackable promotions are applied.
/// 5. Sum total discount.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{
///     Promotion, PromotionRule, PromotionContext, LineDiscount,
///     evaluate_promotions,
/// };
/// use rust_decimal_macros::dec;
/// use chrono::Utc;
///
/// let promos = vec![
///     Promotion {
///         code: "10OFF".into(),
///         discount: LineDiscount::Percentage(dec!(0.10)),
///         rules: vec![PromotionRule::MinimumOrderTotal(dec!(50.00))],
///         stackable: false,
///         max_uses: None,
///         current_uses: 0,
///     },
/// ];
///
/// let ctx = PromotionContext {
///     order_total: dec!(100.00),
///     item_count: 3,
///     skus: vec!["SKU-1".into()],
///     now: Utc::now(),
///     customer_group: None,
///     is_first_order: false,
/// };
///
/// let result = evaluate_promotions(&promos, &ctx);
/// assert_eq!(result.applied.len(), 1);
/// assert_eq!(result.total_discount, dec!(10.00));
/// ```
#[must_use]
pub fn evaluate_promotions(
    promotions: &[Promotion],
    context: &PromotionContext,
) -> PromotionResult {
    let mut eligible_stackable: Vec<(usize, Decimal)> = Vec::new();
    let mut eligible_non_stackable: Vec<(usize, Decimal)> = Vec::new();
    let mut rejected: Vec<RejectedPromotion> = Vec::new();

    for (i, promo) in promotions.iter().enumerate() {
        // Check max uses
        if let Some(max) = promo.max_uses {
            if promo.current_uses >= max {
                rejected.push(RejectedPromotion {
                    code: promo.code.clone(),
                    reason: RejectionReason::MaxUsesExceeded,
                });
                continue;
            }
        }

        // Check rules
        let failed_rules = check_rules(&promo.rules, context);
        if !failed_rules.is_empty() {
            rejected.push(RejectedPromotion {
                code: promo.code.clone(),
                reason: RejectionReason::RulesNotMet(failed_rules),
            });
            continue;
        }

        // Compute discount amount
        let discount_amount = compute_discount_amount(&promo.discount, context.order_total);

        if promo.stackable {
            eligible_stackable.push((i, discount_amount));
        } else {
            eligible_non_stackable.push((i, discount_amount));
        }
    }

    let mut applied: Vec<AppliedPromotion> = Vec::new();
    let mut remaining_discount_budget = context.order_total.max(Decimal::ZERO);

    // Non-stackable: pick best
    if !eligible_non_stackable.is_empty() {
        // Find the one with the highest discount
        let best_idx = eligible_non_stackable
            .iter()
            .enumerate()
            .max_by_key(|(_, (_, amt))| *amt)
            .map(|(idx, _)| idx)
            .expect("non-empty vec");

        for (idx, (promo_idx, discount_amount)) in eligible_non_stackable.iter().enumerate() {
            if idx == best_idx {
                apply_with_budget(
                    &mut applied,
                    &promotions[*promo_idx],
                    *discount_amount,
                    &mut remaining_discount_budget,
                );
            } else {
                let winner_code = promotions[eligible_non_stackable[best_idx].0].code.clone();
                rejected.push(RejectedPromotion {
                    code: promotions[*promo_idx].code.clone(),
                    reason: RejectionReason::SupersededByBetterDeal { winner_code },
                });
            }
        }
    }

    // Stackable: apply all
    for (promo_idx, discount_amount) in &eligible_stackable {
        apply_with_budget(
            &mut applied,
            &promotions[*promo_idx],
            *discount_amount,
            &mut remaining_discount_budget,
        );
    }

    let total_discount: Decimal = applied.iter().map(|a| a.discount_amount).sum();

    PromotionResult { applied, rejected, total_discount }
}

fn apply_with_budget(
    applied: &mut Vec<AppliedPromotion>,
    promo: &Promotion,
    requested_discount_amount: Decimal,
    remaining_discount_budget: &mut Decimal,
) {
    let capped_amount = requested_discount_amount
        .max(Decimal::ZERO)
        .min((*remaining_discount_budget).max(Decimal::ZERO));
    *remaining_discount_budget = (*remaining_discount_budget - capped_amount).max(Decimal::ZERO);

    applied.push(AppliedPromotion {
        code: promo.code.clone(),
        discount_amount: capped_amount,
        discount: promo.discount.clone(),
    });
}

/// Check all rules and return descriptions of any that failed.
fn check_rules(rules: &[PromotionRule], ctx: &PromotionContext) -> Vec<String> {
    let mut failures = Vec::new();
    for rule in rules {
        match rule {
            PromotionRule::MinimumOrderTotal(min) => {
                if ctx.order_total < *min {
                    failures
                        .push(format!("order total {} is below minimum {}", ctx.order_total, min));
                }
            }
            PromotionRule::MinimumQuantity(min) => {
                if ctx.item_count < *min {
                    failures
                        .push(format!("item count {} is below minimum {}", ctx.item_count, min));
                }
            }
            PromotionRule::SpecificSkus(skus) => {
                let has_match = skus.iter().any(|s| ctx.skus.contains(s));
                if !has_match {
                    failures.push(format!("none of the required SKUs found: {skus:?}"));
                }
            }
            PromotionRule::DateRange { start, end } => {
                if ctx.now < *start || ctx.now >= *end {
                    failures.push(format!(
                        "current time {} is outside range {} to {}",
                        ctx.now, start, end
                    ));
                }
            }
            PromotionRule::CustomerGroup(group) => {
                let matches = ctx.customer_group.as_ref().is_some_and(|cg| cg == group);
                if !matches {
                    failures.push(format!(
                        "customer group {:?} does not match required {group}",
                        ctx.customer_group
                    ));
                }
            }
            PromotionRule::FirstOrder => {
                if !ctx.is_first_order {
                    failures.push("not a first order".into());
                }
            }
        }
    }
    failures
}

/// Compute the discount amount given a discount type and the base amount.
fn compute_discount_amount(discount: &LineDiscount, base: Decimal) -> Decimal {
    match discount {
        LineDiscount::Percentage(pct) => {
            let clamped = (*pct).min(Decimal::ONE).max(Decimal::ZERO);
            base * clamped
        }
        LineDiscount::FixedAmount(amt) => amt.min(&base).max(&Decimal::ZERO).to_owned(),
        LineDiscount::FixedPrice(price) => {
            let target = price.max(&Decimal::ZERO).to_owned();
            (base - target).max(Decimal::ZERO)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    fn base_context() -> PromotionContext {
        PromotionContext {
            order_total: dec!(100.00),
            item_count: 5,
            skus: vec!["SKU-A".into(), "SKU-B".into()],
            now: Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap(),
            customer_group: Some("VIP".into()),
            is_first_order: false,
        }
    }

    fn simple_promo(code: &str, discount: LineDiscount, stackable: bool) -> Promotion {
        Promotion {
            code: code.into(),
            discount,
            rules: vec![],
            stackable,
            max_uses: None,
            current_uses: 0,
        }
    }

    // ---- single promotion, no rules ----

    #[test]
    fn single_promo_no_rules() {
        let promos = vec![simple_promo("10OFF", LineDiscount::Percentage(dec!(0.10)), false)];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.rejected.len(), 0);
        assert_eq!(result.total_discount, dec!(10.00));
    }

    // ---- MinimumOrderTotal ----

    #[test]
    fn minimum_order_total_pass() {
        let promos = vec![Promotion {
            code: "BIG".into(),
            discount: LineDiscount::FixedAmount(dec!(20.00)),
            rules: vec![PromotionRule::MinimumOrderTotal(dec!(50.00))],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn minimum_order_total_fail() {
        let promos = vec![Promotion {
            code: "BIG".into(),
            discount: LineDiscount::FixedAmount(dec!(20.00)),
            rules: vec![PromotionRule::MinimumOrderTotal(dec!(200.00))],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
        assert_eq!(result.rejected.len(), 1);
        matches!(&result.rejected[0].reason, RejectionReason::RulesNotMet(_));
    }

    // ---- MinimumQuantity ----

    #[test]
    fn minimum_quantity_pass() {
        let promos = vec![Promotion {
            code: "QTY".into(),
            discount: LineDiscount::Percentage(dec!(0.05)),
            rules: vec![PromotionRule::MinimumQuantity(3)],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn minimum_quantity_fail() {
        let promos = vec![Promotion {
            code: "QTY".into(),
            discount: LineDiscount::Percentage(dec!(0.05)),
            rules: vec![PromotionRule::MinimumQuantity(10)],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
        assert_eq!(result.rejected.len(), 1);
    }

    // ---- SpecificSkus ----

    #[test]
    fn specific_skus_pass() {
        let promos = vec![Promotion {
            code: "SKU".into(),
            discount: LineDiscount::FixedAmount(dec!(5.00)),
            rules: vec![PromotionRule::SpecificSkus(vec!["SKU-A".into()])],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn specific_skus_fail() {
        let promos = vec![Promotion {
            code: "SKU".into(),
            discount: LineDiscount::FixedAmount(dec!(5.00)),
            rules: vec![PromotionRule::SpecificSkus(vec!["SKU-Z".into()])],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    // ---- DateRange ----

    #[test]
    fn date_range_pass() {
        let promos = vec![Promotion {
            code: "SEASONAL".into(),
            discount: LineDiscount::Percentage(dec!(0.15)),
            rules: vec![PromotionRule::DateRange {
                start: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap(),
            }],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn date_range_fail_before() {
        let promos = vec![Promotion {
            code: "FUTURE".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![PromotionRule::DateRange {
                start: Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2027, 12, 31, 23, 59, 59).unwrap(),
            }],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    #[test]
    fn date_range_fail_after() {
        let promos = vec![Promotion {
            code: "PAST".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![PromotionRule::DateRange {
                start: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap(),
            }],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    // ---- CustomerGroup ----

    #[test]
    fn customer_group_pass() {
        let promos = vec![Promotion {
            code: "VIPONLY".into(),
            discount: LineDiscount::Percentage(dec!(0.20)),
            rules: vec![PromotionRule::CustomerGroup("VIP".into())],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn customer_group_fail() {
        let promos = vec![Promotion {
            code: "EMPLOYEE".into(),
            discount: LineDiscount::Percentage(dec!(0.30)),
            rules: vec![PromotionRule::CustomerGroup("EMPLOYEE".into())],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    #[test]
    fn customer_group_none() {
        let mut ctx = base_context();
        ctx.customer_group = None;
        let promos = vec![Promotion {
            code: "VIP".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![PromotionRule::CustomerGroup("VIP".into())],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &ctx);
        assert_eq!(result.applied.len(), 0);
    }

    // ---- FirstOrder ----

    #[test]
    fn first_order_pass() {
        let mut ctx = base_context();
        ctx.is_first_order = true;
        let promos = vec![Promotion {
            code: "WELCOME".into(),
            discount: LineDiscount::FixedAmount(dec!(10.00)),
            rules: vec![PromotionRule::FirstOrder],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &ctx);
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn first_order_fail() {
        let promos = vec![Promotion {
            code: "WELCOME".into(),
            discount: LineDiscount::FixedAmount(dec!(10.00)),
            rules: vec![PromotionRule::FirstOrder],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    // ---- MaxUses ----

    #[test]
    fn max_uses_not_exceeded() {
        let promos = vec![Promotion {
            code: "LIMITED".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![],
            stackable: false,
            max_uses: Some(100),
            current_uses: 50,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn max_uses_exactly_at_limit() {
        let promos = vec![Promotion {
            code: "FULL".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![],
            stackable: false,
            max_uses: Some(100),
            current_uses: 100,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
        assert!(matches!(result.rejected[0].reason, RejectionReason::MaxUsesExceeded));
    }

    #[test]
    fn max_uses_exceeded() {
        let promos = vec![Promotion {
            code: "OVER".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![],
            stackable: false,
            max_uses: Some(10),
            current_uses: 15,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    // ---- non-stackable: best deal wins ----

    #[test]
    fn non_stackable_best_deal_wins() {
        let promos = vec![
            simple_promo("SMALL", LineDiscount::Percentage(dec!(0.05)), false),
            simple_promo("BIG", LineDiscount::Percentage(dec!(0.20)), false),
            simple_promo("MED", LineDiscount::Percentage(dec!(0.10)), false),
        ];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].code, "BIG");
        assert_eq!(result.applied[0].discount_amount, dec!(20.00));
        assert_eq!(result.rejected.len(), 2);
        // The rejected ones should reference the winner
        for r in &result.rejected {
            assert!(matches!(
                &r.reason,
                RejectionReason::SupersededByBetterDeal { winner_code } if winner_code == "BIG"
            ));
        }
    }

    // ---- stackable promotions ----

    #[test]
    fn stackable_all_applied() {
        let promos = vec![
            simple_promo("A", LineDiscount::FixedAmount(dec!(5.00)), true),
            simple_promo("B", LineDiscount::FixedAmount(dec!(3.00)), true),
        ];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 2);
        assert_eq!(result.total_discount, dec!(8.00));
    }

    // ---- mixed stackable + non-stackable ----

    #[test]
    fn mixed_stackable_and_non_stackable() {
        let promos = vec![
            simple_promo("NS1", LineDiscount::Percentage(dec!(0.10)), false),
            simple_promo("NS2", LineDiscount::Percentage(dec!(0.15)), false),
            simple_promo("S1", LineDiscount::FixedAmount(dec!(5.00)), true),
        ];
        let result = evaluate_promotions(&promos, &base_context());
        // NS2 wins among non-stackable (15 > 10)
        // S1 stacks on top
        assert_eq!(result.applied.len(), 2);
        let codes: Vec<&str> = result.applied.iter().map(|a| a.code.as_str()).collect();
        assert!(codes.contains(&"NS2"));
        assert!(codes.contains(&"S1"));
        assert_eq!(result.total_discount, dec!(20.00)); // 15 + 5
    }

    // ---- multiple rules (AND logic) ----

    #[test]
    fn multiple_rules_all_pass() {
        let promos = vec![Promotion {
            code: "MULTI".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![
                PromotionRule::MinimumOrderTotal(dec!(50.00)),
                PromotionRule::MinimumQuantity(3),
                PromotionRule::CustomerGroup("VIP".into()),
            ],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
    }

    #[test]
    fn multiple_rules_one_fails() {
        let promos = vec![Promotion {
            code: "MULTI".into(),
            discount: LineDiscount::Percentage(dec!(0.10)),
            rules: vec![
                PromotionRule::MinimumOrderTotal(dec!(50.00)),
                PromotionRule::MinimumQuantity(100), // fails
            ],
            stackable: false,
            max_uses: None,
            current_uses: 0,
        }];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 0);
    }

    // ---- empty promotions ----

    #[test]
    fn no_promotions() {
        let result = evaluate_promotions(&[], &base_context());
        assert_eq!(result.applied.len(), 0);
        assert_eq!(result.rejected.len(), 0);
        assert_eq!(result.total_discount, Decimal::ZERO);
    }

    // ---- fixed price discount ----

    #[test]
    fn promotion_fixed_price() {
        let promos = vec![simple_promo("SET", LineDiscount::FixedPrice(dec!(75.00)), false)];
        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.total_discount, dec!(25.00)); // 100 - 75
    }

    // ---- serde roundtrip ----

    #[test]
    fn promotion_result_serde() {
        let result = PromotionResult {
            applied: vec![AppliedPromotion {
                code: "X".into(),
                discount_amount: dec!(10.00),
                discount: LineDiscount::Percentage(dec!(0.10)),
            }],
            rejected: vec![],
            total_discount: dec!(10.00),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PromotionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    // ---- FixedAmount discount with zero base ----

    #[test]
    fn discount_on_zero_base() {
        let mut ctx = base_context();
        ctx.order_total = Decimal::ZERO;
        let promos = vec![simple_promo("FREE", LineDiscount::FixedAmount(dec!(10.00)), false)];
        let result = evaluate_promotions(&promos, &ctx);
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.total_discount, Decimal::ZERO); // clamped to base
    }

    #[test]
    fn cumulative_discount_is_capped_to_order_total() {
        let promos = vec![
            simple_promo("BIG", LineDiscount::FixedAmount(dec!(80.00)), true),
            simple_promo("BIGGER", LineDiscount::FixedAmount(dec!(50.00)), true),
        ];

        let result = evaluate_promotions(&promos, &base_context());
        assert_eq!(result.applied.len(), 2);
        assert_eq!(result.applied[0].discount_amount, dec!(80.00));
        assert_eq!(result.applied[1].discount_amount, dec!(20.00));
        assert_eq!(result.total_discount, dec!(100.00));
    }
}
