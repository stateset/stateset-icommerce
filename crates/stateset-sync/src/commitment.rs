use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stateset_crypto::sign::{sign_event_hash, verify_event_signature};
use stateset_crypto::u64_be;
use thiserror::Error;

use crate::state::SyncState;

const COMMITMENT_MANIFEST_DOMAIN: &[u8] = b"STATESET_COMMITMENT_MANIFEST_V1";
const ED25519_SIGNATURE_SCHEME: &str = "ed25519";

/// Signed commitment manifest published by a sequencer or counterparty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitmentManifest {
    /// Unique commitment identifier.
    pub commitment_id: String,
    /// Previous commitment in the chain, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_commitment_id: Option<String>,
    /// Hex-encoded state root committed by the signer.
    pub state_root: String,
    /// Highest canonical remote sequence covered by the commitment.
    pub remote_head: u64,
    /// Logical identifier for the signer.
    pub signer_id: String,
    /// Signature scheme used for the manifest.
    #[serde(default = "default_signature_scheme")]
    pub signature_scheme: String,
    /// Hex-encoded Ed25519 public key of the signer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_public_key: Option<String>,
    /// Hex-encoded detached signature over the manifest hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Timestamp when the manifest was issued.
    pub issued_at: DateTime<Utc>,
}

impl CommitmentManifest {
    /// Create a new unsigned commitment manifest.
    #[must_use]
    pub fn new(
        commitment_id: impl Into<String>,
        state_root: impl Into<String>,
        remote_head: u64,
        signer_id: impl Into<String>,
    ) -> Self {
        Self {
            commitment_id: commitment_id.into(),
            previous_commitment_id: None,
            state_root: state_root.into(),
            remote_head,
            signer_id: signer_id.into(),
            signature_scheme: default_signature_scheme(),
            signer_public_key: None,
            signature: None,
            issued_at: Utc::now(),
        }
    }

    /// Attach a previous commitment id to the manifest.
    #[must_use]
    pub fn with_previous_commitment_id(
        mut self,
        previous_commitment_id: impl Into<String>,
    ) -> Self {
        self.previous_commitment_id = Some(previous_commitment_id.into());
        self
    }

    /// Attach a signature bundle to the manifest.
    #[must_use]
    pub fn with_signature(
        mut self,
        signer_public_key: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        self.signer_public_key = Some(signer_public_key.into());
        self.signature = Some(signature.into());
        self
    }
}

/// Persisted record that a commitment manifest signature was verified locally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedCommitmentManifest {
    /// Unique commitment identifier.
    pub commitment_id: String,
    /// Previous commitment in the chain, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_commitment_id: Option<String>,
    /// Hex-encoded state root anchored by the signer.
    pub state_root: String,
    /// Highest canonical remote sequence covered by the commitment.
    pub remote_head: u64,
    /// Logical identifier for the signer.
    pub signer_id: String,
    /// Signature scheme used by the signer.
    pub signature_scheme: String,
    /// Hex-encoded signer public key.
    pub signer_public_key: String,
    /// Hex-encoded detached signature.
    pub signature: String,
    /// Timestamp when the manifest was issued by the signer.
    pub issued_at: DateTime<Utc>,
    /// Timestamp when the local engine verified the manifest.
    pub verified_at: DateTime<Utc>,
}

/// Errors raised while hashing, signing, or verifying commitment manifests.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ManifestVerificationError {
    #[error("unsupported commitment signature scheme `{0}`")]
    UnsupportedSignatureScheme(String),
    #[error("commitment manifest `{0}` is missing signer_public_key")]
    MissingSignerPublicKey(String),
    #[error("commitment manifest `{0}` is missing signature")]
    MissingSignature(String),
    #[error("invalid hex in `{field}` for commitment `{commitment_id}`")]
    InvalidHex { commitment_id: String, field: String },
    #[error("signature verification failed for commitment `{0}`")]
    SignatureVerificationFailed(String),
    #[error(
        "remote state root mismatch for commitment `{commitment_id}`: expected `{expected}`, got `{actual}`"
    )]
    StateRootMismatch { commitment_id: String, expected: String, actual: String },
    #[error("remote commitment id mismatch: expected `{expected}`, got `{actual}`")]
    CommitmentIdMismatch { expected: String, actual: String },
    #[error(
        "remote head mismatch for commitment `{commitment_id}`: expected {expected}, got {actual}"
    )]
    RemoteHeadMismatch { commitment_id: String, expected: u64, actual: u64 },
    #[error("commitment manifest `{commitment_id}` failed trust policy: {reason}")]
    TrustPolicyViolation { commitment_id: String, reason: String },
    #[error("persist verified commitment manifest `{commitment_id}` failed: {reason}")]
    PersistenceFailed { commitment_id: String, reason: String },
}

/// Compute the deterministic signing hash for a commitment manifest.
///
/// # Errors
///
/// Returns [`ManifestVerificationError`] if the state root is malformed.
pub fn compute_commitment_manifest_hash(
    manifest: &CommitmentManifest,
) -> Result<[u8; 32], ManifestVerificationError> {
    let state_root =
        decode_hex_array32(&manifest.commitment_id, "state_root", &manifest.state_root)?;

    let mut hasher = Sha256::new();
    hasher.update(COMMITMENT_MANIFEST_DOMAIN);
    hasher.update(u64_be(manifest.commitment_id.len() as u64));
    hasher.update(manifest.commitment_id.as_bytes());

    if let Some(previous_commitment_id) = manifest.previous_commitment_id.as_deref() {
        hasher.update([1_u8]);
        hasher.update(u64_be(previous_commitment_id.len() as u64));
        hasher.update(previous_commitment_id.as_bytes());
    } else {
        hasher.update([0_u8]);
    }

    hasher.update(state_root);
    hasher.update(u64_be(manifest.remote_head));
    hasher.update(u64_be(manifest.signer_id.len() as u64));
    hasher.update(manifest.signer_id.as_bytes());
    hasher.update(u64_be(manifest.signature_scheme.len() as u64));
    hasher.update(manifest.signature_scheme.as_bytes());
    let issued_at = manifest.issued_at.to_rfc3339();
    hasher.update(u64_be(issued_at.len() as u64));
    hasher.update(issued_at.as_bytes());
    Ok(hasher.finalize().into())
}

/// Sign a commitment manifest with the provided Ed25519 private/public keypair.
///
/// # Errors
///
/// Returns [`ManifestVerificationError`] if the manifest cannot be hashed.
pub fn sign_commitment_manifest(
    mut manifest: CommitmentManifest,
    private_key: &[u8; 32],
    public_key: &[u8; 32],
) -> Result<CommitmentManifest, ManifestVerificationError> {
    let signing_hash = compute_commitment_manifest_hash(&manifest)?;
    let signature = sign_event_hash(&signing_hash, private_key).map_err(|_| {
        ManifestVerificationError::SignatureVerificationFailed(manifest.commitment_id.clone())
    })?;
    manifest.signer_public_key = Some(hex::encode(public_key));
    manifest.signature = Some(hex::encode(signature));
    Ok(manifest)
}

/// Verify a signed commitment manifest and return the retained verified record.
///
/// # Errors
///
/// Returns [`ManifestVerificationError`] if signature material is missing or invalid.
pub fn verify_commitment_manifest(
    manifest: &CommitmentManifest,
) -> Result<VerifiedCommitmentManifest, ManifestVerificationError> {
    if manifest.signature_scheme != ED25519_SIGNATURE_SCHEME {
        return Err(ManifestVerificationError::UnsupportedSignatureScheme(
            manifest.signature_scheme.clone(),
        ));
    }

    let public_key_hex = manifest.signer_public_key.as_deref().ok_or_else(|| {
        ManifestVerificationError::MissingSignerPublicKey(manifest.commitment_id.clone())
    })?;
    let signature_hex = manifest.signature.as_deref().ok_or_else(|| {
        ManifestVerificationError::MissingSignature(manifest.commitment_id.clone())
    })?;

    let signing_hash = compute_commitment_manifest_hash(manifest)?;
    let public_key =
        decode_hex_array32(&manifest.commitment_id, "signer_public_key", public_key_hex)?;
    let signature = decode_hex_array64(&manifest.commitment_id, "signature", signature_hex)?;
    if !verify_event_signature(&signing_hash, &signature, &public_key) {
        return Err(ManifestVerificationError::SignatureVerificationFailed(
            manifest.commitment_id.clone(),
        ));
    }

    Ok(VerifiedCommitmentManifest {
        commitment_id: manifest.commitment_id.clone(),
        previous_commitment_id: manifest.previous_commitment_id.clone(),
        state_root: manifest.state_root.clone(),
        remote_head: manifest.remote_head,
        signer_id: manifest.signer_id.clone(),
        signature_scheme: manifest.signature_scheme.clone(),
        signer_public_key: public_key_hex.to_owned(),
        signature: signature_hex.to_owned(),
        issued_at: manifest.issued_at,
        verified_at: Utc::now(),
    })
}

/// Verify that a signed commitment manifest is consistent with the engine's current remote state.
///
/// # Errors
///
/// Returns [`ManifestVerificationError`] if the signed manifest does not match known remote state.
pub fn verify_commitment_manifest_against_state(
    manifest: &CommitmentManifest,
    state: &SyncState,
) -> Result<VerifiedCommitmentManifest, ManifestVerificationError> {
    let verified = verify_commitment_manifest(manifest)?;

    if let Some(expected_state_root) = state.remote_state_root.as_deref() {
        if expected_state_root != verified.state_root {
            return Err(ManifestVerificationError::StateRootMismatch {
                commitment_id: verified.commitment_id,
                expected: expected_state_root.to_owned(),
                actual: verified.state_root,
            });
        }
    }
    if let Some(expected_commitment_id) = state.last_commitment_id.as_deref() {
        if expected_commitment_id != verified.commitment_id {
            return Err(ManifestVerificationError::CommitmentIdMismatch {
                expected: expected_commitment_id.to_owned(),
                actual: verified.commitment_id,
            });
        }
    }
    if state.remote_head > 0 && state.remote_head != verified.remote_head {
        return Err(ManifestVerificationError::RemoteHeadMismatch {
            commitment_id: verified.commitment_id,
            expected: state.remote_head,
            actual: verified.remote_head,
        });
    }

    Ok(verified)
}

fn default_signature_scheme() -> String {
    ED25519_SIGNATURE_SCHEME.to_owned()
}

fn decode_hex_array32(
    commitment_id: &str,
    field: &str,
    value: &str,
) -> Result<[u8; 32], ManifestVerificationError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).map_err(|_| ManifestVerificationError::InvalidHex {
        commitment_id: commitment_id.to_owned(),
        field: field.to_owned(),
    })?;
    if bytes.len() != 32 {
        return Err(ManifestVerificationError::InvalidHex {
            commitment_id: commitment_id.to_owned(),
            field: field.to_owned(),
        });
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_hex_array64(
    commitment_id: &str,
    field: &str,
    value: &str,
) -> Result<[u8; 64], ManifestVerificationError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).map_err(|_| ManifestVerificationError::InvalidHex {
        commitment_id: commitment_id.to_owned(),
        field: field.to_owned(),
    })?;
    if bytes.len() != 64 {
        return Err(ManifestVerificationError::InvalidHex {
            commitment_id: commitment_id.to_owned(),
            field: field.to_owned(),
        });
    }
    let mut out = [0_u8; 64];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use stateset_crypto::sign::generate_keypair;

    use super::*;

    #[test]
    fn signed_commitment_manifest_verifies() {
        let (private_key, public_key) = generate_keypair();
        let manifest = sign_commitment_manifest(
            CommitmentManifest::new("BATCH-1", "11".repeat(32), 7, "sequencer-a"),
            &private_key,
            &public_key,
        )
        .unwrap();

        let verified = verify_commitment_manifest(&manifest).unwrap();
        assert_eq!(verified.commitment_id, "BATCH-1");
        assert_eq!(verified.remote_head, 7);
        assert_eq!(verified.signer_id, "sequencer-a");
    }
}
