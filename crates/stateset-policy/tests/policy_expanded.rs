//! Expanded policy tests covering operators, nested conditions,
//! priority ordering, template expansion, history ring buffer,
//! and unknown domain handling.

use serde_json::json;
use stateset_policy::*;

// ---------------------------------------------------------------------------
// 1. All 20 operators tested
// ---------------------------------------------------------------------------

#[test]
fn operator_eq_numbers() {
    assert!(Operator::Eq.evaluate(&json!(42), &json!(42)));
    assert!(!Operator::Eq.evaluate(&json!(42), &json!(43)));
}

#[test]
fn operator_eq_strings() {
    assert!(Operator::Eq.evaluate(&json!("hello"), &json!("hello")));
    assert!(!Operator::Eq.evaluate(&json!("hello"), &json!("world")));
}

#[test]
fn operator_eq_null() {
    assert!(Operator::Eq.evaluate(&json!(null), &json!(null)));
}

#[test]
fn operator_eq_bool() {
    assert!(Operator::Eq.evaluate(&json!(true), &json!(true)));
    assert!(!Operator::Eq.evaluate(&json!(true), &json!(false)));
}

#[test]
fn operator_eq_cross_type_number_string() {
    assert!(Operator::Eq.evaluate(&json!(100), &json!("100")));
}

#[test]
fn operator_neq() {
    assert!(Operator::Neq.evaluate(&json!(1), &json!(2)));
    assert!(!Operator::Neq.evaluate(&json!(1), &json!(1)));
}

#[test]
fn operator_gt() {
    assert!(Operator::Gt.evaluate(&json!(10), &json!(5)));
    assert!(!Operator::Gt.evaluate(&json!(5), &json!(10)));
    assert!(!Operator::Gt.evaluate(&json!(5), &json!(5)));
}

#[test]
fn operator_gte() {
    assert!(Operator::Gte.evaluate(&json!(10), &json!(5)));
    assert!(Operator::Gte.evaluate(&json!(5), &json!(5)));
    assert!(!Operator::Gte.evaluate(&json!(4), &json!(5)));
}

#[test]
fn operator_lt() {
    assert!(Operator::Lt.evaluate(&json!(3), &json!(5)));
    assert!(!Operator::Lt.evaluate(&json!(5), &json!(3)));
    assert!(!Operator::Lt.evaluate(&json!(5), &json!(5)));
}

#[test]
fn operator_lte() {
    assert!(Operator::Lte.evaluate(&json!(3), &json!(5)));
    assert!(Operator::Lte.evaluate(&json!(5), &json!(5)));
    assert!(!Operator::Lte.evaluate(&json!(6), &json!(5)));
}

#[test]
fn operator_contains() {
    assert!(Operator::Contains.evaluate(&json!("hello world"), &json!("world")));
    assert!(!Operator::Contains.evaluate(&json!("hello"), &json!("xyz")));
}

#[test]
fn operator_starts_with() {
    assert!(Operator::StartsWith.evaluate(&json!("hello world"), &json!("hello")));
    assert!(!Operator::StartsWith.evaluate(&json!("hello world"), &json!("world")));
}

#[test]
fn operator_ends_with() {
    assert!(Operator::EndsWith.evaluate(&json!("hello world"), &json!("world")));
    assert!(!Operator::EndsWith.evaluate(&json!("hello world"), &json!("hello")));
}

#[test]
fn operator_matches_regex() {
    assert!(Operator::Matches.evaluate(&json!("order-12345"), &json!("order-\\d+")));
    assert!(!Operator::Matches.evaluate(&json!("cart-abc"), &json!("^order-\\d+$")));
}

#[test]
fn operator_matches_rejects_long_pattern() {
    let long = "a".repeat(201);
    assert!(!Operator::Matches.evaluate(&json!("aaa"), &json!(long)));
}

#[test]
fn operator_in_array() {
    assert!(Operator::In.evaluate(&json!("gold"), &json!(["gold", "platinum"])));
    assert!(!Operator::In.evaluate(&json!("silver"), &json!(["gold", "platinum"])));
}

#[test]
fn operator_not_in_array() {
    assert!(Operator::NotIn.evaluate(&json!("silver"), &json!(["gold", "platinum"])));
    assert!(!Operator::NotIn.evaluate(&json!("gold"), &json!(["gold", "platinum"])));
}

#[test]
fn operator_is_empty() {
    assert!(Operator::IsEmpty.evaluate(&json!(null), &json!(null)));
    assert!(Operator::IsEmpty.evaluate(&json!([]), &json!(null)));
    assert!(Operator::IsEmpty.evaluate(&json!({}), &json!(null)));
    assert!(Operator::IsEmpty.evaluate(&json!(""), &json!(null)));
    assert!(Operator::IsEmpty.evaluate(&json!(false), &json!(null)));
    assert!(!Operator::IsEmpty.evaluate(&json!([1]), &json!(null)));
    assert!(!Operator::IsEmpty.evaluate(&json!(42), &json!(null)));
}

#[test]
fn operator_is_not_empty() {
    assert!(!Operator::IsNotEmpty.evaluate(&json!(null), &json!(null)));
    assert!(Operator::IsNotEmpty.evaluate(&json!([1, 2]), &json!(null)));
    assert!(Operator::IsNotEmpty.evaluate(&json!(true), &json!(null)));
}

#[test]
fn operator_is_null() {
    assert!(Operator::IsNull.evaluate(&json!(null), &json!(null)));
    assert!(!Operator::IsNull.evaluate(&json!(0), &json!(null)));
}

#[test]
fn operator_is_not_null() {
    assert!(!Operator::IsNotNull.evaluate(&json!(null), &json!(null)));
    assert!(Operator::IsNotNull.evaluate(&json!(0), &json!(null)));
}

#[test]
fn operator_is_true() {
    assert!(Operator::IsTrue.evaluate(&json!(true), &json!(null)));
    assert!(!Operator::IsTrue.evaluate(&json!(false), &json!(null)));
    assert!(!Operator::IsTrue.evaluate(&json!(1), &json!(null)));
}

#[test]
fn operator_is_false() {
    assert!(Operator::IsFalse.evaluate(&json!(false), &json!(null)));
    assert!(!Operator::IsFalse.evaluate(&json!(true), &json!(null)));
}

#[test]
fn operator_between() {
    assert!(Operator::Between.evaluate(&json!(5), &json!([1, 10])));
    assert!(Operator::Between.evaluate(&json!(1), &json!([1, 10])));
    assert!(Operator::Between.evaluate(&json!(10), &json!([1, 10])));
    assert!(!Operator::Between.evaluate(&json!(0), &json!([1, 10])));
    assert!(!Operator::Between.evaluate(&json!(11), &json!([1, 10])));
}

#[test]
fn operator_divisible_by() {
    assert!(Operator::DivisibleBy.evaluate(&json!(10), &json!(5)));
    assert!(!Operator::DivisibleBy.evaluate(&json!(10), &json!(3)));
    assert!(!Operator::DivisibleBy.evaluate(&json!(10), &json!(0)));
}

#[test]
fn operator_unary_detection() {
    assert!(Operator::IsNull.is_unary());
    assert!(Operator::IsNotNull.is_unary());
    assert!(Operator::IsTrue.is_unary());
    assert!(Operator::IsFalse.is_unary());
    assert!(Operator::IsEmpty.is_unary());
    assert!(Operator::IsNotEmpty.is_unary());
    assert!(!Operator::Eq.is_unary());
    assert!(!Operator::Gt.is_unary());
    assert!(!Operator::Between.is_unary());
}

// ---------------------------------------------------------------------------
// 2. Nested condition groups (AND within OR, OR within AND)
// ---------------------------------------------------------------------------

#[test]
fn nested_and_within_or() {
    // OR(AND(a=1, b=2), AND(c=3, d=4))
    let inner1 = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
            ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(2))),
        ],
    );
    let inner2 = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("c", Operator::Eq, json!(3))),
            ConditionNode::Leaf(Condition::new("d", Operator::Eq, json!(4))),
        ],
    );
    let outer = ConditionGroup::new(
        Logic::Or,
        vec![ConditionNode::Group(inner1), ConditionNode::Group(inner2)],
    );

    // First inner group matches
    assert!(outer.evaluate(&json!({"a": 1, "b": 2, "c": 0, "d": 0})));
    // Second inner group matches
    assert!(outer.evaluate(&json!({"a": 0, "b": 0, "c": 3, "d": 4})));
    // Neither matches
    assert!(!outer.evaluate(&json!({"a": 1, "b": 0, "c": 3, "d": 0})));
}

#[test]
fn nested_or_within_and() {
    // AND(a=1, OR(b=2, c=3))
    let inner_or = ConditionGroup::new(
        Logic::Or,
        vec![
            ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(2))),
            ConditionNode::Leaf(Condition::new("c", Operator::Eq, json!(3))),
        ],
    );
    let outer = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
            ConditionNode::Group(inner_or),
        ],
    );

    assert!(outer.evaluate(&json!({"a": 1, "b": 2, "c": 0})));
    assert!(outer.evaluate(&json!({"a": 1, "b": 0, "c": 3})));
    assert!(!outer.evaluate(&json!({"a": 0, "b": 2, "c": 3})));
    assert!(!outer.evaluate(&json!({"a": 1, "b": 0, "c": 0})));
}

#[test]
fn deeply_nested_three_levels() {
    // AND(a=1, OR(b=2, AND(c=3, d=4)))
    let deepest = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("c", Operator::Eq, json!(3))),
            ConditionNode::Leaf(Condition::new("d", Operator::Eq, json!(4))),
        ],
    );
    let middle = ConditionGroup::new(
        Logic::Or,
        vec![
            ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(2))),
            ConditionNode::Group(deepest),
        ],
    );
    let outer = ConditionGroup::new(
        Logic::And,
        vec![
            ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
            ConditionNode::Group(middle),
        ],
    );

    assert!(outer.evaluate(&json!({"a": 1, "b": 2, "c": 0, "d": 0})));
    assert!(outer.evaluate(&json!({"a": 1, "b": 0, "c": 3, "d": 4})));
    assert!(!outer.evaluate(&json!({"a": 1, "b": 0, "c": 3, "d": 0})));
    assert!(!outer.evaluate(&json!({"a": 0, "b": 2, "c": 3, "d": 4})));
}

#[test]
fn empty_and_group_is_true() {
    let group = ConditionGroup::new(Logic::And, vec![]);
    assert!(group.evaluate(&json!({})));
}

#[test]
fn empty_or_group_is_false() {
    let group = ConditionGroup::new(Logic::Or, vec![]);
    assert!(!group.evaluate(&json!({})));
}

// ---------------------------------------------------------------------------
// 3. Priority ordering
// ---------------------------------------------------------------------------

#[test]
fn policy_set_rules_sorted_by_priority_descending() {
    let ps = PolicySet::new("test", "orders")
        .with_rule(
            PolicyRule::new("low", "Low priority")
                .with_priority(10)
                .with_conditions(ConditionGroup::new(Logic::And, vec![]))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("high", "High priority")
                .with_priority(100)
                .with_conditions(ConditionGroup::new(Logic::And, vec![]))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("mid", "Mid priority")
                .with_priority(50)
                .with_conditions(ConditionGroup::new(Logic::And, vec![]))
                .with_action(PolicyAction::allow()),
        );

    assert_eq!(ps.rules[0].name, "high");
    assert_eq!(ps.rules[1].name, "mid");
    assert_eq!(ps.rules[2].name, "low");
}

#[test]
fn stop_on_match_prevents_lower_priority_evaluation() {
    let ps = PolicySet::new("stop-test", "orders")
        .with_rule(
            PolicyRule::new("stopper", "Stops evaluation")
                .with_priority(100)
                .with_stop_on_match()
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
                ))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("blocked", "Never reached")
                .with_priority(10)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
                ))
                .with_action(PolicyAction::deny_simple("should not fire")),
        );

    let eval = ps.evaluate(&json!({"x": 1}));
    assert_eq!(eval.matched_rules.len(), 1);
    assert_eq!(eval.matched_rules[0].name, "stopper");
    assert!(eval.should_allow);
    assert!(!eval.should_deny);
}

#[test]
fn deny_overrides_allow_in_same_set() {
    let ps = PolicySet::new("mixed", "orders")
        .with_rule(
            PolicyRule::new("allow-rule", "Allow all")
                .with_priority(50)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "x",
                        Operator::IsNotNull,
                        json!(null),
                    ))],
                ))
                .with_action(PolicyAction::allow()),
        )
        .with_rule(
            PolicyRule::new("deny-rule", "Deny expensive")
                .with_priority(100)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("total", Operator::Gt, json!(1000)))],
                ))
                .with_action(PolicyAction::deny("too expensive", "reduce order")),
        );

    let eval = ps.evaluate(&json!({"x": 1, "total": 2000}));
    assert!(eval.should_deny);
    assert!(!eval.should_allow);
}

// ---------------------------------------------------------------------------
// 4. Template expansion
// ---------------------------------------------------------------------------

#[test]
fn template_returns_auto_approve() {
    let ps = templates::auto_approve_returns_template();
    assert_eq!(ps.domain, "returns");
    assert_eq!(ps.rules.len(), 2);
    // VIP small return triggers auto-approve
    let ctx = json!({
        "return": {"id": "R-1", "value": 50},
        "customer": {"lifetimeValue": 1000, "returnRate": 0.05}
    });
    let eval = ps.evaluate(&ctx);
    assert!(eval.matched);
    assert_eq!(eval.matched_rules[0].name, "auto_approve_small_vip_returns");
}

#[test]
fn template_inventory_restock_critical() {
    let ps = templates::inventory_restock_template();
    assert_eq!(ps.domain, "inventory");
    let ctx = json!({
        "inventory": {"sku": "W-001", "quantity": 2, "reorderPoint": 10, "targetQuantity": 50}
    });
    let eval = ps.evaluate(&ctx);
    assert!(eval.matched);
    assert_eq!(eval.matched_rules[0].name, "critical_stock_alert");
}

#[test]
fn template_fraud_detection_high_value_new_customer() {
    let ps = templates::order_fraud_detection_template();
    assert_eq!(ps.domain, "orders");
    let ctx = json!({
        "order": {"total": 2000, "shippingAddress": {"country": "US"}, "billingAddress": {"country": "US"}},
        "customer": {"id": "C-1", "orderCount": 1, "ordersLast24h": 1}
    });
    let eval = ps.evaluate(&ctx);
    assert!(eval.matched);
    assert!(eval.matched_rules.iter().any(|r| r.name == "high_value_new_customer"));
}

#[test]
fn template_promotion_eligibility_vip_allowed() {
    let ps = templates::promotion_eligibility_template();
    assert_eq!(ps.domain, "promotions");
    let ctx = json!({
        "promotion": {"vipOnly": true, "type": "fixed"},
        "customer": {"tier": "gold"},
        "cart": {"hasPercentageDiscount": false}
    });
    let eval = ps.evaluate(&ctx);
    assert!(eval.should_allow);
}

#[test]
fn template_subscription_auto_cancel() {
    let ps = templates::subscription_rules_template();
    assert_eq!(ps.domain, "subscriptions");
    let ctx = json!({
        "subscription": {"id": "S-1", "consecutiveFailedPayments": 3, "monthsActive": 2},
        "event": "payment_failed",
        "customer": {"id": "C-1"}
    });
    let eval = ps.evaluate(&ctx);
    assert!(eval.matched);
    assert!(eval.matched_rules.iter().any(|r| r.name == "auto_cancel_failed_payments"));
}

// ---------------------------------------------------------------------------
// 5. History ring buffer
// ---------------------------------------------------------------------------

#[test]
fn engine_history_records_evaluations() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("orders-ps", "orders"));

    engine.evaluate("orders", &json!({}));
    engine.evaluate("orders", &json!({}));
    engine.evaluate("orders", &json!({}));

    assert_eq!(engine.get_history().len(), 3);
}

#[test]
fn engine_history_capped_at_1000() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(
        PolicySet::new("test", "orders").with_rule(
            PolicyRule::new("always", "Always matches")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "x",
                        Operator::IsNotNull,
                        json!(null),
                    ))],
                ))
                .with_action(PolicyAction::allow()),
        ),
    );

    for i in 0..1050 {
        engine.evaluate("orders", &json!({"x": i}));
    }
    assert_eq!(engine.get_history().len(), 1000);
}

#[test]
fn engine_recent_history() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("ps", "orders"));

    for _ in 0..5 {
        engine.evaluate("orders", &json!({}));
    }

    let recent = engine.get_recent_history(3);
    assert_eq!(recent.len(), 3);
}

#[test]
fn engine_history_by_domain() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("o", "orders"));
    engine.register_policy_set(PolicySet::new("r", "returns"));

    engine.evaluate("orders", &json!({}));
    engine.evaluate("returns", &json!({}));
    engine.evaluate("orders", &json!({}));

    assert_eq!(engine.get_history_for_domain("orders").len(), 2);
    assert_eq!(engine.get_history_for_domain("returns").len(), 1);
}

#[test]
fn engine_clear_history() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("ps", "orders"));
    engine.evaluate("orders", &json!({}));
    assert!(!engine.get_history().is_empty());
    engine.clear_history();
    assert!(engine.get_history().is_empty());
}

#[test]
fn engine_dry_run_does_not_record_history() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("ps", "orders"));
    let result = engine.evaluate_dry_run("orders", &json!({}));
    assert!(result.dry_run);
    assert!(engine.get_history().is_empty());
}

// ---------------------------------------------------------------------------
// 6. Unknown domain handling (deny vs allow modes)
// ---------------------------------------------------------------------------

#[test]
fn unknown_domain_deny_by_default() {
    let mut engine = PolicyEngine::new();
    // No policy sets registered for "orders"
    let result = engine.evaluate("orders", &json!({}));
    assert!(result.should_deny);
    assert!(!result.should_allow);
}

#[test]
fn unknown_domain_allow_mode() {
    let mut engine = PolicyEngine::new().with_unknown_domain_mode(UnknownDomainMode::Allow);
    let result = engine.evaluate("orders", &json!({}));
    assert!(result.should_allow);
    assert!(!result.should_deny);
}

#[test]
fn set_unknown_domain_mode_at_runtime() {
    let mut engine = PolicyEngine::new();
    assert_eq!(engine.unknown_domain_mode(), UnknownDomainMode::Deny);
    engine.set_unknown_domain_mode(UnknownDomainMode::Allow);
    assert_eq!(engine.unknown_domain_mode(), UnknownDomainMode::Allow);
    let result = engine.evaluate("whatever", &json!({}));
    assert!(result.should_allow);
}

// ---------------------------------------------------------------------------
// 7. Engine management
// ---------------------------------------------------------------------------

#[test]
fn engine_register_and_unregister() {
    let mut engine = PolicyEngine::new();
    let ps = PolicySet::new("limits", "orders");
    let id = ps.id;
    engine.register_policy_set(ps);
    assert_eq!(engine.policy_set_count(), 1);
    assert_eq!(engine.total_rule_count(), 0);

    let removed = engine.unregister_policy_set(&id);
    assert!(removed.is_some());
    assert_eq!(engine.policy_set_count(), 0);
}

#[test]
fn engine_status_reflects_state() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(
        PolicySet::new("o", "orders").with_rule(
            PolicyRule::new("r1", "Rule 1")
                .with_conditions(ConditionGroup::new(Logic::And, vec![]))
                .with_action(PolicyAction::allow()),
        ),
    );
    let status = engine.get_status();
    assert_eq!(status.total_policy_sets, 1);
    assert_eq!(status.total_rules, 1);
    assert_eq!(status.by_domain.get("orders"), Some(&1));
}

#[test]
fn engine_multiple_sets_same_domain() {
    let mut engine = PolicyEngine::new();
    engine.register_policy_set(PolicySet::new("set1", "orders"));
    engine.register_policy_set(PolicySet::new("set2", "orders"));
    assert_eq!(engine.get_policies_for_domain("orders").len(), 2);
}

// ---------------------------------------------------------------------------
// 8. Condition negation and dynamic references
// ---------------------------------------------------------------------------

#[test]
fn condition_negation() {
    let cond = Condition::new_negated("status", Operator::Eq, json!("active"));
    assert!(cond.evaluate(&json!({"status": "inactive"})));
    assert!(!cond.evaluate(&json!({"status": "active"})));
}

#[test]
fn condition_dynamic_ref() {
    let cond =
        Condition::new("inventory.quantity", Operator::Lte, json!("${inventory.reorderPoint}"));
    assert!(cond.evaluate(&json!({"inventory": {"quantity": 5, "reorderPoint": 10}})));
    assert!(!cond.evaluate(&json!({"inventory": {"quantity": 15, "reorderPoint": 10}})));
}

#[test]
fn context_nested_path_resolution() {
    let data = json!({"order": {"customer": {"email": "a@b.com"}}});
    assert_eq!(get_nested_value(&data, "order.customer.email"), Some(&json!("a@b.com")));
    assert_eq!(get_nested_value(&data, "order.missing"), None);
}

#[test]
fn context_dynamic_ref_resolution() {
    let ctx = json!({"order": {"total": 500}});
    let (resolved, is_ref) = resolve_dynamic_ref(&json!("${order.total}"), &ctx);
    assert!(is_ref);
    assert_eq!(resolved, json!(500));

    let (resolved, is_ref) = resolve_dynamic_ref(&json!(42), &ctx);
    assert!(!is_ref);
    assert_eq!(resolved, json!(42));
}

// ---------------------------------------------------------------------------
// 9. PolicyAction and explanation types
// ---------------------------------------------------------------------------

#[test]
fn action_type_display() {
    assert_eq!(ActionType::Allow.to_string(), "allow");
    assert_eq!(ActionType::Deny.to_string(), "deny");
    assert_eq!(ActionType::Agent.to_string(), "agent");
    assert_eq!(ActionType::Workflow.to_string(), "workflow");
    assert_eq!(ActionType::Notify.to_string(), "notify");
    assert_eq!(ActionType::Transform.to_string(), "transform");
}

#[test]
fn policy_action_builders() {
    let allow = PolicyAction::allow();
    assert_eq!(allow.action_type, ActionType::Allow);

    let deny = PolicyAction::deny("reason", "fix");
    assert_eq!(deny.action_type, ActionType::Deny);
    assert_eq!(deny.reason.as_deref(), Some("reason"));
    assert_eq!(deny.remediation.as_deref(), Some("fix"));

    let agent = PolicyAction::agent("returns", "approve");
    assert_eq!(agent.action_type, ActionType::Agent);
    assert_eq!(agent.agent.as_deref(), Some("returns"));

    let workflow = PolicyAction::workflow("fulfillment");
    assert_eq!(workflow.action_type, ActionType::Workflow);
    assert_eq!(workflow.workflow.as_deref(), Some("fulfillment"));
}

#[test]
fn disabled_rule_never_matches() {
    let rule = PolicyRule::new("test", "Test").disabled().with_conditions(ConditionGroup::new(
        Logic::And,
        vec![ConditionNode::Leaf(Condition::new("x", Operator::Gt, json!(0)))],
    ));
    assert!(!rule.matches(&json!({"x": 999})));
}

#[test]
fn rule_serde_roundtrip() {
    let rule = PolicyRule::new("test", "Test rule")
        .with_priority(50)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
        ))
        .with_action(PolicyAction::deny("reason", "fix"));

    let json_str = serde_json::to_string(&rule).unwrap();
    let deser: PolicyRule = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deser.name, "test");
    assert_eq!(deser.priority, 50);
}
