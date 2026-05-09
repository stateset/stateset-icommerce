#![no_main]
//! Fuzz the JSON canonicalization (RFC 8785 / JCS) path.
//!
//! Inputs are arbitrary bytes; we attempt to parse as JSON and, if successful,
//! canonicalize. The function should never panic — only return Err.

use libfuzzer_sys::fuzz_target;
use stateset_crypto::canonicalize::canonicalize_json_bytes;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        // canonicalize_json_bytes should never panic on any parsed Value.
        let _ = canonicalize_json_bytes(&value);
    }
});
