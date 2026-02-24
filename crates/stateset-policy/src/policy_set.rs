use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::action::{ActionType, PolicyAction};
use crate::explanation::{MatchedRule, PolicyExplanation};
use crate::rule::PolicyRule;

/// A collection of rules scoped to a single domain (e.g., "orders", "returns").
///
/// Rules within a set are evaluated in priority order (highest first).
/// The evaluation collects all matching rules and their actions, then applies
/// deny-overrides precedence at the engine level.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::{
///     PolicySet, PolicyRule, PolicyAction, ConditionGroup, ConditionNode,
///     Condition, Operator, Logic,
/// };
///
/// let rule = PolicyRule::new("high-value", "Flag high-value orders")
///     .with_priority(100)
///     .with_conditions(ConditionGroup::new(Logic::And, vec![
///         ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(1000))),
///     ]))
///     .with_action(PolicyAction::deny("Exceeds limit", "Get approval"));
///
/// let policy_set = PolicySet::new("order-limits", "orders").with_rule(rule);
/// let eval = policy_set.evaluate(&json!({"order": {"total": 2000}}));
/// assert!(eval.matched);
/// assert_eq!(eval.matched_rules.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySet {
    /// Unique identifier for this policy set.
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// The domain this policy set applies to (e.g., "orders", "returns", "inventory").
    pub domain: String,
    /// Description of the policy set.
    #[serde(default)]
    pub description: String,
    /// Semantic version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// The rules in this set, sorted by priority descending.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// The default action when no rules match.
    #[serde(default = "PolicyAction::allow")]
    pub default_action: PolicyAction,
    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: Value,
}

fn default_version() -> String {
    "1.0.0".to_owned()
}

impl PolicySet {
    /// Create a new policy set for the given domain.
    pub fn new(name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            domain: domain.into(),
            description: String::new(),
            version: default_version(),
            rules: Vec::new(),
            default_action: PolicyAction::allow(),
            metadata: Value::Null,
        }
    }

    /// Builder: add a rule (maintains priority sort order).
    pub fn with_rule(mut self, rule: PolicyRule) -> Self {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        self
    }

    /// Builder: set the default action.
    pub fn with_default_action(mut self, action: PolicyAction) -> Self {
        self.default_action = action;
        self
    }

    /// Builder: set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Builder: set a specific ID (useful for testing).
    pub const fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Builder: set metadata.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Evaluate all rules against the given context.
    ///
    /// Returns a [`PolicySetEvaluation`] containing all matched rules,
    /// their actions, and per-condition explanations. Rules are evaluated
    /// in priority order; if a matched rule has `stop_on_match`, evaluation stops.
    pub fn evaluate(&self, context: &Value) -> PolicySetEvaluation {
        let mut matched_rules: Vec<MatchedRule> = Vec::new();
        let mut actions: Vec<PolicyAction> = Vec::new();
        let mut explanations: Vec<PolicyExplanation> = Vec::new();

        for rule in &self.rules {
            let (matched, condition_details) = rule.matches_with_detail(context);

            if matched {
                matched_rules.push(MatchedRule { id: rule.id, name: rule.name.clone() });

                // Build the reason from action.reason, action.metadata.reason, or rule description
                let reason = rule
                    .action
                    .reason
                    .clone()
                    .or_else(|| {
                        rule.action
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("reason"))
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| rule.description.clone());

                let remediation = rule.action.remediation.clone().or_else(|| {
                    rule.action
                        .metadata
                        .as_ref()
                        .and_then(|m| m.get("remediation"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });

                explanations.push(PolicyExplanation {
                    policy_set_id: self.id,
                    policy_set_name: self.name.clone(),
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    rule_description: rule.description.clone(),
                    action_type: rule.action.action_type,
                    reason,
                    remediation,
                    conditions: condition_details,
                });

                actions.push(rule.action.clone());

                if rule.stop_on_match {
                    break;
                }
            }
        }

        let matched = !matched_rules.is_empty();

        // Deny-overrides within this set
        let has_deny = actions.iter().any(|a| a.action_type == ActionType::Deny);
        let has_allow = actions.iter().any(|a| a.action_type == ActionType::Allow);

        PolicySetEvaluation {
            policy_set_id: self.id,
            policy_set_name: self.name.clone(),
            matched,
            matched_rules,
            actions,
            explanations,
            default_applied: !matched,
            should_allow: !has_deny && (has_allow || !matched),
            should_deny: has_deny,
        }
    }
}

/// Result of evaluating a single [`PolicySet`] against a context.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySetEvaluation {
    /// The evaluated policy set's ID.
    pub policy_set_id: Uuid,
    /// The evaluated policy set's name.
    pub policy_set_name: String,
    /// Whether any rules matched.
    pub matched: bool,
    /// The rules that matched, in evaluation order.
    pub matched_rules: Vec<MatchedRule>,
    /// The actions from matched rules.
    pub actions: Vec<PolicyAction>,
    /// Explanations for each matched rule.
    pub explanations: Vec<PolicyExplanation>,
    /// Whether the default action was applied (no rules matched).
    pub default_applied: bool,
    /// Whether the evaluation allows the operation.
    pub should_allow: bool,
    /// Whether the evaluation denies the operation.
    pub should_deny: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, ConditionGroup, ConditionNode, Logic, Operator};
    use serde_json::json;

    fn make_deny_rule(
        name: &str,
        field: &str,
        op: Operator,
        value: Value,
        priority: i32,
    ) -> PolicyRule {
        PolicyRule::new(name, format!("{name} description"))
            .with_priority(priority)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(field, op, value))],
            ))
            .with_action(PolicyAction::deny("denied", "fix it"))
    }

    fn make_allow_rule(
        name: &str,
        field: &str,
        op: Operator,
        value: Value,
        priority: i32,
    ) -> PolicyRule {
        PolicyRule::new(name, format!("{name} description"))
            .with_priority(priority)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(field, op, value))],
            ))
            .with_action(PolicyAction::allow())
    }

    #[test]
    fn no_rules_returns_default() {
        let ps = PolicySet::new("empty", "orders");
        let eval = ps.evaluate(&json!({"order": {"total": 100}}));
        assert!(!eval.matched);
        assert!(eval.default_applied);
        assert!(eval.should_allow);
        assert!(!eval.should_deny);
    }

    #[test]
    fn single_deny_rule_matches() {
        let ps = PolicySet::new("limits", "orders").with_rule(make_deny_rule(
            "high-value",
            "total",
            Operator::Gt,
            json!(1000),
            100,
        ));

        let eval = ps.evaluate(&json!({"total": 2000}));
        assert!(eval.matched);
        assert!(eval.should_deny);
        assert!(!eval.should_allow);
        assert_eq!(eval.matched_rules.len(), 1);
        assert_eq!(eval.explanations.len(), 1);
    }

    #[test]
    fn single_deny_rule_does_not_match() {
        let ps = PolicySet::new("limits", "orders").with_rule(make_deny_rule(
            "high-value",
            "total",
            Operator::Gt,
            json!(1000),
            100,
        ));

        let eval = ps.evaluate(&json!({"total": 500}));
        assert!(!eval.matched);
        assert!(eval.default_applied);
        assert!(eval.should_allow);
    }

    #[test]
    fn priority_order_is_descending() {
        let ps = PolicySet::new("multi", "orders")
            .with_rule(make_allow_rule("low-pri", "x", Operator::Eq, json!(1), 10))
            .with_rule(make_deny_rule("high-pri", "x", Operator::Eq, json!(1), 100));

        // Rules should be sorted: high-pri (100) first, then low-pri (10)
        assert_eq!(ps.rules[0].name, "high-pri");
        assert_eq!(ps.rules[1].name, "low-pri");
    }

    #[test]
    fn deny_overrides_allow() {
        let ps = PolicySet::new("mixed", "orders")
            .with_rule(make_allow_rule("allow-rule", "x", Operator::Eq, json!(1), 50))
            .with_rule(make_deny_rule("deny-rule", "x", Operator::Eq, json!(1), 100));

        let eval = ps.evaluate(&json!({"x": 1}));
        assert!(eval.matched);
        assert!(eval.should_deny);
        assert!(!eval.should_allow);
        assert_eq!(eval.matched_rules.len(), 2);
    }

    #[test]
    fn stop_on_match_stops_evaluation() {
        let stop_rule = PolicyRule::new("first", "First rule")
            .with_priority(100)
            .with_stop_on_match()
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
            ))
            .with_action(PolicyAction::agent("returns", "auto-approve"));

        let second_rule = make_deny_rule("second", "x", Operator::Eq, json!(1), 50);

        let ps = PolicySet::new("stop-test", "returns").with_rule(stop_rule).with_rule(second_rule);

        let eval = ps.evaluate(&json!({"x": 1}));
        // Only the first rule should match because stop_on_match is true
        assert_eq!(eval.matched_rules.len(), 1);
        assert_eq!(eval.matched_rules[0].name, "first");
    }

    #[test]
    fn explanations_contain_condition_details() {
        let ps = PolicySet::new("explain-test", "orders").with_rule(
            PolicyRule::new("check", "Check total")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("total", Operator::Gt, json!(100)))],
                ))
                .with_action(PolicyAction::deny("too high", "reduce")),
        );

        let eval = ps.evaluate(&json!({"total": 200}));
        assert_eq!(eval.explanations.len(), 1);
        let exp = &eval.explanations[0];
        assert_eq!(exp.action_type, ActionType::Deny);
        assert_eq!(exp.reason, "too high");
        assert_eq!(exp.remediation.as_deref(), Some("reduce"));
        assert!(!exp.conditions.is_empty());
        assert!(exp.conditions[0].matched);
    }

    #[test]
    fn serde_roundtrip() {
        let ps = PolicySet::new("test-set", "orders")
            .with_description("A test policy set")
            .with_rule(make_deny_rule("rule1", "x", Operator::Eq, json!(1), 10));

        let json_str = serde_json::to_string(&ps).unwrap();
        let deser: PolicySet = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.name, "test-set");
        assert_eq!(deser.domain, "orders");
        assert_eq!(deser.rules.len(), 1);
    }
}
