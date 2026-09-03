//! Verified replay of durable receipts.
//!
//! A stored receipt is only authoritative when it still matches the sealed
//! audit-log entry it claims. Backends load the entry whose `audit_hash`
//! equals the materialized receipt's `audit_hash`; this module recomputes the
//! chain link and refuses tampered rows with
//! [`CommerceError::KernelReceiptTampered`] (`kernel.receipt_tampered`).

use crate::KernelReceiptRecord;
use crate::kernel::receipt::rejected_receipt;
use crate::kernel_outbox::receipt_audit_hash;
use serde::de::DeserializeOwned;
use serde_json::Value;
use stateset_core::{
    CommandEnvelope, CommerceError, ExecutionMode, ExecutionReceipt, ExecutionStatus,
    RetryDisposition,
};

/// The audit-log row a materialized receipt claims through its `audit_hash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedAuditEntry {
    pub previous_audit_hash: Option<String>,
    pub request_hash: String,
}

/// Outcome of consulting an existing receipt for the same idempotency key.
#[derive(Debug)]
// `Return` is much larger than `Promote`, but a `Replay` is built once per
// command and matched immediately, so boxing would only add an allocation to
// the replay path without shrinking anything that is kept.
#[allow(clippy::large_enum_variant)]
pub enum Replay<T> {
    /// Return this receipt without executing anything.
    Return(ExecutionReceipt<T>),
    /// A durable preview exists and the caller asked to apply: continue.
    Promote,
}

/// Recompute the audit link for a materialized receipt.
pub fn verify_sealed_receipt(
    existing: &KernelReceiptRecord,
    audit: Option<&SealedAuditEntry>,
) -> Result<(), CommerceError> {
    let claimed = existing.receipt.get("audit_hash").and_then(Value::as_str).map(str::to_owned);
    let tampered = || CommerceError::KernelReceiptTampered {
        idempotency_key: existing.idempotency_key.clone(),
        audit_hash: claimed.clone(),
    };
    let (Some(claimed_hash), Some(audit)) = (claimed.as_deref(), audit) else {
        return Err(tampered());
    };
    if audit.request_hash != existing.request_hash {
        return Err(tampered());
    }
    let mut unsealed = existing.receipt.clone();
    if let Some(object) = unsealed.as_object_mut() {
        object.insert("audit_hash".into(), Value::Null);
    }
    let recomputed =
        receipt_audit_hash(audit.previous_audit_hash.as_deref(), &existing.request_hash, &unsealed)
            .map_err(CommerceError::DatabaseError)?;
    if recomputed != claimed_hash {
        return Err(tampered());
    }
    Ok(())
}

/// Verify, then decide between replay, conflict, and preview promotion.
pub fn resolve_replay<C, T: DeserializeOwned>(
    command: &CommandEnvelope<C>,
    request_hash: &str,
    existing: KernelReceiptRecord,
    audit: Option<&SealedAuditEntry>,
    aggregate_type: &str,
) -> Result<Replay<T>, CommerceError> {
    verify_sealed_receipt(&existing, audit)?;
    if existing.request_hash != request_hash {
        return Ok(Replay::Return(rejected_receipt(
            command,
            None,
            "kernel.idempotency_conflict",
            "idempotency key is already bound to a different semantic request",
            RetryDisposition::Never,
            aggregate_type,
        )));
    }
    let stored: ExecutionReceipt<T> = serde_json::from_value(existing.receipt)
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    if stored.status == ExecutionStatus::Previewed && command.mode == ExecutionMode::Apply {
        Ok(Replay::Promote)
    } else {
        Ok(Replay::Return(stored))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn sealed_record() -> (KernelReceiptRecord, SealedAuditEntry) {
        let receipt = serde_json::json!({
            "receipt_id": Uuid::new_v4(),
            "status": "succeeded",
            "audit_hash": null,
        });
        let audit = SealedAuditEntry { previous_audit_hash: None, request_hash: "h".into() };
        let hash = receipt_audit_hash(None, "h", &receipt).expect("hash");
        let mut sealed = receipt;
        sealed["audit_hash"] = Value::String(hash);
        let now = Utc::now();
        (
            KernelReceiptRecord {
                command_id: Uuid::new_v4(),
                idempotency_key: "k".into(),
                command_type: "payments.create".into(),
                contract_version: "1.0".into(),
                request_hash: "h".into(),
                status: "succeeded".into(),
                receipt: sealed,
                created_at: now,
                completed_at: now,
            },
            audit,
        )
    }

    #[test]
    fn intact_receipt_verifies_and_tampered_fields_are_refused() {
        let (record, audit) = sealed_record();
        verify_sealed_receipt(&record, Some(&audit)).expect("intact");

        let mut edited = record.clone();
        edited.receipt["status"] = Value::String("rejected".into());
        assert!(matches!(
            verify_sealed_receipt(&edited, Some(&audit)),
            Err(CommerceError::KernelReceiptTampered { .. })
        ));

        assert!(matches!(
            verify_sealed_receipt(&record, None),
            Err(CommerceError::KernelReceiptTampered { .. })
        ));

        let mut rebound = record.clone();
        rebound.request_hash = "other".into();
        assert!(matches!(
            verify_sealed_receipt(&rebound, Some(&audit)),
            Err(CommerceError::KernelReceiptTampered { .. })
        ));

        let mut unsealed = record;
        unsealed.receipt["audit_hash"] = Value::Null;
        assert!(matches!(
            verify_sealed_receipt(&unsealed, Some(&audit)),
            Err(CommerceError::KernelReceiptTampered { .. })
        ));
    }
}
