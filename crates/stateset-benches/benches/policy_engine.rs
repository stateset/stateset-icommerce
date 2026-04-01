use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;
use stateset_benches::perf_gate::run_gate_if_enabled_with_iterations;
use stateset_policy::{
    Condition, ConditionGroup, ConditionNode, Logic, Operator, PolicyAction, PolicyEngine,
    PolicyRule, PolicySet,
};

/// Build a policy set with `n` rules of varying operators and priorities.
///
/// Each rule tests a different field path + operator to exercise real evaluation
/// paths (comparison, string, collection, type).
fn build_policy_set(name: &str, domain: &str, rule_count: usize) -> PolicySet {
    let mut ps = PolicySet::new(name, domain);
    for i in 0..rule_count {
        let (field, operator, value) = match i % 5 {
            0 => ("order.total", Operator::Gt, json!(i * 100)),
            1 => ("customer.tier", Operator::In, json!(["gold", "platinum", "diamond"])),
            2 => ("order.shipping.country", Operator::Eq, json!("US")),
            3 => ("order.item_count", Operator::Gte, json!(i)),
            _ => ("customer.email", Operator::Contains, json!("@example.com")),
        };
        let action = if i % 3 == 0 {
            PolicyAction::deny(format!("Rule {i} denied"), format!("Fix for rule {i}"))
        } else {
            PolicyAction::allow()
        };
        let rule = PolicyRule::new(format!("rule-{i}"), format!("Benchmark rule {i}"))
            .with_priority((rule_count - i) as i32)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(field, operator, value))],
            ))
            .with_action(action);
        ps = ps.with_rule(rule);
    }
    ps
}

/// Build a context that will match roughly half the rules.
fn build_context() -> serde_json::Value {
    json!({
        "order": {
            "total": 5000,
            "item_count": 7,
            "shipping": {
                "country": "US",
                "method": "express"
            }
        },
        "customer": {
            "tier": "gold",
            "email": "alice@example.com",
            "total_orders": 42
        }
    })
}

/// Benchmark: evaluate a single policy set with 10 rules.
fn bench_policy_10_rules(c: &mut Criterion) {
    let context = build_context();

    run_gate_if_enabled_with_iterations("policy_10_rules", 1_000, || {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(build_policy_set("bench-10", "orders", 10));
        let _ = engine.evaluate("orders", &context);
    });

    c.bench_function("policy_10_rules", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut engine = PolicyEngine::new();
                engine.register_policy_set(build_policy_set("bench-10", "orders", 10));
                engine
            },
            |mut engine| {
                let result = engine.evaluate("orders", black_box(&context));
                black_box(result.should_deny);
            },
        );
    });
}

/// Benchmark: evaluate 50 rules in a single policy set.
fn bench_policy_50_rules(c: &mut Criterion) {
    let context = build_context();

    run_gate_if_enabled_with_iterations("policy_50_rules", 500, || {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(build_policy_set("bench-50", "orders", 50));
        let _ = engine.evaluate("orders", &context);
    });

    c.bench_function("policy_50_rules", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut engine = PolicyEngine::new();
                engine.register_policy_set(build_policy_set("bench-50", "orders", 50));
                engine
            },
            |mut engine| {
                let result = engine.evaluate("orders", black_box(&context));
                black_box(result.should_deny);
            },
        );
    });
}

/// Benchmark: evaluate with 100 policy sets, each having 1 rule.
///
/// Exercises the engine's domain index lookup and cross-set aggregation.
fn bench_policy_100_sets(c: &mut Criterion) {
    let context = build_context();

    run_gate_if_enabled_with_iterations("policy_100_sets", 200, || {
        let mut engine = PolicyEngine::new();
        for i in 0..100 {
            engine.register_policy_set(build_policy_set(&format!("set-{i}"), "orders", 1));
        }
        let _ = engine.evaluate("orders", &context);
    });

    c.bench_function("policy_100_sets", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut engine = PolicyEngine::new();
                for i in 0..100 {
                    engine.register_policy_set(build_policy_set(&format!("set-{i}"), "orders", 1));
                }
                engine
            },
            |mut engine| {
                let result = engine.evaluate("orders", black_box(&context));
                black_box(result.should_deny);
            },
        );
    });
}

/// Benchmark: dry-run evaluation (no history recording).
fn bench_policy_dry_run(c: &mut Criterion) {
    let context = build_context();

    run_gate_if_enabled_with_iterations("policy_dry_run_10", 2_000, || {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(build_policy_set("bench-dry", "orders", 10));
        let _ = engine.evaluate_dry_run("orders", &context);
    });

    c.bench_function("policy_dry_run_10", |bencher| {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(build_policy_set("bench-dry", "orders", 10));
        bencher.iter(|| {
            let result = engine.evaluate_dry_run("orders", black_box(&context));
            black_box(result.should_deny)
        });
    });
}

/// Benchmark: policy evaluation with complex AND+OR condition trees.
fn bench_policy_complex_conditions(c: &mut Criterion) {
    let context = build_context();

    let complex_rule = PolicyRule::new("complex", "Complex condition tree")
        .with_priority(100)
        .with_conditions(ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(1000))),
                ConditionNode::Group(ConditionGroup::new(
                    Logic::Or,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "customer.tier",
                            Operator::Eq,
                            json!("gold"),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "customer.tier",
                            Operator::Eq,
                            json!("platinum"),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "customer.tier",
                            Operator::Eq,
                            json!("diamond"),
                        )),
                    ],
                )),
                ConditionNode::Group(ConditionGroup::new(
                    Logic::And,
                    vec![
                        ConditionNode::Leaf(Condition::new(
                            "order.shipping.country",
                            Operator::In,
                            json!(["US", "CA", "GB", "AU"]),
                        )),
                        ConditionNode::Leaf(Condition::new(
                            "order.item_count",
                            Operator::Gte,
                            json!(3),
                        )),
                    ],
                )),
            ],
        ))
        .with_action(PolicyAction::deny("Complex deny", "Complex fix"));

    run_gate_if_enabled_with_iterations("policy_complex_conditions", 2_000, || {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(
            PolicySet::new("complex-set", "orders").with_rule(complex_rule.clone()),
        );
        let _ = engine.evaluate("orders", &context);
    });

    c.bench_function("policy_complex_conditions", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut engine = PolicyEngine::new();
                engine.register_policy_set(
                    PolicySet::new("complex-set", "orders").with_rule(complex_rule.clone()),
                );
                engine
            },
            |mut engine| {
                let result = engine.evaluate("orders", black_box(&context));
                black_box(result.should_deny);
            },
        );
    });
}

/// Benchmark: policy registration (inserting 100 sets into the engine).
fn bench_policy_registration(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("policy_register_100", 100, || {
        let mut engine = PolicyEngine::new();
        for i in 0..100 {
            engine.register_policy_set(build_policy_set(&format!("reg-{i}"), "orders", 5));
        }
    });

    c.bench_function("policy_register_100", |bencher| {
        bencher.iter(|| {
            let mut engine = PolicyEngine::new();
            for i in 0..100 {
                engine.register_policy_set(build_policy_set(&format!("reg-{i}"), "orders", 5));
            }
            black_box(engine.policy_set_count())
        });
    });
}

criterion_group!(
    benches,
    bench_policy_10_rules,
    bench_policy_50_rules,
    bench_policy_100_sets,
    bench_policy_dry_run,
    bench_policy_complex_conditions,
    bench_policy_registration,
);
criterion_main!(benches);
