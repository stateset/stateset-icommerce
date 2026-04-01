//! Sync batch wire type.
//!
//! A [`SyncBatch`] groups multiple [`EventEnvelope`]s into a single unit for
//! synchronization between nodes. It includes a Merkle root computed from
//! per-leaf integrity hashes of all contained events, plus optional signatures
//! and inclusion proofs.
//!
//! # Example
//!
//! ```rust
//! use stateset_protocol::{EventEnvelope, SyncBatch};
//!
//! let envelope = EventEnvelope::builder()
//!     .event_type("order.created")
//!     .entity_type("order")
//!     .entity_id("ord_1")
//!     .payload(b"{}".to_vec())
//!     .build()
//!     .unwrap();
//!
//! let batch = SyncBatch::new("node_alpha", vec![envelope]);
//! assert!(batch.verify_merkle_root());
//! ```

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stateset_crypto::pqc::{
    HybridSignatureBundle, HybridSigningPublicKey, StrictSigningPublicKey,
    hybrid_verify_event_signature, strict_verify_event_signature,
};
use uuid::Uuid;

use crate::envelope::EventEnvelope;
use crate::error::{ProtocolError, Result};
use crate::merkle;

/// Merkle leaf-hash algorithm used by a [`SyncBatch`].
///
/// - `payload_hash_v1`: legacy mode that hashes only envelope payload hashes.
/// - `envelope_hash_v2`: secure mode that binds full envelope integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MerkleLeafHashMode {
    /// Secure mode that hashes full envelope integrity leaves.
    #[default]
    EnvelopeHashV2,
    /// Legacy mode for compatibility with previously emitted batches.
    PayloadHashV1,
}

/// A batch of events for synchronization between nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncBatch {
    /// Unique batch identifier.
    pub batch_id: Uuid,
    /// The node that produced this batch.
    pub source_node_id: String,
    /// The events in this batch.
    pub leaves: Vec<EventEnvelope>,
    /// Merkle root computed from leaf hashes determined by `merkle_leaf_hash_mode`.
    pub merkle_root: [u8; 32],
    /// Which leaf-hash algorithm this batch uses.
    #[serde(default)]
    pub merkle_leaf_hash_mode: MerkleLeafHashMode,
    /// Cryptographic signatures over the batch.
    pub signatures: Vec<BatchSignature>,
    /// Merkle inclusion proofs for individual leaves.
    pub proofs: Vec<MerkleProof>,
    /// Protocol version for forward compatibility.
    pub protocol_version: u16,
    /// When the batch was created.
    pub created_at: DateTime<Utc>,
}

impl SyncBatch {
    /// Create a new batch from a set of event envelopes.
    ///
    /// Automatically computes the Merkle root from secure envelope leaf hashes.
    ///
    /// ```rust
    /// use stateset_protocol::{EventEnvelope, SyncBatch};
    ///
    /// let events = vec![
    ///     EventEnvelope::builder()
    ///         .event_type("t").entity_type("e").entity_id("1")
    ///         .payload(b"a".to_vec()).build().unwrap(),
    ///     EventEnvelope::builder()
    ///         .event_type("t").entity_type("e").entity_id("2")
    ///         .payload(b"b".to_vec()).build().unwrap(),
    /// ];
    /// let batch = SyncBatch::new("node_1", events);
    /// assert_eq!(batch.leaves.len(), 2);
    /// ```
    #[must_use]
    pub fn new(source_node_id: &str, leaves: Vec<EventEnvelope>) -> Self {
        let merkle_leaf_hash_mode = MerkleLeafHashMode::EnvelopeHashV2;
        let leaf_hashes = Self::leaf_hashes_with_mode(&leaves, merkle_leaf_hash_mode);
        let merkle_root = merkle::compute_merkle_root(&leaf_hashes);

        Self {
            batch_id: Uuid::new_v4(),
            source_node_id: source_node_id.to_owned(),
            leaves,
            merkle_root,
            merkle_leaf_hash_mode,
            signatures: Vec::new(),
            proofs: Vec::new(),
            protocol_version: 1,
            created_at: Utc::now(),
        }
    }

    /// Verify that the Merkle root matches the expected leaf hashes.
    ///
    /// ```rust
    /// use stateset_protocol::{EventEnvelope, SyncBatch};
    ///
    /// let batch = SyncBatch::new("n", vec![
    ///     EventEnvelope::builder()
    ///         .event_type("t").entity_type("e").entity_id("1")
    ///         .payload(b"x".to_vec()).build().unwrap(),
    /// ]);
    /// assert!(batch.verify_merkle_root());
    /// ```
    #[must_use]
    pub fn verify_merkle_root(&self) -> bool {
        let leaf_hashes = self.leaf_hashes();
        let computed = merkle::compute_merkle_root(&leaf_hashes);
        computed == self.merkle_root
    }

    fn leaf_hashes(&self) -> Vec<[u8; 32]> {
        Self::leaf_hashes_with_mode(&self.leaves, self.merkle_leaf_hash_mode)
    }

    fn leaf_hashes_with_mode(leaves: &[EventEnvelope], mode: MerkleLeafHashMode) -> Vec<[u8; 32]> {
        match mode {
            MerkleLeafHashMode::PayloadHashV1 => leaves.iter().map(|e| e.payload_hash).collect(),
            MerkleLeafHashMode::EnvelopeHashV2 => {
                leaves.iter().map(EventEnvelope::merkle_leaf_hash).collect()
            }
        }
    }

    /// Validate the entire batch: non-empty, valid envelopes, and correct root.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidBatch`] if the batch is empty,
    /// [`ProtocolError::InvalidEnvelope`] if any envelope is invalid, or
    /// [`ProtocolError::MerkleVerificationFailed`] if the root does not match.
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != 1 {
            return Err(ProtocolError::UnsupportedVersion(format!(
                "unsupported batch protocol_version {} (expected 1)",
                self.protocol_version
            )));
        }
        validate_required_batch_str("source_node_id", &self.source_node_id)?;
        if self.leaves.is_empty() {
            return Err(ProtocolError::InvalidBatch(
                "batch must contain at least one event".into(),
            ));
        }
        if self.merkle_leaf_hash_mode != MerkleLeafHashMode::EnvelopeHashV2 {
            return Err(ProtocolError::UnsupportedVersion(
                "legacy merkle_leaf_hash_mode payload_hash_v1 is not accepted".into(),
            ));
        }

        for (i, envelope) in self.leaves.iter().enumerate() {
            envelope
                .validate()
                .map_err(|e| ProtocolError::InvalidBatch(format!("leaf[{i}]: {e}")))?;
        }

        if !self.verify_merkle_root() {
            return Err(ProtocolError::MerkleVerificationFailed(
                "merkle root does not match leaves".into(),
            ));
        }

        self.validate_signatures()?;
        self.validate_proofs()?;

        Ok(())
    }

    /// Add a signature to this batch.
    pub fn add_signature(&mut self, sig: BatchSignature) {
        self.signatures.push(sig);
    }

    /// Add a Merkle proof to this batch.
    pub fn add_proof(&mut self, proof: MerkleProof) {
        self.proofs.push(proof);
    }

    /// Return the number of events in this batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Return whether this batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    fn validate_signatures(&self) -> Result<()> {
        if self.signatures.is_empty() {
            return Ok(());
        }

        let message = self.signature_message()?;

        for (idx, signature) in self.signatures.iter().enumerate() {
            self.validate_signature(signature, &message).map_err(|err| {
                ProtocolError::InvalidSignature(format!("signature[{idx}]: {err}"))
            })?;
        }

        Ok(())
    }

    fn validate_signature(&self, signature: &BatchSignature, message: &[u8]) -> Result<()> {
        if signature.signer_id.trim().is_empty() {
            return Err(ProtocolError::InvalidSignature("signer_id must not be empty".into()));
        }

        match signature.algorithm {
            SignatureAlgorithm::Ed25519 => {
                let public_key_bytes: [u8; 32] =
                    signature.public_key.as_slice().try_into().map_err(|_| {
                        ProtocolError::InvalidSignature(
                            "ed25519 public_key must be exactly 32 bytes".into(),
                        )
                    })?;
                let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|e| {
                    ProtocolError::InvalidSignature(format!("invalid ed25519 public_key: {e}"))
                })?;
                let parsed_signature = Signature::from_slice(signature.signature.as_slice())
                    .map_err(|e| {
                        ProtocolError::InvalidSignature(format!("invalid ed25519 signature: {e}"))
                    })?;
                verifying_key.verify_strict(message, &parsed_signature).map_err(|e| {
                    ProtocolError::InvalidSignature(format!("ed25519 verification failed: {e}"))
                })?;
            }
            SignatureAlgorithm::MlDsa65 => {
                let signing_hash = pqc_signature_hash(message);
                let public_key_bundle = signature.public_key_bundle.as_ref().ok_or_else(|| {
                    ProtocolError::InvalidSignature(
                        "mldsa65 signatures require public_key_bundle".into(),
                    )
                })?;
                let signature_bundle = signature.signature_bundle.as_ref().ok_or_else(|| {
                    ProtocolError::InvalidSignature(
                        "mldsa65 signatures require signature_bundle".into(),
                    )
                })?;
                let public_key = StrictSigningPublicKey {
                    ml_dsa_65_public_key: public_key_bundle
                        .ml_dsa_65_public_key
                        .clone()
                        .ok_or_else(|| {
                            ProtocolError::InvalidSignature(
                                "mldsa65 public_key_bundle is missing ml_dsa_65_public_key".into(),
                            )
                        })?,
                };
                let ml_dsa_signature =
                    signature_bundle.ml_dsa_65_signature.as_deref().ok_or_else(|| {
                        ProtocolError::InvalidSignature(
                            "mldsa65 signature_bundle is missing ml_dsa_65_signature".into(),
                        )
                    })?;
                if !strict_verify_event_signature(&signing_hash, ml_dsa_signature, &public_key) {
                    return Err(ProtocolError::InvalidSignature(
                        "mldsa65 verification failed".into(),
                    ));
                }
            }
            SignatureAlgorithm::Ed25519MlDsa65 => {
                let signing_hash = pqc_signature_hash(message);
                let ed25519_public_key: [u8; 32] =
                    signature.public_key.as_slice().try_into().map_err(|_| {
                        ProtocolError::InvalidSignature(
                            "hybrid public_key must be exactly 32 bytes".into(),
                        )
                    })?;
                let ed25519_signature: [u8; 64] =
                    signature.signature.as_slice().try_into().map_err(|_| {
                        ProtocolError::InvalidSignature(
                            "hybrid signature must be exactly 64 bytes".into(),
                        )
                    })?;
                let public_key_bundle = signature.public_key_bundle.as_ref().ok_or_else(|| {
                    ProtocolError::InvalidSignature(
                        "hybrid signatures require public_key_bundle".into(),
                    )
                })?;
                let signature_bundle = signature.signature_bundle.as_ref().ok_or_else(|| {
                    ProtocolError::InvalidSignature(
                        "hybrid signatures require signature_bundle".into(),
                    )
                })?;
                let public_key = HybridSigningPublicKey {
                    ed25519_public_key,
                    ml_dsa_65_public_key: public_key_bundle
                        .ml_dsa_65_public_key
                        .clone()
                        .ok_or_else(|| {
                            ProtocolError::InvalidSignature(
                                "hybrid public_key_bundle is missing ml_dsa_65_public_key".into(),
                            )
                        })?,
                };
                let signature_bundle = HybridSignatureBundle {
                    ed25519_signature,
                    ml_dsa_65_signature: signature_bundle.ml_dsa_65_signature.clone().ok_or_else(
                        || {
                            ProtocolError::InvalidSignature(
                                "hybrid signature_bundle is missing ml_dsa_65_signature".into(),
                            )
                        },
                    )?,
                };
                if !hybrid_verify_event_signature(&signing_hash, &signature_bundle, &public_key) {
                    return Err(ProtocolError::InvalidSignature(
                        "hybrid signature verification failed".into(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_proofs(&self) -> Result<()> {
        if self.proofs.is_empty() {
            return Ok(());
        }

        let leaf_hashes = self.leaf_hashes();
        for (idx, proof) in self.proofs.iter().enumerate() {
            if proof.leaf_index >= leaf_hashes.len() {
                return Err(ProtocolError::MerkleVerificationFailed(format!(
                    "proof[{idx}] leaf_index {} is out of bounds for {} leaves",
                    proof.leaf_index,
                    leaf_hashes.len()
                )));
            }

            let expected_leaf_hash = leaf_hashes[proof.leaf_index];
            if proof.leaf_hash != expected_leaf_hash {
                return Err(ProtocolError::MerkleVerificationFailed(format!(
                    "proof[{idx}] leaf_hash does not match leaf at index {}",
                    proof.leaf_index
                )));
            }

            if proof.root != self.merkle_root {
                return Err(ProtocolError::MerkleVerificationFailed(format!(
                    "proof[{idx}] root does not match batch merkle_root"
                )));
            }

            if !proof.verify() {
                return Err(ProtocolError::MerkleVerificationFailed(format!(
                    "proof[{idx}] failed verification"
                )));
            }
        }

        Ok(())
    }

    fn signature_message(&self) -> Result<Vec<u8>> {
        #[derive(Serialize)]
        struct SigningPayload<'a> {
            batch_id: Uuid,
            source_node_id: &'a str,
            merkle_root: [u8; 32],
            merkle_leaf_hash_mode: MerkleLeafHashMode,
            protocol_version: u16,
            created_at: DateTime<Utc>,
            leaves: &'a [EventEnvelope],
        }

        let payload = SigningPayload {
            batch_id: self.batch_id,
            source_node_id: &self.source_node_id,
            merkle_root: self.merkle_root,
            merkle_leaf_hash_mode: self.merkle_leaf_hash_mode,
            protocol_version: self.protocol_version,
            created_at: self.created_at,
            leaves: &self.leaves,
        };

        let canonical = serde_jcs::to_string(&payload)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
        Ok(canonical.into_bytes())
    }
}

fn validate_required_batch_str(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(ProtocolError::InvalidBatch(format!("{field} must not be empty")));
    }
    Ok(())
}

fn pqc_signature_hash(message: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(message);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

/// A cryptographic signature over a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BatchSignature {
    /// Identifier of the signer.
    pub signer_id: String,
    /// The signing algorithm used.
    pub algorithm: SignatureAlgorithm,
    /// The raw signature bytes.
    pub signature: Vec<u8>,
    /// The signer's public key.
    pub public_key: Vec<u8>,
    /// Optional multi-algorithm signature components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_bundle: Option<BatchSignatureBundle>,
    /// Optional multi-algorithm public-key components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key_bundle: Option<BatchPublicKeyBundle>,
}

/// Optional multi-algorithm batch signature components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct BatchSignatureBundle {
    /// ML-DSA-65 signature bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml_dsa_65_signature: Option<Vec<u8>>,
}

/// Optional multi-algorithm public-key components for batch signatures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct BatchPublicKeyBundle {
    /// ML-DSA-65 public key bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml_dsa_65_public_key: Option<Vec<u8>>,
}

/// Supported signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SignatureAlgorithm {
    /// Ed25519 (RFC 8032).
    Ed25519,
    /// ML-DSA-65 only over the SHA-256 of the canonical batch signature payload.
    MlDsa65,
    /// Hybrid Ed25519 + ML-DSA-65 over the SHA-256 of the canonical batch signature payload.
    Ed25519MlDsa65,
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ed25519 => write!(f, "ed25519"),
            Self::MlDsa65 => write!(f, "mldsa65"),
            Self::Ed25519MlDsa65 => write!(f, "ed25519_mldsa65"),
        }
    }
}

/// A Merkle inclusion proof for a single leaf.
///
/// Contains the sibling hashes along the path from the leaf to the root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MerkleProof {
    /// Index of the leaf in the original leaf array.
    pub leaf_index: usize,
    /// Hash of the leaf.
    pub leaf_hash: [u8; 32],
    /// Sibling hashes from leaf to root.
    pub siblings: Vec<[u8; 32]>,
    /// Expected Merkle root.
    pub root: [u8; 32],
}

impl MerkleProof {
    /// Verify this proof by recomputing the root from the leaf and siblings.
    ///
    /// ```rust
    /// use stateset_protocol::merkle::{compute_merkle_proof, verify_merkle_proof};
    ///
    /// let leaves = vec![[1u8; 32], [2u8; 32]];
    /// let proof = compute_merkle_proof(&leaves, 0).unwrap();
    /// assert!(proof.verify());
    /// ```
    #[must_use]
    pub fn verify(&self) -> bool {
        merkle::verify_merkle_proof(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use stateset_crypto::pqc::{
        generate_hybrid_signing_keypair, generate_strict_signing_keypair, hybrid_sign_event_hash,
        strict_sign_event_hash,
    };

    fn make_envelope(entity_id: &str, payload: &[u8]) -> EventEnvelope {
        EventEnvelope::builder()
            .event_type("test.event")
            .entity_type("test")
            .entity_id(entity_id)
            .payload(payload.to_vec())
            .build()
            .unwrap()
    }

    // --- SyncBatch::new tests ---

    #[test]
    fn new_single_event() {
        let env = make_envelope("1", b"data");
        let batch = SyncBatch::new("node_a", vec![env.clone()]);
        assert_eq!(batch.leaves.len(), 1);
        assert_eq!(batch.source_node_id, "node_a");
        assert_eq!(batch.protocol_version, 1);
        assert!(batch.signatures.is_empty());
        assert!(batch.proofs.is_empty());
        assert_eq!(batch.merkle_leaf_hash_mode, MerkleLeafHashMode::EnvelopeHashV2);
        // Single leaf => merkle root = leaf hash for configured mode
        assert_eq!(batch.merkle_root, env.merkle_leaf_hash());
    }

    #[test]
    fn new_multiple_events() {
        let envs: Vec<EventEnvelope> = (0..4)
            .map(|i| make_envelope(&format!("e_{i}"), format!("payload_{i}").as_bytes()))
            .collect();
        let batch = SyncBatch::new("node_b", envs);
        assert_eq!(batch.leaves.len(), 4);
    }

    #[test]
    fn new_empty_events() {
        let batch = SyncBatch::new("node_c", vec![]);
        assert!(batch.leaves.is_empty());
        assert_eq!(batch.merkle_root, merkle::ZERO_HASH);
    }

    // --- verify_merkle_root tests ---

    #[test]
    fn verify_root_valid() {
        let envs: Vec<EventEnvelope> =
            (0..3).map(|i| make_envelope(&format!("e_{i}"), format!("p_{i}").as_bytes())).collect();
        let batch = SyncBatch::new("node", envs);
        assert!(batch.verify_merkle_root());
    }

    #[test]
    fn verify_root_tampered() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.merkle_root = [0xFF; 32];
        assert!(!batch.verify_merkle_root());
    }

    #[test]
    fn verify_root_detects_metadata_tampering_under_v2() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.leaves[0].event_type = "tampered.event".to_string();
        assert!(!batch.verify_merkle_root());
    }

    #[test]
    fn verify_root_legacy_payload_hash_mode_compatible() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.merkle_leaf_hash_mode = MerkleLeafHashMode::PayloadHashV1;
        let legacy_leaf_hashes: Vec<[u8; 32]> =
            batch.leaves.iter().map(|e| e.payload_hash).collect();
        batch.merkle_root = merkle::compute_merkle_root(&legacy_leaf_hashes);
        assert!(batch.verify_merkle_root());
        assert!(matches!(batch.validate(), Err(ProtocolError::UnsupportedVersion(_))));
    }

    #[test]
    fn verify_root_empty_batch() {
        let batch = SyncBatch::new("node", vec![]);
        assert!(batch.verify_merkle_root()); // empty => ZERO_HASH matches
    }

    // --- validate tests ---

    #[test]
    fn validate_valid_batch() {
        let envs = vec![make_envelope("1", b"data")];
        let batch = SyncBatch::new("node", envs);
        assert!(batch.validate().is_ok());
    }

    #[test]
    fn validate_empty_source_node_id() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.source_node_id = String::new();
        assert!(batch.validate().is_err());
    }

    #[test]
    fn validate_whitespace_source_node_id() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.source_node_id = "   ".to_string();
        assert!(batch.validate().is_err());
    }

    #[test]
    fn validate_empty_leaves() {
        let batch = SyncBatch::new("node", vec![]);
        assert!(batch.validate().is_err());
    }

    #[test]
    fn validate_invalid_envelope_in_batch() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.leaves[0].event_type = String::new(); // invalidate envelope
        let result = batch.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("leaf[0]"));
    }

    #[test]
    fn validate_tampered_root() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.merkle_root = [0xFF; 32];
        let result = batch.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::MerkleVerificationFailed(_)));
    }

    #[test]
    fn validate_rejects_unsupported_protocol_version() {
        let envs = vec![make_envelope("1", b"data")];
        let mut batch = SyncBatch::new("node", envs);
        batch.protocol_version = 2;
        assert!(matches!(batch.validate(), Err(ProtocolError::UnsupportedVersion(_))));
    }

    // --- add_signature / add_proof tests ---

    #[test]
    fn add_signature() {
        let mut batch = SyncBatch::new("node", vec![make_envelope("1", b"d")]);
        assert!(batch.signatures.is_empty());

        let sig = BatchSignature {
            signer_id: "signer_1".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: vec![0u8; 64],
            public_key: vec![0u8; 32],
            signature_bundle: None,
            public_key_bundle: None,
        };
        batch.add_signature(sig);
        assert_eq!(batch.signatures.len(), 1);
    }

    #[test]
    fn add_proof() {
        let envs = vec![make_envelope("1", b"d"), make_envelope("2", b"e")];
        let mut batch = SyncBatch::new("node", envs);
        assert!(batch.proofs.is_empty());

        let leaf_hashes = batch.leaf_hashes();
        let proof = merkle::compute_merkle_proof(&leaf_hashes, 0).unwrap();
        batch.add_proof(proof);
        assert_eq!(batch.proofs.len(), 1);
    }

    #[test]
    fn validate_accepts_valid_signature_and_proof() {
        let envs = vec![make_envelope("1", b"d"), make_envelope("2", b"e")];
        let mut batch = SyncBatch::new("node", envs);

        let leaf_hashes = batch.leaf_hashes();
        let proof = merkle::compute_merkle_proof(&leaf_hashes, 0).unwrap();
        batch.add_proof(proof);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let signature = signing_key.sign(&batch.signature_message().unwrap());
        batch.add_signature(BatchSignature {
            signer_id: "node_signer".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: signature.to_bytes().to_vec(),
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature_bundle: None,
            public_key_bundle: None,
        });

        assert!(batch.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_signature_bytes() {
        let envs = vec![make_envelope("1", b"d")];
        let mut batch = SyncBatch::new("node", envs);
        batch.add_signature(BatchSignature {
            signer_id: "node_signer".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: vec![1, 2, 3],
            public_key: vec![0u8; 32],
            signature_bundle: None,
            public_key_bundle: None,
        });

        let err = batch.validate().unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidSignature(_)));
    }

    #[test]
    fn validate_rejects_whitespace_signer_id() {
        let envs = vec![make_envelope("1", b"d")];
        let mut batch = SyncBatch::new("node", envs);
        batch.add_signature(BatchSignature {
            signer_id: "   ".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: vec![0u8; 64],
            public_key: vec![0u8; 32],
            signature_bundle: None,
            public_key_bundle: None,
        });

        let err = batch.validate().unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidSignature(_)));
    }

    #[test]
    fn validate_rejects_proof_with_wrong_leaf_hash() {
        let envs = vec![make_envelope("1", b"d"), make_envelope("2", b"e")];
        let mut batch = SyncBatch::new("node", envs);
        let leaf_hashes = batch.leaf_hashes();
        let mut proof = merkle::compute_merkle_proof(&leaf_hashes, 0).unwrap();
        proof.leaf_hash = [0xAB; 32];
        batch.add_proof(proof);

        let err = batch.validate().unwrap_err();
        assert!(matches!(err, ProtocolError::MerkleVerificationFailed(_)));
    }

    // --- len / is_empty tests ---

    #[test]
    fn len_and_is_empty() {
        let empty_batch = SyncBatch::new("node", vec![]);
        assert_eq!(empty_batch.len(), 0);
        assert!(empty_batch.is_empty());

        let batch = SyncBatch::new("node", vec![make_envelope("1", b"d")]);
        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());
    }

    // --- SignatureAlgorithm tests ---

    #[test]
    fn signature_algorithm_display() {
        assert_eq!(SignatureAlgorithm::Ed25519.to_string(), "ed25519");
        assert_eq!(SignatureAlgorithm::MlDsa65.to_string(), "mldsa65");
        assert_eq!(SignatureAlgorithm::Ed25519MlDsa65.to_string(), "ed25519_mldsa65");
    }

    #[test]
    fn signature_algorithm_serde_roundtrip() {
        let json = serde_json::to_string(&SignatureAlgorithm::Ed25519).unwrap();
        assert_eq!(json, r#""ed25519""#);
        let deserialized: SignatureAlgorithm = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SignatureAlgorithm::Ed25519);
    }

    // --- BatchSignature tests ---

    #[test]
    fn batch_signature_serde_roundtrip() {
        let sig = BatchSignature {
            signer_id: "agent_x".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: vec![1, 2, 3],
            public_key: vec![4, 5, 6],
            signature_bundle: None,
            public_key_bundle: None,
        };
        let json = serde_json::to_string(&sig).unwrap();
        let deserialized: BatchSignature = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, sig);
    }

    #[test]
    fn batch_signature_is_debug() {
        let sig = BatchSignature {
            signer_id: "s".into(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: vec![],
            public_key: vec![],
            signature_bundle: None,
            public_key_bundle: None,
        };
        let debug = format!("{sig:?}");
        assert!(debug.contains("BatchSignature"));
    }

    #[test]
    fn validate_accepts_strict_pqc_signature() {
        let envs = vec![make_envelope("1", b"d")];
        let mut batch = SyncBatch::new("node", envs);
        let signing_hash = pqc_signature_hash(&batch.signature_message().unwrap());
        let keypair = generate_strict_signing_keypair().unwrap();
        let signature = strict_sign_event_hash(&signing_hash, &keypair.private).unwrap();

        batch.add_signature(BatchSignature {
            signer_id: "strict".into(),
            algorithm: SignatureAlgorithm::MlDsa65,
            signature: Vec::new(),
            public_key: Vec::new(),
            signature_bundle: Some(BatchSignatureBundle { ml_dsa_65_signature: Some(signature) }),
            public_key_bundle: Some(BatchPublicKeyBundle {
                ml_dsa_65_public_key: Some(keypair.public.ml_dsa_65_public_key),
            }),
        });

        assert!(batch.validate().is_ok());
    }

    #[test]
    fn validate_accepts_hybrid_pqc_signature() {
        let envs = vec![make_envelope("1", b"d")];
        let mut batch = SyncBatch::new("node", envs);
        let signing_hash = pqc_signature_hash(&batch.signature_message().unwrap());
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let signature = hybrid_sign_event_hash(&signing_hash, &keypair.private).unwrap();

        batch.add_signature(BatchSignature {
            signer_id: "hybrid".into(),
            algorithm: SignatureAlgorithm::Ed25519MlDsa65,
            signature: signature.ed25519_signature.to_vec(),
            public_key: keypair.public.ed25519_public_key.to_vec(),
            signature_bundle: Some(BatchSignatureBundle {
                ml_dsa_65_signature: Some(signature.ml_dsa_65_signature),
            }),
            public_key_bundle: Some(BatchPublicKeyBundle {
                ml_dsa_65_public_key: Some(keypair.public.ml_dsa_65_public_key),
            }),
        });

        assert!(batch.validate().is_ok());
    }

    // --- MerkleProof tests ---

    #[test]
    fn merkle_proof_verify() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let proof = merkle::compute_merkle_proof(&leaves, 2).unwrap();
        assert!(proof.verify());
    }

    #[test]
    fn merkle_proof_serde_roundtrip() {
        let leaves = vec![[10u8; 32], [20u8; 32]];
        let proof = merkle::compute_merkle_proof(&leaves, 0).unwrap();
        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: MerkleProof = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, proof);
        assert!(deserialized.verify());
    }

    #[test]
    fn merkle_proof_is_debug() {
        let proof =
            MerkleProof { leaf_index: 0, leaf_hash: [0u8; 32], siblings: vec![], root: [0u8; 32] };
        let debug = format!("{proof:?}");
        assert!(debug.contains("MerkleProof"));
    }

    // --- Serde round-trip tests ---

    #[test]
    fn sync_batch_serde_roundtrip() {
        let envs = vec![make_envelope("1", b"data1"), make_envelope("2", b"data2")];
        let batch = SyncBatch::new("node_x", envs);
        let json = serde_json::to_string(&batch).unwrap();
        let deserialized: SyncBatch = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.batch_id, batch.batch_id);
        assert_eq!(deserialized.source_node_id, batch.source_node_id);
        assert_eq!(deserialized.merkle_root, batch.merkle_root);
        assert_eq!(deserialized.merkle_leaf_hash_mode, batch.merkle_leaf_hash_mode);
        assert_eq!(deserialized.leaves.len(), batch.leaves.len());
    }

    #[test]
    fn sync_batch_deserialize_without_leaf_mode_defaults_to_secure_mode() {
        let batch = SyncBatch::new("node", vec![make_envelope("1", b"d")]);
        let mut as_value = serde_json::to_value(&batch).unwrap();
        as_value.as_object_mut().unwrap().remove("merkle_leaf_hash_mode");
        let deserialized: SyncBatch = serde_json::from_value(as_value).unwrap();
        assert_eq!(deserialized.merkle_leaf_hash_mode, MerkleLeafHashMode::EnvelopeHashV2);
    }

    #[test]
    fn sync_batch_is_clone() {
        let batch = SyncBatch::new("node", vec![make_envelope("1", b"d")]);
        let cloned = batch.clone();
        assert_eq!(batch, cloned);
    }

    #[test]
    fn sync_batch_is_debug() {
        let batch = SyncBatch::new("node", vec![make_envelope("1", b"d")]);
        let debug = format!("{batch:?}");
        assert!(debug.contains("SyncBatch"));
    }
}
