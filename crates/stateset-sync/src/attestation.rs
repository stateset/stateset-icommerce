use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stateset_crypto::merkle::compute_node_hash;
use stateset_crypto::u64_be;
use thiserror::Error;

use crate::engine::{KernelReceipt, KernelReceiptStatus};
use crate::state::SyncState;

const COMMAND_ATTESTATION_DOMAIN: &[u8] = b"STATESET_COMMAND_ATTEST_V1";

/// Inclusion proof supplied by a counterparty or sequencer for a command settlement leaf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandInclusionProof {
    /// Logical command identifier being attested.
    pub command_id: String,
    /// Merkle root or state root that anchors the proof.
    pub state_root: String,
    /// Optional commitment identifier associated with the proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_id: Option<String>,
    /// Leaf index within the committed Merkle tree.
    pub leaf_index: u32,
    /// Total leaves represented by the committed Merkle tree.
    pub total_leaves: u32,
    /// Hex-encoded sibling hashes needed for inclusion verification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_hashes: Vec<String>,
}

impl CommandInclusionProof {
    /// Create a new inclusion proof for a command.
    #[must_use]
    pub fn new(
        command_id: impl Into<String>,
        state_root: impl Into<String>,
        leaf_index: u32,
        total_leaves: u32,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            state_root: state_root.into(),
            commitment_id: None,
            leaf_index,
            total_leaves,
            sibling_hashes: Vec::new(),
        }
    }

    /// Attach a commitment identifier to the proof.
    #[must_use]
    pub fn with_commitment_id(mut self, commitment_id: impl Into<String>) -> Self {
        self.commitment_id = Some(commitment_id.into());
        self
    }

    /// Attach the sibling hashes used for inclusion verification.
    #[must_use]
    pub fn with_sibling_hashes(mut self, sibling_hashes: Vec<String>) -> Self {
        self.sibling_hashes = sibling_hashes;
        self
    }
}

/// Persisted verification record proving that a command was included in a committed remote root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandAttestation {
    /// Logical command identifier that was proven.
    pub command_id: String,
    /// Root against which the inclusion proof was verified.
    pub state_root: String,
    /// Optional commitment identifier associated with the attested root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_id: Option<String>,
    /// Optional signer identifier from a separately verified commitment manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_signer_id: Option<String>,
    /// Optional timestamp when the commitment manifest was verified locally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_verified_at: Option<DateTime<Utc>>,
    /// Hex-encoded settlement leaf hash derived from the command receipts.
    pub leaf_hash: String,
    /// Highest canonical remote sequence covered by the attested command.
    pub max_remote_sequence: u64,
    /// Highest remote head known when the attestation was verified.
    pub remote_head: u64,
    /// Highest remote cursor fully observed when the attestation was verified.
    pub remote_cursor: u64,
    /// Whether the local cursor had already observed the attested remote sequence at verification time.
    pub settled: bool,
    /// Leaf index proven within the commitment tree.
    pub leaf_index: u32,
    /// Total leaves represented by the commitment tree.
    pub total_leaves: u32,
    /// Hex-encoded sibling hashes used to verify inclusion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_hashes: Vec<String>,
    /// Timestamp when the local engine verified the proof.
    pub verified_at: DateTime<Utc>,
}

/// Errors raised while verifying a command inclusion proof.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AttestationError {
    #[error("command `{0}` has no confirmed receipts to attest")]
    NoConfirmedReceipts(String),
    #[error("command `{0}` still has local-pending receipts")]
    CommandStillPending(String),
    #[error("command `{0}` has rejected receipts and cannot be attested")]
    CommandRejected(String),
    #[error("command `{0}` has confirmed receipts without remote sequences")]
    MissingRemoteSequence(String),
    #[error(
        "remote head {remote_head} is behind attested sequence {max_remote_sequence} for command `{command_id}`"
    )]
    RemoteHeadBehind { command_id: String, remote_head: u64, max_remote_sequence: u64 },
    #[error("local state is missing a remote state root for command `{0}`")]
    MissingStateRoot(String),
    #[error(
        "state root mismatch for command `{command_id}`: expected `{expected}`, got `{actual}`"
    )]
    StateRootMismatch { command_id: String, expected: String, actual: String },
    #[error(
        "commitment id mismatch for command `{command_id}`: expected `{expected}`, got `{actual}`"
    )]
    CommitmentIdMismatch { command_id: String, expected: String, actual: String },
    #[error("invalid proof shape for command `{command_id}`: {reason}")]
    InvalidProofShape { command_id: String, reason: String },
    #[error("invalid hex in `{field}` for command `{command_id}`")]
    InvalidHex { command_id: String, field: String },
    #[error("merkle inclusion verification failed for command `{0}`")]
    ProofVerificationFailed(String),
}

/// Compute the deterministic settlement leaf hash for a command from its confirmed receipts.
///
/// # Errors
///
/// Returns [`AttestationError`] if the receipts are not in an attestable state.
pub fn compute_command_settlement_leaf(
    command_id: &str,
    receipts: &[KernelReceipt],
) -> Result<[u8; 32], AttestationError> {
    if receipts.iter().any(|receipt| receipt.status == KernelReceiptStatus::RejectedRemote) {
        return Err(AttestationError::CommandRejected(command_id.to_owned()));
    }
    if receipts.iter().any(|receipt| receipt.status == KernelReceiptStatus::LocalPending) {
        return Err(AttestationError::CommandStillPending(command_id.to_owned()));
    }

    let mut confirmed: Vec<_> = receipts
        .iter()
        .filter(|receipt| receipt.status == KernelReceiptStatus::ConfirmedRemote)
        .collect();
    if confirmed.is_empty() {
        return Err(AttestationError::NoConfirmedReceipts(command_id.to_owned()));
    }
    confirmed.sort_by_key(|receipt| {
        (
            receipt.remote_sequence.unwrap_or(0),
            receipt.local_sequence.unwrap_or(0),
            receipt.event_id,
        )
    });

    let mut hasher = Sha256::new();
    hasher.update(COMMAND_ATTESTATION_DOMAIN);
    hasher.update(u64_be(command_id.len() as u64));
    hasher.update(command_id.as_bytes());
    hasher.update(u64_be(confirmed.len() as u64));

    for receipt in confirmed {
        let Some(remote_sequence) = receipt.remote_sequence else {
            return Err(AttestationError::MissingRemoteSequence(command_id.to_owned()));
        };
        let hash_bytes = decode_hex_array32(command_id, "receipt.hash", &receipt.hash)?;

        hasher.update(receipt.event_id.as_bytes());
        hasher.update(u64_be(remote_sequence));
        hasher.update(hash_bytes);

        if let Some(remote_receipt) = receipt.remote_receipt.as_deref() {
            hasher.update([1_u8]);
            hasher.update(u64_be(remote_receipt.len() as u64));
            hasher.update(remote_receipt.as_bytes());
        } else {
            hasher.update([0_u8]);
        }
    }

    Ok(hasher.finalize().into())
}

/// Verify a command inclusion proof against retained receipts and current sync state.
///
/// # Errors
///
/// Returns [`AttestationError`] if the receipts are not attestable or the proof fails verification.
pub fn verify_command_inclusion_proof(
    proof: &CommandInclusionProof,
    receipts: &[KernelReceipt],
    state: &SyncState,
) -> Result<CommandAttestation, AttestationError> {
    if proof.total_leaves == 0 {
        return Err(AttestationError::InvalidProofShape {
            command_id: proof.command_id.clone(),
            reason: "total_leaves must be greater than zero".into(),
        });
    }
    if proof.leaf_index >= proof.total_leaves {
        return Err(AttestationError::InvalidProofShape {
            command_id: proof.command_id.clone(),
            reason: "leaf_index must be less than total_leaves".into(),
        });
    }

    let Some(state_root) = state.remote_state_root.as_deref() else {
        return Err(AttestationError::MissingStateRoot(proof.command_id.clone()));
    };
    if state_root != proof.state_root {
        return Err(AttestationError::StateRootMismatch {
            command_id: proof.command_id.clone(),
            expected: state_root.to_owned(),
            actual: proof.state_root.clone(),
        });
    }
    if let Some(expected_commitment_id) = state.last_commitment_id.as_deref() {
        match proof.commitment_id.as_deref() {
            Some(actual_commitment_id) if actual_commitment_id == expected_commitment_id => {}
            Some(actual_commitment_id) => {
                return Err(AttestationError::CommitmentIdMismatch {
                    command_id: proof.command_id.clone(),
                    expected: expected_commitment_id.to_owned(),
                    actual: actual_commitment_id.to_owned(),
                });
            }
            None => {
                return Err(AttestationError::CommitmentIdMismatch {
                    command_id: proof.command_id.clone(),
                    expected: expected_commitment_id.to_owned(),
                    actual: "<missing>".into(),
                });
            }
        }
    }

    let leaf_hash = compute_command_settlement_leaf(&proof.command_id, receipts)?;
    let max_remote_sequence =
        receipts
            .iter()
            .filter_map(|receipt| receipt.remote_sequence)
            .max()
            .ok_or_else(|| AttestationError::NoConfirmedReceipts(proof.command_id.clone()))?;
    if state.remote_head < max_remote_sequence {
        return Err(AttestationError::RemoteHeadBehind {
            command_id: proof.command_id.clone(),
            remote_head: state.remote_head,
            max_remote_sequence,
        });
    }

    let state_root_hash = decode_hex_array32(&proof.command_id, "state_root", &proof.state_root)?;
    let sibling_hashes: Vec<[u8; 32]> = proof
        .sibling_hashes
        .iter()
        .map(|hash| decode_hex_array32(&proof.command_id, "sibling_hash", hash))
        .collect::<Result<_, _>>()?;

    if !verify_merkle_path(leaf_hash, proof.leaf_index as usize, &sibling_hashes, state_root_hash) {
        return Err(AttestationError::ProofVerificationFailed(proof.command_id.clone()));
    }

    Ok(CommandAttestation {
        command_id: proof.command_id.clone(),
        state_root: proof.state_root.clone(),
        commitment_id: proof.commitment_id.clone(),
        manifest_signer_id: None,
        manifest_verified_at: None,
        leaf_hash: hex::encode(leaf_hash),
        max_remote_sequence,
        remote_head: state.remote_head,
        remote_cursor: state.remote_cursor,
        settled: state.remote_cursor >= max_remote_sequence,
        leaf_index: proof.leaf_index,
        total_leaves: proof.total_leaves,
        sibling_hashes: proof.sibling_hashes.clone(),
        verified_at: Utc::now(),
    })
}

fn decode_hex_array32(
    command_id: &str,
    field: &str,
    value: &str,
) -> Result<[u8; 32], AttestationError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).map_err(|_| AttestationError::InvalidHex {
        command_id: command_id.to_owned(),
        field: field.to_owned(),
    })?;
    if bytes.len() != 32 {
        return Err(AttestationError::InvalidHex {
            command_id: command_id.to_owned(),
            field: field.to_owned(),
        });
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn verify_merkle_path(
    leaf_hash: [u8; 32],
    mut leaf_index: usize,
    sibling_hashes: &[[u8; 32]],
    expected_root: [u8; 32],
) -> bool {
    let mut current = leaf_hash;
    for sibling_hash in sibling_hashes {
        current = if leaf_index % 2 == 0 {
            compute_node_hash(&current, sibling_hash)
        } else {
            compute_node_hash(sibling_hash, &current)
        };
        leaf_index /= 2;
    }
    current == expected_root
}

#[cfg(test)]
mod tests {
    use stateset_crypto::merkle::compute_merkle_root;
    use uuid::Uuid;

    use super::*;

    fn confirmed_receipt(command_id: &str, remote_sequence: u64) -> KernelReceipt {
        KernelReceipt {
            event_id: Uuid::new_v4(),
            status: KernelReceiptStatus::ConfirmedRemote,
            command_id: Some(command_id.into()),
            event_type: "order.created".into(),
            entity_type: "order".into(),
            entity_id: "ORD-1".into(),
            local_sequence: Some(remote_sequence),
            remote_sequence: Some(remote_sequence),
            hash: "11".repeat(32),
            source_agent_id: None,
            kernel: None,
            event_timestamp: Utc::now(),
            observed_at: Utc::now(),
            remote_receipt: Some(format!("receipt-{remote_sequence}")),
            rejection_code: None,
            rejection_reason: None,
            retryable: None,
        }
    }

    #[test]
    fn settlement_leaf_requires_only_confirmed_receipts() {
        let mut receipt = confirmed_receipt("cmd-1", 1);
        receipt.status = KernelReceiptStatus::LocalPending;

        let error = compute_command_settlement_leaf("cmd-1", &[receipt]).unwrap_err();
        assert_eq!(error, AttestationError::CommandStillPending("cmd-1".into()));
    }

    #[test]
    fn inclusion_proof_verifies_for_two_leaf_tree() {
        let receipts = vec![confirmed_receipt("cmd-1", 7)];
        let leaf_hash = compute_command_settlement_leaf("cmd-1", &receipts).unwrap();
        let sibling_hash = [9_u8; 32];
        let root = compute_merkle_root(&[leaf_hash, sibling_hash]);
        let proof = CommandInclusionProof::new("cmd-1", hex::encode(root), 0, 2)
            .with_commitment_id("BATCH-9")
            .with_sibling_hashes(vec![hex::encode(sibling_hash)]);

        let attestation = verify_command_inclusion_proof(
            &proof,
            &receipts,
            &SyncState {
                remote_head: 9,
                remote_cursor: 9,
                remote_state_root: Some(hex::encode(root)),
                last_commitment_id: Some("BATCH-9".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(attestation.command_id, "cmd-1");
        assert!(attestation.settled);
        assert_eq!(attestation.leaf_hash, hex::encode(leaf_hash));
    }
}
