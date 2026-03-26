//! Automated dispute resolution rules engine.
//!
//! Configurable rules that auto-resolve disputes based on conditions:
//! - Amount thresholds (auto-refund if amount < $X)
//! - Seller reputation (auto-favor buyer if seller score < Y)
//! - Time-based (auto-resolve if no evidence within Z days)
//! - Evidence-based (auto-resolve if both parties submitted evidence)

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of condition that triggers a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    /// Dispute amount is below threshold.
    AmountBelow,
    /// Seller reputation score is below threshold.
    SellerScoreBelow,
    /// Days since dispute was filed exceeds threshold.
    DaysExceeded,
    /// Both parties have submitted evidence.
    BothPartiesEvidence,
}

/// Action to take when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeAction {
    /// Refund the buyer in full.
    RefundBuyer,
    /// Release funds to seller.
    ReleaseSeller,
    /// Split the disputed amount (e.g., 50/50).
    Split { buyer_percent: u32 },
    /// Escalate to human review.
    Escalate,
}

/// A configurable dispute resolution rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeRule {
    pub id: Uuid,
    pub name: String,
    pub condition_type: ConditionType,
    pub condition_value: String,
    pub action: DisputeAction,
    pub priority: i32,
    pub is_active: bool,
}

/// Context for evaluating dispute rules.
#[derive(Debug, Clone)]
pub struct DisputeContext {
    pub dispute_amount: Decimal,
    pub seller_score: f64,
    pub days_since_filed: u32,
    pub buyer_evidence_submitted: bool,
    pub seller_evidence_submitted: bool,
}

/// Evaluate all active rules against a dispute context.
/// Returns the action from the highest-priority matching rule, or None.
#[must_use]
pub fn evaluate_rules(rules: &[DisputeRule], context: &DisputeContext) -> Option<DisputeAction> {
    let mut active_rules: Vec<&DisputeRule> = rules.iter().filter(|r| r.is_active).collect();
    active_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    for rule in active_rules {
        if matches_condition(rule, context) {
            return Some(rule.action.clone());
        }
    }
    None
}

fn matches_condition(rule: &DisputeRule, ctx: &DisputeContext) -> bool {
    match rule.condition_type {
        ConditionType::AmountBelow => {
            if let Ok(threshold) = rule.condition_value.parse::<Decimal>() {
                ctx.dispute_amount < threshold
            } else {
                false
            }
        }
        ConditionType::SellerScoreBelow => {
            if let Ok(threshold) = rule.condition_value.parse::<f64>() {
                ctx.seller_score < threshold
            } else {
                false
            }
        }
        ConditionType::DaysExceeded => {
            if let Ok(threshold) = rule.condition_value.parse::<u32>() {
                ctx.days_since_filed > threshold
            } else {
                false
            }
        }
        ConditionType::BothPartiesEvidence => {
            ctx.buyer_evidence_submitted && ctx.seller_evidence_submitted
        }
    }
}

/// Create a standard set of default dispute rules.
#[must_use]
pub fn default_rules() -> Vec<DisputeRule> {
    vec![
        DisputeRule {
            id: Uuid::new_v4(),
            name: "Auto-refund small disputes".into(),
            condition_type: ConditionType::AmountBelow,
            condition_value: "25.00".into(),
            action: DisputeAction::RefundBuyer,
            priority: 100,
            is_active: true,
        },
        DisputeRule {
            id: Uuid::new_v4(),
            name: "Favor buyer for low-rep sellers".into(),
            condition_type: ConditionType::SellerScoreBelow,
            condition_value: "2.0".into(),
            action: DisputeAction::RefundBuyer,
            priority: 90,
            is_active: true,
        },
        DisputeRule {
            id: Uuid::new_v4(),
            name: "Auto-resolve after 30 days".into(),
            condition_type: ConditionType::DaysExceeded,
            condition_value: "30".into(),
            action: DisputeAction::Split { buyer_percent: 50 },
            priority: 50,
            is_active: true,
        },
        DisputeRule {
            id: Uuid::new_v4(),
            name: "Escalate when both submit evidence".into(),
            condition_type: ConditionType::BothPartiesEvidence,
            condition_value: "true".into(),
            action: DisputeAction::Escalate,
            priority: 80,
            is_active: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn auto_refund_small_dispute() {
        let rules = default_rules();
        let ctx = DisputeContext {
            dispute_amount: dec!(10.00),
            seller_score: 4.5,
            days_since_filed: 2,
            buyer_evidence_submitted: false,
            seller_evidence_submitted: false,
        };
        let action = evaluate_rules(&rules, &ctx);
        assert_eq!(action, Some(DisputeAction::RefundBuyer));
    }

    #[test]
    fn favor_buyer_low_rep_seller() {
        let rules = default_rules();
        let ctx = DisputeContext {
            dispute_amount: dec!(500.00),
            seller_score: 1.5,
            days_since_filed: 5,
            buyer_evidence_submitted: true,
            seller_evidence_submitted: false,
        };
        let action = evaluate_rules(&rules, &ctx);
        assert_eq!(action, Some(DisputeAction::RefundBuyer));
    }

    #[test]
    fn escalate_when_both_submit_evidence() {
        let rules = default_rules();
        let ctx = DisputeContext {
            dispute_amount: dec!(500.00),
            seller_score: 4.0,
            days_since_filed: 10,
            buyer_evidence_submitted: true,
            seller_evidence_submitted: true,
        };
        let action = evaluate_rules(&rules, &ctx);
        assert_eq!(action, Some(DisputeAction::Escalate));
    }

    #[test]
    fn split_after_30_days() {
        let rules = default_rules();
        let ctx = DisputeContext {
            dispute_amount: dec!(200.00),
            seller_score: 3.5,
            days_since_filed: 35,
            buyer_evidence_submitted: false,
            seller_evidence_submitted: false,
        };
        let action = evaluate_rules(&rules, &ctx);
        assert_eq!(action, Some(DisputeAction::Split { buyer_percent: 50 }));
    }

    #[test]
    fn no_match_returns_none() {
        let rules = default_rules();
        let ctx = DisputeContext {
            dispute_amount: dec!(100.00),
            seller_score: 4.5,
            days_since_filed: 5,
            buyer_evidence_submitted: false,
            seller_evidence_submitted: false,
        };
        let action = evaluate_rules(&rules, &ctx);
        assert!(action.is_none());
    }

    #[test]
    fn priority_ordering() {
        let rules = vec![
            DisputeRule {
                id: Uuid::new_v4(),
                name: "Low priority".into(),
                condition_type: ConditionType::AmountBelow,
                condition_value: "1000".into(),
                action: DisputeAction::Split { buyer_percent: 50 },
                priority: 10,
                is_active: true,
            },
            DisputeRule {
                id: Uuid::new_v4(),
                name: "High priority".into(),
                condition_type: ConditionType::AmountBelow,
                condition_value: "1000".into(),
                action: DisputeAction::RefundBuyer,
                priority: 100,
                is_active: true,
            },
        ];
        let ctx = DisputeContext {
            dispute_amount: dec!(50.00),
            seller_score: 4.0,
            days_since_filed: 1,
            buyer_evidence_submitted: false,
            seller_evidence_submitted: false,
        };
        // High priority rule should win
        assert_eq!(evaluate_rules(&rules, &ctx), Some(DisputeAction::RefundBuyer));
    }
}
