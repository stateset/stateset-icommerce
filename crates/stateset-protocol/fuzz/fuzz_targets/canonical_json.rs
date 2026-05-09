#![no_main]
//! Fuzz canonical JSON serialization (RFC 8785) used for envelope/batch hashing.

use libfuzzer_sys::fuzz_target;
use stateset_protocol::canonical::canonical_json;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        // canonical_json must never panic on any parsed Value.
        let _ = canonical_json(&value);
    }
});
