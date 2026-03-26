//! Pre-built policy templates for common commerce scenarios.
//!
//! These correspond to the `PolicyTemplates` object in the JS engine
//! (lines 884-1136 of `engine.js`).

use serde_json::json;

use crate::action::PolicyAction;
use crate::condition::{Condition, ConditionGroup, ConditionNode, Logic};
use crate::operator::Operator;
use crate::policy_set::PolicySet;
use crate::rule::PolicyRule;

/// Auto-approve returns under $100 for VIP customers.
///
/// Rules:
/// 1. **`auto_approve_small_vip_returns`** (priority 100, stop-on-match):
///    `return.value` < 100 AND `customer.lifetimeValue` > 500 AND `customer.returnRate` < 0.1
///    => Agent action: approve return
/// 2. **`flag_high_value_returns`** (priority 50):
///    `return.value` >= 500 OR `customer.returnRate` >= 0.2
///    => Workflow: `returnProcessing` (requires approval)
#[must_use] 
pub fn auto_approve_returns_template() -> PolicySet {
    PolicySet::new("Auto-Approve Small Returns", "returns")
        .with_description(
            "Automatically approve returns under $100 for customers with high lifetime value",
        )
        .with_rule(
            PolicyRule::new(
                "auto_approve_small_vip_returns",
                "Auto-approve returns < $100 for VIP customers",
            )
            .with_priority(100)
            .with_stop_on_match()
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new("return.value", Operator::Lt, json!(100))),
                    ConditionNode::Leaf(Condition::new(
                        "customer.lifetimeValue",
                        Operator::Gt,
                        json!(500),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "customer.returnRate",
                        Operator::Lt,
                        json!(0.1),
                    )),
                ],
            ))
            .with_action(PolicyAction::agent(
                "returns",
                "Approve return {return.id} - auto-approved per policy",
            )),
        )
        .with_rule(
            PolicyRule::new("flag_high_value_returns", "Flag high-value returns for manual review")
                .with_priority(50)
                .with_conditions(ConditionGroup::new(
                    Logic::Or,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "return.value",
                            Operator::Gte,
                            json!(500),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "customer.returnRate",
                            Operator::Gte,
                            json!(0.2),
                        )),
                    ],
                ))
                .with_action(
                    PolicyAction::workflow("returnProcessing")
                        .with_metadata(json!({"requiresApproval": true})),
                ),
        )
}

/// Inventory restock triggers.
///
/// Rules:
/// 1. **`critical_stock_alert`** (priority 100, stop-on-match):
///    `inventory.quantity` <= 5 AND `inventory.reorderPoint` > 0
///    => Agent: create urgent PO
/// 2. **`low_stock_reorder`** (priority 50):
///    `inventory.quantity` <= `${inventory.reorderPoint}` (dynamic ref)
///    => Agent: create standard PO
#[must_use] 
pub fn inventory_restock_template() -> PolicySet {
    PolicySet::new("Inventory Restock Rules", "inventory")
        .with_description("Automatically trigger restock when inventory is low")
        .with_rule(
            PolicyRule::new(
                "critical_stock_alert",
                "Create urgent PO when stock is critically low",
            )
            .with_priority(100)
            .with_stop_on_match()
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new(
                        "inventory.quantity",
                        Operator::Lte,
                        json!(5),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "inventory.reorderPoint",
                        Operator::Gt,
                        json!(0),
                    )),
                ],
            ))
            .with_action(PolicyAction::agent(
                "suppliers",
                "Create urgent purchase order for SKU {inventory.sku} - critical stock level",
            )),
        )
        .with_rule(
            PolicyRule::new(
                "low_stock_reorder",
                "Create standard PO when below reorder point",
            )
            .with_priority(50)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "inventory.quantity",
                    Operator::Lte,
                    json!("${inventory.reorderPoint}"),
                ))],
            ))
            .with_action(PolicyAction::agent(
                "suppliers",
                "Create purchase order for SKU {inventory.sku} to restock to {inventory.targetQuantity}",
            )),
        )
}

/// Order fraud detection rules.
///
/// Rules:
/// 1. **`high_value_new_customer`** (priority 100):
///    `order.total` > 1000 AND `customer.orderCount` < 2
///    => Workflow: `orderFulfillment` (high risk)
/// 2. **`velocity_check`** (priority 90):
///    `customer.ordersLast24h` > 3 AND `order.total` > 200
///    => Notify: Slack velocity alert
/// 3. **`shipping_billing_mismatch`** (priority 80):
///    `order.shippingAddress.country` != `${order.billingAddress.country}` AND `order.total` > 500
///    => Workflow: `orderFulfillment` (medium risk)
#[must_use] 
pub fn order_fraud_detection_template() -> PolicySet {
    PolicySet::new("Order Fraud Detection", "orders")
        .with_description("Flag potentially fraudulent orders")
        .with_rule(
            PolicyRule::new(
                "high_value_new_customer",
                "Flag high-value orders from new customers",
            )
            .with_priority(100)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(1000),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "customer.orderCount",
                        Operator::Lt,
                        json!(2),
                    )),
                ],
            ))
            .with_action(
                PolicyAction::workflow("orderFulfillment")
                    .with_metadata(json!({"requiresReview": true, "riskLevel": "high"})),
            ),
        )
        .with_rule(
            PolicyRule::new(
                "velocity_check",
                "Flag multiple orders in short time",
            )
            .with_priority(90)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new(
                        "customer.ordersLast24h",
                        Operator::Gt,
                        json!(3),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(200),
                    )),
                ],
            ))
            .with_action(PolicyAction::notify(json!({
                "channel": "slack",
                "message": "Velocity alert: Customer {customer.id} placed {customer.ordersLast24h} orders in 24h"
            }))),
        )
        .with_rule(
            PolicyRule::new(
                "shipping_billing_mismatch",
                "Flag orders with mismatched addresses",
            )
            .with_priority(80)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new(
                        "order.shippingAddress.country",
                        Operator::Neq,
                        json!("${order.billingAddress.country}"),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(500),
                    )),
                ],
            ))
            .with_action(
                PolicyAction::workflow("orderFulfillment")
                    .with_metadata(json!({"requiresReview": true, "riskLevel": "medium"})),
            ),
        )
}

/// Promotion eligibility rules.
///
/// Rules:
/// 1. **`vip_exclusive`** (priority 100):
///    `promotion.vipOnly` is true AND `customer.tier` in \["gold", "platinum"\]
///    => Allow
/// 2. **`block_vip_for_regular`** (priority 99, stop-on-match):
///    `promotion.vipOnly` is true AND `customer.tier` not in \["gold", "platinum"\]
///    => Deny
/// 3. **`no_double_discount`** (priority 50, stop-on-match):
///    `cart.hasPercentageDiscount` is true AND `promotion.type` == "percentage"
///    => Deny
#[must_use] 
pub fn promotion_eligibility_template() -> PolicySet {
    PolicySet::new("Promotion Eligibility Rules", "promotions")
        .with_description("Determine promotion eligibility and stacking")
        .with_rule(
            PolicyRule::new("vip_exclusive", "Allow VIP-only promotions")
                .with_priority(100)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "promotion.vipOnly",
                            Operator::IsTrue,
                            json!(null),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "customer.tier",
                            Operator::In,
                            json!(["gold", "platinum"]),
                        )),
                    ],
                ))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("block_vip_for_regular", "Block VIP promotions for regular customers")
                .with_priority(99)
                .with_stop_on_match()
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "promotion.vipOnly",
                            Operator::IsTrue,
                            json!(null),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "customer.tier",
                            Operator::NotIn,
                            json!(["gold", "platinum"]),
                        )),
                    ],
                ))
                .with_action(PolicyAction::deny_simple("VIP-only promotion")),
        )
        .with_rule(
            PolicyRule::new("no_double_discount", "Prevent stacking percentage discounts")
                .with_priority(50)
                .with_stop_on_match()
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "cart.hasPercentageDiscount",
                            Operator::IsTrue,
                            json!(null),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "promotion.type",
                            Operator::Eq,
                            json!("percentage"),
                        )),
                    ],
                ))
                .with_action(PolicyAction::deny_simple("Cannot stack percentage discounts")),
        )
}

/// Subscription lifecycle management rules.
///
/// Rules:
/// 1. **`auto_cancel_failed_payments`** (priority 100):
///    `subscription.consecutiveFailedPayments` >= 3
///    => Agent: cancel subscription
/// 2. **`offer_discount_on_cancel`** (priority 80):
///    event == `"cancellation_requested"` AND `subscription.monthsActive` >= 6
///    => Agent: offer retention discount
#[must_use] 
pub fn subscription_rules_template() -> PolicySet {
    PolicySet::new("Subscription Management Rules", "subscriptions")
        .with_description("Handle subscription lifecycle events")
        .with_rule(
            PolicyRule::new(
                "auto_cancel_failed_payments",
                "Cancel subscription after 3 failed payments",
            )
            .with_priority(100)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "subscription.consecutiveFailedPayments",
                    Operator::Gte,
                    json!(3),
                ))],
            ))
            .with_action(PolicyAction::agent(
                "subscriptions",
                "Cancel subscription {subscription.id} due to payment failures",
            )),
        )
        .with_rule(
            PolicyRule::new(
                "offer_discount_on_cancel",
                "Offer discount when long-term customer cancels",
            )
            .with_priority(80)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![
                    ConditionNode::Leaf(Condition::new(
                        "event",
                        Operator::Eq,
                        json!("cancellation_requested"),
                    )),
                    ConditionNode::Leaf(Condition::new(
                        "subscription.monthsActive",
                        Operator::Gte,
                        json!(6),
                    )),
                ],
            ))
            .with_action(PolicyAction::agent(
                "subscriptions",
                "Offer 20% retention discount to customer {customer.id} for subscription {subscription.id}",
            )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- Auto-approve returns ----

    #[test]
    fn auto_approve_returns_vip_small() {
        let ps = auto_approve_returns_template();
        let ctx = json!({
            "return": {"id": "R-001", "value": 50},
            "customer": {"lifetimeValue": 1000, "returnRate": 0.05}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        // Should match the auto-approve rule (stop-on-match)
        assert_eq!(eval.matched_rules.len(), 1);
        assert_eq!(eval.matched_rules[0].name, "auto_approve_small_vip_returns");
    }

    #[test]
    fn auto_approve_returns_high_value_flagged() {
        let ps = auto_approve_returns_template();
        let ctx = json!({
            "return": {"id": "R-002", "value": 600},
            "customer": {"lifetimeValue": 1000, "returnRate": 0.05}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert_eq!(eval.matched_rules[0].name, "flag_high_value_returns");
    }

    #[test]
    fn auto_approve_returns_normal_no_match() {
        let ps = auto_approve_returns_template();
        let ctx = json!({
            "return": {"id": "R-003", "value": 200},
            "customer": {"lifetimeValue": 100, "returnRate": 0.05}
        });
        let eval = ps.evaluate(&ctx);
        assert!(!eval.matched);
        assert!(eval.default_applied);
    }

    // ---- Inventory restock ----

    #[test]
    fn inventory_critical_stock() {
        let ps = inventory_restock_template();
        let ctx = json!({
            "inventory": {"sku": "WIDGET-001", "quantity": 3, "reorderPoint": 10, "targetQuantity": 100}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        // Critical stock alert has stop_on_match
        assert_eq!(eval.matched_rules.len(), 1);
        assert_eq!(eval.matched_rules[0].name, "critical_stock_alert");
    }

    #[test]
    fn inventory_low_stock_with_dynamic_ref() {
        let ps = inventory_restock_template();
        let ctx = json!({
            "inventory": {"sku": "WIDGET-002", "quantity": 8, "reorderPoint": 10, "targetQuantity": 100}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        // quantity=8 <= reorderPoint=10, so low_stock_reorder should match
        assert!(eval.matched_rules.iter().any(|r| r.name == "low_stock_reorder"));
    }

    // ---- Order fraud detection ----

    #[test]
    fn fraud_high_value_new_customer() {
        let ps = order_fraud_detection_template();
        let ctx = json!({
            "order": {
                "total": 2000,
                "shippingAddress": {"country": "US"},
                "billingAddress": {"country": "US"}
            },
            "customer": {"id": "C-001", "orderCount": 1, "ordersLast24h": 1}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.matched_rules.iter().any(|r| r.name == "high_value_new_customer"));
    }

    #[test]
    fn fraud_velocity_check() {
        let ps = order_fraud_detection_template();
        let ctx = json!({
            "order": {
                "total": 500,
                "shippingAddress": {"country": "US"},
                "billingAddress": {"country": "US"}
            },
            "customer": {"id": "C-002", "orderCount": 10, "ordersLast24h": 5}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.matched_rules.iter().any(|r| r.name == "velocity_check"));
    }

    #[test]
    fn fraud_address_mismatch() {
        let ps = order_fraud_detection_template();
        let ctx = json!({
            "order": {
                "total": 800,
                "shippingAddress": {"country": "NG"},
                "billingAddress": {"country": "US"}
            },
            "customer": {"id": "C-003", "orderCount": 10, "ordersLast24h": 1}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.matched_rules.iter().any(|r| r.name == "shipping_billing_mismatch"));
    }

    #[test]
    fn fraud_no_flags_normal_order() {
        let ps = order_fraud_detection_template();
        let ctx = json!({
            "order": {
                "total": 50,
                "shippingAddress": {"country": "US"},
                "billingAddress": {"country": "US"}
            },
            "customer": {"id": "C-004", "orderCount": 20, "ordersLast24h": 1}
        });
        let eval = ps.evaluate(&ctx);
        assert!(!eval.matched);
    }

    // ---- Promotion eligibility ----

    #[test]
    fn promotion_vip_allowed() {
        let ps = promotion_eligibility_template();
        let ctx = json!({
            "promotion": {"vipOnly": true, "type": "fixed"},
            "customer": {"tier": "gold"},
            "cart": {"hasPercentageDiscount": false}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.should_allow);
    }

    #[test]
    fn promotion_vip_blocked_for_regular() {
        let ps = promotion_eligibility_template();
        let ctx = json!({
            "promotion": {"vipOnly": true, "type": "fixed"},
            "customer": {"tier": "standard"},
            "cart": {"hasPercentageDiscount": false}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.should_deny);
    }

    #[test]
    fn promotion_no_stacking() {
        let ps = promotion_eligibility_template();
        let ctx = json!({
            "promotion": {"vipOnly": false, "type": "percentage"},
            "customer": {"tier": "standard"},
            "cart": {"hasPercentageDiscount": true}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.should_deny);
    }

    // ---- Subscription rules ----

    #[test]
    fn subscription_auto_cancel() {
        let ps = subscription_rules_template();
        let ctx = json!({
            "subscription": {"id": "SUB-001", "consecutiveFailedPayments": 3, "monthsActive": 2},
            "event": "payment_failed",
            "customer": {"id": "C-100"}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.matched_rules.iter().any(|r| r.name == "auto_cancel_failed_payments"));
    }

    #[test]
    fn subscription_retention_offer() {
        let ps = subscription_rules_template();
        let ctx = json!({
            "subscription": {"id": "SUB-002", "consecutiveFailedPayments": 0, "monthsActive": 12},
            "event": "cancellation_requested",
            "customer": {"id": "C-101"}
        });
        let eval = ps.evaluate(&ctx);
        assert!(eval.matched);
        assert!(eval.matched_rules.iter().any(|r| r.name == "offer_discount_on_cancel"));
    }

    // ---- Template counts ----

    #[test]
    fn template_rule_counts() {
        assert_eq!(auto_approve_returns_template().rules.len(), 2);
        assert_eq!(inventory_restock_template().rules.len(), 2);
        assert_eq!(order_fraud_detection_template().rules.len(), 3);
        assert_eq!(promotion_eligibility_template().rules.len(), 3);
        assert_eq!(subscription_rules_template().rules.len(), 2);
    }

    #[test]
    fn template_domains() {
        assert_eq!(auto_approve_returns_template().domain, "returns");
        assert_eq!(inventory_restock_template().domain, "inventory");
        assert_eq!(order_fraud_detection_template().domain, "orders");
        assert_eq!(promotion_eligibility_template().domain, "promotions");
        assert_eq!(subscription_rules_template().domain, "subscriptions");
    }
}
