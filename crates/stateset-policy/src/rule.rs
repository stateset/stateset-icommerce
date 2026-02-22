use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::action::PolicyAction;
use crate::condition::{ConditionDetail, ConditionGroup};

/// A single policy rule: conditions + action + metadata.
///
/// Rules are evaluated in priority order (higher priority first).
/// When a rule matches, its action is collected; if `stop_on_match` is set,
/// no further rules in the same [`PolicySet`](crate::PolicySet) are evaluated.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::{
///     PolicyRule, PolicyAction, ConditionGroup, ConditionNode,
///     Condition, Operator, Logic,
/// };
///
/// let rule = PolicyRule::new("high-value", "Flag high-value orders")
///     .with_priority(100)
///     .with_conditions(ConditionGroup::new(Logic::And, vec![
///         ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(1000))),
///     ]))
///     .with_action(PolicyAction::deny("Order exceeds limit", "Request approval"));
///
/// assert!(rule.matches(&json!({"order": {"total": 2000}})));
/// assert!(!rule.matches(&json!({"order": {"total": 500}})));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    /// Unique identifier for this rule.
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    /// Human-readable rule name.
    pub name: String,
    /// Description of what this rule does.
    #[serde(default)]
    pub description: String,
    /// Whether this rule is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Priority (higher = evaluated first).
    #[serde(default)]
    pub priority: i32,
    /// The conditions that must be satisfied.
    pub conditions: ConditionGroup,
    /// The action to take when all conditions match.
    pub action: PolicyAction,
    /// If true, stop evaluating further rules after this one matches.
    #[serde(default)]
    pub stop_on_match: bool,
    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: Value,
}

const fn default_enabled() -> bool {
    true
}

impl PolicyRule {
    /// Create a new enabled rule with default (empty) conditions and an `Allow` action.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            enabled: true,
            priority: 0,
            conditions: ConditionGroup::new(crate::Logic::And, Vec::new()),
            action: PolicyAction::allow(),
            stop_on_match: false,
            metadata: Value::Null,
        }
    }

    /// Builder: set the priority.
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set the conditions.
    pub fn with_conditions(mut self, conditions: ConditionGroup) -> Self {
        self.conditions = conditions;
        self
    }

    /// Builder: set the action.
    pub fn with_action(mut self, action: PolicyAction) -> Self {
        self.action = action;
        self
    }

    /// Builder: enable `stop_on_match`.
    pub const fn with_stop_on_match(mut self) -> Self {
        self.stop_on_match = true;
        self
    }

    /// Builder: set metadata.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Builder: set a specific ID (useful for testing or deserialization).
    pub const fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Builder: disable this rule.
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Check if this rule's conditions match the given context.
    ///
    /// Returns `false` if the rule is disabled.
    pub fn matches(&self, context: &Value) -> bool {
        if !self.enabled {
            return false;
        }
        self.conditions.evaluate(context)
    }

    /// Match and return detailed condition results.
    ///
    /// Returns `(matched, condition_details)`. If the rule is disabled,
    /// returns `(false, [])`.
    pub fn matches_with_detail(&self, context: &Value) -> (bool, Vec<ConditionDetail>) {
        if !self.enabled {
            return (false, Vec::new());
        }
        self.conditions.evaluate_full(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, ConditionNode, Logic, Operator};
    use serde_json::json;

    #[test]
    fn rule_matches() {
        let rule = PolicyRule::new("test", "Test rule")
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "total",
                    Operator::Gt,
                    json!(100),
                ))],
            ))
            .with_action(PolicyAction::deny("too high", "reduce"));

        assert!(rule.matches(&json!({"total": 200})));
        assert!(!rule.matches(&json!({"total": 50})));
    }

    #[test]
    fn disabled_rule_never_matches() {
        let rule = PolicyRule::new("test", "Test rule")
            .disabled()
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "total",
                    Operator::Gt,
                    json!(0),
                ))],
            ));

        assert!(!rule.matches(&json!({"total": 999})));
    }

    #[test]
    fn rule_with_detail() {
        let rule = PolicyRule::new("test", "Test rule").with_conditions(ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("b", Operator::Gt, json!(10))),
            ],
        ));

        let (matched, details) = rule.matches_with_detail(&json!({"a": 1, "b": 5}));
        assert!(!matched);
        assert_eq!(details.len(), 2);
        assert!(details[0].matched);
        assert!(!details[1].matched);
    }

    #[test]
    fn rule_priority_and_stop() {
        let rule = PolicyRule::new("high-pri", "High priority rule")
            .with_priority(100)
            .with_stop_on_match();
        assert_eq!(rule.priority, 100);
        assert!(rule.stop_on_match);
    }

    #[test]
    fn serde_roundtrip() {
        let rule = PolicyRule::new("test", "Test")
            .with_priority(50)
            .with_conditions(ConditionGroup::new(
                Logic::And,
                vec![ConditionNode::Leaf(Condition::new(
                    "x",
                    Operator::Eq,
                    json!(1),
                ))],
            ))
            .with_action(PolicyAction::deny("reason", "fix"));

        let json_str = serde_json::to_string(&rule).unwrap();
        let deser: PolicyRule = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.name, "test");
        assert_eq!(deser.priority, 50);
        assert!(deser.enabled);
    }

    #[test]
    fn disabled_rule_with_detail_returns_empty() {
        let rule = PolicyRule::new("test", "Test").disabled();
        let (matched, details) = rule.matches_with_detail(&json!({}));
        assert!(!matched);
        assert!(details.is_empty());
    }
}
