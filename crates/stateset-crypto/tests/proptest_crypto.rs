//! Property-based tests for the stateset-crypto crate.
//!
//! Uses `proptest` to verify invariants over randomised inputs.

use proptest::prelude::*;
use stateset_crypto::canonicalize::canonicalize_json;
use stateset_crypto::encrypt::{RecipientKey, generate_x25519_keypair};
use stateset_crypto::hash::{PayloadAadParams, compute_payload_aad, compute_payload_plain_hash};
use stateset_crypto::merkle::{compute_merkle_root, compute_node_hash};
use stateset_crypto::sign::{generate_keypair, sign_event_hash, verify_event_signature};

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

// ---------------------------------------------------------------------------
// Helpers & strategies
// ---------------------------------------------------------------------------

/// Generate an arbitrary 32-byte array (used as key seed, hash, etc.).
fn arb_32_bytes() -> impl Strategy<Value = [u8; 32]> {
    prop::array::uniform32(any::<u8>())
}

/// Generate arbitrary message bytes (as a 32-byte hash, matching the API).
fn arb_hash() -> impl Strategy<Value = [u8; 32]> {
    arb_32_bytes()
}

/// Generate a valid JSON value suitable for JCS canonicalization.
/// We build values from primitives to avoid Infinity/NaN floats which JCS
/// rejects.
fn arb_json_value() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        // Use integers to avoid NaN/Infinity issues
        (-1_000_000i64..1_000_000i64)
            .prop_map(|n| serde_json::Value::Number(serde_json::Number::from(n))),
        "[a-zA-Z0-9 _]{0,20}".prop_map(serde_json::Value::String),
    ];
    leaf.prop_recursive(
        3,  // depth
        64, // max nodes
        8,  // items per collection
        |inner| {
            prop_oneof![
                // Array of values
                prop::collection::vec(inner.clone(), 0..6).prop_map(serde_json::Value::Array),
                // Object with sorted string keys
                prop::collection::vec(("[a-z]{1,8}", inner), 0..6,)
                    .prop_map(|pairs| { serde_json::Value::Object(pairs.into_iter().collect(),) }),
            ]
        },
    )
}

/// Generate a JSON object value (for encrypt/decrypt round-trip).
fn arb_json_object() -> impl Strategy<Value = serde_json::Value> {
    prop::collection::vec(
        ("[a-z]{1,8}", "[a-zA-Z0-9]{0,16}".prop_map(serde_json::Value::String)),
        1..6,
    )
    .prop_map(|pairs| serde_json::Value::Object(pairs.into_iter().collect()))
}

/// Generate a vector of leaf hashes for Merkle tree tests.
fn arb_leaves() -> impl Strategy<Value = Vec<[u8; 32]>> {
    prop::collection::vec(arb_32_bytes(), 1..33)
}

// ---------------------------------------------------------------------------
// 1. Sign/verify round-trip
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn sign_verify_roundtrip(message in arb_hash()) {
        let (private_key, public_key) = generate_keypair();
        let signature = sign_event_hash(&message, &private_key)
            .expect("signing should not fail");
        prop_assert!(
            verify_event_signature(&message, &signature, &public_key),
            "signature should verify with the correct key",
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Sign/verify fails with wrong key
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn sign_verify_wrong_key_fails(message in arb_hash()) {
        let (private_key_a, _public_key_a) = generate_keypair();
        let (_private_key_b, public_key_b) = generate_keypair();

        let signature = sign_event_hash(&message, &private_key_a)
            .expect("signing should not fail");

        prop_assert!(
            !verify_event_signature(&message, &signature, &public_key_b),
            "verification should fail with a different public key",
        );
    }
}

// ---------------------------------------------------------------------------
// 3. JCS canonicalization idempotency
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn jcs_canonicalize_idempotent(json in arb_json_value()) {
        let first = canonicalize_json(&json)
            .expect("first canonicalization should succeed");

        // Parse the canonical string back to a Value
        let reparsed: serde_json::Value = serde_json::from_str(&first)
            .expect("canonical JSON should be parseable");

        let second = canonicalize_json(&reparsed)
            .expect("second canonicalization should succeed");

        prop_assert_eq!(
            &first, &second,
            "JCS was not idempotent: first={:?}, second={:?}",
            first, second,
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Merkle root determinism
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn merkle_root_deterministic(leaves in arb_leaves()) {
        let root1 = compute_merkle_root(&leaves);
        let root2 = compute_merkle_root(&leaves);
        prop_assert_eq!(
            root1, root2,
            "Merkle root was not deterministic for {} leaves",
            leaves.len(),
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Encrypt/decrypt round-trip (full VES-ENC-1 pipeline)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn encrypt_decrypt_roundtrip(payload in arb_json_object()) {
        let plain_hash_placeholder = [0u8; 32];
        let aad_params = PayloadAadParams {
            ves_version: 1,
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            source_agent_id: TEST_UUID,
            agent_key_id: 1,
            entity_type: "order",
            entity_id: "ord_prop",
            event_type: "order.created",
            created_at: "2026-02-23T00:00:00Z",
            payload_plain_hash: &plain_hash_placeholder,
        };

        let (private_key, public_key) = generate_x25519_keypair();
        let recipients = vec![RecipientKey { kid: 1, public_key }];

        let enc_result = stateset_crypto::encrypt::encrypt_payload(
            &payload, &aad_params, &recipients,
        ).expect("encryption should succeed");

        // Compute AAD for decryption using the actual plain hash
        let dec_aad_params = PayloadAadParams {
            payload_plain_hash: &enc_result.payload_plain_hash,
            ..aad_params
        };
        let dec_payload_aad = compute_payload_aad(&dec_aad_params)
            .expect("AAD computation should succeed");

        let decrypted = stateset_crypto::encrypt::decrypt_payload(
            &enc_result.payload_encrypted,
            &dec_payload_aad,
            1,
            &private_key,
            &enc_result.payload_plain_hash,
        ).expect("decryption should succeed");

        prop_assert_eq!(
            &decrypted, &payload,
            "encrypt/decrypt round-trip should recover original payload",
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Domain hash determinism (payload_plain_hash)
//
//    For any payload and optional salt, calling the hash function twice
//    produces the same 32-byte result.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn domain_hash_deterministic(json in arb_json_value()) {
        let h1 = compute_payload_plain_hash(&json, None)
            .expect("hash should succeed");
        let h2 = compute_payload_plain_hash(&json, None)
            .expect("hash should succeed");
        prop_assert_eq!(
            h1, h2,
            "domain hash was not deterministic",
        );
        prop_assert_eq!(h1.len(), 32);
    }
}

// ---------------------------------------------------------------------------
// 7. Domain hash with salt determinism
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn domain_hash_with_salt_deterministic(
        json in arb_json_value(),
        salt in prop::array::uniform16(any::<u8>()),
    ) {
        let h1 = compute_payload_plain_hash(&json, Some(&salt))
            .expect("hash should succeed");
        let h2 = compute_payload_plain_hash(&json, Some(&salt))
            .expect("hash should succeed");
        prop_assert_eq!(h1, h2);
    }
}

// ---------------------------------------------------------------------------
// 8. Merkle node hash determinism (sub-property)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn merkle_node_hash_deterministic(
        left in arb_32_bytes(),
        right in arb_32_bytes(),
    ) {
        let h1 = compute_node_hash(&left, &right);
        let h2 = compute_node_hash(&left, &right);
        prop_assert_eq!(h1, h2);
    }
}

// ---------------------------------------------------------------------------
// 9. Salted vs unsalted hashes differ
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn salted_hash_differs_from_unsalted(
        json in arb_json_value(),
        salt in prop::array::uniform16(any::<u8>()),
    ) {
        let unsalted = compute_payload_plain_hash(&json, None)
            .expect("unsalted hash should succeed");
        let salted = compute_payload_plain_hash(&json, Some(&salt))
            .expect("salted hash should succeed");
        // With overwhelmingly high probability, these differ
        prop_assert_ne!(
            unsalted, salted,
            "salted and unsalted hashes should differ for the same payload",
        );
    }
}

// ---------------------------------------------------------------------------
// 10. Ed25519 signature is 64 bytes
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn signature_is_64_bytes(message in arb_hash()) {
        let (private_key, _) = generate_keypair();
        let sig = sign_event_hash(&message, &private_key)
            .expect("signing should not fail");
        prop_assert_eq!(sig.len(), 64);
    }
}

// ---------------------------------------------------------------------------
// 11. Ed25519 signatures are deterministic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn ed25519_deterministic(message in arb_hash()) {
        let (private_key, _) = generate_keypair();
        let sig1 = sign_event_hash(&message, &private_key)
            .expect("first signing should not fail");
        let sig2 = sign_event_hash(&message, &private_key)
            .expect("second signing should not fail");
        prop_assert_eq!(
            sig1, sig2,
            "Ed25519 signatures should be deterministic for the same key and message",
        );
    }
}
