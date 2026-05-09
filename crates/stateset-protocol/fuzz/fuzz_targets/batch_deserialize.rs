#![no_main]
//! Fuzz `SyncBatch` JSON deserialization. Adversarial batches must NEVER panic.
//! `validate()` and `verify_merkle_root()` are also exercised on success.

use libfuzzer_sys::fuzz_target;
use stateset_protocol::SyncBatch;

fuzz_target!(|data: &[u8]| {
    if let Ok(batch) = serde_json::from_slice::<SyncBatch>(data) {
        let _ = batch.validate();
        let _ = batch.verify_merkle_root();
        let _ = batch.len();
        let _ = batch.is_empty();
    }
});
