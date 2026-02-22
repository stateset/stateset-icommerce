use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The kind of action a policy rule can trigger.
///
/// These map to the JS `PolicyAction.type` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ActionType {
    /// Allow the operation to proceed.
    Allow,
    /// Deny the operation (deny-overrides).
    Deny,
    /// Invoke an AI agent.
    Agent,
    /// Start a workflow.
    Workflow,
    /// Send a notification.
    Notify,
    /// Transform the request data.
    Transform,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Agent => "agent",
            Self::Workflow => "workflow",
            Self::Notify => "notify",
            Self::Transform => "transform",
        };
        f.write_str(s)
    }
}

/// A policy action definition — what happens when a rule matches.
///
/// Depending on `action_type`, different fields are relevant:
///
/// | Type | Relevant fields |
/// |------|-----------------|
/// | `Allow` / `Deny` | `reason`, `remediation` |
/// | `Agent` | `agent`, `request` |
/// | `Workflow` | `workflow`, `metadata` |
/// | `Notify` | `notification` |
/// | `Transform` | `transform` |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// The type of action.
    #[serde(rename = "type")]
    pub action_type: ActionType,

    /// Agent name to invoke (for `Agent` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// Request string to send to the agent (for `Agent` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,

    /// Workflow name to trigger (for `Workflow` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,

    /// Notification configuration (for `Notify` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification: Option<Value>,

    /// Transform configuration (for `Transform` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Value>,

    /// Human-readable reason for this action (especially useful for denials).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Suggested fix or workaround when the action is a denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,

    /// Arbitrary metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

impl PolicyAction {
    /// Create an `Allow` action.
    pub const fn allow() -> Self {
        Self {
            action_type: ActionType::Allow,
            agent: None,
            request: None,
            workflow: None,
            notification: None,
            transform: None,
            reason: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Create a `Deny` action with a reason and remediation hint.
    pub fn deny(reason: impl Into<String>, remediation: impl Into<String>) -> Self {
        Self {
            action_type: ActionType::Deny,
            agent: None,
            request: None,
            workflow: None,
            notification: None,
            transform: None,
            reason: Some(reason.into()),
            remediation: Some(remediation.into()),
            metadata: None,
        }
    }

    /// Create a `Deny` action with only a reason (no remediation).
    pub fn deny_simple(reason: impl Into<String>) -> Self {
        Self {
            action_type: ActionType::Deny,
            agent: None,
            request: None,
            workflow: None,
            notification: None,
            transform: None,
            reason: Some(reason.into()),
            remediation: None,
            metadata: None,
        }
    }

    /// Create an `Agent` action.
    pub fn agent(agent: impl Into<String>, request: impl Into<String>) -> Self {
        Self {
            action_type: ActionType::Agent,
            agent: Some(agent.into()),
            request: Some(request.into()),
            workflow: None,
            notification: None,
            transform: None,
            reason: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Create a `Workflow` action.
    pub fn workflow(workflow: impl Into<String>) -> Self {
        Self {
            action_type: ActionType::Workflow,
            agent: None,
            request: None,
            workflow: Some(workflow.into()),
            notification: None,
            transform: None,
            reason: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Create a `Workflow` action with metadata.
    pub fn workflow_with_metadata(workflow: impl Into<String>, metadata: Value) -> Self {
        Self {
            action_type: ActionType::Workflow,
            agent: None,
            request: None,
            workflow: Some(workflow.into()),
            notification: None,
            transform: None,
            reason: None,
            remediation: None,
            metadata: Some(metadata),
        }
    }

    /// Create a `Notify` action.
    pub const fn notify(notification: Value) -> Self {
        Self {
            action_type: ActionType::Notify,
            agent: None,
            request: None,
            workflow: None,
            notification: Some(notification),
            transform: None,
            reason: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Create a `Transform` action.
    pub const fn transform(transform: Value) -> Self {
        Self {
            action_type: ActionType::Transform,
            agent: None,
            request: None,
            workflow: None,
            notification: None,
            transform: Some(transform),
            reason: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Builder: set reason.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Builder: set remediation.
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    /// Builder: set metadata.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn allow_action() {
        let action = PolicyAction::allow();
        assert_eq!(action.action_type, ActionType::Allow);
        assert!(action.reason.is_none());
    }

    #[test]
    fn deny_action() {
        let action = PolicyAction::deny("too expensive", "get approval");
        assert_eq!(action.action_type, ActionType::Deny);
        assert_eq!(action.reason.as_deref(), Some("too expensive"));
        assert_eq!(action.remediation.as_deref(), Some("get approval"));
    }

    #[test]
    fn agent_action() {
        let action = PolicyAction::agent("returns", "Approve return R-123");
        assert_eq!(action.action_type, ActionType::Agent);
        assert_eq!(action.agent.as_deref(), Some("returns"));
        assert_eq!(action.request.as_deref(), Some("Approve return R-123"));
    }

    #[test]
    fn workflow_action_with_metadata() {
        let action = PolicyAction::workflow_with_metadata(
            "orderFulfillment",
            json!({"requiresReview": true}),
        );
        assert_eq!(action.action_type, ActionType::Workflow);
        assert_eq!(action.workflow.as_deref(), Some("orderFulfillment"));
        assert_eq!(
            action.metadata,
            Some(json!({"requiresReview": true}))
        );
    }

    #[test]
    fn builder_chain() {
        let action = PolicyAction::deny_simple("limit exceeded")
            .with_remediation("contact support")
            .with_metadata(json!({"code": "LIMIT_EXCEEDED"}));
        assert_eq!(action.action_type, ActionType::Deny);
        assert_eq!(action.reason.as_deref(), Some("limit exceeded"));
        assert_eq!(action.remediation.as_deref(), Some("contact support"));
    }

    #[test]
    fn serde_roundtrip() {
        let action = PolicyAction::deny("too high", "reduce amount");
        let json_str = serde_json::to_string(&action).unwrap();
        let deser: PolicyAction = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.action_type, ActionType::Deny);
        assert_eq!(deser.reason.as_deref(), Some("too high"));
    }

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
    fn action_type_serde() {
        assert_eq!(
            serde_json::to_string(&ActionType::Allow).unwrap(),
            "\"allow\""
        );
        let deser: ActionType = serde_json::from_str("\"deny\"").unwrap();
        assert_eq!(deser, ActionType::Deny);
    }
}
