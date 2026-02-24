//! Merkle tree operations for sync batches.
//!
//! Provides pure-function Merkle tree construction, proof generation, and
//! proof verification. Uses SHA-256 for all hashing.
//!
//! # Design
//!
//! - Empty leaf set produces a zero hash (`[0u8; 32]`).
//! - Single leaf returns the leaf hash itself.
//! - Non-power-of-2 leaf counts are padded with zero hashes.
//! - Tree is built bottom-up with pairwise node hashing.
//!
//! # Example
//!
//! ```rust
//! use stateset_protocol::merkle::{compute_merkle_root, compute_merkle_proof, verify_merkle_proof};
//!
//! let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
//! let root = compute_merkle_root(&leaves);
//! let proof = compute_merkle_proof(&leaves, 1).unwrap();
//! assert!(verify_merkle_proof(&proof));
//! ```

use sha2::{Digest, Sha256};

use crate::batch::MerkleProof;
use crate::error::{ProtocolError, Result};

/// The zero hash: 32 bytes of zeros, used for empty trees and padding.
pub const ZERO_HASH: [u8; 32] = [0u8; 32];

/// Compute the SHA-256 hash of a Merkle node from its two children.
#[must_use]
fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// Compute the Merkle root from a set of leaf hashes.
///
/// - Empty input returns [`ZERO_HASH`].
/// - Single leaf returns the leaf itself.
/// - Non-power-of-2 counts are padded to the next power of 2 with [`ZERO_HASH`].
///
/// # Example
///
/// ```rust
/// use stateset_protocol::merkle::{compute_merkle_root, ZERO_HASH};
///
/// assert_eq!(compute_merkle_root(&[]), ZERO_HASH);
///
/// let leaf = [42u8; 32];
/// assert_eq!(compute_merkle_root(&[leaf]), leaf);
/// ```
#[must_use]
pub fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return ZERO_HASH;
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    // Pad to next power of 2
    let target = leaves.len().next_power_of_two();
    let mut current: Vec<[u8; 32]> = leaves.to_vec();
    current.resize(target, ZERO_HASH);

    // Build tree bottom-up
    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len() / 2);
        for chunk in current.chunks(2) {
            next.push(node_hash(&chunk[0], &chunk[1]));
        }
        current = next;
    }

    current[0]
}

/// Compute a Merkle inclusion proof for the leaf at `index`.
///
/// The proof contains the sibling hashes needed to reconstruct the root
/// from the leaf hash.
///
/// # Errors
///
/// Returns [`ProtocolError::MerkleVerificationFailed`] if `index >= leaves.len()`
/// or if the leaf set is empty.
///
/// # Example
///
/// ```rust
/// use stateset_protocol::merkle::{compute_merkle_proof, verify_merkle_proof};
///
/// let leaves = vec![[1u8; 32], [2u8; 32]];
/// let proof = compute_merkle_proof(&leaves, 0).unwrap();
/// assert!(verify_merkle_proof(&proof));
/// ```
pub fn compute_merkle_proof(leaves: &[[u8; 32]], index: usize) -> Result<MerkleProof> {
    if leaves.is_empty() {
        return Err(ProtocolError::MerkleVerificationFailed(
            "cannot create proof for empty leaf set".into(),
        ));
    }
    if index >= leaves.len() {
        return Err(ProtocolError::MerkleVerificationFailed(format!(
            "leaf index {index} out of bounds (len={})",
            leaves.len()
        )));
    }

    // Pad to next power of 2
    let target = leaves.len().next_power_of_two();
    let mut padded: Vec<[u8; 32]> = leaves.to_vec();
    padded.resize(target, ZERO_HASH);

    let mut siblings = Vec::new();
    let mut current_index = index;
    let mut current_layer = padded;

    while current_layer.len() > 1 {
        // Determine sibling index
        let sibling_index =
            if current_index % 2 == 0 { current_index + 1 } else { current_index - 1 };
        siblings.push(current_layer[sibling_index]);

        // Build next layer
        let mut next_layer = Vec::with_capacity(current_layer.len() / 2);
        for chunk in current_layer.chunks(2) {
            next_layer.push(node_hash(&chunk[0], &chunk[1]));
        }

        current_index /= 2;
        current_layer = next_layer;
    }

    let root = current_layer[0];

    Ok(MerkleProof { leaf_index: index, leaf_hash: leaves[index], siblings, root })
}

/// Verify a Merkle inclusion proof.
///
/// Recomputes the root from the leaf hash and sibling path, then compares
/// against the expected root in the proof.
///
/// # Example
///
/// ```rust
/// use stateset_protocol::merkle::{compute_merkle_proof, verify_merkle_proof};
///
/// let leaves = vec![[10u8; 32], [20u8; 32], [30u8; 32], [40u8; 32]];
/// let proof = compute_merkle_proof(&leaves, 2).unwrap();
/// assert!(verify_merkle_proof(&proof));
/// ```
#[must_use]
pub fn verify_merkle_proof(proof: &MerkleProof) -> bool {
    let mut current = proof.leaf_hash;
    let mut index = proof.leaf_index;

    for sibling in &proof.siblings {
        current = if index % 2 == 0 {
            node_hash(&current, sibling)
        } else {
            node_hash(sibling, &current)
        };
        index /= 2;
    }

    current == proof.root
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- compute_merkle_root tests ---

    #[test]
    fn root_empty_returns_zero_hash() {
        assert_eq!(compute_merkle_root(&[]), ZERO_HASH);
    }

    #[test]
    fn root_single_leaf() {
        let leaf = [42u8; 32];
        assert_eq!(compute_merkle_root(&[leaf]), leaf);
    }

    #[test]
    fn root_two_leaves() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let root = compute_merkle_root(&[a, b]);
        assert_eq!(root, node_hash(&a, &b));
    }

    #[test]
    fn root_three_leaves_pads_to_four() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let root = compute_merkle_root(&[a, b, c]);
        let expected = node_hash(&node_hash(&a, &b), &node_hash(&c, &ZERO_HASH));
        assert_eq!(root, expected);
    }

    #[test]
    fn root_four_leaves() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let d = [4u8; 32];
        let root = compute_merkle_root(&[a, b, c, d]);
        let expected = node_hash(&node_hash(&a, &b), &node_hash(&c, &d));
        assert_eq!(root, expected);
    }

    #[test]
    fn root_five_leaves_pads_to_eight() {
        let leaves: Vec<[u8; 32]> = (0..5).map(|i| [i as u8; 32]).collect();
        let root = compute_merkle_root(&leaves);
        // Should produce a valid 32-byte hash
        assert_eq!(root.len(), 32);
        assert_ne!(root, ZERO_HASH);
    }

    #[test]
    fn root_deterministic() {
        let leaves: Vec<[u8; 32]> = (0..7).map(|i| [i as u8; 32]).collect();
        let r1 = compute_merkle_root(&leaves);
        let r2 = compute_merkle_root(&leaves);
        assert_eq!(r1, r2);
    }

    #[test]
    fn root_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let r1 = compute_merkle_root(&[a, b]);
        let r2 = compute_merkle_root(&[b, a]);
        assert_ne!(r1, r2);
    }

    #[test]
    fn root_power_of_two_leaves() {
        let leaves: Vec<[u8; 32]> = (0..8).map(|i| [i as u8; 32]).collect();
        let root = compute_merkle_root(&leaves);
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn root_sixteen_leaves() {
        let leaves: Vec<[u8; 32]> = (0..16).map(|i| [i as u8; 32]).collect();
        let root = compute_merkle_root(&leaves);
        assert_eq!(root.len(), 32);
        assert_ne!(root, ZERO_HASH);
    }

    // --- compute_merkle_proof tests ---

    #[test]
    fn proof_empty_leaves_error() {
        let result = compute_merkle_proof(&[], 0);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::MerkleVerificationFailed(_)));
    }

    #[test]
    fn proof_index_out_of_bounds() {
        let leaves = vec![[1u8; 32]];
        let result = compute_merkle_proof(&leaves, 1);
        assert!(result.is_err());
    }

    #[test]
    fn proof_single_leaf() {
        let leaf = [42u8; 32];
        let proof = compute_merkle_proof(&[leaf], 0).unwrap();
        assert_eq!(proof.leaf_index, 0);
        assert_eq!(proof.leaf_hash, leaf);
        assert_eq!(proof.root, leaf);
        assert!(proof.siblings.is_empty());
    }

    #[test]
    fn proof_two_leaves_first() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let proof = compute_merkle_proof(&[a, b], 0).unwrap();
        assert_eq!(proof.leaf_index, 0);
        assert_eq!(proof.leaf_hash, a);
        assert_eq!(proof.siblings.len(), 1);
        assert_eq!(proof.siblings[0], b);
        assert_eq!(proof.root, node_hash(&a, &b));
    }

    #[test]
    fn proof_two_leaves_second() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let proof = compute_merkle_proof(&[a, b], 1).unwrap();
        assert_eq!(proof.leaf_index, 1);
        assert_eq!(proof.leaf_hash, b);
        assert_eq!(proof.siblings[0], a);
    }

    #[test]
    fn proof_four_leaves_each_index() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        for i in 0..4 {
            let proof = compute_merkle_proof(&leaves, i).unwrap();
            assert_eq!(proof.leaf_index, i);
            assert_eq!(proof.leaf_hash, leaves[i]);
            assert_eq!(proof.siblings.len(), 2); // log2(4) = 2
        }
    }

    #[test]
    fn proof_three_leaves_padded() {
        let leaves: Vec<[u8; 32]> = (0..3).map(|i| [i as u8; 32]).collect();
        let proof = compute_merkle_proof(&leaves, 2).unwrap();
        assert_eq!(proof.leaf_index, 2);
        assert_eq!(proof.siblings.len(), 2); // padded to 4, so log2(4) = 2
    }

    #[test]
    fn proof_root_matches_computed_root() {
        let leaves: Vec<[u8; 32]> = (0..5).map(|i| [(i + 10) as u8; 32]).collect();
        let root = compute_merkle_root(&leaves);
        for i in 0..leaves.len() {
            let proof = compute_merkle_proof(&leaves, i).unwrap();
            assert_eq!(proof.root, root);
        }
    }

    // --- verify_merkle_proof tests ---

    #[test]
    fn verify_single_leaf() {
        let leaf = [42u8; 32];
        let proof = compute_merkle_proof(&[leaf], 0).unwrap();
        assert!(verify_merkle_proof(&proof));
    }

    #[test]
    fn verify_two_leaves() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        for i in 0..2 {
            let proof = compute_merkle_proof(&[a, b], i).unwrap();
            assert!(verify_merkle_proof(&proof));
        }
    }

    #[test]
    fn verify_four_leaves() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        for i in 0..4 {
            let proof = compute_merkle_proof(&leaves, i).unwrap();
            assert!(verify_merkle_proof(&proof));
        }
    }

    #[test]
    fn verify_seven_leaves() {
        let leaves: Vec<[u8; 32]> = (0..7).map(|i| [(i * 3) as u8; 32]).collect();
        for i in 0..7 {
            let proof = compute_merkle_proof(&leaves, i).unwrap();
            assert!(verify_merkle_proof(&proof));
        }
    }

    #[test]
    fn verify_sixteen_leaves() {
        let leaves: Vec<[u8; 32]> = (0..16).map(|i| [i as u8; 32]).collect();
        for i in 0..16 {
            let proof = compute_merkle_proof(&leaves, i).unwrap();
            assert!(verify_merkle_proof(&proof));
        }
    }

    #[test]
    fn verify_tampered_leaf_fails() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        let mut proof = compute_merkle_proof(&leaves, 1).unwrap();
        proof.leaf_hash = [0xFF; 32]; // tamper with leaf
        assert!(!verify_merkle_proof(&proof));
    }

    #[test]
    fn verify_tampered_root_fails() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        let mut proof = compute_merkle_proof(&leaves, 0).unwrap();
        proof.root = [0xFF; 32]; // tamper with root
        assert!(!verify_merkle_proof(&proof));
    }

    #[test]
    fn verify_tampered_sibling_fails() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        let mut proof = compute_merkle_proof(&leaves, 0).unwrap();
        proof.siblings[0] = [0xFF; 32]; // tamper with sibling
        assert!(!verify_merkle_proof(&proof));
    }

    #[test]
    fn verify_wrong_index_fails() {
        let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
        let mut proof = compute_merkle_proof(&leaves, 0).unwrap();
        proof.leaf_index = 1; // wrong index
        assert!(!verify_merkle_proof(&proof));
    }

    // --- node_hash tests ---

    #[test]
    fn node_hash_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_eq!(node_hash(&a, &b), node_hash(&a, &b));
    }

    #[test]
    fn node_hash_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_ne!(node_hash(&a, &b), node_hash(&b, &a));
    }
}
