//! Versioned command and receipt contracts for agent-safe commerce execution.
//!
//! Domain repositories remain usable directly. This module supplies the
//! stable envelope an AI runtime can place around any domain command so
//! identity, intent, authorization evidence, retries, and outcomes are
//! explicit and machine-verifiable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use uuid::Uuid;

/// Current version of the kernel command/receipt wire contract.
pub const KERNEL_CONTRACT_VERSION: &str = "1.0";

/// Identity class responsible for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrincipalKind {
    /// An authenticated person.
    Human,
    /// An autonomous or delegated software agent.
    Agent,
    /// An internal system process.
    System,
    /// An external integration.
    Integration,
}

/// Authenticated identity and delegation context for a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPrincipal {
    /// Stable subject identifier.
    pub id: String,
    /// Subject category.
    pub kind: PrincipalKind,
    /// Tenant boundary, when the store is multi-tenant.
    pub tenant_id: Option<String>,
    /// Principal that delegated authority to this subject.
    pub delegated_by: Option<String>,
    /// Capabilities asserted by the caller and checked by policy.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Whether execution is a non-mutating preview or an authorized mutation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Validate and describe effects without committing them.
    #[default]
    Preview,
    /// Commit the command if all guards pass.
    Apply,
}

/// Evidence for an approval required by policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalEvidence {
    /// Stable approval identifier.
    pub approval_id: String,
    /// Principal that granted approval.
    pub approved_by: String,
    /// Policy-defined scope of the approval.
    pub scope: String,
    /// Tenant this approval authorizes.
    pub tenant_id: Option<String>,
    /// Store this approval authorizes.
    pub store_id: Option<String>,
    /// Semantic retry key this approval authorizes.
    pub idempotency_key: Option<String>,
    /// When the approval was granted.
    pub approved_at: DateTime<Utc>,
    /// Optional expiry after which this evidence must be rejected.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Cryptographic proof that a trusted issuer authorized the semantic command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityEvidence {
    pub issuer: String,
    pub key_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Hex-encoded Ed25519 signature over [`authority_signing_hash`].
    pub signature: String,
}

/// Versioned execution request shared by every commerce command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope<T> {
    /// Wire-contract version.
    pub contract_version: String,
    /// Unique identity for this invocation.
    pub command_id: Uuid,
    /// Stable retry key. Retries must reuse this value.
    pub idempotency_key: String,
    /// Stable namespaced command name, such as `payments.create`.
    pub command_type: String,
    /// Authenticated actor and delegation context.
    pub principal: KernelPrincipal,
    /// Logical store boundary.
    pub store_id: Option<String>,
    /// Root workflow identifier.
    pub correlation_id: Option<Uuid>,
    /// Command or event that caused this command.
    pub causation_id: Option<Uuid>,
    /// Optimistic concurrency version expected by the caller.
    pub expected_version: Option<i32>,
    /// Policy revision the caller expects to govern execution.
    pub policy_version: Option<String>,
    /// Human or automated approval evidence.
    pub approval: Option<ApprovalEvidence>,
    /// Optional signed authority, required when the command policy enables it.
    pub authority: Option<AuthorityEvidence>,
    /// Time after which execution should not begin.
    pub deadline: Option<DateTime<Utc>>,
    /// Distributed tracing identifier.
    pub trace_id: Option<String>,
    /// Preview or apply posture.
    pub mode: ExecutionMode,
    /// Domain-specific request.
    pub payload: T,
    /// Time the command was issued.
    pub issued_at: DateTime<Utc>,
}

impl<T> CommandEnvelope<T> {
    /// Construct a safe-by-default preview command.
    pub fn preview(
        command_type: impl Into<String>,
        idempotency_key: impl Into<String>,
        principal: KernelPrincipal,
        payload: T,
    ) -> Self {
        Self {
            contract_version: KERNEL_CONTRACT_VERSION.into(),
            command_id: Uuid::new_v4(),
            idempotency_key: idempotency_key.into(),
            command_type: command_type.into(),
            principal,
            store_id: None,
            correlation_id: None,
            causation_id: None,
            expected_version: None,
            policy_version: None,
            approval: None,
            authority: None,
            deadline: None,
            trace_id: None,
            mode: ExecutionMode::Preview,
            payload,
            issued_at: Utc::now(),
        }
    }

    /// Explicitly opt this command into mutation after preview/authorization.
    #[must_use]
    pub const fn into_apply(mut self) -> Self {
        self.mode = ExecutionMode::Apply;
        self
    }

    /// Validate the cross-domain kernel contract.
    pub fn validate_contract(&self) -> Result<(), KernelContractError> {
        if self.contract_version != KERNEL_CONTRACT_VERSION {
            return Err(KernelContractError::UnsupportedVersion(self.contract_version.clone()));
        }
        if self.command_id.is_nil() {
            return Err(KernelContractError::MissingField("command_id"));
        }
        if self.command_type.trim().is_empty() {
            return Err(KernelContractError::MissingField("command_type"));
        }
        if self.idempotency_key.trim().is_empty() {
            return Err(KernelContractError::MissingField("idempotency_key"));
        }
        if self.principal.id.trim().is_empty() {
            return Err(KernelContractError::MissingField("principal.id"));
        }
        Ok(())
    }
}

/// Result category recorded in an execution receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Guards passed and predicted effects were returned without mutation.
    Previewed,
    /// Mutation committed.
    Succeeded,
    /// Policy, approval, validation, or concurrency guard rejected execution.
    Rejected,
    /// Execution began but failed.
    Failed,
}

/// Machine-readable guidance for retrying a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDisposition {
    /// Retrying cannot resolve this outcome.
    Never,
    /// Retry using exactly the same idempotency key.
    SameKey,
    /// Reload state and retry after resolving an optimistic conflict.
    AfterConflict,
    /// Retry later using exactly the same idempotency key.
    AfterDelay,
}

/// Policy decision captured with a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionEvidence {
    /// Policy revision evaluated.
    pub policy_version: String,
    /// Stable decision identifier.
    pub decision_id: String,
    /// Whether policy allowed execution.
    pub allowed: bool,
    /// Stable reason codes suitable for automation.
    #[serde(default)]
    pub reason_codes: Vec<String>,
}

/// Policy requirements for one namespaced kernel command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCommandPolicy {
    /// Capabilities the authenticated principal must hold.
    #[serde(default)]
    pub required_capabilities: BTreeSet<String>,
    /// Whether unexpired approval evidence scoped to this command is required.
    #[serde(default)]
    pub requires_approval: bool,
    /// Require a non-empty tenant boundary on the principal.
    #[serde(default = "default_true")]
    pub requires_tenant: bool,
    /// Require a non-empty logical store boundary on the command.
    #[serde(default = "default_true")]
    pub requires_store: bool,
    /// Optional tenant allowlist. Empty permits any non-empty tenant when
    /// `requires_tenant` is enabled.
    #[serde(default)]
    pub allowed_tenant_ids: BTreeSet<String>,
    /// Optional logical-store allowlist. Empty permits any non-empty store
    /// when `requires_store` is enabled.
    #[serde(default)]
    pub allowed_store_ids: BTreeSet<String>,
    /// Require autonomous agents to identify their delegating principal.
    #[serde(default = "default_true")]
    pub requires_agent_delegation: bool,
    /// Require a valid signature from a key trusted by the policy.
    #[serde(default)]
    pub requires_signed_authority: bool,
}

const fn default_true() -> bool {
    true
}

impl KernelCommandPolicy {
    /// Require the supplied capabilities without requiring separate approval.
    pub fn requiring(capabilities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            required_capabilities: capabilities.into_iter().map(Into::into).collect(),
            requires_approval: false,
            requires_tenant: true,
            requires_store: true,
            allowed_tenant_ids: BTreeSet::new(),
            allowed_store_ids: BTreeSet::new(),
            requires_agent_delegation: true,
            requires_signed_authority: false,
        }
    }

    /// Require explicit approval in addition to the configured capabilities.
    #[must_use]
    pub const fn with_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }

    /// Require cryptographic authorization of the semantic command.
    #[must_use]
    pub const fn with_signed_authority(mut self) -> Self {
        self.requires_signed_authority = true;
        self
    }

    /// Restrict this command to explicitly configured tenant identities.
    #[must_use]
    pub fn for_tenants(mut self, tenants: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tenant_ids = tenants.into_iter().map(Into::into).collect();
        self
    }

    /// Restrict this command to explicitly configured logical stores.
    #[must_use]
    pub fn for_stores(mut self, stores: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_store_ids = stores.into_iter().map(Into::into).collect();
        self
    }
}

/// Deterministic, versioned allow-list evaluated before kernel execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPolicy {
    /// Stable policy revision included in every decision receipt.
    pub version: String,
    /// Command-specific rules. Commands absent from this map are denied.
    #[serde(default)]
    pub commands: BTreeMap<String, KernelCommandPolicy>,
    /// Trusted Ed25519 verifying keys, hex encoded and addressed by key ID.
    #[serde(default)]
    pub trusted_authority_keys: BTreeMap<String, String>,
}

impl KernelPolicy {
    /// Create a deny-by-default policy revision.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            commands: BTreeMap::new(),
            trusted_authority_keys: BTreeMap::new(),
        }
    }

    /// Add or replace a command rule.
    #[must_use]
    pub fn allow(mut self, command_type: impl Into<String>, rule: KernelCommandPolicy) -> Self {
        self.commands.insert(command_type.into(), rule);
        self
    }

    /// Trust an Ed25519 authority key under a stable key ID.
    #[must_use]
    pub fn with_trusted_authority_key(
        mut self,
        key_id: impl Into<String>,
        public_key: [u8; 32],
    ) -> Self {
        self.trusted_authority_keys.insert(key_id.into(), hex::encode(public_key));
        self
    }

    /// Evaluate a command without side effects.
    #[must_use]
    pub fn evaluate<T: Serialize>(
        &self,
        command: &CommandEnvelope<T>,
        now: DateTime<Utc>,
    ) -> PolicyDecisionEvidence {
        let mut reasons = Vec::new();
        if command.policy_version.as_deref().is_some_and(|version| version != self.version) {
            reasons.push("policy.version_conflict".to_string());
        }

        match self.commands.get(&command.command_type) {
            None => reasons.push("policy.command_not_allowed".to_string()),
            Some(rule) => {
                if rule.requires_tenant
                    && command.principal.tenant_id.as_deref().is_none_or(str::is_empty)
                {
                    reasons.push("policy.tenant_required".to_string());
                }
                if rule.requires_store && command.store_id.as_deref().is_none_or(str::is_empty) {
                    reasons.push("policy.store_required".to_string());
                }
                if !rule.allowed_tenant_ids.is_empty()
                    && command
                        .principal
                        .tenant_id
                        .as_ref()
                        .is_none_or(|tenant| !rule.allowed_tenant_ids.contains(tenant))
                {
                    reasons.push("policy.tenant_not_allowed".to_string());
                }
                if !rule.allowed_store_ids.is_empty()
                    && command
                        .store_id
                        .as_ref()
                        .is_none_or(|store| !rule.allowed_store_ids.contains(store))
                {
                    reasons.push("policy.store_not_allowed".to_string());
                }
                if rule.requires_agent_delegation
                    && command.principal.kind == PrincipalKind::Agent
                    && command.principal.delegated_by.as_deref().is_none_or(str::is_empty)
                {
                    reasons.push("policy.agent_delegation_required".to_string());
                }
                if rule.requires_signed_authority {
                    match &command.authority {
                        None => reasons.push("policy.signed_authority_required".to_string()),
                        Some(authority) => {
                            if authority.issued_at > now {
                                reasons.push("policy.authority_not_yet_valid".to_string());
                            }
                            if authority.expires_at <= now {
                                reasons.push("policy.authority_expired".to_string());
                            }
                            if command.principal.kind == PrincipalKind::Agent
                                && command.principal.delegated_by.as_deref()
                                    != Some(authority.issuer.as_str())
                            {
                                reasons.push("policy.authority_issuer_mismatch".to_string());
                            }
                            match self.trusted_authority_keys.get(&authority.key_id) {
                                None => reasons.push("policy.authority_key_untrusted".to_string()),
                                Some(public_key) => {
                                    if !verify_authority(command, authority, public_key) {
                                        reasons
                                            .push("policy.authority_signature_invalid".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                let held: BTreeSet<&str> =
                    command.principal.capabilities.iter().map(String::as_str).collect();
                for capability in &rule.required_capabilities {
                    if !held.contains(capability.as_str()) {
                        reasons.push(format!("policy.capability_missing:{capability}"));
                    }
                }
                if rule.requires_approval {
                    match &command.approval {
                        None => reasons.push("policy.approval_required".to_string()),
                        Some(approval) => {
                            if approval.approval_id.trim().is_empty() {
                                reasons.push("policy.approval_id_missing".to_string());
                            }
                            if approval.approved_by.trim().is_empty() {
                                reasons.push("policy.approver_missing".to_string());
                            }
                            if approval.scope != command.command_type {
                                reasons.push("policy.approval_scope_mismatch".to_string());
                            }
                            if approval.tenant_id != command.principal.tenant_id {
                                reasons.push("policy.approval_tenant_mismatch".to_string());
                            }
                            if approval.store_id != command.store_id {
                                reasons.push("policy.approval_store_mismatch".to_string());
                            }
                            if approval.idempotency_key.as_deref()
                                != Some(command.idempotency_key.as_str())
                            {
                                reasons.push("policy.approval_intent_mismatch".to_string());
                            }
                            if approval.approved_at > now {
                                reasons.push("policy.approval_not_yet_valid".to_string());
                            }
                            if approval.expires_at.is_some_and(|expires_at| expires_at <= now) {
                                reasons.push("policy.approval_expired".to_string());
                            }
                        }
                    }
                }
            }
        }

        PolicyDecisionEvidence {
            policy_version: self.version.clone(),
            decision_id: Uuid::new_v4().to_string(),
            allowed: reasons.is_empty(),
            reason_codes: reasons,
        }
    }
}

/// Canonical SHA-256 digest signed by command authorities.
pub fn authority_signing_hash<T: Serialize>(
    command: &CommandEnvelope<T>,
) -> Result<[u8; 32], KernelContractError> {
    let value = serde_json::json!({
        "contract_version": command.contract_version,
        "idempotency_key": command.idempotency_key,
        "command_type": command.command_type,
        "principal": command.principal,
        "store_id": command.store_id,
        "correlation_id": command.correlation_id,
        "causation_id": command.causation_id,
        "expected_version": command.expected_version,
        "policy_version": command.policy_version,
        "approval": command.approval,
        "deadline": command.deadline,
        "payload": command.payload,
        "issued_at": command.issued_at,
    });
    let canonical = stateset_crypto::canonicalize::canonicalize_json(&value)
        .map_err(|error| KernelContractError::Serialization(error.to_string()))?;
    Ok(Sha256::digest(canonical.as_bytes()).into())
}

fn verify_authority<T: Serialize>(
    command: &CommandEnvelope<T>,
    authority: &AuthorityEvidence,
    public_key_hex: &str,
) -> bool {
    let Ok(hash) = authority_signing_hash(command) else { return false };
    let Ok(public_key) = hex::decode(public_key_hex) else { return false };
    let Ok(public_key): Result<[u8; 32], _> = public_key.try_into() else { return false };
    let Ok(signature) = hex::decode(&authority.signature) else { return false };
    let Ok(signature): Result<[u8; 64], _> = signature.try_into() else { return false };
    stateset_crypto::sign::verify_event_signature(&hash, &signature, &public_key)
}

/// Durable, machine-readable outcome of a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt<T> {
    /// Wire-contract version.
    pub contract_version: String,
    /// Unique receipt identity.
    pub receipt_id: Uuid,
    /// Command this receipt answers.
    pub command_id: Uuid,
    /// Retry key copied from the command.
    pub idempotency_key: String,
    /// Stable namespaced command name.
    pub command_type: String,
    /// Outcome category.
    pub status: ExecutionStatus,
    /// Applied domain result. A preview may omit it when no aggregate exists yet.
    pub result: Option<T>,
    /// Stable error code, never parsed from prose.
    pub error_code: Option<String>,
    /// Human-readable diagnostic.
    pub error_message: Option<String>,
    /// Machine-readable retry instruction.
    pub retry: RetryDisposition,
    /// Affected aggregate category.
    pub aggregate_type: Option<String>,
    /// Affected aggregate identity.
    pub aggregate_id: Option<String>,
    /// Aggregate version observed before execution.
    pub version_before: Option<i32>,
    /// Aggregate version after execution.
    pub version_after: Option<i32>,
    /// Durable events committed atomically with the mutation.
    #[serde(default)]
    pub event_ids: Vec<Uuid>,
    /// Policy evidence used for this outcome.
    pub policy: Option<PolicyDecisionEvidence>,
    /// Optional hash anchoring the corresponding audit record.
    pub audit_hash: Option<String>,
    /// Time execution began.
    pub started_at: DateTime<Utc>,
    /// Time the outcome became final.
    pub completed_at: DateTime<Utc>,
}

impl<T> ExecutionReceipt<T> {
    /// Build a successful receipt from an applied command.
    pub fn succeeded(command: &CommandEnvelope<impl Sized>, result: T) -> Self {
        let now = Utc::now();
        Self {
            contract_version: KERNEL_CONTRACT_VERSION.into(),
            receipt_id: Uuid::new_v4(),
            command_id: command.command_id,
            idempotency_key: command.idempotency_key.clone(),
            command_type: command.command_type.clone(),
            status: ExecutionStatus::Succeeded,
            result: Some(result),
            error_code: None,
            error_message: None,
            retry: RetryDisposition::SameKey,
            aggregate_type: None,
            aggregate_id: None,
            version_before: None,
            version_after: None,
            event_ids: Vec::new(),
            policy: None,
            audit_hash: None,
            started_at: now,
            completed_at: now,
        }
    }
}

/// Invalid kernel envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KernelContractError {
    /// A required string/id was empty.
    MissingField(&'static str),
    /// The consumer does not support this wire version.
    UnsupportedVersion(String),
    /// Semantic command canonicalization failed.
    Serialization(String),
}

impl fmt::Display for KernelContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "kernel command is missing {field}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported kernel contract version: {version}")
            }
            Self::Serialization(message) => {
                write!(f, "kernel command serialization failed: {message}")
            }
        }
    }
}

impl std::error::Error for KernelContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> KernelPrincipal {
        KernelPrincipal {
            id: "agent:buyer-7".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-42".into()),
            capabilities: vec!["payments.create".into()],
        }
    }

    #[test]
    fn commands_default_to_preview_and_require_a_retry_key() {
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        assert_eq!(command.mode, ExecutionMode::Preview);
        assert!(command.validate_contract().is_ok());

        command.idempotency_key.clear();
        assert_eq!(
            command.validate_contract(),
            Err(KernelContractError::MissingField("idempotency_key"))
        );
    }

    #[test]
    fn receipt_preserves_command_identity_across_json() {
        let command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        let receipt = ExecutionReceipt::succeeded(&command, "pay_123".to_string());
        let json = serde_json::to_string(&receipt).expect("receipt should serialize");
        let decoded: ExecutionReceipt<String> =
            serde_json::from_str(&json).expect("receipt should deserialize");

        assert_eq!(decoded.command_id, command.command_id);
        assert_eq!(decoded.idempotency_key, command.idempotency_key);
        assert_eq!(decoded.status, ExecutionStatus::Succeeded);
    }

    #[test]
    fn policy_is_deny_by_default_and_checks_capability_version_and_approval() {
        let policy = KernelPolicy::new("policy-2").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"]).with_approval(),
        );
        let now = Utc::now();
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.policy_version = Some("policy-1".into());
        let denied = policy.evaluate(&command, now);
        assert!(!denied.allowed);
        assert!(denied.reason_codes.contains(&"policy.version_conflict".to_string()));
        assert!(denied.reason_codes.contains(&"policy.approval_required".to_string()));

        command.policy_version = Some("policy-2".into());
        command.approval = Some(ApprovalEvidence {
            approval_id: "approval-1".into(),
            approved_by: "user-42".into(),
            scope: "payments.create".into(),
            tenant_id: Some("tenant-1".into()),
            store_id: Some("store-1".into()),
            idempotency_key: Some("retry-1".into()),
            approved_at: now,
            expires_at: None,
        });
        assert!(policy.evaluate(&command, now).allowed);

        let mut wrong_store = command.clone();
        wrong_store.store_id = Some("store-2".into());
        assert!(
            policy
                .evaluate(&wrong_store, now)
                .reason_codes
                .contains(&"policy.approval_store_mismatch".to_string())
        );

        let mut unscoped = command.clone();
        unscoped.store_id = None;
        unscoped.principal.tenant_id = None;
        unscoped.principal.delegated_by = None;
        let denied = policy.evaluate(&unscoped, now);
        assert!(denied.reason_codes.contains(&"policy.store_required".to_string()));
        assert!(denied.reason_codes.contains(&"policy.tenant_required".to_string()));
        assert!(denied.reason_codes.contains(&"policy.agent_delegation_required".to_string()));

        command.command_type = "ledger.post".into();
        assert!(
            policy
                .evaluate(&command, now)
                .reason_codes
                .contains(&"policy.command_not_allowed".to_string())
        );
    }

    #[test]
    fn policy_binds_commands_to_explicit_tenant_and_store_allowlists() {
        let policy = KernelPolicy::new("policy-tenant-scope").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"])
                .for_tenants(["tenant-1"])
                .for_stores(["store-1"]),
        );
        let now = Utc::now();
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        assert!(policy.evaluate(&command, now).allowed);

        command.principal.tenant_id = Some("tenant-2".into());
        command.store_id = Some("store-2".into());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.tenant_not_allowed".to_string()));
        assert!(denied.reason_codes.contains(&"policy.store_not_allowed".to_string()));
    }

    #[test]
    fn signed_authority_is_bound_to_the_semantic_intent() {
        let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
        let policy = KernelPolicy::new("policy-1")
            .allow(
                "payments.create",
                KernelCommandPolicy::requiring(["payments.create"]).with_signed_authority(),
            )
            .with_trusted_authority_key("delegator-key-1", public_key);
        let now = Utc::now();
        let mut command =
            CommandEnvelope::preview("payments.create", "retry-signed-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.policy_version = Some("policy-1".into());
        let hash = authority_signing_hash(&command).expect("canonical intent");
        let signature = stateset_crypto::sign::sign_event_hash(&hash, &private_key).expect("sign");
        command.authority = Some(AuthorityEvidence {
            issuer: "user-42".into(),
            key_id: "delegator-key-1".into(),
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(5),
            signature: hex::encode(signature),
        });
        assert!(policy.evaluate(&command, now).allowed);

        command.payload = 43;
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.authority_signature_invalid".to_string()));
    }
}
