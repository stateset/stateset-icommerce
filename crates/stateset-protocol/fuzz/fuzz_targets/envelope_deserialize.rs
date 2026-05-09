#![no_main]
//! Fuzz `EventEnvelope` JSON deserialization. Adversarial input must NEVER
//! panic — only return `Err`. Successfully-deserialized envelopes must also
//! survive `validate()` without panicking.

use libfuzzer_sys::fuzz_target;
use stateset_protocol::EventEnvelope;

fuzz_target!(|data: &[u8]| {
    if let Ok(env) = serde_json::from_slice::<EventEnvelope>(data) {
        // validate() must terminate without panicking on any deserialized envelope.
        let _ = env.validate();
        // merkle_leaf_hash() and compute_payload_hash() must also be panic-free.
        let _ = env.merkle_leaf_hash();
    }
});
