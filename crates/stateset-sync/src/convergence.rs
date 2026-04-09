use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::commitment::VerifiedCommitmentManifest;
use crate::engine::{KernelReceipt, KernelReceiptStatus};
use crate::state::SyncState;

/// Counterparty convergence state for a logical command across local and remote systems.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CounterpartyConvergenceStatus {
    /// One or more events are still only present in the local outbox.
    LocalPending,
    /// All observed events were confirmed by the remote sequencer, but no commitment proof is known yet.
    ConfirmedRemote,
    /// A remote commitment is known, but the local pull cursor has not fully observed it yet.
    CommittedRemote,
    /// The command is both committed remotely and fully observed by the local pull cursor.
    Settled,
    /// One or more events were explicitly rejected by the remote sequencer.
    RejectedRemote,
}

/// Snapshot of the remote commitment information currently known for a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterpartyCommitment {
    /// Highest canonical remote sequence currently known on the remote sequencer.
    pub remote_head: u64,
    /// Highest canonical remote sequence fully observed by the local cursor.
    pub remote_cursor: u64,
    /// Optional remote state root associated with the known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_root: Option<String>,
    /// Optional commitment identifier associated with the known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_id: Option<String>,
    /// Optional signer identity from a verified commitment manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_id: Option<String>,
    /// Optional local verification timestamp for the commitment manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_verified_at: Option<DateTime<Utc>>,
}

/// Command-level convergence snapshot derived from retained kernel receipts and remote sync state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandConvergence {
    /// Logical upstream command identifier.
    pub command_id: String,
    /// Current convergence status for the command.
    pub status: CounterpartyConvergenceStatus,
    /// Number of local-pending event receipts for this command.
    pub pending_receipts: usize,
    /// Number of confirmed-remote event receipts for this command.
    pub confirmed_receipts: usize,
    /// Number of rejected-remote event receipts for this command.
    pub rejected_receipts: usize,
    /// Highest provisional local sequence retained for this command, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_local_sequence: Option<u64>,
    /// Highest canonical remote sequence retained for this command, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_remote_sequence: Option<u64>,
    /// Any remote receipt handles associated with confirmed command events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_receipts: Vec<String>,
    /// Any rejection codes associated with rejected command events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejection_codes: Vec<String>,
    /// Any rejection reasons associated with rejected command events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejection_reasons: Vec<String>,
    /// Remote commitment metadata currently known for the command, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<CounterpartyCommitment>,
    /// Underlying event-level receipts used to derive the convergence state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<KernelReceipt>,
}

impl CommandConvergence {
    /// Build a convergence snapshot for a command from its retained receipts and current sync state.
    #[must_use]
    pub(crate) fn from_receipts(
        command_id: impl Into<String>,
        mut receipts: Vec<KernelReceipt>,
        state: &SyncState,
        verified_manifest: Option<&VerifiedCommitmentManifest>,
    ) -> Self {
        receipts.sort_by_key(KernelReceipt::ordering_key);

        let pending_receipts = receipts
            .iter()
            .filter(|receipt| receipt.status == KernelReceiptStatus::LocalPending)
            .count();
        let confirmed_receipts = receipts
            .iter()
            .filter(|receipt| receipt.status == KernelReceiptStatus::ConfirmedRemote)
            .count();
        let rejected_receipts = receipts
            .iter()
            .filter(|receipt| receipt.status == KernelReceiptStatus::RejectedRemote)
            .count();

        let max_local_sequence = receipts.iter().filter_map(|receipt| receipt.local_sequence).max();
        let max_remote_sequence =
            receipts.iter().filter_map(|receipt| receipt.remote_sequence).max();

        let mut remote_receipts = Vec::new();
        let mut rejection_codes = Vec::new();
        let mut rejection_reasons = Vec::new();
        for receipt in &receipts {
            if let Some(remote_receipt) = receipt.remote_receipt.as_ref() {
                if !remote_receipts.contains(remote_receipt) {
                    remote_receipts.push(remote_receipt.clone());
                }
            }
            if let Some(code) = receipt.rejection_code.as_ref() {
                if !rejection_codes.contains(code) {
                    rejection_codes.push(code.clone());
                }
            }
            if let Some(reason) = receipt.rejection_reason.as_ref() {
                if !rejection_reasons.contains(reason) {
                    rejection_reasons.push(reason.clone());
                }
            }
        }

        let commitment = max_remote_sequence.and_then(|max_remote_sequence| {
            if state.remote_head < max_remote_sequence {
                return None;
            }
            if state.remote_state_root.is_none() && state.last_commitment_id.is_none() {
                return None;
            }
            Some(CounterpartyCommitment {
                remote_head: state.remote_head,
                remote_cursor: state.remote_cursor,
                state_root: state.remote_state_root.clone(),
                commitment_id: state.last_commitment_id.clone(),
                signer_id: verified_manifest.map(|manifest| manifest.signer_id.clone()),
                manifest_verified_at: verified_manifest.map(|manifest| manifest.verified_at),
            })
        });

        let status = if rejected_receipts > 0 {
            CounterpartyConvergenceStatus::RejectedRemote
        } else if pending_receipts > 0 {
            CounterpartyConvergenceStatus::LocalPending
        } else if let Some(max_remote_sequence) = max_remote_sequence {
            if commitment.is_some() && state.remote_cursor >= max_remote_sequence {
                CounterpartyConvergenceStatus::Settled
            } else if commitment.is_some() {
                CounterpartyConvergenceStatus::CommittedRemote
            } else {
                CounterpartyConvergenceStatus::ConfirmedRemote
            }
        } else {
            CounterpartyConvergenceStatus::LocalPending
        };

        Self {
            command_id: command_id.into(),
            status,
            pending_receipts,
            confirmed_receipts,
            rejected_receipts,
            max_local_sequence,
            max_remote_sequence,
            remote_receipts,
            rejection_codes,
            rejection_reasons,
            commitment,
            receipts,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    fn receipt(
        event_id: Uuid,
        status: KernelReceiptStatus,
        local_sequence: Option<u64>,
        remote_sequence: Option<u64>,
    ) -> KernelReceipt {
        KernelReceipt {
            event_id,
            status,
            command_id: Some("cmd-1".into()),
            event_type: "order.created".into(),
            entity_type: "order".into(),
            entity_id: "ORD-1".into(),
            local_sequence,
            remote_sequence,
            hash: "abc".into(),
            source_agent_id: None,
            kernel: None,
            event_timestamp: Utc::now(),
            observed_at: Utc::now(),
            remote_receipt: None,
            rejection_code: None,
            rejection_reason: None,
            retryable: None,
        }
    }

    #[test]
    fn convergence_reports_settled_when_commitment_and_cursor_cover_remote_sequence() {
        let convergence = CommandConvergence::from_receipts(
            "cmd-1",
            vec![receipt(Uuid::new_v4(), KernelReceiptStatus::ConfirmedRemote, Some(1), Some(9))],
            &SyncState {
                remote_head: 12,
                remote_cursor: 9,
                remote_state_root: Some("root-12".into()),
                last_commitment_id: Some("BATCH-12".into()),
                ..Default::default()
            },
            None,
        );

        assert_eq!(convergence.status, CounterpartyConvergenceStatus::Settled);
        assert_eq!(
            convergence
                .commitment
                .as_ref()
                .and_then(|commitment| commitment.commitment_id.as_deref()),
            Some("BATCH-12")
        );
    }

    #[test]
    fn convergence_carries_verified_manifest_signer_metadata() {
        let verified_at = Utc::now();
        let convergence = CommandConvergence::from_receipts(
            "cmd-1",
            vec![receipt(Uuid::new_v4(), KernelReceiptStatus::ConfirmedRemote, Some(1), Some(9))],
            &SyncState {
                remote_head: 12,
                remote_cursor: 9,
                remote_state_root: Some("root-12".into()),
                last_commitment_id: Some("BATCH-12".into()),
                ..Default::default()
            },
            Some(&VerifiedCommitmentManifest {
                commitment_id: "BATCH-12".into(),
                previous_commitment_id: Some("BATCH-11".into()),
                state_root: "root-12".into(),
                remote_head: 12,
                signer_id: "sequencer-a".into(),
                signature_scheme: "ed25519".into(),
                signer_public_key: "33".repeat(32),
                signature: "44".repeat(64),
                issued_at: verified_at,
                verified_at,
            }),
        );

        assert_eq!(
            convergence.commitment.as_ref().and_then(|commitment| commitment.signer_id.as_deref()),
            Some("sequencer-a")
        );
        assert_eq!(
            convergence.commitment.as_ref().and_then(|commitment| commitment.manifest_verified_at),
            Some(verified_at)
        );
    }
}
