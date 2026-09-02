//! Static envelope checks shared by every governed command.

use chrono::{DateTime, Utc};
use stateset_core::{CommandEnvelope, PolicyDecisionEvidence, PrincipalKind, RetryDisposition};

/// A typed rejection produced before any aggregate state is consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRejection {
    /// Stable machine-readable code (`kernel.*` or `commerce.*`).
    pub code: &'static str,
    /// Human-readable explanation recorded on the receipt.
    pub message: String,
    /// Retry guidance recorded on the receipt.
    pub retry: RetryDisposition,
}

impl GuardRejection {
    /// A rejection that retrying can never resolve.
    #[must_use]
    pub fn never(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), retry: RetryDisposition::Never }
    }

    /// A rejection to retry after reloading state and resolving a conflict.
    #[must_use]
    pub fn after_conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), retry: RetryDisposition::AfterConflict }
    }
}

/// Whether the command may carry `expected_version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionExpectation {
    /// The command targets a versioned aggregate; `expected_version` is honoured.
    Aggregate,
    /// The command creates an aggregate (or targets an unversioned one);
    /// `expected_version` is rejected with `kernel.expected_version_not_applicable`.
    NotApplicable(&'static str),
}

/// Envelope-level guard chain evaluated identically on every backend.
///
/// Check order is stable and observable through receipts:
/// `kernel.idempotency_key_mismatch` → `kernel.command_type_mismatch` →
/// `kernel.deadline_exceeded` → `kernel.policy_denied` →
/// `kernel.actor_mismatch` → `kernel.expected_version_not_applicable`.
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeGuard<'a> {
    /// Command type the executor entry point serves.
    pub expected_type: &'a str,
    /// `expected_version` applicability.
    pub version: VersionExpectation,
    /// Payload-level idempotency key (payments carry one) that must agree
    /// with the envelope key when present.
    pub payload_idempotency_key: Option<&'a str>,
}

impl<'a> EnvelopeGuard<'a> {
    /// Guard for a command that mutates a versioned aggregate.
    #[must_use]
    pub const fn aggregate(expected_type: &'a str) -> Self {
        Self {
            expected_type,
            version: VersionExpectation::Aggregate,
            payload_idempotency_key: None,
        }
    }

    /// Guard for a command that creates an aggregate.
    #[must_use]
    pub const fn create(expected_type: &'a str) -> Self {
        Self {
            expected_type,
            version: VersionExpectation::NotApplicable(
                "create commands cannot carry an expected aggregate version",
            ),
            payload_idempotency_key: None,
        }
    }

    /// Guard for a command whose aggregate exposes no version.
    #[must_use]
    pub const fn unversioned(expected_type: &'a str, message: &'static str) -> Self {
        Self {
            expected_type,
            version: VersionExpectation::NotApplicable(message),
            payload_idempotency_key: None,
        }
    }

    /// Require the payload idempotency key (when present) to equal the envelope key.
    #[must_use]
    pub const fn with_payload_key(mut self, key: Option<&'a str>) -> Self {
        self.payload_idempotency_key = key;
        self
    }

    /// Evaluate the chain; `None` means every envelope check passed.
    #[must_use]
    pub fn evaluate<C>(
        &self,
        command: &CommandEnvelope<C>,
        policy: &PolicyDecisionEvidence,
        now: DateTime<Utc>,
    ) -> Option<GuardRejection> {
        if self.payload_idempotency_key.is_some_and(|key| key != command.idempotency_key) {
            return Some(GuardRejection::never(
                "kernel.idempotency_key_mismatch",
                "payload idempotency key does not match the command envelope",
            ));
        }
        if command.command_type != self.expected_type {
            return Some(GuardRejection::never(
                "kernel.command_type_mismatch",
                format!("expected {} command type", self.expected_type),
            ));
        }
        if command.deadline.is_some_and(|deadline| deadline <= now) {
            return Some(GuardRejection::never(
                "kernel.deadline_exceeded",
                "command deadline elapsed before execution",
            ));
        }
        if !policy.allowed {
            return Some(GuardRejection::never(
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ));
        }
        if let Some(reason) = actor_mismatch(command) {
            return Some(GuardRejection::never("kernel.actor_mismatch", reason));
        }
        if let VersionExpectation::NotApplicable(message) = self.version
            && command.expected_version.is_some()
        {
            return Some(GuardRejection::never("kernel.expected_version_not_applicable", message));
        }
        None
    }
}

/// The actor claiming authority over a command must not be the actor being
/// governed: an agent cannot delegate to itself, and no principal can approve
/// its own command.
fn actor_mismatch<C>(command: &CommandEnvelope<C>) -> Option<&'static str> {
    let principal = &command.principal;
    if principal.kind == PrincipalKind::Agent
        && principal.delegated_by.as_deref() == Some(principal.id.as_str())
    {
        return Some("an agent principal cannot be delegated by itself");
    }
    if command.approval.as_ref().is_some_and(|approval| approval.approved_by == principal.id) {
        return Some("a principal cannot approve its own command");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::{ApprovalEvidence, KernelPrincipal};

    fn command() -> CommandEnvelope<serde_json::Value> {
        CommandEnvelope::preview(
            "orders.transition",
            "guard-key",
            KernelPrincipal {
                id: "agent:one".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant".into()),
                delegated_by: Some("user:one".into()),
                capabilities: vec![],
            },
            serde_json::json!({}),
        )
    }

    fn allowed() -> PolicyDecisionEvidence {
        PolicyDecisionEvidence {
            policy_version: "p".into(),
            decision_id: "d".into(),
            allowed: true,
            reason_codes: vec![],
        }
    }

    #[test]
    fn guard_chain_orders_envelope_rejections_stably() {
        let now = Utc::now();
        let guard = EnvelopeGuard::create("orders.transition").with_payload_key(Some("other"));
        let mut command = command();
        command.command_type = "orders.ship".into();
        command.deadline = Some(now - chrono::Duration::seconds(1));
        command.expected_version = Some(1);
        let denied = PolicyDecisionEvidence { allowed: false, ..allowed() };
        let codes: Vec<&str> =
            std::iter::successors(Some((guard, command, denied)), |(guard, command, policy)| {
                let mut guard = *guard;
                let mut command = command.clone();
                let mut policy = policy.clone();
                if guard.payload_idempotency_key.is_some() {
                    guard.payload_idempotency_key = None;
                } else if command.command_type != "orders.transition" {
                    command.command_type = "orders.transition".into();
                } else if command.deadline.is_some() {
                    command.deadline = None;
                } else if !policy.allowed {
                    policy.allowed = true;
                } else {
                    return None;
                }
                Some((guard, command, policy))
            })
            .map(|(guard, command, policy)| {
                guard.evaluate(&command, &policy, now).expect("rejection").code
            })
            .collect();
        assert_eq!(
            codes,
            [
                "kernel.idempotency_key_mismatch",
                "kernel.command_type_mismatch",
                "kernel.deadline_exceeded",
                "kernel.policy_denied",
                "kernel.expected_version_not_applicable",
            ]
        );
    }

    #[test]
    fn actor_mismatch_rejects_self_delegation_and_self_approval() {
        let now = Utc::now();
        let guard = EnvelopeGuard::aggregate("orders.transition");
        let mut command = command();
        assert_eq!(guard.evaluate(&command, &allowed(), now), None);
        command.principal.delegated_by = Some("agent:one".into());
        assert_eq!(
            guard.evaluate(&command, &allowed(), now).map(|r| r.code),
            Some("kernel.actor_mismatch")
        );
        command.principal.delegated_by = Some("user:one".into());
        command.approval = Some(ApprovalEvidence {
            approval_id: "a".into(),
            approved_by: "agent:one".into(),
            scope: "orders.transition".into(),
            tenant_id: None,
            store_id: None,
            idempotency_key: None,
            approved_at: now,
            expires_at: None,
        });
        assert_eq!(
            guard.evaluate(&command, &allowed(), now).map(|r| r.code),
            Some("kernel.actor_mismatch")
        );
    }

    #[test]
    fn aggregate_guard_accepts_expected_version() {
        let mut command = command();
        command.expected_version = Some(3);
        assert_eq!(
            EnvelopeGuard::aggregate("orders.transition").evaluate(
                &command,
                &allowed(),
                Utc::now()
            ),
            None
        );
    }
}
