//! Merkle tree hashing per VES v1.0 Section 10
//!
//! - Leaf hashing (Section 10.2)
//! - Padding leaf (Section 10.3)
//! - Node hashing (Section 10.4)
//! - Stream ID (Section 12.2)
//! - Receipt hash (Section 6.4)

use sha2::{Digest, Sha256};

use crate::encoding::{u64_be, uuid_to_bytes};
use crate::{CryptoError, domain};

/// Parameters for leaf hash computation
#[derive(Debug)]
pub struct LeafParams<'a> {
    /// Tenant UUID
    pub tenant_id: &'a str,
    /// Store UUID
    pub store_id: &'a str,
    /// Sequence number in the event log
    pub sequence_number: u64,
    /// 32-byte event signing hash
    pub event_signing_hash: &'a [u8; 32],
    /// 64-byte agent Ed25519 signature
    pub agent_signature: &'a [u8; 64],
}

/// Compute leaf hash per VES v1.0 Section 10.2
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_leaf_hash(params: &LeafParams<'_>) -> Result<[u8; 32], CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(domain::LEAF);
    hasher.update(uuid_to_bytes(params.tenant_id)?);
    hasher.update(uuid_to_bytes(params.store_id)?);
    hasher.update(u64_be(params.sequence_number));
    hasher.update(params.event_signing_hash);
    hasher.update(params.agent_signature);
    Ok(hasher.finalize().into())
}

/// Compute Merkle node hash per VES v1.0 Section 10.4
#[must_use]
pub fn compute_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain::NODE);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// Compute padding leaf per VES v1.0 Section 10.3
///
/// The result is memoized since it is deterministic and called frequently.
#[must_use]
pub fn compute_pad_leaf() -> [u8; 32] {
    static PAD_LEAF: std::sync::LazyLock<[u8; 32]> = std::sync::LazyLock::new(|| {
        let mut hasher = Sha256::new();
        hasher.update(domain::PAD_LEAF);
        hasher.finalize().into()
    });
    *PAD_LEAF
}

/// Compute stream ID per VES v1.0 Section 12.2
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_stream_id(tenant_id: &str, store_id: &str) -> Result<[u8; 32], CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(domain::STREAM);
    hasher.update(uuid_to_bytes(tenant_id)?);
    hasher.update(uuid_to_bytes(store_id)?);
    Ok(hasher.finalize().into())
}

/// Parameters for receipt hash computation
#[derive(Debug)]
pub struct ReceiptParams<'a> {
    /// Tenant UUID
    pub tenant_id: &'a str,
    /// Store UUID
    pub store_id: &'a str,
    /// Event UUID
    pub event_id: &'a str,
    /// Sequence number in the event log
    pub sequence_number: u64,
    /// 32-byte event signing hash
    pub event_signing_hash: &'a [u8; 32],
}

/// Compute receipt hash per VES v1.0 Section 6.4
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if any UUID field is invalid.
pub fn compute_receipt_hash(params: &ReceiptParams<'_>) -> Result<[u8; 32], CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(domain::RECEIPT);
    hasher.update(uuid_to_bytes(params.tenant_id)?);
    hasher.update(uuid_to_bytes(params.store_id)?);
    hasher.update(uuid_to_bytes(params.event_id)?);
    hasher.update(u64_be(params.sequence_number));
    hasher.update(params.event_signing_hash);
    Ok(hasher.finalize().into())
}

/// Compute Merkle root from a list of leaf hashes
///
/// Pads to the next power of 2 with `pad_leaf` hashes.
#[must_use]
pub fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return compute_pad_leaf();
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    // Pad to next power of 2
    let target = leaves.len().next_power_of_two();
    let pad = compute_pad_leaf();
    let mut buf_a: Vec<[u8; 32]> = Vec::with_capacity(target);
    buf_a.extend_from_slice(leaves);
    buf_a.resize(target, pad);
    let mut buf_b: Vec<[u8; 32]> = Vec::with_capacity(target / 2);

    // Build tree bottom-up using double-buffer swap and reusable hasher
    let mut hasher = Sha256::new();
    let mut current = &mut buf_a;
    let mut next = &mut buf_b;
    while current.len() > 1 {
        next.clear();
        for chunk in current.chunks(2) {
            hasher.update(domain::NODE);
            hasher.update(chunk[0]);
            hasher.update(chunk[1]);
            next.push(hasher.finalize_reset().into());
        }
        std::mem::swap(&mut current, &mut next);
    }

    current[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn pad_leaf_deterministic() {
        let p1 = compute_pad_leaf();
        let p2 = compute_pad_leaf();
        assert_eq!(p1, p2);
        assert_eq!(p1.len(), 32);
    }

    #[test]
    fn node_hash_deterministic() {
        let left = [1u8; 32];
        let right = [2u8; 32];
        let h1 = compute_node_hash(&left, &right);
        let h2 = compute_node_hash(&left, &right);
        assert_eq!(h1, h2);
    }

    #[test]
    fn node_hash_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let h1 = compute_node_hash(&a, &b);
        let h2 = compute_node_hash(&b, &a);
        assert_ne!(h1, h2);
    }

    #[test]
    fn stream_id_deterministic() {
        let s1 = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
        let s2 = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn leaf_hash_deterministic() {
        let event_hash = [0u8; 32];
        let sig = [0u8; 64];
        let params = LeafParams {
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            sequence_number: 1,
            event_signing_hash: &event_hash,
            agent_signature: &sig,
        };
        let h1 = compute_leaf_hash(&params).unwrap();
        let h2 = compute_leaf_hash(&params).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn leaf_hash_varies_with_sequence() {
        let event_hash = [0u8; 32];
        let sig = [0u8; 64];
        let p1 = LeafParams {
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            sequence_number: 1,
            event_signing_hash: &event_hash,
            agent_signature: &sig,
        };
        let p2 = LeafParams { sequence_number: 2, ..p1 };
        assert_ne!(compute_leaf_hash(&p1).unwrap(), compute_leaf_hash(&p2).unwrap());
    }

    #[test]
    fn receipt_hash_deterministic() {
        let event_hash = [0u8; 32];
        let params = ReceiptParams {
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            sequence_number: 42,
            event_signing_hash: &event_hash,
        };
        let h1 = compute_receipt_hash(&params).unwrap();
        let h2 = compute_receipt_hash(&params).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn merkle_root_single_leaf() {
        let leaf = [42u8; 32];
        let root = compute_merkle_root(&[leaf]);
        assert_eq!(root, leaf);
    }

    #[test]
    fn merkle_root_empty() {
        let root = compute_merkle_root(&[]);
        assert_eq!(root, compute_pad_leaf());
    }

    #[test]
    fn merkle_root_two_leaves() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let root = compute_merkle_root(&[a, b]);
        assert_eq!(root, compute_node_hash(&a, &b));
    }

    #[test]
    fn merkle_root_three_leaves_pads_to_four() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let pad = compute_pad_leaf();
        let root = compute_merkle_root(&[a, b, c]);
        let expected = compute_node_hash(&compute_node_hash(&a, &b), &compute_node_hash(&c, &pad));
        assert_eq!(root, expected);
    }

    #[test]
    fn merkle_root_four_leaves() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let d = [4u8; 32];
        let root = compute_merkle_root(&[a, b, c, d]);
        let expected = compute_node_hash(&compute_node_hash(&a, &b), &compute_node_hash(&c, &d));
        assert_eq!(root, expected);
    }

    #[test]
    fn merkle_root_deterministic() {
        let leaves: Vec<[u8; 32]> = (0..7).map(|i| [i as u8; 32]).collect();
        let r1 = compute_merkle_root(&leaves);
        let r2 = compute_merkle_root(&leaves);
        assert_eq!(r1, r2);
    }
}
