//! Durable event records emitted atomically with commerce mutations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use stateset_core::{CommandEnvelope, CommerceError, ExecutionStatus, Result};
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
    /// Opaque token for this claim, including a worker label and random nonce.
    /// Pass the returned value to settlement APIs; a worker label alone is insufficient.
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub dead_lettered_at: Option<DateTime<Utc>>,
    /// Which tier produced this fact: `governed` (a kernel command, with
    /// policy, budget and a sealed receipt) or `recorded` (an ordinary
    /// mutation, fact only).
    pub tier: String,
}

/// A fact emitted by an ordinary mutation, carrying no governance metadata.
#[derive(Debug, Clone)]
pub struct RecordedFact<'a> {
    pub event_type: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub payload: Value,
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
///
/// `valid` covers two things: every chain link recomputes, and the
/// materialized `kernel_receipts` index corresponds exactly to the chain (see
/// [`KernelAuditFailure`] for the rule). On failure `first_invalid_sequence`
/// names the audit entry concerned — `Some(0)` when the failure is a
/// materialized row tied to no sealed entry — and `failure` says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelAuditVerification {
    pub valid: bool,
    /// Entries walked. On a broken link this counts up to and including the
    /// broken entry; otherwise it is the length of the whole chain.
    pub entries: u64,
    /// Hash of the last entry whose link verified.
    pub head_hash: Option<String>,
    pub first_invalid_sequence: Option<i64>,
    /// Typed reason for the first failure; `None` when `valid`. Absent from
    /// documents serialized before typed reasons existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<KernelAuditFailure>,
}

/// A receipt field a verification failure is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum KernelAuditField {
    ReceiptId,
    CommandId,
    IdempotencyKey,
    CommandType,
    ContractVersion,
    RequestHash,
    Status,
    /// The `audit_hash` a materialized receipt carries, which must name the
    /// newest sealed entry for its idempotency key.
    AuditHash,
    /// The full receipt document.
    Receipt,
}

/// Why the receipt audit chain, or the materialized index built from it,
/// failed verification.
///
/// # The correspondence rule
///
/// Every governed append (`append_kernel_receipt_tx`) writes one sealed
/// audit entry and, in the same transaction, upserts the `kernel_receipts`
/// row for that receipt's idempotency key. The audit entry stores the receipt
/// as hashed (`audit_hash: null`); the row stores the same receipt with
/// `audit_hash` set to the entry's hash. That happens for applied, previewed
/// and rejected receipts alike. A replay, an idempotency-conflict rejection,
/// a tampered-receipt refusal and a rolled-back transaction append nothing.
///
/// A key can therefore gain a second entry in exactly one way: a durable
/// preview promoted by an apply under the same key and request (the upsert
/// overwrites the preview row; promotion never re-previews, and every receipt
/// after a non-preview is a replay). So, per idempotency key, the sealed
/// entries are either one receipt, or a `previewed` receipt followed by one
/// non-`previewed` receipt with the same request hash and command type. The
/// newest entry must have exactly one row agreeing with it on key, request
/// hash, command id and type, contract version, status and full receipt; a
/// superseded preview has none; and every row must be that newest-entry row
/// for its key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
#[non_exhaustive]
pub enum KernelAuditFailure {
    /// The sealed receipt is not a JSON document.
    MalformedEntry { sequence: i64 },
    /// `previous_audit_hash` or `audit_hash` does not recompute.
    BrokenLink { sequence: i64 },
    /// An indexed audit-log column (outside the hash) disagrees with the
    /// receipt it seals, or the sealed receipt lacks a field the rule needs.
    InvalidEntryField { sequence: i64, field: KernelAuditField },
    /// A second entry for an idempotency key that is not the one promotion a
    /// preview allows: the key executed again (for example after its
    /// materialized receipt was deleted) or a preview followed a terminal
    /// receipt.
    InvalidSupersession { sequence: i64, superseded_sequence: i64, idempotency_key: String },
    /// A preview was promoted by an entry bound to a different request.
    SupersessionMismatch {
        sequence: i64,
        superseded_sequence: i64,
        idempotency_key: String,
        field: KernelAuditField,
    },
    /// The newest entry for a key has no materialized receipt, so a retry
    /// under that key would execute again instead of replaying.
    MissingMaterializedReceipt { sequence: i64, idempotency_key: String },
    /// More than one materialized receipt for the newest entry of a key.
    DuplicateMaterializedReceipt { sequence: i64, idempotency_key: String },
    /// The materialized receipt for a key is a superseded preview, not the
    /// newest entry: promotion could run again.
    StaleMaterializedReceipt { sequence: i64, materialized_sequence: i64, idempotency_key: String },
    /// The materialized receipt for the entry at `sequence` (whose key is
    /// `idempotency_key`) disagrees with it on `field`. `IdempotencyKey`
    /// means the row was filed under another key (re-keyed).
    MaterializedReceiptMismatch { sequence: i64, idempotency_key: String, field: KernelAuditField },
    /// A materialized receipt whose key has no sealed entry and whose
    /// `audit_hash` names none.
    OrphanMaterializedReceipt { idempotency_key: String, claimed_audit_hash: Option<String> },
}

impl KernelAuditFailure {
    /// The audit sequence this failure concerns, if it concerns one.
    #[must_use]
    pub const fn sequence(&self) -> Option<i64> {
        match self {
            Self::MalformedEntry { sequence }
            | Self::BrokenLink { sequence }
            | Self::InvalidEntryField { sequence, .. }
            | Self::InvalidSupersession { sequence, .. }
            | Self::SupersessionMismatch { sequence, .. }
            | Self::MissingMaterializedReceipt { sequence, .. }
            | Self::DuplicateMaterializedReceipt { sequence, .. }
            | Self::StaleMaterializedReceipt { sequence, .. }
            | Self::MaterializedReceiptMismatch { sequence, .. } => Some(*sequence),
            Self::OrphanMaterializedReceipt { .. } => None,
        }
    }
}

/// One `kernel_receipt_audit_log` row as a backend read it.
pub(crate) struct AuditEntryRow {
    pub(crate) sequence: i64,
    pub(crate) receipt_id: String,
    pub(crate) command_id: String,
    pub(crate) idempotency_key: String,
    pub(crate) request_hash: String,
    pub(crate) previous_audit_hash: Option<String>,
    pub(crate) audit_hash: String,
    /// `None` when the stored document does not parse as JSON.
    pub(crate) receipt: Option<Value>,
}

/// One `kernel_receipts` row, joined to the audit entry whose `audit_hash`
/// equals the row receipt's `audit_hash`, if any.
pub(crate) struct MaterializedReceiptRow {
    pub(crate) command_id: String,
    pub(crate) idempotency_key: String,
    pub(crate) command_type: String,
    pub(crate) contract_version: String,
    pub(crate) request_hash: String,
    pub(crate) status: String,
    /// `None` when the stored document does not parse as JSON.
    pub(crate) receipt: Option<Value>,
    pub(crate) claimed: Option<ClaimedAuditEntry>,
}

/// The audit entry a materialized receipt names through its `audit_hash`.
pub(crate) struct ClaimedAuditEntry {
    pub(crate) sequence: i64,
    pub(crate) audit_hash: String,
    pub(crate) request_hash: String,
    pub(crate) receipt: Option<Value>,
}

/// Newest sealed entry seen for one idempotency key.
struct KeyHistory {
    sequence: i64,
    previewed: bool,
    request_hash: String,
    command_type: String,
    materialized: bool,
}

/// Rank of a failure tied to no audit entry: reported after every other one.
const UNSEQUENCED_RANK: i64 = i64::MAX;

/// Backend-neutral receipt audit verification.
///
/// Backends feed every audit entry in ascending `sequence` order through
/// [`Self::entry`], then — when the chain is intact — every materialized
/// receipt through [`Self::materialized`], all from one read snapshot, and
/// call [`Self::finish`]. A broken link stops the walk and is reported as
/// before; otherwise the earliest correspondence failure (by audit sequence,
/// unsequenced orphans last) is reported. See [`KernelAuditFailure`].
#[derive(Default)]
pub(crate) struct ReceiptAuditVerifier {
    previous: Option<String>,
    entries: u64,
    keys: std::collections::HashMap<String, KeyHistory>,
    broken: Option<KernelAuditFailure>,
    failure: Option<(i64, KernelAuditFailure)>,
}

impl ReceiptAuditVerifier {
    /// Verify the next entry's link and record its key. Returns `false` once
    /// the chain is broken, after which callers stop feeding entries.
    pub(crate) fn entry(&mut self, row: AuditEntryRow) -> Result<bool> {
        if self.broken.is_some() {
            return Ok(false);
        }
        self.entries += 1;
        let Some(receipt) = row.receipt.as_ref() else {
            self.broken = Some(KernelAuditFailure::MalformedEntry { sequence: row.sequence });
            return Ok(false);
        };
        let computed = receipt_audit_hash(self.previous.as_deref(), &row.request_hash, receipt)
            .map_err(CommerceError::DatabaseError)?;
        if row.previous_audit_hash != self.previous || row.audit_hash != computed {
            self.broken = Some(KernelAuditFailure::BrokenLink { sequence: row.sequence });
            return Ok(false);
        }
        self.previous = Some(row.audit_hash.clone());
        if let Some(sealed) = self.sealed_fields(&row, receipt) {
            self.track_key(&row, sealed);
        }
        Ok(true)
    }

    /// Whether every link verified so far, so materialized rows are worth
    /// checking.
    pub(crate) const fn chain_intact(&self) -> bool {
        self.broken.is_none()
    }

    /// Check one materialized receipt against the newest entry for its key.
    pub(crate) fn materialized(&mut self, row: MaterializedReceiptRow) {
        let claimed_key = row
            .claimed
            .as_ref()
            .and_then(|claimed| claimed.receipt.as_ref())
            .and_then(|receipt| receipt.get("idempotency_key"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(history) = self.keys.get_mut(&row.idempotency_key) else {
            let failure = match (&row.claimed, claimed_key) {
                // The row names a sealed entry filed under another key.
                (Some(claimed), Some(entry_key)) => (
                    claimed.sequence,
                    KernelAuditFailure::MaterializedReceiptMismatch {
                        sequence: claimed.sequence,
                        idempotency_key: entry_key,
                        field: KernelAuditField::IdempotencyKey,
                    },
                ),
                _ => (
                    UNSEQUENCED_RANK,
                    KernelAuditFailure::OrphanMaterializedReceipt {
                        claimed_audit_hash: row
                            .receipt
                            .as_ref()
                            .and_then(|receipt| receipt.get("audit_hash"))
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        idempotency_key: row.idempotency_key,
                    },
                ),
            };
            self.record(failure.0, failure.1);
            return;
        };
        let expected = history.sequence;
        let key = row.idempotency_key.clone();
        if history.materialized {
            self.record(
                expected,
                KernelAuditFailure::DuplicateMaterializedReceipt {
                    sequence: expected,
                    idempotency_key: key,
                },
            );
            return;
        }
        history.materialized = true;
        let mismatch = |field| KernelAuditFailure::MaterializedReceiptMismatch {
            sequence: expected,
            idempotency_key: key.clone(),
            field,
        };
        let Some(claimed) = row.claimed.as_ref() else {
            self.record(expected, mismatch(KernelAuditField::AuditHash));
            return;
        };
        if claimed.sequence != expected {
            let failure = if claimed.sequence < expected && claimed_key.as_deref() == Some(&key) {
                KernelAuditFailure::StaleMaterializedReceipt {
                    sequence: expected,
                    materialized_sequence: claimed.sequence,
                    idempotency_key: key.clone(),
                }
            } else {
                mismatch(KernelAuditField::AuditHash)
            };
            self.record(expected, failure);
            return;
        }
        if let Some(field) = materialized_mismatch(&row, claimed) {
            self.record(expected, mismatch(field));
        }
    }

    /// Report the outcome. Unmaterialized newest entries are found here.
    pub(crate) fn finish(mut self) -> KernelAuditVerification {
        if let Some(failure) = self.broken.take() {
            return KernelAuditVerification {
                valid: false,
                entries: self.entries,
                head_hash: self.previous,
                first_invalid_sequence: failure.sequence(),
                failure: Some(failure),
            };
        }
        let missing: Vec<(i64, String)> = self
            .keys
            .iter()
            .filter(|(_, history)| !history.materialized)
            .map(|(key, history)| (history.sequence, key.clone()))
            .collect();
        for (sequence, idempotency_key) in missing {
            self.record(
                sequence,
                KernelAuditFailure::MissingMaterializedReceipt { sequence, idempotency_key },
            );
        }
        match self.failure {
            None => KernelAuditVerification {
                valid: true,
                entries: self.entries,
                head_hash: self.previous,
                first_invalid_sequence: None,
                failure: None,
            },
            Some((_, failure)) => KernelAuditVerification {
                valid: false,
                entries: self.entries,
                head_hash: self.previous,
                first_invalid_sequence: Some(failure.sequence().unwrap_or(0)),
                failure: Some(failure),
            },
        }
    }

    /// Keep the earliest failure; on a tie the first recorded wins.
    fn record(&mut self, rank: i64, failure: KernelAuditFailure) {
        if self.failure.as_ref().is_none_or(|(current, _)| rank < *current) {
            self.failure = Some((rank, failure));
        }
    }

    /// The sealed fields the correspondence rule keys on, after checking the
    /// unhashed indexed columns against them. `None` when the entry cannot be
    /// keyed at all (a failure is recorded).
    fn sealed_fields(&mut self, row: &AuditEntryRow, receipt: &Value) -> Option<SealedFields> {
        let text = |name: &str| receipt.get(name).and_then(Value::as_str);
        let invalid =
            |field| KernelAuditFailure::InvalidEntryField { sequence: row.sequence, field };
        let Some(key) = text("idempotency_key") else {
            self.record(row.sequence, invalid(KernelAuditField::IdempotencyKey));
            return None;
        };
        let checks = [
            (KernelAuditField::IdempotencyKey, key == row.idempotency_key),
            (KernelAuditField::ReceiptId, same_uuid(text("receipt_id"), &row.receipt_id)),
            (KernelAuditField::CommandId, same_uuid(text("command_id"), &row.command_id)),
        ];
        if let Some((field, _)) = checks.iter().find(|(_, ok)| !ok) {
            self.record(row.sequence, invalid(*field));
        }
        let Some(status) = sealed_status(receipt) else {
            self.record(row.sequence, invalid(KernelAuditField::Status));
            return None;
        };
        let Some(command_type) = text("command_type") else {
            self.record(row.sequence, invalid(KernelAuditField::CommandType));
            return None;
        };
        Some(SealedFields {
            key: key.to_owned(),
            previewed: status == ExecutionStatus::Previewed,
            command_type: command_type.to_owned(),
        })
    }

    /// Apply the per-key supersession rule and remember the newest entry.
    fn track_key(&mut self, row: &AuditEntryRow, sealed: SealedFields) {
        let SealedFields { key, previewed, command_type } = sealed;
        let failure = self.keys.get(&key).and_then(|previous| {
            let mismatch = |field| KernelAuditFailure::SupersessionMismatch {
                sequence: row.sequence,
                superseded_sequence: previous.sequence,
                idempotency_key: key.clone(),
                field,
            };
            if !previous.previewed || previewed {
                Some(KernelAuditFailure::InvalidSupersession {
                    sequence: row.sequence,
                    superseded_sequence: previous.sequence,
                    idempotency_key: key.clone(),
                })
            } else if previous.request_hash != row.request_hash {
                Some(mismatch(KernelAuditField::RequestHash))
            } else if previous.command_type != command_type {
                Some(mismatch(KernelAuditField::CommandType))
            } else {
                None
            }
        });
        if let Some(failure) = failure {
            self.record(row.sequence, failure);
        }
        self.keys.insert(
            key,
            KeyHistory {
                sequence: row.sequence,
                previewed,
                request_hash: row.request_hash.clone(),
                command_type,
                materialized: false,
            },
        );
    }
}

struct SealedFields {
    key: String,
    previewed: bool,
    command_type: String,
}

fn sealed_status(receipt: &Value) -> Option<ExecutionStatus> {
    receipt.get("status").cloned().and_then(|status| serde_json::from_value(status).ok())
}

/// Compare identifiers by value, so a backend's UUID rendering cannot matter.
fn same_uuid(sealed: Option<&str>, column: &str) -> bool {
    match (sealed.map(Uuid::parse_str), Uuid::parse_str(column)) {
        (Some(Ok(sealed)), Ok(column)) => sealed == column,
        _ => false,
    }
}

/// The first field on which a materialized receipt disagrees with the entry
/// it is the newest row for.
fn materialized_mismatch(
    row: &MaterializedReceiptRow,
    claimed: &ClaimedAuditEntry,
) -> Option<KernelAuditField> {
    let Some(entry) = claimed.receipt.as_ref() else {
        return Some(KernelAuditField::Receipt);
    };
    let text = |name: &str| entry.get(name).and_then(Value::as_str);
    if row.request_hash != claimed.request_hash {
        return Some(KernelAuditField::RequestHash);
    }
    if !same_uuid(text("command_id"), &row.command_id) {
        return Some(KernelAuditField::CommandId);
    }
    if text("command_type") != Some(row.command_type.as_str()) {
        return Some(KernelAuditField::CommandType);
    }
    if text("contract_version") != Some(row.contract_version.as_str()) {
        return Some(KernelAuditField::ContractVersion);
    }
    if sealed_status(entry).map(crate::kernel::receipt::status_name) != Some(row.status.as_str()) {
        return Some(KernelAuditField::Status);
    }
    let mut sealed = entry.clone();
    if let Some(object) = sealed.as_object_mut() {
        object.insert("audit_hash".into(), Value::String(claimed.audit_hash.clone()));
    }
    if row.receipt.as_ref() != Some(&sealed) {
        return Some(KernelAuditField::Receipt);
    }
    None
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
    let mut value = serde_json::json!({
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
    if let serde_json::Value::Object(fields) = &mut value {
        if let Some(mandate) = &command.mandate {
            fields.insert(
                "mandate".into(),
                serde_json::to_value(mandate)
                    .map_err(|error| CommerceError::ValidationError(error.to_string()))?,
            );
        }
        if let Some(commitment) = &command.commitment {
            fields.insert(
                "commitment".into(),
                serde_json::to_value(commitment)
                    .map_err(|error| CommerceError::ValidationError(error.to_string()))?,
            );
        }
    }
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
            tier: "governed".to_string(),
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
