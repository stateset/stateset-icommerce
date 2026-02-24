use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::action::ActionType;
use crate::condition::ConditionDetail;

/// Structured explanation of a policy evaluation outcome.
///
/// Provides the full "why" behind a denial, allow, or transform decision,
/// including per-condition details that show which fields matched or failed.
///
/// This type is the Rust equivalent of the JS `PolicyExplanation` class.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyExplanation {
    /// ID of the policy set that produced this explanation.
    pub policy_set_id: Uuid,
    /// Name of the policy set.
    pub policy_set_name: String,
    /// ID of the rule that matched.
    pub rule_id: Uuid,
    /// Name of the rule.
    pub rule_name: String,
    /// Description of the rule.
    #[serde(default)]
    pub rule_description: String,
    /// The action type that was triggered.
    pub action_type: ActionType,
    /// Human-readable reason for the action.
    #[serde(default)]
    pub reason: String,
    /// Suggested remediation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    /// Per-condition evaluation details.
    #[serde(default)]
    pub conditions: Vec<ConditionDetail>,
}

impl std::fmt::Display for PolicyExplanation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Matches JS toString() format
        write!(
            f,
            "Policy \"{}\" / Rule \"{}\": {}",
            self.policy_set_name, self.rule_name, self.action_type
        )?;

        if !self.reason.is_empty() {
            write!(f, "\n  Reason: {}", self.reason)?;
        }

        for c in &self.conditions {
            write!(
                f,
                "\n  - {} {} {} (actual: {}, matched: {})",
                c.field, c.operator, c.expected_value, c.actual_value, c.matched,
            )?;
        }

        if let Some(ref rem) = self.remediation {
            write!(f, "\n  Remediation: {rem}")?;
        }

        Ok(())
    }
}

/// Audit entry for a policy transform — records before/after values.
///
/// Created when a `Transform` action is applied, allowing downstream
/// systems to audit exactly what changed and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformAuditEntry {
    /// ID of the rule that triggered the transform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<Uuid>,
    /// Name of the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,
    /// ID of the policy set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_set_id: Option<Uuid>,
    /// The dot-notation field that was transformed.
    pub field: String,
    /// The value before the transform.
    pub before: Value,
    /// The value after the transform.
    pub after: Value,
    /// When the transform occurred.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl TransformAuditEntry {
    /// Create a new transform audit entry with the current timestamp.
    pub fn new(field: impl Into<String>, before: Value, after: Value) -> Self {
        Self {
            rule_id: None,
            rule_name: None,
            policy_set_id: None,
            field: field.into(),
            before,
            after,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Builder: set the rule context.
    pub fn with_rule(mut self, rule_id: Uuid, rule_name: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id);
        self.rule_name = Some(rule_name.into());
        self
    }

    /// Builder: set the policy set ID.
    pub const fn with_policy_set(mut self, policy_set_id: Uuid) -> Self {
        self.policy_set_id = Some(policy_set_id);
        self
    }
}

/// Information about a rule that matched during policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedRule {
    /// The rule's unique ID.
    pub id: Uuid,
    /// The rule's name.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Operator;
    use serde_json::json;

    #[test]
    fn explanation_display() {
        let explanation = PolicyExplanation {
            policy_set_id: Uuid::nil(),
            policy_set_name: "Order Limits".into(),
            rule_id: Uuid::nil(),
            rule_name: "high-value".into(),
            rule_description: "Flag high-value orders".into(),
            action_type: ActionType::Deny,
            reason: "Order exceeds $10,000".into(),
            remediation: Some("Request manager approval".into()),
            conditions: vec![ConditionDetail {
                matched: true,
                field: "order.total".into(),
                operator: Operator::Gt,
                expected_value: json!(10000),
                actual_value: json!(15000),
            }],
        };

        let display = explanation.to_string();
        assert!(display.contains("Order Limits"));
        assert!(display.contains("high-value"));
        assert!(display.contains("deny"));
        assert!(display.contains("Order exceeds $10,000"));
        assert!(display.contains("order.total"));
        assert!(display.contains("Remediation: Request manager approval"));
    }

    #[test]
    fn explanation_serde_roundtrip() {
        let explanation = PolicyExplanation {
            policy_set_id: Uuid::nil(),
            policy_set_name: "test".into(),
            rule_id: Uuid::nil(),
            rule_name: "rule1".into(),
            rule_description: String::new(),
            action_type: ActionType::Allow,
            reason: String::new(),
            remediation: None,
            conditions: Vec::new(),
        };

        let json_str = serde_json::to_string(&explanation).unwrap();
        let deser: PolicyExplanation = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.policy_set_name, "test");
        assert_eq!(deser.action_type, ActionType::Allow);
    }

    #[test]
    fn transform_audit_entry() {
        let entry = TransformAuditEntry::new("price", json!(100), json!(90))
            .with_rule(Uuid::nil(), "discount-rule")
            .with_policy_set(Uuid::nil());

        assert_eq!(entry.field, "price");
        assert_eq!(entry.before, json!(100));
        assert_eq!(entry.after, json!(90));
        assert!(entry.rule_name.is_some());
        assert!(entry.policy_set_id.is_some());
    }

    #[test]
    fn matched_rule_serde() {
        let mr = MatchedRule { id: Uuid::nil(), name: "test-rule".into() };
        let json_str = serde_json::to_string(&mr).unwrap();
        assert!(json_str.contains("test-rule"));
        let deser: MatchedRule = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.name, "test-rule");
    }
}
