# stateset-policy

[![crates.io](https://img.shields.io/crates/v/stateset-policy.svg)](https://crates.io/crates/stateset-policy)
[![docs.rs](https://docs.rs/stateset-policy/badge.svg)](https://docs.rs/stateset-policy)

A declarative, condition-based rule engine for commerce decisions — refund limits,
fraud holds, inventory guards, promotion eligibility — that can explain exactly why
it denied something.

Agents make decisions faster than humans can review them. The useful property here
isn't the allow/deny bit, it's the per-condition breakdown that comes with it: when a
$15,000 refund is blocked, you get the rule, the condition, the expected value, and
the actual value, which is what turns an automated denial into something you can put
in front of a customer or an auditor.

**Status: opt-in library.** The engine does *not* evaluate policies implicitly on
commerce operations — you call `evaluate` at your own decision points.

## Features

- **20 operators** — comparison, string, collection, type, and numeric conditions
- **Deny-overrides precedence** — any deny action overrides all allow actions
- **Secure default** — unconfigured domains deny by default (configurable)
- **Explainable denials** — per-condition breakdown of why a request was denied
- **Transform audit trail** — records before/after values for policy transforms
- **YAML/JSON policy loading** from the filesystem (YAML behind the `yaml` feature)
- **Dry-run evaluation** — test policies without recording history
- **5 pre-built templates** — returns, inventory, fraud, promotions, subscriptions

## Usage

```rust
use stateset_policy::{
    PolicyEngine, PolicySet, PolicyRule, PolicyAction,
    ConditionGroup, ConditionNode, Condition, Operator, Logic,
};
use serde_json::json;

let mut engine = PolicyEngine::new();

// Deny orders over $10,000 pending manager approval
let rule = PolicyRule::new("high-value-review", "Require review for high-value orders")
    .with_priority(10)
    .with_conditions(ConditionGroup::new(Logic::And, vec![
        ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(10000))),
    ]))
    .with_action(PolicyAction::deny(
        "Order exceeds $10,000 limit",
        "Request manager approval",
    ));

engine.register_policy_set(PolicySet::new("order-limits", "orders").with_rule(rule));

let context = json!({
    "order": { "total": 15000, "customer": { "tier": "standard" } }
});

let result = engine.evaluate("orders", &context);
assert!(result.should_deny);
```

Pre-built templates cover the common cases without writing rules by hand:

```rust
use stateset_policy::{PolicyEngine, templates::auto_approve_returns_template};
use serde_json::json;

let mut engine = PolicyEngine::new();
engine.register_policy_set(auto_approve_returns_template());

// A small return from a high-value, low-return-rate customer is auto-approved
let result = engine.evaluate("returns", &json!({
    "return": { "id": "ret_1", "value": 45 },
    "customer": { "lifetimeValue": 2400, "returnRate": 0.02 }
}));
assert!(!result.should_deny);
```

The five templates are `auto_approve_returns_template`,
`inventory_restock_template`, `order_fraud_detection_template`,
`promotion_eligibility_template`, and `subscription_rules_template`.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `yaml` | Load policy sets from YAML files via `serde_yaml` | Yes |

## Part of StateSet iCommerce

Available through [`stateset-sdk`](https://crates.io/crates/stateset-sdk)'s `policy`
feature as `stateset_sdk::policy`. This crate is a faithful Rust port of the
JavaScript policy engine that ships with the StateSet CLI, so policies written in
YAML evaluate identically on both sides.

## License

MIT OR Apache-2.0
