#![no_main]
//! Fuzz `compute_payload_plain_hash`. Inputs are JSON; the hash should be
//! deterministic and never panic for any well-formed value.

use libfuzzer_sys::fuzz_target;
use stateset_crypto::hash::compute_payload_plain_hash;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        if let Ok(h1) = compute_payload_plain_hash(&value) {
            // Determinism: hashing the same value twice must yield the same digest.
            let h2 = compute_payload_plain_hash(&value).expect("second pass also succeeds");
            assert_eq!(h1, h2, "compute_payload_plain_hash is not deterministic");
        }
    }
});
