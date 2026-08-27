//! Durable event records emitted atomically with commerce mutations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use stateset_core::{CommandEnvelope, CommerceError, Result};
use uuid::Uuid;

/// Event waiting for reliable publication to downstream consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelOutboxEvent {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub command_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
    pub principal_type: Option<String>,
    pub principal_id: Option<String>,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub dead_lettered_at: Option<DateTime<Utc>>,
}

/// Durable idempotency record containing a serialized execution receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelReceiptRecord {
    pub command_id: Uuid,
    pub idempotency_key: String,
    pub command_type: String,
    pub contract_version: String,
    /// SHA-256 of the semantic request fields bound to the idempotency key.
    pub request_hash: String,
    pub status: String,
    pub receipt: Value,
    pub created_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// Operational snapshot of durable event delivery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelOutboxHealth {
    pub ready: u64,
    pub leased: u64,
    pub delayed: u64,
    pub dead_lettered: u64,
    pub published: u64,
}

/// Result of independently recomputing the append-only receipt audit chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelAuditVerification {
    pub valid: bool,
    pub entries: u64,
    pub head_hash: Option<String>,
    pub first_invalid_sequence: Option<i64>,
}

/// Portable audit-chain checkpoint intended for publication outside the
/// commerce database (transparency log, object store, ledger, or notarizer).
/// Keeping this document externally makes later database rewrites detectable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelAuditCheckpoint {
    pub contract_version: String,
    pub algorithm: String,
    pub entries: u64,
    pub head_hash: Option<String>,
    pub generated_at: DateTime<Utc>,
    /// SHA-256/JCS digest of every preceding checkpoint field.
    pub checkpoint_hash: String,
}

pub(crate) fn build_audit_checkpoint(
    verification: &KernelAuditVerification,
) -> std::result::Result<KernelAuditCheckpoint, String> {
    if !verification.valid {
        return Err("cannot checkpoint an invalid kernel audit chain".into());
    }
    let generated_at = Utc::now();
    let preimage = serde_json::json!({
        "contract_version": "1.0",
        "algorithm": "sha256-jcs-v1",
        "entries": verification.entries,
        "head_hash": verification.head_hash,
        "generated_at": generated_at,
    });
    let canonical = serde_jcs::to_vec(&preimage).map_err(|error| error.to_string())?;
    Ok(KernelAuditCheckpoint {
        contract_version: "1.0".into(),
        algorithm: "sha256-jcs-v1".into(),
        entries: verification.entries,
        head_hash: verification.head_hash.clone(),
        generated_at,
        checkpoint_hash: format!("{:x}", Sha256::digest(canonical)),
    })
}

pub(crate) fn audit_checkpoint_hash_is_valid(
    checkpoint: &KernelAuditCheckpoint,
) -> std::result::Result<bool, String> {
    let preimage = serde_json::json!({
        "contract_version": checkpoint.contract_version,
        "algorithm": checkpoint.algorithm,
        "entries": checkpoint.entries,
        "head_hash": checkpoint.head_hash,
        "generated_at": checkpoint.generated_at,
    });
    let canonical = serde_jcs::to_vec(&preimage).map_err(|error| error.to_string())?;
    Ok(checkpoint.checkpoint_hash == format!("{:x}", Sha256::digest(canonical)))
}

pub(crate) fn receipt_audit_hash(
    previous_audit_hash: Option<&str>,
    request_hash: &str,
    receipt: &Value,
) -> std::result::Result<String, String> {
    let preimage = serde_json::json!({
        "algorithm": "sha256-jcs-v1",
        "previous_audit_hash": previous_audit_hash,
        "request_hash": request_hash,
        "receipt": receipt,
    });
    let canonical = serde_jcs::to_vec(&preimage).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

/// Canonical hash of the semantic intent bound to a durable idempotency key.
/// Invocation identity, tracing, issue time, and preview/apply posture are
/// deliberately excluded so a fresh invocation can promote an authorized
/// preview. Authority evidence is included so it cannot be replaced or have
/// its validity window extended under an existing retry key.
pub(crate) fn semantic_request_hash<C: Serialize, T: Serialize>(
    command: &CommandEnvelope<C>,
    payload: &T,
) -> Result<String> {
    let mut capabilities = command.principal.capabilities.clone();
    capabilities.sort();
    capabilities.dedup();
    let value = serde_json::json!({
        "contract_version": command.contract_version,
        "command_type": command.command_type,
        "principal": {
            "id": command.principal.id,
            "kind": command.principal.kind,
            "tenant_id": command.principal.tenant_id,
            "delegated_by": command.principal.delegated_by,
            "capabilities": capabilities,
        },
        "store_id": command.store_id,
        "expected_version": command.expected_version,
        "policy_version": command.policy_version,
        "approval": command.approval,
        "authority": command.authority,
        "deadline": command.deadline,
        "payload": payload,
    });
    let canonical = serde_jcs::to_vec(&value)
        .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

impl KernelOutboxEvent {
    /// Build an event for a repository mutation. Kernel-aware executors can
    /// additionally attach command, principal, and causal context.
    #[must_use]
    pub fn domain(
        event_type: impl Into<String>,
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        payload: Value,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.into(),
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            payload,
            command_id: None,
            idempotency_key,
            principal_type: None,
            principal_id: None,
            correlation_id: None,
            causation_id: None,
            created_at: Utc::now(),
            published_at: None,
            attempts: 0,
            last_error: None,
            lease_owner: None,
            lease_expires_at: None,
            next_attempt_at: None,
            dead_lettered_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::{AuthorityEvidence, ExecutionMode, KernelPrincipal, PrincipalKind};

    #[test]
    fn semantic_request_hash_binds_authority_but_not_retry_invocation_metadata() {
        let now = Utc::now();
        let mut command = CommandEnvelope::preview(
            "payments.create",
            "semantic-authority-1",
            KernelPrincipal {
                id: "agent:checkout".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant:one".into()),
                delegated_by: Some("user:operator".into()),
                capabilities: vec!["payments.create".into(), "payments.create".into()],
            },
            serde_json::json!({"amount": "12.34", "currency": "USD"}),
        );
        command.store_id = Some("store:one".into());
        command.policy_version = Some("policy:one".into());
        command.authority = Some(AuthorityEvidence {
            issuer: "user:operator".into(),
            key_id: "authority-key-1".into(),
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(5),
            signature: "ab".repeat(64),
        });
        let baseline = semantic_request_hash(&command, &command.payload).expect("baseline hash");

        let mut changed = command.clone();
        changed.authority.as_mut().expect("authority").expires_at += chrono::Duration::minutes(5);
        assert_ne!(
            semantic_request_hash(&changed, &changed.payload).expect("expiry hash"),
            baseline
        );
        let mut changed = command.clone();
        changed.authority.as_mut().expect("authority").signature = "cd".repeat(64);
        assert_ne!(
            semantic_request_hash(&changed, &changed.payload).expect("signature hash"),
            baseline
        );

        let mut retry = command;
        retry.command_id = Uuid::new_v4();
        retry.issued_at += chrono::Duration::seconds(1);
        retry.trace_id = Some("new-retry-trace".into());
        retry.mode = ExecutionMode::Apply;
        assert_eq!(semantic_request_hash(&retry, &retry.payload).expect("retry hash"), baseline);
    }
}
