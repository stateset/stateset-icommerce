//! Policy evaluation integration tests.
//!
//! These tests verify that the policy engine correctly evaluates rules
//! against commerce contexts, exercising the policy crate in conjunction
//! with serialized commerce data.

use serde_json::json;
use stateset_policy::{
    Condition, ConditionGroup, ConditionNode, Logic, Operator, PolicyAction, PolicyEngine,
    PolicyRule, PolicySet,
};

// ---------------------------------------------------------------------------
// Deny Rule Tests
// ---------------------------------------------------------------------------

#[test]
fn deny_high_value_orders() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("high-value-review", "Require review for high-value orders")
        .with_priority(10)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(
                "order.total",
                Operator::Gt,
                json!(10000),
            ))],
        ))
        .with_action(PolicyAction::deny(
            "Order exceeds $10,000 limit",
            "Request manager approval",
        ));

    let policy_set = PolicySet::new("order-limits", "orders").with_rule(rule);
    engine.register_policy_set(policy_set);

    // Order above limit => deny
    let result = engine.evaluate(
        "orders",
        &json!({
            "order": { "total": 15000, "customer": { "tier": "standard" } }
        }),
    );

    assert!(result.should_deny);
    assert!(!result.should_allow);
    assert_eq!(result.explanations.len(), 1);
}

#[test]
fn allow_normal_value_orders() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("high-value-review", "Require review for high-value orders")
        .with_priority(10)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(
                "order.total",
                Operator::Gt,
                json!(10000),
            ))],
        ))
        .with_action(PolicyAction::deny(
            "Order exceeds $10,000 limit",
            "Request manager approval",
        ));

    let policy_set = PolicySet::new("order-limits", "orders")
        .with_rule(rule)
        .with_default_action(PolicyAction::allow());
    engine.register_policy_set(policy_set);

    // Order below limit => allow (default action)
    let result = engine.evaluate(
        "orders",
        &json!({
            "order": { "total": 500 }
        }),
    );

    assert!(result.should_allow);
    assert!(!result.should_deny);
}

// ---------------------------------------------------------------------------
// Deny-Overrides Precedence
// ---------------------------------------------------------------------------

#[test]
fn deny_overrides_allow_across_policy_sets() {
    let mut engine = PolicyEngine::new();

    // Policy set 1: Allow all orders
    let allow_set = PolicySet::new("allow-all", "orders").with_rule(
        PolicyRule::new("allow", "Allow everything")
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "order.total",
                    Operator::Gt,
                    json!(0),
                ))],
            ))
            .with_action(PolicyAction::allow()),
    );

    // Policy set 2: Deny high-value
    let deny_set = PolicySet::new("deny-high", "orders").with_rule(
        PolicyRule::new("deny-high-value", "Block orders over 10k")
            .with_priority(100)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "order.total",
                    Operator::Gt,
                    json!(10000),
                ))],
            ))
            .with_action(PolicyAction::deny(
                "Too expensive",
                "Get approval",
            )),
    );

    engine.register_policy_set(allow_set);
    engine.register_policy_set(deny_set);

    // Even though allow-all matches, deny should override
    let result = engine.evaluate(
        "orders",
        &json!({ "order": { "total": 25000 } }),
    );

    assert!(result.should_deny, "Deny should override allow");
    assert!(!result.should_allow);
}

// ---------------------------------------------------------------------------
// Dry-Run Mode
// ---------------------------------------------------------------------------

#[test]
fn dry_run_does_not_record_history() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("test", "Test rule")
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(
                "x",
                Operator::Gt,
                json!(0),
            ))],
        ))
        .with_action(PolicyAction::deny("denied", "fix"));

    engine.register_policy_set(PolicySet::new("test-set", "orders").with_rule(rule));

    let result = engine.evaluate_dry_run("orders", &json!({ "x": 5 }));

    assert!(result.should_deny);
    assert!(result.dry_run);
    assert!(engine.get_history().is_empty(), "Dry-run should not record history");
}

#[test]
fn evaluate_records_history() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("test", "Test rule")
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(
                "x",
                Operator::Gt,
                json!(0),
            ))],
        ))
        .with_action(PolicyAction::allow());

    engine.register_policy_set(PolicySet::new("test-set", "orders").with_rule(rule));

    engine.evaluate("orders", &json!({ "x": 5 }));
    engine.evaluate("orders", &json!({ "x": 10 }));

    assert_eq!(engine.get_history().len(), 2);

    let recent = engine.get_recent_history(1);
    assert_eq!(recent.len(), 1);
}

// ---------------------------------------------------------------------------
// Policy from JSON
// ---------------------------------------------------------------------------

#[test]
fn load_policy_set_from_json_value() {
    let mut engine = PolicyEngine::new();

    // Build policy programmatically as if loaded from JSON context
    let rule = PolicyRule::new("max-items", "Limit order items")
        .with_priority(5)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(
                "order.item_count",
                Operator::Gt,
                json!(50),
            ))],
        ))
        .with_action(PolicyAction::deny(
            "Too many items in order",
            "Split into multiple orders",
        ));

    engine.register_policy_set(PolicySet::new("item-limits", "orders").with_rule(rule));

    // Under limit
    let result = engine.evaluate("orders", &json!({ "order": { "item_count": 10 } }));
    assert!(result.should_allow);

    // Over limit
    let result = engine.evaluate("orders", &json!({ "order": { "item_count": 100 } }));
    assert!(result.should_deny);
}

// ---------------------------------------------------------------------------
// Complex Conditions (AND/OR)
// ---------------------------------------------------------------------------

#[test]
fn complex_or_conditions() {
    let mut engine = PolicyEngine::new();

    // Deny if total > 5000 OR customer is flagged
    let rule = PolicyRule::new("risk-check", "Risk assessment")
        .with_conditions(ConditionGroup::new(
            Logic::Or,
            vec![
                ConditionNode::Leaf(Condition::new(
                    "order.total",
                    Operator::Gt,
                    json!(5000),
                )),
                ConditionNode::Leaf(Condition::new(
                    "customer.flagged",
                    Operator::Eq,
                    json!(true),
                )),
            ],
        ))
        .with_action(PolicyAction::deny("Risk detected", "Manual review"));

    engine.register_policy_set(PolicySet::new("risk", "orders").with_rule(rule));

    // High total => deny
    let result = engine.evaluate(
        "orders",
        &json!({ "order": { "total": 6000 }, "customer": { "flagged": false } }),
    );
    assert!(result.should_deny);

    // Flagged customer => deny
    let result = engine.evaluate(
        "orders",
        &json!({ "order": { "total": 100 }, "customer": { "flagged": true } }),
    );
    assert!(result.should_deny);

    // Neither condition => allow (no default means vacuous allow)
    let result = engine.evaluate(
        "orders",
        &json!({ "order": { "total": 100 }, "customer": { "flagged": false } }),
    );
    assert!(result.should_allow);
}

#[test]
fn multiple_domains_independent() {
    let mut engine = PolicyEngine::new();

    // Orders domain: deny high value
    engine.register_policy_set(
        PolicySet::new("order-limits", "orders").with_rule(
            PolicyRule::new("high-value", "High value check")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "total",
                        Operator::Gt,
                        json!(1000),
                    ))],
                ))
                .with_action(PolicyAction::deny("Too high", "Lower amount")),
        ),
    );

    // Returns domain: deny old returns
    engine.register_policy_set(
        PolicySet::new("return-limits", "returns").with_rule(
            PolicyRule::new("too-old", "Return window check")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "days_since_purchase",
                        Operator::Gt,
                        json!(30),
                    ))],
                ))
                .with_action(PolicyAction::deny("Too old", "Contact support")),
        ),
    );

    // Order evaluation should not affect returns
    let order_result = engine.evaluate("orders", &json!({ "total": 5000 }));
    assert!(order_result.should_deny);

    let return_result = engine.evaluate("returns", &json!({ "days_since_purchase": 10 }));
    assert!(return_result.should_allow);

    let old_return = engine.evaluate("returns", &json!({ "days_since_purchase": 45 }));
    assert!(old_return.should_deny);
}

#[test]
fn engine_status_reflects_registered_policies() {
    let mut engine = PolicyEngine::new();

    engine.register_policy_set(
        PolicySet::new("set-1", "orders").with_rule(
            PolicyRule::new("r1", "Rule 1")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("x", Operator::Gt, json!(0)))],
                ))
                .with_action(PolicyAction::allow()),
        ),
    );

    engine.register_policy_set(
        PolicySet::new("set-2", "orders").with_rule(
            PolicyRule::new("r2", "Rule 2")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("y", Operator::Gt, json!(0)))],
                ))
                .with_action(PolicyAction::allow()),
        ),
    );

    let status = engine.get_status();
    assert_eq!(status.total_policy_sets, 2);
    assert_eq!(status.total_rules, 2);
    assert_eq!(status.by_domain.get("orders"), Some(&2));
}
