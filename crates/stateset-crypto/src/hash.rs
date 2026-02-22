//! Domain-separated SHA-256 hashing per VES v1.0
//!
//! All hash functions use the pattern: SHA256(domain_prefix || structured_data)

use sha2::{Digest, Sha256};

use crate::canonicalize::canonicalize_json;
use crate::encoding::{encode_string, u32_be, uuid_to_bytes};
use crate::{domain, CryptoError, ZERO_HASH};

/// Compute payload_plain_hash per VES v1.0 Section 5.2
///
/// H = SHA256(DOMAIN.PAYLOAD_PLAIN || \[salt\] || JCS(payload))
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the payload cannot be canonicalized,
/// or [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_payload_plain_hash(
    payload: &serde_json::Value,
    salt: Option<&[u8; 16]>,
) -> Result<[u8; 32], CryptoError> {
    let canonical = canonicalize_json(payload)?;
    let mut hasher = Sha256::new();
    hasher.update(domain::PAYLOAD_PLAIN);
    if let Some(s) = salt {
        hasher.update(s);
    }
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().into())
}

/// Compute legacy payload hash (no domain prefix)
///
/// Used for compatibility with legacy gRPC API that uses `EventEnvelope`.
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the payload cannot be canonicalized.
pub fn compute_legacy_payload_hash(
    payload: &serde_json::Value,
) -> Result<[u8; 32], CryptoError> {
    let canonical = canonicalize_json(payload)?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().into())
}

/// Parameters for computing `payload_cipher_hash`
#[derive(Debug)]
pub struct PayloadCipherParams<'a> {
    /// 12-byte nonce
    pub nonce: &'a [u8],
    /// 32-byte payload AAD
    pub payload_aad: &'a [u8],
    /// Variable-length ciphertext
    pub ciphertext: &'a [u8],
    /// 16-byte authentication tag
    pub tag: &'a [u8],
    /// 32-byte recipients hash
    pub recipients_hash: &'a [u8],
}

/// Compute payload_cipher_hash per VES v1.0 Section 5.3
///
/// For plaintext events, returns `ZERO_HASH` (32 zero bytes).
#[must_use]
pub fn compute_payload_cipher_hash(params: Option<&PayloadCipherParams<'_>>) -> [u8; 32] {
    match params {
        None => ZERO_HASH,
        Some(p) => {
            let mut hasher = Sha256::new();
            hasher.update(domain::PAYLOAD_CIPHER);
            hasher.update(p.nonce);
            hasher.update(p.payload_aad);
            hasher.update(p.ciphertext);
            hasher.update(p.tag);
            hasher.update(p.recipients_hash);
            hasher.finalize().into()
        }
    }
}

/// Parameters for event signing hash
#[derive(Debug)]
pub struct EventSigningParams<'a> {
    /// VES protocol version
    pub ves_version: u32,
    /// Tenant UUID
    pub tenant_id: &'a str,
    /// Store UUID
    pub store_id: &'a str,
    /// Event UUID
    pub event_id: &'a str,
    /// Source agent UUID
    pub source_agent_id: &'a str,
    /// Agent key identifier
    pub agent_key_id: u32,
    /// Entity type string (e.g. "order")
    pub entity_type: &'a str,
    /// Entity identifier string
    pub entity_id: &'a str,
    /// Event type string (e.g. "order.created")
    pub event_type: &'a str,
    /// RFC 3339 timestamp
    pub created_at: &'a str,
    /// 0 for plaintext, 1 for encrypted
    pub payload_kind: u32,
    /// 32-byte payload plain hash
    pub payload_plain_hash: &'a [u8; 32],
    /// 32-byte payload cipher hash
    pub payload_cipher_hash: &'a [u8; 32],
}

/// Compute event signing hash per VES v1.0 Section 6.2
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_event_signing_hash(
    params: &EventSigningParams<'_>,
) -> Result<[u8; 32], CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(domain::EVENTSIG);
    hasher.update(u32_be(params.ves_version));
    hasher.update(uuid_to_bytes(params.tenant_id)?);
    hasher.update(uuid_to_bytes(params.store_id)?);
    hasher.update(uuid_to_bytes(params.event_id)?);
    hasher.update(uuid_to_bytes(params.source_agent_id)?);
    hasher.update(u32_be(params.agent_key_id));
    hasher.update(encode_string(params.entity_type));
    hasher.update(encode_string(params.entity_id));
    hasher.update(encode_string(params.event_type));
    hasher.update(encode_string(params.created_at));
    hasher.update(u32_be(params.payload_kind));
    hasher.update(params.payload_plain_hash);
    hasher.update(params.payload_cipher_hash);
    Ok(hasher.finalize().into())
}

/// Parameters for payload AAD computation
#[derive(Debug, Clone, Copy)]
pub struct PayloadAadParams<'a> {
    /// VES protocol version
    pub ves_version: u32,
    /// Tenant UUID
    pub tenant_id: &'a str,
    /// Store UUID
    pub store_id: &'a str,
    /// Event UUID
    pub event_id: &'a str,
    /// Source agent UUID
    pub source_agent_id: &'a str,
    /// Agent key identifier
    pub agent_key_id: u32,
    /// Entity type string
    pub entity_type: &'a str,
    /// Entity identifier string
    pub entity_id: &'a str,
    /// Event type string
    pub event_type: &'a str,
    /// RFC 3339 timestamp
    pub created_at: &'a str,
    /// 32-byte payload plain hash
    pub payload_plain_hash: &'a [u8; 32],
}

/// Compute payload AAD per VES-ENC-1
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_payload_aad(
    params: &PayloadAadParams<'_>,
) -> Result<[u8; 32], CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(domain::PAYLOAD_AAD);
    hasher.update(u32_be(params.ves_version));
    hasher.update(uuid_to_bytes(params.tenant_id)?);
    hasher.update(uuid_to_bytes(params.store_id)?);
    hasher.update(uuid_to_bytes(params.event_id)?);
    hasher.update(uuid_to_bytes(params.source_agent_id)?);
    hasher.update(u32_be(params.agent_key_id));
    hasher.update(encode_string(params.entity_type));
    hasher.update(encode_string(params.entity_id));
    hasher.update(encode_string(params.event_type));
    hasher.update(encode_string(params.created_at));
    hasher.update(params.payload_plain_hash);
    Ok(hasher.finalize().into())
}

/// Compute recipients hash per VES-ENC-1
///
/// Recipients are sorted by `recipient_kid` for deterministic ordering.
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the recipients cannot be canonicalized.
pub fn compute_recipients_hash(
    recipients: &[serde_json::Value],
) -> Result<[u8; 32], CryptoError> {
    // Sort by recipient_kid
    let mut sorted: Vec<serde_json::Value> = recipients.to_vec();
    sorted.sort_by(|a, b| {
        let a_kid = a.get("recipient_kid").and_then(|v| v.as_u64()).unwrap_or(0);
        let b_kid = b.get("recipient_kid").and_then(|v| v.as_u64()).unwrap_or(0);
        a_kid.cmp(&b_kid)
    });

    let canonical = canonicalize_json(&serde_json::Value::Array(sorted))?;
    let mut hasher = Sha256::new();
    hasher.update(domain::RECIPIENTS);
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn payload_plain_hash_no_salt() {
        let payload = json!({"key": "value"});
        let hash = compute_payload_plain_hash(&payload, None).unwrap();
        assert_eq!(hash.len(), 32);
        // Deterministic -- same input always gives same output
        let hash2 = compute_payload_plain_hash(&payload, None).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn payload_plain_hash_with_salt() {
        let payload = json!({"key": "value"});
        let salt = [0u8; 16];
        let hash_salted = compute_payload_plain_hash(&payload, Some(&salt)).unwrap();
        let hash_unsalted = compute_payload_plain_hash(&payload, None).unwrap();
        assert_ne!(hash_salted, hash_unsalted);
    }

    #[test]
    fn legacy_payload_hash() {
        let payload = json!({"key": "value"});
        let hash = compute_legacy_payload_hash(&payload).unwrap();
        assert_eq!(hash.len(), 32);
        // Legacy has no domain prefix, so differs from plain hash
        let plain = compute_payload_plain_hash(&payload, None).unwrap();
        assert_ne!(hash, plain);
    }

    #[test]
    fn payload_cipher_hash_none_returns_zeros() {
        let hash = compute_payload_cipher_hash(None);
        assert_eq!(hash, ZERO_HASH);
    }

    #[test]
    fn payload_cipher_hash_with_params() {
        let params = PayloadCipherParams {
            nonce: &[0u8; 12],
            payload_aad: &[1u8; 32],
            ciphertext: b"encrypted_data",
            tag: &[2u8; 16],
            recipients_hash: &[3u8; 32],
        };
        let hash = compute_payload_cipher_hash(Some(&params));
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, ZERO_HASH);
    }

    #[test]
    fn event_signing_hash_deterministic() {
        let plain_hash = [0u8; 32];
        let cipher_hash = [0u8; 32];
        let params = EventSigningParams {
            ves_version: 1,
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            source_agent_id: TEST_UUID,
            agent_key_id: 1,
            entity_type: "order",
            entity_id: "ord_001",
            event_type: "order.created",
            created_at: "2026-02-21T00:00:00Z",
            payload_kind: 0,
            payload_plain_hash: &plain_hash,
            payload_cipher_hash: &cipher_hash,
        };
        let hash1 = compute_event_signing_hash(&params).unwrap();
        let hash2 = compute_event_signing_hash(&params).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32);
    }

    #[test]
    fn payload_aad_deterministic() {
        let plain_hash = [0u8; 32];
        let params = PayloadAadParams {
            ves_version: 1,
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            source_agent_id: TEST_UUID,
            agent_key_id: 1,
            entity_type: "order",
            entity_id: "ord_001",
            event_type: "order.created",
            created_at: "2026-02-21T00:00:00Z",
            payload_plain_hash: &plain_hash,
        };
        let aad1 = compute_payload_aad(&params).unwrap();
        let aad2 = compute_payload_aad(&params).unwrap();
        assert_eq!(aad1, aad2);
    }

    #[test]
    fn recipients_hash_sorts_by_kid() {
        let r1 = json!({"recipient_kid": 2, "enc_b64u": "a", "ct_b64u": "b"});
        let r2 = json!({"recipient_kid": 1, "enc_b64u": "c", "ct_b64u": "d"});
        let hash_unsorted = compute_recipients_hash(&[r1.clone(), r2.clone()]).unwrap();
        let hash_sorted = compute_recipients_hash(&[r2, r1]).unwrap();
        assert_eq!(hash_unsorted, hash_sorted);
    }

    #[test]
    fn event_signing_hash_different_event_type() {
        let plain_hash = [0u8; 32];
        let cipher_hash = [0u8; 32];
        let base = EventSigningParams {
            ves_version: 1,
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            source_agent_id: TEST_UUID,
            agent_key_id: 1,
            entity_type: "order",
            entity_id: "ord_001",
            event_type: "order.created",
            created_at: "2026-02-21T00:00:00Z",
            payload_kind: 0,
            payload_plain_hash: &plain_hash,
            payload_cipher_hash: &cipher_hash,
        };
        let hash1 = compute_event_signing_hash(&base).unwrap();
        let modified = EventSigningParams {
            event_type: "order.cancelled",
            ..base
        };
        let hash2 = compute_event_signing_hash(&modified).unwrap();
        assert_ne!(hash1, hash2);
    }
}
