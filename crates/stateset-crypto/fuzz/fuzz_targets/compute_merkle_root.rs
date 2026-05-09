#![no_main]
//! Fuzz `compute_merkle_root`. Treats the input bytes as a stream of 32-byte
//! leaves and verifies the function never panics regardless of leaf count.

use libfuzzer_sys::fuzz_target;
use stateset_crypto::merkle::compute_merkle_root;

fuzz_target!(|data: &[u8]| {
    let leaves: Vec<[u8; 32]> = data
        .chunks_exact(32)
        .map(|c| {
            let mut leaf = [0_u8; 32];
            leaf.copy_from_slice(c);
            leaf
        })
        .collect();

    // compute_merkle_root must terminate and never panic for any leaf count
    // (including zero — must return the empty root, not panic).
    let root1 = compute_merkle_root(&leaves);
    let root2 = compute_merkle_root(&leaves);
    assert_eq!(root1, root2, "compute_merkle_root is not deterministic");
});
