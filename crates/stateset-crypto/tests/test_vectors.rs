//! Cross-language crypto test vectors
//!
//! These tests verify that the Rust implementation in `stateset-crypto`
//! produces IDENTICAL output to the JS implementation in
//! `cli/src/sync/crypto.js`. The expected hex values are hardcoded and
//! shared between both test suites so any drift is caught immediately.
//!
//! Counterpart: `cli/test/unit/crypto-vectors.test.js`

use serde_json::json;
use stateset_crypto::canonicalize::canonicalize_json;
use stateset_crypto::hash::{
    EventSigningParams, PayloadAadParams, PayloadCipherParams, compute_event_signing_hash,
    compute_legacy_payload_hash, compute_payload_aad, compute_payload_cipher_hash,
    compute_payload_plain_hash, compute_recipients_hash,
};
use stateset_crypto::merkle::{
    LeafParams, ReceiptParams, compute_leaf_hash, compute_node_hash, compute_pad_leaf,
    compute_receipt_hash, compute_stream_id,
};
use stateset_crypto::{bytes_to_hex, encode_string, u32_be, u64_be, uuid_to_bytes};

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

// =============================================================================
// 1. JCS Canonicalization
// =============================================================================

#[test]
fn jcs_null() {
    assert_eq!(canonicalize_json(&json!(null)).unwrap(), "null");
}

#[test]
fn jcs_true() {
    assert_eq!(canonicalize_json(&json!(true)).unwrap(), "true");
}

#[test]
fn jcs_false() {
    assert_eq!(canonicalize_json(&json!(false)).unwrap(), "false");
}

#[test]
fn jcs_integer_42() {
    assert_eq!(canonicalize_json(&json!(42)).unwrap(), "42");
}

#[test]
fn jcs_string_hello() {
    assert_eq!(canonicalize_json(&json!("hello")).unwrap(), "\"hello\"");
}

#[test]
fn jcs_object_sorted_keys() {
    let val = json!({"b": 2, "a": 1});
    assert_eq!(canonicalize_json(&val).unwrap(), "{\"a\":1,\"b\":2}");
}

#[test]
fn jcs_array_preserves_order() {
    assert_eq!(canonicalize_json(&json!([3, 1, 2])).unwrap(), "[3,1,2]");
}

#[test]
fn jcs_nested_object() {
    let val = json!({"z": {"b": 2, "a": 1}, "a": []});
    assert_eq!(canonicalize_json(&val).unwrap(), "{\"a\":[],\"z\":{\"a\":1,\"b\":2}}");
}

#[test]
fn jcs_key_value() {
    assert_eq!(canonicalize_json(&json!({"key": "value"})).unwrap(), "{\"key\":\"value\"}");
}

// =============================================================================
// 2. Encoding helpers
// =============================================================================

#[test]
fn encoding_u32_be_zero() {
    assert_eq!(bytes_to_hex(&u32_be(0)), "0x00000000");
}

#[test]
fn encoding_u32_be_one() {
    assert_eq!(bytes_to_hex(&u32_be(1)), "0x00000001");
}

#[test]
fn encoding_u32_be_256() {
    assert_eq!(bytes_to_hex(&u32_be(256)), "0x00000100");
}

#[test]
fn encoding_u32_be_max() {
    assert_eq!(bytes_to_hex(&u32_be(u32::MAX)), "0xffffffff");
}

#[test]
fn encoding_u64_be_zero() {
    assert_eq!(bytes_to_hex(&u64_be(0)), "0x0000000000000000");
}

#[test]
fn encoding_u64_be_one() {
    assert_eq!(bytes_to_hex(&u64_be(1)), "0x0000000000000001");
}

#[test]
fn encoding_u64_be_42() {
    assert_eq!(bytes_to_hex(&u64_be(42)), "0x000000000000002a");
}

#[test]
fn encoding_encode_string_hello() {
    assert_eq!(bytes_to_hex(&encode_string("hello")), "0x0000000568656c6c6f");
}

#[test]
fn encoding_encode_string_empty() {
    assert_eq!(bytes_to_hex(&encode_string("")), "0x00000000");
}

#[test]
fn encoding_uuid_to_bytes() {
    let bytes = uuid_to_bytes(TEST_UUID).unwrap();
    assert_eq!(bytes_to_hex(&bytes), "0x550e8400e29b41d4a716446655440000");
}

// =============================================================================
// 3. Domain-separated hashing (deterministic)
// =============================================================================

#[test]
fn hash_payload_plain_no_salt() {
    let payload = json!({"key": "value"});
    let hash = compute_payload_plain_hash(&payload, None).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0x618fdef1f66e6d7ae46216d2b7a778898e02137c502255397d277dd3c8727bca"
    );
}

#[test]
fn hash_payload_plain_with_zeros_salt() {
    let payload = json!({"key": "value"});
    let salt = [0u8; 16];
    let hash = compute_payload_plain_hash(&payload, Some(&salt)).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0xdf9c1da34c08c2c46e3ac7d850e8b90271a02cf0e8cc4e820dead6c89e7bbdf7"
    );
}

#[test]
fn hash_legacy_payload() {
    let payload = json!({"key": "value"});
    let hash = compute_legacy_payload_hash(&payload).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0xe43abcf3375244839c012f9633f95862d232a95b00d5bc7348b3098b9fed7f32"
    );
}

#[test]
fn hash_payload_cipher_none_returns_zero() {
    let hash = compute_payload_cipher_hash(None);
    assert_eq!(
        bytes_to_hex(&hash),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(hash, stateset_crypto::ZERO_HASH);
}

#[test]
fn hash_payload_cipher_with_params() {
    let params = PayloadCipherParams {
        nonce: &[0u8; 12],
        payload_aad: &[1u8; 32],
        ciphertext: b"encrypted_data",
        tag: &[2u8; 16],
        recipients_hash: &[3u8; 32],
    };
    let hash = compute_payload_cipher_hash(Some(&params));
    assert_eq!(
        bytes_to_hex(&hash),
        "0xd75837f51bbb8cbdfddc4c3838e9e183939cbe506b4ada53ac56a6878c98631c"
    );
}

// =============================================================================
// 4. Stream ID
// =============================================================================

#[test]
fn stream_id_with_test_uuid() {
    let id = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
    assert_eq!(
        bytes_to_hex(&id),
        "0x399cd60b39a2c65ab8a50de811a4d2a8efa8e191961a8fbd9bcc21174b0dd731"
    );
}

// =============================================================================
// 5. Merkle hashing
// =============================================================================

#[test]
fn merkle_pad_leaf() {
    let pad = compute_pad_leaf();
    assert_eq!(
        bytes_to_hex(&pad),
        "0xd9dd0e003ba5370a698013c48ed69c6c41d9ebc1236d44b280c52ceacfdad524"
    );
}

#[test]
fn merkle_node_hash_all_1s_all_2s() {
    let left = [1u8; 32];
    let right = [2u8; 32];
    let hash = compute_node_hash(&left, &right);
    assert_eq!(
        bytes_to_hex(&hash),
        "0x5186fbc7094f70b9fc71bcf269fda0530c1c2bd675de918ef39562a6f18752fd"
    );
}

// =============================================================================
// 6. Event signing hash
// =============================================================================

#[test]
fn event_signing_hash_known_params() {
    let zero_hash = [0u8; 32];
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
        payload_plain_hash: &zero_hash,
        payload_cipher_hash: &zero_hash,
    };
    let hash = compute_event_signing_hash(&params).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0xdfc1efa1fb34966a13ed60a1d92a9f8ec56d4bf9bef521ed0808bcb43d069235"
    );
}

// =============================================================================
// 7. Payload AAD
// =============================================================================

#[test]
fn payload_aad_known_params() {
    let zero_hash = [0u8; 32];
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
        payload_plain_hash: &zero_hash,
    };
    let aad = compute_payload_aad(&params).unwrap();
    assert_eq!(
        bytes_to_hex(&aad),
        "0xcdc1245d41bb28b1e9a5c49bfd76f32c276bf6c42f6cb68cd3990df80c4e7905"
    );
}

// =============================================================================
// 8. Leaf hash
// =============================================================================

#[test]
fn leaf_hash_known_params() {
    let zero_hash = [0u8; 32];
    let zero_sig = [0u8; 64];
    let params = LeafParams {
        tenant_id: TEST_UUID,
        store_id: TEST_UUID,
        sequence_number: 1,
        event_signing_hash: &zero_hash,
        agent_signature: &zero_sig,
    };
    let hash = compute_leaf_hash(&params).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0x6cefa2e2572cf1223d741e18caeb5dc3732b7e1e99fbab361883acd5be63fb48"
    );
}

// =============================================================================
// 9. Receipt hash
// =============================================================================

#[test]
fn receipt_hash_known_params() {
    let zero_hash = [0u8; 32];
    let params = ReceiptParams {
        tenant_id: TEST_UUID,
        store_id: TEST_UUID,
        event_id: TEST_UUID,
        sequence_number: 42,
        event_signing_hash: &zero_hash,
    };
    let hash = compute_receipt_hash(&params).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0x90be3aa44a2d74ea2688c5d583053247acdd47e2bbbd80db73b683cb7329638a"
    );
}

// =============================================================================
// 10. Recipients hash
// =============================================================================

#[test]
fn recipients_hash_sorts_by_kid() {
    let recipients = vec![
        json!({"recipient_kid": 2, "enc_b64u": "a", "ct_b64u": "b"}),
        json!({"recipient_kid": 1, "enc_b64u": "c", "ct_b64u": "d"}),
    ];
    let hash = compute_recipients_hash(&recipients).unwrap();
    assert_eq!(
        bytes_to_hex(&hash),
        "0x9209fbf107e6f97f3fe2c4179d90e8ab7be79d1528eeaeb82b83f2b832c91d94"
    );
}
