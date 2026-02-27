use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::{get_nested_value, resolve_dynamic_ref};
use crate::operator::Operator;

/// A single condition that compares a context field against an expected value.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::{Condition, Operator};
///
/// let cond = Condition::new("order.total", Operator::Gt, json!(100));
/// assert!(cond.evaluate(&json!({"order": {"total": 200}})));
/// assert!(!cond.evaluate(&json!({"order": {"total": 50}})));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Dot-notation path to the field in the evaluation context.
    pub field: String,
    /// The comparison operator to apply.
    pub operator: Operator,
    /// The value to compare against (ignored for unary operators).
    #[serde(default)]
    pub value: Value,
    /// If `true`, the result of the operator is negated.
    #[serde(default)]
    pub negate: bool,
}

impl Condition {
    /// Create a new condition.
    pub fn new(field: impl Into<String>, operator: Operator, value: Value) -> Self {
        Self { field: field.into(), operator, value, negate: false }
    }

    /// Create a new negated condition.
    pub fn new_negated(field: impl Into<String>, operator: Operator, value: Value) -> Self {
        Self { field: field.into(), operator, value, negate: true }
    }

    /// Evaluate this condition against the given context.
    ///
    /// For non-unary operators, dynamic references (e.g., `"${order.total}"`)
    /// in `self.value` are resolved against `context`. If a dynamic reference
    /// cannot be resolved, the condition returns `false` (safe default).
    pub fn evaluate(&self, context: &Value) -> bool {
        let field_value = get_nested_value(context, &self.field).cloned().unwrap_or(Value::Null);

        let is_unary = self.operator.is_unary();

        let compare_value = if is_unary {
            Value::Null
        } else {
            let (resolved, is_dynamic) = resolve_dynamic_ref(&self.value, context);

            // Missing dynamic references => non-match (safe default)
            if is_dynamic && resolved.is_null() {
                // Check if the original path truly resolved to null vs missing
                // In JS: `isDynamicRef && compareValue === undefined`
                // We treat null as the "missing" sentinel for dynamic refs
                if get_nested_value(context, extract_ref_path(&self.value).unwrap_or("")).is_none()
                {
                    return false;
                }
            }

            resolved
        };

        let result = self.operator.evaluate(&field_value, &compare_value);

        if self.negate { !result } else { result }
    }

    /// Evaluate this condition and return detailed results for explainable decisions.
    pub fn evaluate_with_detail(&self, context: &Value) -> ConditionDetail {
        let field_value = get_nested_value(context, &self.field).cloned().unwrap_or(Value::Null);

        let is_unary = self.operator.is_unary();

        let compare_value = if is_unary {
            Value::Null
        } else {
            let (resolved, is_dynamic) = resolve_dynamic_ref(&self.value, context);

            if is_dynamic
                && resolved.is_null()
                && get_nested_value(context, extract_ref_path(&self.value).unwrap_or("")).is_none()
            {
                return ConditionDetail {
                    matched: false,
                    field: self.field.clone(),
                    operator: self.operator,
                    expected_value: self.value.clone(),
                    actual_value: field_value,
                };
            }

            resolved
        };

        let mut result = self.operator.evaluate(&field_value, &compare_value);
        if self.negate {
            result = !result;
        }

        ConditionDetail {
            matched: result,
            field: self.field.clone(),
            operator: self.operator,
            expected_value: if is_unary { Value::Null } else { compare_value },
            actual_value: field_value,
        }
    }
}

/// Extract the ref path from a `"${path}"` string.
fn extract_ref_path(value: &Value) -> Option<&str> {
    if let Value::String(s) = value {
        let s = s.trim();
        if s.starts_with("${") && s.ends_with('}') {
            let inner = &s[2..s.len() - 1];
            let trimmed = inner.trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed);
        }
    }
    None
}

/// Detailed result of evaluating a single condition.
///
/// Used for building [`PolicyExplanation`](crate::PolicyExplanation) objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionDetail {
    /// Whether this condition matched.
    pub matched: bool,
    /// The dot-path that was evaluated.
    pub field: String,
    /// The operator that was applied.
    pub operator: Operator,
    /// The value the condition expected (or the resolved dynamic ref).
    pub expected_value: Value,
    /// The actual value found in the context.
    pub actual_value: Value,
}

/// The logical combinator for a group of conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Logic {
    /// All conditions must match.
    And,
    /// At least one condition must match.
    Or,
}

/// A tree node that is either a leaf [`Condition`] or a nested [`ConditionGroup`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConditionNode {
    /// A single condition (leaf).
    Leaf(Condition),
    /// A nested group of conditions.
    Group(ConditionGroup),
}

impl ConditionNode {
    /// Evaluate this node against the given context.
    pub fn evaluate(&self, context: &Value) -> bool {
        match self {
            Self::Leaf(c) => c.evaluate(context),
            Self::Group(g) => g.evaluate(context),
        }
    }

    /// Evaluate with detail.
    pub fn evaluate_with_detail(&self, context: &Value) -> Vec<ConditionDetail> {
        match self {
            Self::Leaf(c) => vec![c.evaluate_with_detail(context)],
            Self::Group(g) => g.evaluate_with_detail(context),
        }
    }

    /// Evaluate this node and return its match result plus flattened leaf details.
    pub fn evaluate_full(&self, context: &Value) -> (bool, Vec<ConditionDetail>) {
        match self {
            Self::Leaf(c) => {
                let detail = c.evaluate_with_detail(context);
                (detail.matched, vec![detail])
            }
            Self::Group(g) => g.evaluate_full(context),
        }
    }
}

/// A group of conditions combined with [`Logic::And`] or [`Logic::Or`].
///
/// Groups can be nested to create complex condition trees.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::{ConditionGroup, ConditionNode, Condition, Operator, Logic};
///
/// let group = ConditionGroup::new(Logic::And, vec![
///     ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(100))),
///     ConditionNode::Leaf(Condition::new("customer.tier", Operator::In, json!(["gold", "platinum"]))),
/// ]);
///
/// let ctx = json!({
///     "order": {"total": 200},
///     "customer": {"tier": "gold"}
/// });
/// assert!(group.evaluate(&ctx));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionGroup {
    /// The logical combinator for this group.
    pub logic: Logic,
    /// The child conditions (leaf or nested group).
    pub conditions: Vec<ConditionNode>,
}

impl ConditionGroup {
    /// Create a new condition group.
    pub const fn new(logic: Logic, conditions: Vec<ConditionNode>) -> Self {
        Self { logic, conditions }
    }

    /// Evaluate the group.
    ///
    /// Empty groups follow standard boolean identity semantics:
    /// - empty `And` is `true`
    /// - empty `Or` is `false`
    pub fn evaluate(&self, context: &Value) -> bool {
        if self.conditions.is_empty() {
            return matches!(self.logic, Logic::And);
        }

        match self.logic {
            Logic::And => self.conditions.iter().all(|c| c.evaluate(context)),
            Logic::Or => self.conditions.iter().any(|c| c.evaluate(context)),
        }
    }

    /// Evaluate all conditions and return per-condition details.
    ///
    /// The `matched` result of the group (And/Or) can be derived from the
    /// individual details, but is also available from [`evaluate`](Self::evaluate).
    pub fn evaluate_with_detail(&self, context: &Value) -> Vec<ConditionDetail> {
        if self.conditions.is_empty() {
            return Vec::new();
        }

        self.conditions.iter().flat_map(|c| c.evaluate_with_detail(context)).collect()
    }

    /// Evaluate and return both the match result and the details.
    pub fn evaluate_full(&self, context: &Value) -> (bool, Vec<ConditionDetail>) {
        if self.conditions.is_empty() {
            return (matches!(self.logic, Logic::And), Vec::new());
        }

        // Preserve nested AND/OR semantics by aggregating each child node's
        // boolean result (not every flattened leaf result).
        let mut details = Vec::new();
        let mut child_matches = Vec::with_capacity(self.conditions.len());

        for condition in &self.conditions {
            let (matched, mut node_details) = condition.evaluate_full(context);
            child_matches.push(matched);
            details.append(&mut node_details);
        }

        let matched = match self.logic {
            Logic::And => child_matches.into_iter().all(std::convert::identity),
            Logic::Or => child_matches.into_iter().any(std::convert::identity),
        };

        (matched, details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn condition_simple_match() {
        let cond = Condition::new("order.total", Operator::Gt, json!(100));
        assert!(cond.evaluate(&json!({"order": {"total": 200}})));
        assert!(!cond.evaluate(&json!({"order": {"total": 50}})));
    }

    #[test]
    fn condition_missing_field() {
        let cond = Condition::new("order.missing", Operator::Eq, json!(42));
        assert!(!cond.evaluate(&json!({"order": {"total": 42}})));
    }

    #[test]
    fn condition_negate() {
        let cond = Condition::new_negated("status", Operator::Eq, json!("active"));
        assert!(cond.evaluate(&json!({"status": "inactive"})));
        assert!(!cond.evaluate(&json!({"status": "active"})));
    }

    #[test]
    fn condition_unary_is_null() {
        let cond = Condition::new("email", Operator::IsNull, json!(null));
        assert!(cond.evaluate(&json!({})));
        assert!(!cond.evaluate(&json!({"email": "a@b.com"})));
    }

    #[test]
    fn condition_unary_is_true() {
        let cond = Condition::new("vip", Operator::IsTrue, json!(null));
        assert!(cond.evaluate(&json!({"vip": true})));
        assert!(!cond.evaluate(&json!({"vip": false})));
    }

    #[test]
    fn condition_dynamic_ref() {
        let cond =
            Condition::new("inventory.quantity", Operator::Lte, json!("${inventory.reorderPoint}"));
        let ctx = json!({"inventory": {"quantity": 5, "reorderPoint": 10}});
        assert!(cond.evaluate(&ctx));

        let ctx2 = json!({"inventory": {"quantity": 15, "reorderPoint": 10}});
        assert!(!cond.evaluate(&ctx2));
    }

    #[test]
    fn condition_dynamic_ref_missing_returns_false() {
        let cond = Condition::new("order.total", Operator::Neq, json!("${order.missingField}"));
        // Dynamic ref missing => false (safe default)
        assert!(!cond.evaluate(&json!({"order": {"total": 100}})));
    }

    #[test]
    fn condition_with_detail() {
        let cond = Condition::new("price", Operator::Gt, json!(50));
        let detail = cond.evaluate_with_detail(&json!({"price": 75}));
        assert!(detail.matched);
        assert_eq!(detail.field, "price");
        assert_eq!(detail.actual_value, json!(75));
        assert_eq!(detail.expected_value, json!(50));
    }

    #[test]
    fn group_and_all_match() {
        let group = ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(2))),
            ],
        );
        assert!(group.evaluate(&json!({"a": 1, "b": 2})));
    }

    #[test]
    fn group_and_one_fails() {
        let group = ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(999))),
            ],
        );
        assert!(!group.evaluate(&json!({"a": 1, "b": 2})));
    }

    #[test]
    fn group_or_one_matches() {
        let group = ConditionGroup::new(
            Logic::Or,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(999))),
                ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(2))),
            ],
        );
        assert!(group.evaluate(&json!({"a": 1, "b": 2})));
    }

    #[test]
    fn group_or_none_match() {
        let group = ConditionGroup::new(
            Logic::Or,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(999))),
                ConditionNode::Leaf(Condition::new("b", Operator::Eq, json!(999))),
            ],
        );
        assert!(!group.evaluate(&json!({"a": 1, "b": 2})));
    }

    #[test]
    fn group_empty_returns_true() {
        let group = ConditionGroup::new(Logic::And, vec![]);
        assert!(group.evaluate(&json!({})));
    }

    #[test]
    fn group_nested() {
        let inner = ConditionGroup::new(
            Logic::Or,
            vec![
                ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("y", Operator::Eq, json!(2))),
            ],
        );
        let outer = ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("z", Operator::Eq, json!(3))),
                ConditionNode::Group(inner),
            ],
        );
        // z=3 AND (x=1 OR y=2) => x=1 matches, z=3 matches => true
        assert!(outer.evaluate(&json!({"x": 1, "y": 99, "z": 3})));
        // z=3 AND (x=1 OR y=2) => neither x=1 nor y=2 matches => false
        assert!(!outer.evaluate(&json!({"x": 99, "y": 99, "z": 3})));
    }

    #[test]
    fn group_evaluate_full() {
        let group = ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("a", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("b", Operator::Gt, json!(10))),
            ],
        );

        let (matched, details) = group.evaluate_full(&json!({"a": 1, "b": 5}));
        assert!(!matched);
        assert_eq!(details.len(), 2);
        assert!(details[0].matched); // a == 1
        assert!(!details[1].matched); // b > 10 fails
    }

    #[test]
    fn group_evaluate_full_preserves_nested_logic() {
        let nested_or = ConditionGroup::new(
            Logic::Or,
            vec![
                ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1))),
                ConditionNode::Leaf(Condition::new("y", Operator::Eq, json!(2))),
            ],
        );

        let outer = ConditionGroup::new(
            Logic::And,
            vec![
                ConditionNode::Leaf(Condition::new("z", Operator::Eq, json!(3))),
                ConditionNode::Group(nested_or),
            ],
        );

        // z == 3 AND (x == 1 OR y == 2)
        let (matched, details) = outer.evaluate_full(&json!({"x": 1, "y": 0, "z": 3}));
        assert!(matched);
        assert_eq!(details.len(), 3);
        assert!(details[0].matched);
        assert!(details[1].matched);
        assert!(!details[2].matched);
    }

    #[test]
    fn serde_condition_roundtrip() {
        let cond = Condition::new("order.total", Operator::Gte, json!(100));
        let json_str = serde_json::to_string(&cond).unwrap();
        let deser: Condition = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.field, "order.total");
        assert_eq!(deser.operator, Operator::Gte);
        assert_eq!(deser.value, json!(100));
        assert!(!deser.negate);
    }

    #[test]
    fn serde_condition_group_roundtrip() {
        let group = ConditionGroup::new(
            Logic::And,
            vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
        );
        let json_str = serde_json::to_string(&group).unwrap();
        let deser: ConditionGroup = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.logic, Logic::And);
        assert_eq!(deser.conditions.len(), 1);
    }

    #[test]
    fn serde_logic_values() {
        assert_eq!(serde_json::to_string(&Logic::And).unwrap(), "\"and\"");
        assert_eq!(serde_json::to_string(&Logic::Or).unwrap(), "\"or\"");
    }
}
