//! Comprehensive integration tests for the stateset-policy crate.
//!
//! These tests exercise the full public API through the `PolicyEngine` entry
//! point, covering:
//!
//! 1. Basic policy evaluation (allow, deny, empty, unconditional)
//! 2. Deny-overrides precedence
//! 3. Condition operator evaluation (20 operators)
//! 4. Condition groups with boolean logic (AND/OR, nesting)
//! 5. Explainable denials (`PolicyExplanation` detail)
//! 6. Dry-run evaluation
//! 7. Priority ordering and stop-on-match
//! 8. Transform actions and `TransformAuditEntry`

use serde_json::json;
use stateset_policy::{
    ActionType, Condition, ConditionGroup, ConditionNode, Logic, Operator, PolicyAction,
    PolicyEngine, PolicyExplanation, PolicyRule, PolicySet, TransformAuditEntry, UnknownDomainMode,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a single-condition rule with the given action.
fn rule_with_condition(
    name: &str,
    field: &str,
    op: Operator,
    value: serde_json::Value,
    priority: i32,
    action: PolicyAction,
) -> PolicyRule {
    PolicyRule::new(name, format!("{name} description"))
        .with_priority(priority)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new(field, op, value))],
        ))
        .with_action(action)
}

fn deny_rule(
    name: &str,
    field: &str,
    op: Operator,
    value: serde_json::Value,
    priority: i32,
) -> PolicyRule {
    rule_with_condition(
        name,
        field,
        op,
        value,
        priority,
        PolicyAction::deny(format!("{name} denied"), format!("{name} remediation")),
    )
}

fn allow_rule(
    name: &str,
    field: &str,
    op: Operator,
    value: serde_json::Value,
    priority: i32,
) -> PolicyRule {
    rule_with_condition(name, field, op, value, priority, PolicyAction::allow())
}

// ===========================================================================
// 1. Basic Policy Evaluation
// ===========================================================================

#[test]
fn single_allow_rule_permits() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("allow-set", "orders").with_rule(allow_rule(
        "allow-all",
        "order.total",
        Operator::Gt,
        json!(0),
        10,
    )));

    let result = engine.evaluate("orders", &json!({"order": {"total": 100}}));
    assert!(result.should_allow);
    assert!(!result.should_deny);
}

#[test]
fn single_deny_rule_denies() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("deny-set", "orders").with_rule(deny_rule(
        "deny-high",
        "order.total",
        Operator::Gt,
        json!(1000),
        10,
    )));

    let result = engine.evaluate("orders", &json!({"order": {"total": 5000}}));
    assert!(result.should_deny);
    assert!(!result.should_allow);
}

#[test]
fn empty_policy_engine_denies_by_default() {
    let mut engine = PolicyEngine::new();
    let result = engine.evaluate("orders", &json!({"order": {"total": 100}}));
    assert!(!result.should_allow);
    assert!(result.should_deny);
}

#[test]
fn unknown_domain_can_be_configured_to_allow() {
    let mut engine = PolicyEngine::new().with_unknown_domain_mode(UnknownDomainMode::Allow);
    let result = engine.evaluate("orders", &json!({"order": {"total": 100}}));
    assert!(result.should_allow);
    assert!(!result.should_deny);
}

#[test]
fn rule_with_no_conditions_always_matches() {
    let mut engine = PolicyEngine::new();

    // PolicyRule::new creates a rule with an empty ConditionGroup (evaluates to true)
    let unconditional = PolicyRule::new("unconditional", "Always fires")
        .with_priority(10)
        .with_action(PolicyAction::deny("always denied", "no fix"));

    engine.register_policy_set(PolicySet::new("always-deny", "orders").with_rule(unconditional));

    let result = engine.evaluate("orders", &json!({}));
    assert!(result.should_deny);
    assert!(!result.should_allow);

    // Even with arbitrary context, it still matches
    let result2 = engine.evaluate("orders", &json!({"foo": "bar", "baz": 123}));
    assert!(result2.should_deny);
}

#[test]
fn unmatched_rule_falls_through_to_default_action() {
    let mut engine = PolicyEngine::new();

    let ps = PolicySet::new("guarded", "orders")
        .with_rule(deny_rule("high-value", "order.total", Operator::Gt, json!(10000), 100))
        .with_default_action(PolicyAction::allow());

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"order": {"total": 50}}));
    assert!(result.should_allow);
    assert!(!result.should_deny);
}

// ===========================================================================
// 2. Deny-Overrides Precedence
// ===========================================================================

#[test]
fn deny_overrides_single_allow() {
    let mut engine = PolicyEngine::new();

    let allow_set = PolicySet::new("allow-set", "orders").with_rule(allow_rule(
        "allow-all",
        "x",
        Operator::IsNotNull,
        json!(null),
        50,
    ));

    let deny_set = PolicySet::new("deny-set", "orders").with_rule(deny_rule(
        "deny-fraud",
        "x",
        Operator::IsNotNull,
        json!(null),
        100,
    ));

    engine.register_policy_set(allow_set);
    engine.register_policy_set(deny_set);

    let result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(result.should_deny, "Deny should override the Allow");
    assert!(!result.should_allow);
}

#[test]
fn deny_overrides_multiple_allows() {
    let mut engine = PolicyEngine::new();

    // Three Allow sets, one Deny set
    for i in 0..3 {
        engine.register_policy_set(PolicySet::new(format!("allow-{i}"), "orders").with_rule(
            allow_rule(&format!("allow-rule-{i}"), "x", Operator::Eq, json!(1), 10 + i),
        ));
    }

    engine.register_policy_set(PolicySet::new("deny-set", "orders").with_rule(deny_rule(
        "deny-rule",
        "x",
        Operator::Eq,
        json!(1),
        200,
    )));

    let result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(result.should_deny, "A single deny must override three allows");
    assert!(!result.should_allow);
}

#[test]
fn only_allow_rules_results_in_allow() {
    let mut engine = PolicyEngine::new();

    engine.register_policy_set(PolicySet::new("allow-1", "orders").with_rule(allow_rule(
        "a1",
        "x",
        Operator::Eq,
        json!(1),
        10,
    )));
    engine.register_policy_set(PolicySet::new("allow-2", "orders").with_rule(allow_rule(
        "a2",
        "x",
        Operator::Eq,
        json!(1),
        20,
    )));

    let result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(result.should_allow);
    assert!(!result.should_deny);
}

#[test]
fn deny_in_same_policy_set_overrides_allow() {
    let mut engine = PolicyEngine::new();

    let ps = PolicySet::new("mixed", "orders")
        .with_rule(allow_rule("allow-r", "x", Operator::Eq, json!(1), 10))
        .with_rule(deny_rule("deny-r", "x", Operator::Eq, json!(1), 100));

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(result.should_deny);
    assert!(!result.should_allow);
    assert_eq!(result.results[0].matched_rules.len(), 2);
}

// ===========================================================================
// 3. Condition Evaluation — Operators
// ===========================================================================

/// Convenience: evaluate a single condition against a context and return whether it matched.
fn eval_condition(
    field: &str,
    op: Operator,
    value: serde_json::Value,
    ctx: &serde_json::Value,
) -> bool {
    Condition::new(field, op, value).evaluate(ctx)
}

#[test]
fn operator_gt_gte_lt_lte() {
    let ctx = json!({"amount": 50});

    assert!(eval_condition("amount", Operator::Gt, json!(49), &ctx));
    assert!(!eval_condition("amount", Operator::Gt, json!(50), &ctx));

    assert!(eval_condition("amount", Operator::Gte, json!(50), &ctx));
    assert!(!eval_condition("amount", Operator::Gte, json!(51), &ctx));

    assert!(eval_condition("amount", Operator::Lt, json!(51), &ctx));
    assert!(!eval_condition("amount", Operator::Lt, json!(50), &ctx));

    assert!(eval_condition("amount", Operator::Lte, json!(50), &ctx));
    assert!(!eval_condition("amount", Operator::Lte, json!(49), &ctx));
}

#[test]
fn operator_eq_ne() {
    let ctx = json!({"status": "active", "count": 10});

    assert!(eval_condition("status", Operator::Eq, json!("active"), &ctx));
    assert!(!eval_condition("status", Operator::Eq, json!("inactive"), &ctx));

    assert!(eval_condition("status", Operator::Neq, json!("inactive"), &ctx));
    assert!(!eval_condition("status", Operator::Neq, json!("active"), &ctx));

    assert!(eval_condition("count", Operator::Eq, json!(10), &ctx));
    assert!(eval_condition("count", Operator::Neq, json!(20), &ctx));
}

#[test]
fn operator_in_and_not_in() {
    let ctx = json!({"tier": "gold"});

    assert!(eval_condition("tier", Operator::In, json!(["gold", "platinum"]), &ctx));
    assert!(!eval_condition("tier", Operator::In, json!(["silver", "bronze"]), &ctx));

    assert!(eval_condition("tier", Operator::NotIn, json!(["silver", "bronze"]), &ctx));
    assert!(!eval_condition("tier", Operator::NotIn, json!(["gold", "platinum"]), &ctx));
}

#[test]
fn operator_in_with_numeric_values() {
    let ctx = json!({"region_id": 3});
    assert!(eval_condition("region_id", Operator::In, json!([1, 2, 3, 4]), &ctx));
    assert!(!eval_condition("region_id", Operator::In, json!([5, 6, 7]), &ctx));
}

#[test]
fn operator_contains_starts_with_ends_with() {
    let ctx = json!({"email": "john@example.com", "name": "John Doe"});

    assert!(eval_condition("email", Operator::Contains, json!("example"), &ctx));
    assert!(!eval_condition("email", Operator::Contains, json!("gmail"), &ctx));

    assert!(eval_condition("email", Operator::StartsWith, json!("john@"), &ctx));
    assert!(!eval_condition("email", Operator::StartsWith, json!("jane@"), &ctx));

    assert!(eval_condition("email", Operator::EndsWith, json!(".com"), &ctx));
    assert!(!eval_condition("email", Operator::EndsWith, json!(".org"), &ctx));

    assert!(eval_condition("name", Operator::StartsWith, json!("John"), &ctx));
    assert!(eval_condition("name", Operator::EndsWith, json!("Doe"), &ctx));
}

#[test]
fn operator_is_null_and_is_not_null() {
    let ctx = json!({"email": "a@b.com", "phone": null});

    assert!(!eval_condition("email", Operator::IsNull, json!(null), &ctx));
    assert!(eval_condition("email", Operator::IsNotNull, json!(null), &ctx));

    assert!(eval_condition("phone", Operator::IsNull, json!(null), &ctx));
    assert!(!eval_condition("phone", Operator::IsNotNull, json!(null), &ctx));

    // Missing field resolves to null
    assert!(eval_condition("address", Operator::IsNull, json!(null), &ctx));
    assert!(!eval_condition("address", Operator::IsNotNull, json!(null), &ctx));
}

#[test]
fn operator_between_and_divisible_by() {
    let ctx = json!({"age": 25, "quantity": 12});

    assert!(eval_condition("age", Operator::Between, json!([18, 65]), &ctx));
    assert!(!eval_condition("age", Operator::Between, json!([30, 65]), &ctx));

    assert!(eval_condition("quantity", Operator::DivisibleBy, json!(3), &ctx));
    assert!(eval_condition("quantity", Operator::DivisibleBy, json!(4), &ctx));
    assert!(!eval_condition("quantity", Operator::DivisibleBy, json!(5), &ctx));
}

#[test]
fn operator_matches_regex() {
    let ctx = json!({"order_id": "ORD-2024-00123"});
    assert!(eval_condition("order_id", Operator::Matches, json!(r"^ORD-\d{4}-\d+$"), &ctx));
    assert!(!eval_condition("order_id", Operator::Matches, json!(r"^RET-\d+$"), &ctx));
}

#[test]
fn operator_is_empty_is_not_empty() {
    let ctx = json!({"tags": [], "items": [1, 2], "description": "", "title": "Widget"});

    assert!(eval_condition("tags", Operator::IsEmpty, json!(null), &ctx));
    assert!(!eval_condition("items", Operator::IsEmpty, json!(null), &ctx));

    assert!(eval_condition("items", Operator::IsNotEmpty, json!(null), &ctx));
    assert!(!eval_condition("tags", Operator::IsNotEmpty, json!(null), &ctx));

    assert!(eval_condition("description", Operator::IsEmpty, json!(null), &ctx));
    assert!(eval_condition("title", Operator::IsNotEmpty, json!(null), &ctx));
}

#[test]
fn nested_dot_notation_field_paths() {
    let ctx = json!({
        "order": {
            "customer": {
                "email": "vip@example.com",
                "address": {
                    "country": "US",
                    "zip": "90210"
                }
            },
            "items": [
                {"sku": "SKU-001", "qty": 5}
            ],
            "total": 250.50
        }
    });

    assert!(eval_condition(
        "order.customer.email",
        Operator::EndsWith,
        json!("@example.com"),
        &ctx
    ));
    assert!(eval_condition("order.customer.address.country", Operator::Eq, json!("US"), &ctx));
    assert!(eval_condition("order.customer.address.zip", Operator::StartsWith, json!("90"), &ctx));
    assert!(eval_condition("order.total", Operator::Gte, json!(250), &ctx));
    // Array indexing
    assert!(eval_condition("order.items[0].sku", Operator::Eq, json!("SKU-001"), &ctx));
    assert!(eval_condition("order.items[0].qty", Operator::Lt, json!(10), &ctx));
}

// ===========================================================================
// 4. Condition Groups — Boolean Logic
// ===========================================================================

#[test]
fn and_group_all_must_match() {
    let group = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("age", Operator::Gte, json!(18))),
            ConditionNode::Leaf(Condition::new("country", Operator::Eq, json!("US"))),
            ConditionNode::Leaf(Condition::new("verified", Operator::IsTrue, json!(null))),
        ],
    );

    let matching = json!({"age": 21, "country": "US", "verified": true});
    assert!(group.evaluate(&matching));

    let fails_age = json!({"age": 16, "country": "US", "verified": true});
    assert!(!group.evaluate(&fails_age));

    let fails_country = json!({"age": 21, "country": "CA", "verified": true});
    assert!(!group.evaluate(&fails_country));

    let fails_verified = json!({"age": 21, "country": "US", "verified": false});
    assert!(!group.evaluate(&fails_verified));
}

#[test]
fn or_group_any_can_match() {
    let group = ConditionGroup::new(
        Logic::Or,
        vec![
            ConditionNode::Leaf(Condition::new("tier", Operator::Eq, json!("gold"))),
            ConditionNode::Leaf(Condition::new("tier", Operator::Eq, json!("platinum"))),
            ConditionNode::Leaf(Condition::new("lifetime_value", Operator::Gt, json!(10000))),
        ],
    );

    assert!(group.evaluate(&json!({"tier": "gold", "lifetime_value": 500})));
    assert!(group.evaluate(&json!({"tier": "platinum", "lifetime_value": 500})));
    assert!(group.evaluate(&json!({"tier": "bronze", "lifetime_value": 50000})));
    assert!(!group.evaluate(&json!({"tier": "bronze", "lifetime_value": 500})));
}

#[test]
fn nested_and_inside_or() {
    // (country == "US" AND age >= 21) OR (country == "CA" AND age >= 19)
    let us_group = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("country", Operator::Eq, json!("US"))),
            ConditionNode::Leaf(Condition::new("age", Operator::Gte, json!(21))),
        ],
    );
    let ca_group = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("country", Operator::Eq, json!("CA"))),
            ConditionNode::Leaf(Condition::new("age", Operator::Gte, json!(19))),
        ],
    );
    let outer = ConditionGroup::new(
        Logic::Or,
        vec![ConditionNode::Group(us_group), ConditionNode::Group(ca_group)],
    );

    // US resident, age 22 => matches first group
    assert!(outer.evaluate(&json!({"country": "US", "age": 22})));
    // CA resident, age 20 => matches second group
    assert!(outer.evaluate(&json!({"country": "CA", "age": 20})));
    // US resident, age 18 => too young for US rules
    assert!(!outer.evaluate(&json!({"country": "US", "age": 18})));
    // UK resident => neither group
    assert!(!outer.evaluate(&json!({"country": "UK", "age": 30})));
}

#[test]
fn nested_or_inside_and() {
    // verified == true AND (tier IN ["gold","platinum"] OR lifetime_value > 5000)
    let tier_or_value = ConditionGroup::new(
        Logic::Or,
        vec![
            ConditionNode::Leaf(Condition::new("tier", Operator::In, json!(["gold", "platinum"]))),
            ConditionNode::Leaf(Condition::new("lifetime_value", Operator::Gt, json!(5000))),
        ],
    );
    let outer = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("verified", Operator::IsTrue, json!(null))),
            ConditionNode::Group(tier_or_value),
        ],
    );

    // Verified gold customer => matches
    assert!(outer.evaluate(&json!({"verified": true, "tier": "gold", "lifetime_value": 100})));
    // Verified high-value bronze => matches via lifetime_value
    assert!(outer.evaluate(&json!({"verified": true, "tier": "bronze", "lifetime_value": 10000})));
    // Not verified => fails
    assert!(!outer.evaluate(&json!({"verified": false, "tier": "gold", "lifetime_value": 10000})));
    // Verified but low-tier and low-value => fails
    assert!(!outer.evaluate(&json!({"verified": true, "tier": "bronze", "lifetime_value": 100})));
}

#[test]
fn empty_and_group_evaluates_to_true() {
    let group = ConditionGroup::new(Logic::And, vec![]);
    assert!(group.evaluate(&json!({})));
}

#[test]
fn empty_or_group_evaluates_to_false() {
    let group = ConditionGroup::new(Logic::Or, vec![]);
    assert!(!group.evaluate(&json!({})));
}

// ===========================================================================
// 5. Explainable Denials
// ===========================================================================

#[test]
fn denial_includes_explanation_with_condition_breakdown() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("high-value-check", "Deny orders over $10,000")
        .with_priority(100)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(10000)))],
        ))
        .with_action(PolicyAction::deny(
            "Order exceeds $10,000 spending limit",
            "Request manager approval or split into smaller orders",
        ));

    engine.register_policy_set(PolicySet::new("spending-limits", "orders").with_rule(rule));

    let result = engine.evaluate("orders", &json!({"order": {"total": 15000}}));

    assert!(result.should_deny);
    assert_eq!(result.explanations.len(), 1);

    let explanation = &result.explanations[0];
    assert_eq!(explanation.policy_set_name, "spending-limits");
    assert_eq!(explanation.rule_name, "high-value-check");
    assert_eq!(explanation.action_type, ActionType::Deny);
    assert_eq!(explanation.reason, "Order exceeds $10,000 spending limit");
    assert_eq!(
        explanation.remediation.as_deref(),
        Some("Request manager approval or split into smaller orders")
    );
}

#[test]
fn explanation_has_field_operator_expected_actual() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("check-amount", "Amount check")
        .with_priority(10)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("payment.amount", Operator::Gt, json!(500)))],
        ))
        .with_action(PolicyAction::deny("Amount too high", "Lower the amount"));

    engine.register_policy_set(PolicySet::new("payment-limits", "payments").with_rule(rule));

    let result = engine.evaluate("payments", &json!({"payment": {"amount": 750}}));

    assert_eq!(result.explanations.len(), 1);
    let conditions = &result.explanations[0].conditions;
    assert_eq!(conditions.len(), 1);

    let detail = &conditions[0];
    assert!(detail.matched);
    assert_eq!(detail.field, "payment.amount");
    assert_eq!(detail.operator, Operator::Gt);
    assert_eq!(detail.expected_value, json!(500));
    assert_eq!(detail.actual_value, json!(750));
}

#[test]
fn multiple_conditions_each_produce_explanation_detail() {
    let mut engine = PolicyEngine::new();

    let rule = PolicyRule::new("multi-check", "Multi-condition denial")
        .with_priority(10)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(1000))),
                ConditionNode::Leaf(Condition::new(
                    "customer.tier",
                    Operator::NotIn,
                    json!(["gold", "platinum"]),
                )),
                ConditionNode::Leaf(Condition::new(
                    "customer.verified",
                    Operator::IsFalse,
                    json!(null),
                )),
            ],
        ))
        .with_action(PolicyAction::deny("Risky order", "Verify customer"));

    engine.register_policy_set(PolicySet::new("fraud-check", "orders").with_rule(rule));

    let result = engine.evaluate(
        "orders",
        &json!({
            "order": {"total": 5000},
            "customer": {"tier": "bronze", "verified": false}
        }),
    );

    assert!(result.should_deny);
    assert_eq!(result.explanations.len(), 1);

    let conditions = &result.explanations[0].conditions;
    assert_eq!(conditions.len(), 3, "Each condition should produce a detail");
    assert!(conditions.iter().all(|c| c.matched));

    // Verify each condition's field is present
    let fields: Vec<&str> = conditions.iter().map(|c| c.field.as_str()).collect();
    assert!(fields.contains(&"order.total"));
    assert!(fields.contains(&"customer.tier"));
    assert!(fields.contains(&"customer.verified"));
}

#[test]
fn explanation_display_format_is_human_readable() {
    let explanation = PolicyExplanation {
        policy_set_id: Uuid::nil(),
        policy_set_name: "Order Limits".into(),
        rule_id: Uuid::nil(),
        rule_name: "high-value".into(),
        rule_description: "Flag high-value orders".into(),
        action_type: ActionType::Deny,
        reason: "Order exceeds $10,000 limit".into(),
        remediation: Some("Request manager approval".into()),
        conditions: vec![stateset_policy::ConditionDetail {
            matched: true,
            field: "order.total".into(),
            operator: Operator::Gt,
            expected_value: json!(10000),
            actual_value: json!(15000),
        }],
    };

    let display = explanation.to_string();
    assert!(display.contains("Order Limits"), "Should contain policy set name");
    assert!(display.contains("high-value"), "Should contain rule name");
    assert!(display.contains("deny"), "Should contain action type");
    assert!(display.contains("order.total"), "Should contain field name");
    assert!(display.contains("Remediation:"), "Should contain remediation");
}

// ===========================================================================
// 6. Dry-Run Evaluation
// ===========================================================================

#[test]
fn dry_run_returns_same_result_as_evaluate() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("limits", "orders").with_rule(deny_rule(
        "high-value",
        "order.total",
        Operator::Gt,
        json!(1000),
        100,
    )));

    let ctx = json!({"order": {"total": 5000}});

    let dry = engine.evaluate_dry_run("orders", &ctx);
    let live = engine.evaluate("orders", &ctx);

    assert_eq!(dry.should_deny, live.should_deny);
    assert_eq!(dry.should_allow, live.should_allow);
    assert_eq!(dry.explanations.len(), live.explanations.len());
}

#[test]
fn dry_run_does_not_record_history() {
    let engine = PolicyEngine::new();

    let _result = engine.evaluate_dry_run("orders", &json!({}));
    assert!(engine.get_history().is_empty(), "Dry run must not record history");
}

#[test]
fn dry_run_flag_is_set() {
    let engine = PolicyEngine::new();
    let result = engine.evaluate_dry_run("orders", &json!({}));
    assert!(result.dry_run, "dry_run flag should be true");
}

#[test]
fn evaluate_sets_dry_run_to_false() {
    let mut engine = PolicyEngine::new();
    let result = engine.evaluate("orders", &json!({}));
    assert!(!result.dry_run, "evaluate should set dry_run to false");
}

// ===========================================================================
// 7. Priority and Ordering
// ===========================================================================

#[test]
fn higher_priority_rules_evaluated_first() {
    let mut engine = PolicyEngine::new();

    // Low-priority allow (10), high-priority deny (100)
    let ps = PolicySet::new("priority-test", "orders")
        .with_rule(allow_rule("low-pri-allow", "x", Operator::Eq, json!(1), 10))
        .with_rule(deny_rule("high-pri-deny", "x", Operator::Eq, json!(1), 100));

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1}));

    // Both rules match, but the deny overrides
    assert!(result.should_deny);
    assert_eq!(result.results[0].matched_rules.len(), 2);
    // First matched should be the high-priority one
    assert_eq!(result.results[0].matched_rules[0].name, "high-pri-deny");
    assert_eq!(result.results[0].matched_rules[1].name, "low-pri-allow");
}

#[test]
fn stop_on_match_prevents_lower_priority_rules() {
    let mut engine = PolicyEngine::new();

    let stop_rule = PolicyRule::new("stopper", "Stops evaluation here")
        .with_priority(100)
        .with_stop_on_match()
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
        ))
        .with_action(PolicyAction::allow());

    let would_deny = deny_rule("would-deny", "x", Operator::Eq, json!(1), 50);

    let ps = PolicySet::new("stop-test", "orders").with_rule(stop_rule).with_rule(would_deny);

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1}));

    // Only the stopper should have matched; the deny never fires
    assert_eq!(result.results[0].matched_rules.len(), 1);
    assert_eq!(result.results[0].matched_rules[0].name, "stopper");
    assert!(result.should_allow, "Allow from stopper, deny never evaluated");
    assert!(!result.should_deny);
}

#[test]
fn stop_on_match_does_not_stop_if_not_matched() {
    let mut engine = PolicyEngine::new();

    let stop_rule = PolicyRule::new("stopper", "Would stop but does not match")
        .with_priority(100)
        .with_stop_on_match()
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(999)))],
        ))
        .with_action(PolicyAction::allow());

    let will_deny = deny_rule("will-deny", "x", Operator::Eq, json!(1), 50);

    let ps = PolicySet::new("stop-test-2", "orders").with_rule(stop_rule).with_rule(will_deny);

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1}));

    assert!(result.should_deny, "Deny rule should fire since stopper didn't match");
    assert_eq!(result.results[0].matched_rules.len(), 1);
    assert_eq!(result.results[0].matched_rules[0].name, "will-deny");
}

#[test]
fn rules_with_same_priority_both_evaluated() {
    let mut engine = PolicyEngine::new();

    let ps = PolicySet::new("same-pri", "orders")
        .with_rule(allow_rule("rule-a", "x", Operator::Eq, json!(1), 50))
        .with_rule(allow_rule("rule-b", "y", Operator::Eq, json!(2), 50));

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1, "y": 2}));
    assert!(result.should_allow);
    assert_eq!(result.results[0].matched_rules.len(), 2);
}

// ===========================================================================
// 8. Transform Actions and TransformAuditEntry
// ===========================================================================

#[test]
fn transform_action_appears_in_evaluation_result() {
    let mut engine = PolicyEngine::new();

    let transform_rule = PolicyRule::new("discount", "Apply discount")
        .with_priority(50)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(100)))],
        ))
        .with_action(PolicyAction::transform(json!({
            "order.discount": 0.10,
            "order.discount_reason": "Loyalty discount"
        })));

    engine.register_policy_set(PolicySet::new("discounts", "orders").with_rule(transform_rule));

    let result = engine.evaluate("orders", &json!({"order": {"total": 200}}));

    // Transform actions don't trigger deny or allow — they're a different ActionType
    assert!(!result.should_deny);
    // No Allow/Deny action => should_allow is false when no policy sets match with allow
    // Actually, let's check what happens: there's a transform, no allow/deny
    // has_deny=false, has_allow=false, policy_sets is NOT empty => should_allow = false
    assert!(!result.should_allow);

    assert_eq!(result.actions.len(), 1);
    assert_eq!(result.actions[0].action_type, ActionType::Transform);
    assert!(result.actions[0].transform.is_some());
}

#[test]
fn transform_alongside_allow() {
    let mut engine = PolicyEngine::new();

    let ps = PolicySet::new("transform-and-allow", "orders")
        .with_rule(
            PolicyRule::new("always-allow", "Allow everything")
                .with_priority(100)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::IsNotNull,
                        json!(null),
                    ))],
                ))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("apply-tax", "Apply tax transform")
                .with_priority(50)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(0),
                    ))],
                ))
                .with_action(PolicyAction::transform(json!({"tax_rate": 0.08}))),
        );

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"order": {"total": 100}}));
    assert!(result.should_allow, "Allow + Transform should still allow");
    assert!(!result.should_deny);
    assert_eq!(result.actions.len(), 2);
}

#[test]
fn transform_audit_entry_captures_before_after() {
    let entry = TransformAuditEntry::new("order.total", json!(100), json!(90));

    assert_eq!(entry.field, "order.total");
    assert_eq!(entry.before, json!(100));
    assert_eq!(entry.after, json!(90));
    assert!(entry.rule_id.is_none());
    assert!(entry.rule_name.is_none());
    assert!(entry.policy_set_id.is_none());
}

#[test]
fn transform_audit_entry_with_rule_context() {
    let rule_id = Uuid::new_v4();
    let ps_id = Uuid::new_v4();

    let entry = TransformAuditEntry::new("price", json!(50.0), json!(45.0))
        .with_rule(rule_id, "discount-rule")
        .with_policy_set(ps_id);

    assert_eq!(entry.field, "price");
    assert_eq!(entry.before, json!(50.0));
    assert_eq!(entry.after, json!(45.0));
    assert_eq!(entry.rule_id, Some(rule_id));
    assert_eq!(entry.rule_name.as_deref(), Some("discount-rule"));
    assert_eq!(entry.policy_set_id, Some(ps_id));
    // Timestamp should be set
    assert!(entry.timestamp.timestamp() > 0);
}

#[test]
fn transform_audit_entry_serializes() {
    let entry =
        TransformAuditEntry::new("qty", json!(10), json!(8)).with_rule(Uuid::nil(), "reduce-qty");

    let json_str = serde_json::to_string(&entry).unwrap();
    assert!(json_str.contains("\"field\":\"qty\""));
    assert!(json_str.contains("\"before\":10"));
    assert!(json_str.contains("\"after\":8"));
    assert!(json_str.contains("\"ruleName\":\"reduce-qty\""));
}

// ===========================================================================
// Additional edge-case integration tests
// ===========================================================================

#[test]
fn disabled_rule_is_skipped() {
    let mut engine = PolicyEngine::new();

    let disabled = PolicyRule::new("disabled-deny", "Would deny but disabled")
        .disabled()
        .with_priority(200)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("x", Operator::IsNotNull, json!(null)))],
        ))
        .with_action(PolicyAction::deny("disabled", "n/a"));

    let enabled = allow_rule("enabled-allow", "x", Operator::IsNotNull, json!(null), 100);

    let ps = PolicySet::new("mixed-enabled", "orders").with_rule(disabled).with_rule(enabled);

    engine.register_policy_set(ps);

    let result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(result.should_allow, "Disabled deny rule should be skipped");
    assert!(!result.should_deny);
    assert_eq!(result.results[0].matched_rules.len(), 1);
    assert_eq!(result.results[0].matched_rules[0].name, "enabled-allow");
}

#[test]
fn negated_condition_inverts_result() {
    let cond = Condition::new_negated("status", Operator::Eq, json!("active"));
    // status == "active" => true, negated => false
    assert!(!cond.evaluate(&json!({"status": "active"})));
    // status == "inactive" => false, negated => true
    assert!(cond.evaluate(&json!({"status": "inactive"})));
}

#[test]
fn evaluation_across_different_domains_is_isolated() {
    let mut engine = PolicyEngine::new();

    engine.register_policy_set(PolicySet::new("order-deny", "orders").with_rule(deny_rule(
        "deny-all-orders",
        "x",
        Operator::IsNotNull,
        json!(null),
        100,
    )));
    engine.register_policy_set(PolicySet::new("return-allow", "returns").with_rule(allow_rule(
        "allow-all-returns",
        "x",
        Operator::IsNotNull,
        json!(null),
        100,
    )));

    let order_result = engine.evaluate("orders", &json!({"x": 1}));
    assert!(order_result.should_deny, "Orders domain should deny");

    let return_result = engine.evaluate("returns", &json!({"x": 1}));
    assert!(return_result.should_allow, "Returns domain should allow");
}

#[test]
fn history_records_evaluations_correctly() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("test", "orders").with_rule(deny_rule(
        "deny-high",
        "total",
        Operator::Gt,
        json!(1000),
        10,
    )));

    engine.evaluate("orders", &json!({"total": 5000})); // deny
    engine.evaluate("orders", &json!({"total": 100})); // allow (no match => default)

    let history = engine.get_history();
    assert_eq!(history.len(), 2);

    let first = &history[0];
    assert!(first.should_deny);
    assert!(!first.should_allow);

    let second = &history[1];
    assert!(!second.should_deny);
    assert!(second.should_allow);
}

#[test]
fn policy_set_evaluation_serializable_as_json() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("test", "orders").with_rule(deny_rule(
        "r1",
        "x",
        Operator::Eq,
        json!(1),
        10,
    )));

    let result = engine.evaluate("orders", &json!({"x": 1}));
    let json_str = serde_json::to_string_pretty(&result).unwrap();

    // Verify camelCase serialization
    assert!(json_str.contains("\"shouldDeny\""));
    assert!(json_str.contains("\"shouldAllow\""));
    assert!(json_str.contains("\"dryRun\""));
    assert!(json_str.contains("\"matchedRules\""));
    assert!(json_str.contains("\"actionType\""));
}

#[test]
fn template_returns_policy_is_functional() {
    let mut engine = PolicyEngine::new();
    let returns_policy = stateset_policy::templates::auto_approve_returns_template();
    engine.register_policy_set(returns_policy);

    // VIP customer with small return => should trigger auto-approve agent action
    let vip_small = json!({
        "return": {"value": 50},
        "customer": {"lifetimeValue": 1000, "returnRate": 0.05}
    });
    let result = engine.evaluate("returns", &vip_small);
    assert!(result.results[0].matched);
    assert_eq!(result.results[0].matched_rules.len(), 1);

    // High return value => should flag for workflow
    let high_value = json!({
        "return": {"value": 600},
        "customer": {"lifetimeValue": 1000, "returnRate": 0.05}
    });
    let result2 = engine.evaluate("returns", &high_value);
    assert!(result2.results[0].matched);
}
