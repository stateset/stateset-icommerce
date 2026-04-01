//! Expanded crypto tests covering Merkle trees, hashing, signing,
//! encoding, canonicalization, PQC, and edge cases.

use serde_json::json;
use stateset_crypto::canonicalize::*;
use stateset_crypto::hash::*;
use stateset_crypto::merkle::*;
#[cfg(feature = "pqc")]
use stateset_crypto::pqc::*;
use stateset_crypto::sign::*;
use stateset_crypto::*;

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_UUID2: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

// ---------------------------------------------------------------------------
// 1. Merkle tree construction and proof verification
// ---------------------------------------------------------------------------

#[test]
fn merkle_root_empty_returns_pad_leaf() {
    let root = compute_merkle_root(&[]);
    assert_eq!(root, compute_pad_leaf());
}

#[test]
fn merkle_root_single_leaf_returns_leaf() {
    let leaf = [42u8; 32];
    let root = compute_merkle_root(&[leaf]);
    assert_eq!(root, leaf);
}

#[test]
fn merkle_root_two_leaves_equals_node_hash() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let root = compute_merkle_root(&[a, b]);
    let expected = compute_node_hash(&a, &b);
    assert_eq!(root, expected);
}

#[test]
fn merkle_root_three_leaves_pads_to_four() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let c = [3u8; 32];
    let pad = compute_pad_leaf();
    let root = compute_merkle_root(&[a, b, c]);
    let left = compute_node_hash(&a, &b);
    let right = compute_node_hash(&c, &pad);
    let expected = compute_node_hash(&left, &right);
    assert_eq!(root, expected);
}

#[test]
fn merkle_root_four_leaves_exact_power_of_two() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let c = [3u8; 32];
    let d = [4u8; 32];
    let root = compute_merkle_root(&[a, b, c, d]);
    let left = compute_node_hash(&a, &b);
    let right = compute_node_hash(&c, &d);
    let expected = compute_node_hash(&left, &right);
    assert_eq!(root, expected);
}

#[test]
fn merkle_root_five_leaves_pads_to_eight() {
    let leaves: Vec<[u8; 32]> = (0..5).map(|i| [i as u8; 32]).collect();
    let pad = compute_pad_leaf();
    let root = compute_merkle_root(&leaves);
    // Should pad to 8 leaves
    let mut padded = leaves.clone();
    padded.resize(8, pad);
    // Manually compute for 8 leaves
    let l01 = compute_node_hash(&padded[0], &padded[1]);
    let l23 = compute_node_hash(&padded[2], &padded[3]);
    let l45 = compute_node_hash(&padded[4], &padded[5]);
    let l67 = compute_node_hash(&padded[6], &padded[7]);
    let l0123 = compute_node_hash(&l01, &l23);
    let l4567 = compute_node_hash(&l45, &l67);
    let expected = compute_node_hash(&l0123, &l4567);
    assert_eq!(root, expected);
}

#[test]
fn merkle_root_deterministic() {
    let leaves: Vec<[u8; 32]> = (0..7).map(|i| [i as u8; 32]).collect();
    let r1 = compute_merkle_root(&leaves);
    let r2 = compute_merkle_root(&leaves);
    assert_eq!(r1, r2);
}

#[test]
fn merkle_node_hash_order_matters() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    assert_ne!(compute_node_hash(&a, &b), compute_node_hash(&b, &a));
}

#[test]
fn merkle_pad_leaf_is_deterministic() {
    let p1 = compute_pad_leaf();
    let p2 = compute_pad_leaf();
    assert_eq!(p1, p2);
    assert_eq!(p1.len(), 32);
}

#[test]
fn merkle_leaf_hash_deterministic() {
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
    assert_eq!(h1.len(), 32);
}

#[test]
fn merkle_leaf_hash_varies_with_sequence() {
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
fn merkle_leaf_hash_invalid_uuid_fails() {
    let event_hash = [0u8; 32];
    let sig = [0u8; 64];
    let params = LeafParams {
        tenant_id: "not-a-uuid",
        store_id: TEST_UUID,
        sequence_number: 1,
        event_signing_hash: &event_hash,
        agent_signature: &sig,
    };
    assert!(compute_leaf_hash(&params).is_err());
}

// ---------------------------------------------------------------------------
// 2. Hashing
// ---------------------------------------------------------------------------

#[test]
fn payload_plain_hash_deterministic() {
    let payload = json!({"key": "value"});
    let h1 = compute_payload_plain_hash(&payload, None).unwrap();
    let h2 = compute_payload_plain_hash(&payload, None).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 32);
}

#[test]
fn payload_plain_hash_with_salt_differs() {
    let payload = json!({"key": "value"});
    let salt = [0u8; 16];
    let salted = compute_payload_plain_hash(&payload, Some(&salt)).unwrap();
    let unsalted = compute_payload_plain_hash(&payload, None).unwrap();
    assert_ne!(salted, unsalted);
}

#[test]
fn payload_plain_hash_different_payloads_differ() {
    let h1 = compute_payload_plain_hash(&json!({"a": 1}), None).unwrap();
    let h2 = compute_payload_plain_hash(&json!({"b": 2}), None).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn legacy_payload_hash_differs_from_plain() {
    let payload = json!({"key": "value"});
    let legacy = compute_legacy_payload_hash(&payload).unwrap();
    let plain = compute_payload_plain_hash(&payload, None).unwrap();
    assert_ne!(legacy, plain);
}

#[test]
fn payload_cipher_hash_none_returns_zeros() {
    let hash = compute_payload_cipher_hash(None);
    assert_eq!(hash, [0u8; 32]);
}

#[test]
fn payload_cipher_hash_with_params_nonzero() {
    let params = PayloadCipherParams {
        nonce: &[0u8; 12],
        payload_aad: &[1u8; 32],
        ciphertext: b"encrypted_data",
        tag: &[2u8; 16],
        recipients_hash: &[3u8; 32],
    };
    let hash = compute_payload_cipher_hash(Some(&params));
    assert_ne!(hash, [0u8; 32]);
    assert_eq!(hash.len(), 32);
}

#[test]
fn event_signing_hash_deterministic() {
    let plain_hash = [0u8; 32];
    let cipher_hash = [0u8; 32];
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
        payload_plain_hash: &plain_hash,
        payload_cipher_hash: &cipher_hash,
    };
    let h1 = compute_event_signing_hash(&params).unwrap();
    let h2 = compute_event_signing_hash(&params).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn event_signing_hash_varies_with_event_type() {
    let plain_hash = [0u8; 32];
    let cipher_hash = [0u8; 32];
    let base = EventSigningParams {
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
        payload_plain_hash: &plain_hash,
        payload_cipher_hash: &cipher_hash,
    };
    let h1 = compute_event_signing_hash(&base).unwrap();
    let modified = EventSigningParams { event_type: "order.cancelled", ..base };
    let h2 = compute_event_signing_hash(&modified).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn event_signing_hash_invalid_uuid_fails() {
    let plain_hash = [0u8; 32];
    let cipher_hash = [0u8; 32];
    let params = EventSigningParams {
        ves_version: 1,
        tenant_id: "bad-uuid",
        store_id: TEST_UUID,
        event_id: TEST_UUID,
        source_agent_id: TEST_UUID,
        agent_key_id: 1,
        entity_type: "order",
        entity_id: "ord_001",
        event_type: "order.created",
        created_at: "2026-02-21T00:00:00Z",
        payload_kind: 0,
        payload_plain_hash: &plain_hash,
        payload_cipher_hash: &cipher_hash,
    };
    assert!(compute_event_signing_hash(&params).is_err());
}

#[test]
fn recipients_hash_order_independent() {
    let r1 = json!({"recipient_kid": 2, "enc_b64u": "a", "ct_b64u": "b"});
    let r2 = json!({"recipient_kid": 1, "enc_b64u": "c", "ct_b64u": "d"});
    let h1 = compute_recipients_hash(&[r1.clone(), r2.clone()]).unwrap();
    let h2 = compute_recipients_hash(&[r2, r1]).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn stream_id_deterministic() {
    let s1 = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
    let s2 = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
    assert_eq!(s1, s2);
}

#[test]
fn stream_id_varies_with_store() {
    let s1 = compute_stream_id(TEST_UUID, TEST_UUID).unwrap();
    let s2 = compute_stream_id(TEST_UUID, TEST_UUID2).unwrap();
    assert_ne!(s1, s2);
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

// ---------------------------------------------------------------------------
// 3. Key generation and signing
// ---------------------------------------------------------------------------

#[test]
fn generate_keypair_produces_valid_keys() {
    let (private_key, public_key) = generate_keypair();
    assert_eq!(private_key.len(), 32);
    assert_eq!(public_key.len(), 32);
    // Keys should not be all zeros
    assert_ne!(private_key, [0u8; 32]);
    assert_ne!(public_key, [0u8; 32]);
}

#[test]
fn generate_keypair_produces_different_keys() {
    let (pk1, _) = generate_keypair();
    let (pk2, _) = generate_keypair();
    // Different calls should produce different keys (probability of collision negligible)
    assert_ne!(pk1, pk2);
}

#[test]
fn sign_and_verify_roundtrip() {
    let (private_key, public_key) = generate_keypair();
    let hash = [42u8; 32];
    let signature = sign_event_hash(&hash, &private_key).unwrap();
    assert_eq!(signature.len(), 64);
    assert!(verify_event_signature(&hash, &signature, &public_key));
}

#[test]
fn signature_deterministic_for_same_key_and_message() {
    let (private_key, _) = generate_keypair();
    let hash = [42u8; 32];
    let sig1 = sign_event_hash(&hash, &private_key).unwrap();
    let sig2 = sign_event_hash(&hash, &private_key).unwrap();
    assert_eq!(sig1, sig2);
}

#[test]
fn verify_wrong_key_fails() {
    let (private_key, _) = generate_keypair();
    let (_, other_public) = generate_keypair();
    let hash = [42u8; 32];
    let signature = sign_event_hash(&hash, &private_key).unwrap();
    assert!(!verify_event_signature(&hash, &signature, &other_public));
}

#[test]
fn verify_wrong_hash_fails() {
    let (private_key, public_key) = generate_keypair();
    let hash = [42u8; 32];
    let other_hash = [99u8; 32];
    let signature = sign_event_hash(&hash, &private_key).unwrap();
    assert!(!verify_event_signature(&other_hash, &signature, &public_key));
}

#[test]
fn verify_tampered_signature_fails() {
    let (private_key, public_key) = generate_keypair();
    let hash = [42u8; 32];
    let mut signature = sign_event_hash(&hash, &private_key).unwrap();
    signature[0] ^= 0xFF;
    assert!(!verify_event_signature(&hash, &signature, &public_key));
}

#[test]
fn verify_invalid_public_key_returns_false() {
    let (private_key, _) = generate_keypair();
    let hash = [42u8; 32];
    let signature = sign_event_hash(&hash, &private_key).unwrap();
    let bad_key = [0u8; 32]; // Not a valid Ed25519 point
    assert!(!verify_event_signature(&hash, &signature, &bad_key));
}

#[test]
fn sign_different_messages_produce_different_signatures() {
    let (private_key, _) = generate_keypair();
    let hash1 = [1u8; 32];
    let hash2 = [2u8; 32];
    let sig1 = sign_event_hash(&hash1, &private_key).unwrap();
    let sig2 = sign_event_hash(&hash2, &private_key).unwrap();
    assert_ne!(sig1, sig2);
}

// ---------------------------------------------------------------------------
// 4. Encoding
// ---------------------------------------------------------------------------

#[test]
fn uuid_to_bytes_valid() {
    let bytes = uuid_to_bytes(TEST_UUID).unwrap();
    assert_eq!(bytes.len(), 16);
    assert_eq!(bytes[0], 0x55);
}

#[test]
fn uuid_to_bytes_invalid() {
    assert!(uuid_to_bytes("not-a-uuid").is_err());
    assert!(uuid_to_bytes("").is_err());
    assert!(uuid_to_bytes("12345").is_err());
}

#[test]
fn hex_to_bytes_with_prefix() {
    let bytes = hex_to_bytes("0xdeadbeef").unwrap();
    assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn hex_to_bytes_without_prefix() {
    let bytes = hex_to_bytes("deadbeef").unwrap();
    assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn hex_to_bytes_invalid() {
    assert!(hex_to_bytes("xyz").is_err());
    assert!(hex_to_bytes("0xgg").is_err());
}

#[test]
fn bytes_to_hex_roundtrip() {
    let original = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let hex_str = bytes_to_hex(&original);
    assert_eq!(hex_str, "0xdeadbeef");
    let back = hex_to_bytes(&hex_str).unwrap();
    assert_eq!(back, original);
}

#[test]
fn bytes_to_hex_empty() {
    assert_eq!(bytes_to_hex(&[]), "0x");
}

#[test]
fn encode_string_empty() {
    let result = encode_string("");
    assert_eq!(result, vec![0, 0, 0, 0]);
}

#[test]
fn encode_string_hello() {
    let result = encode_string("hello");
    assert_eq!(&result[..4], &[0, 0, 0, 5]);
    assert_eq!(&result[4..], b"hello");
}

#[test]
fn u32_be_encoding() {
    assert_eq!(u32_be(0), [0, 0, 0, 0]);
    assert_eq!(u32_be(1), [0, 0, 0, 1]);
    assert_eq!(u32_be(256), [0, 0, 1, 0]);
    assert_eq!(u32_be(u32::MAX), [0xFF, 0xFF, 0xFF, 0xFF]);
}

#[test]
fn u64_be_encoding() {
    assert_eq!(u64_be(0), [0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(u64_be(1), [0, 0, 0, 0, 0, 0, 0, 1]);
}

// ---------------------------------------------------------------------------
// 5. Canonicalization
// ---------------------------------------------------------------------------

#[test]
fn canonicalize_sorts_object_keys() {
    let val = json!({"b": 2, "a": 1});
    let result = canonicalize_json(&val).unwrap();
    assert_eq!(result, "{\"a\":1,\"b\":2}");
}

#[test]
fn canonicalize_nested_object() {
    let val = json!({"z": {"b": 2, "a": 1}, "a": []});
    let result = canonicalize_json(&val).unwrap();
    assert_eq!(result, "{\"a\":[],\"z\":{\"a\":1,\"b\":2}}");
}

#[test]
fn canonicalize_primitives() {
    assert_eq!(canonicalize_json(&json!(null)).unwrap(), "null");
    assert_eq!(canonicalize_json(&json!(true)).unwrap(), "true");
    assert_eq!(canonicalize_json(&json!(false)).unwrap(), "false");
    assert_eq!(canonicalize_json(&json!(42)).unwrap(), "42");
    assert_eq!(canonicalize_json(&json!("hello")).unwrap(), "\"hello\"");
}

#[test]
fn canonicalize_array() {
    assert_eq!(canonicalize_json(&json!([])).unwrap(), "[]");
    assert_eq!(canonicalize_json(&json!([1, "two", true])).unwrap(), "[1,\"two\",true]");
}

#[test]
fn canonicalize_bytes_variant() {
    let bytes = canonicalize_json_bytes(&json!({"key": "value"})).unwrap();
    assert_eq!(bytes, b"{\"key\":\"value\"}");
}

// ---------------------------------------------------------------------------
// 6. Error types
// ---------------------------------------------------------------------------

#[test]
fn crypto_error_display() {
    let e = CryptoError::InvalidUuid("bad".into());
    assert!(e.to_string().contains("bad"));

    let e = CryptoError::InvalidHex("xyz".into());
    assert!(e.to_string().contains("xyz"));

    let e = CryptoError::InvalidSalt;
    assert!(e.to_string().contains("16 bytes"));

    let e = CryptoError::NoRecipients;
    assert!(e.to_string().contains("recipient"));

    let e = CryptoError::PayloadHashMismatch;
    assert!(e.to_string().contains("mismatch"));

    let e = CryptoError::JcsInvalidNumber;
    assert!(e.to_string().contains("Infinity"));
}

// ---------------------------------------------------------------------------
// 7. Domain constants
// ---------------------------------------------------------------------------

#[test]
fn domain_prefixes_are_unique() {
    let domains: Vec<&[u8]> = vec![
        stateset_crypto::domain::PAYLOAD_PLAIN,
        stateset_crypto::domain::PAYLOAD_AAD,
        stateset_crypto::domain::PAYLOAD_CIPHER,
        stateset_crypto::domain::RECIPIENTS,
        stateset_crypto::domain::EVENTSIG,
        stateset_crypto::domain::LEAF,
        stateset_crypto::domain::NODE,
        stateset_crypto::domain::PAD_LEAF,
        stateset_crypto::domain::STREAM,
        stateset_crypto::domain::RECEIPT,
    ];
    for i in 0..domains.len() {
        for j in (i + 1)..domains.len() {
            assert_ne!(domains[i], domains[j], "Domain prefix collision at indices {i} and {j}");
        }
    }
}

#[test]
fn zero_hash_constant() {
    assert_eq!(stateset_crypto::ZERO_HASH, [0u8; 32]);
    assert_eq!(stateset_crypto::ZERO_HASH.len(), 32);
}

// ---------------------------------------------------------------------------
// PQC integration tests
// ---------------------------------------------------------------------------

#[cfg(feature = "pqc")]
mod pqc_integration {
    use super::*;
    use serde_json::json;
    use stateset_crypto::hash::{PayloadAadParams, compute_payload_aad, compute_payload_plain_hash};
    use stateset_crypto::pqc::*;

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn test_aad_params(plain_hash: &[u8; 32]) -> PayloadAadParams<'_> {
        PayloadAadParams {
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
            payload_plain_hash: plain_hash,
        }
    }

    // -- Security profile serde --

    #[test]
    fn security_profile_deserializes_from_kebab_case() {
        let profiles: Vec<SecurityProfile> =
            serde_json::from_str(r#"["legacy", "hybrid", "pqc-strict"]"#).unwrap();
        assert_eq!(
            profiles,
            vec![SecurityProfile::Legacy, SecurityProfile::Hybrid, SecurityProfile::PqcStrict]
        );
    }

    // -- Hybrid sign + encrypt end-to-end --

    #[test]
    fn hybrid_sign_and_encrypt_end_to_end() {
        let signing_kp = generate_hybrid_signing_keypair().unwrap();
        let recipient_kp = generate_hybrid_recipient_keypair(1).unwrap();
        let payload = json!({"order_id": "ORD-E2E", "total": 999});

        // Sign
        let event_hash = [0xAA; 32];
        let sig = hybrid_sign_event_hash(&event_hash, &signing_kp.private).unwrap();
        assert!(hybrid_verify_event_signature(&event_hash, &sig, &signing_kp.public));

        // Encrypt
        let pph = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&pph);
        let enc =
            encrypt_payload_hybrid(&payload, &aad, &[recipient_kp.public.clone()]).unwrap();

        // Decrypt
        let dec_aad =
            PayloadAadParams { payload_plain_hash: &enc.payload_plain_hash, ..aad };
        let paad = compute_payload_aad(&dec_aad).unwrap();
        let dec = decrypt_payload_hybrid(
            &enc.payload_encrypted,
            &paad,
            1,
            &recipient_kp.private,
            &enc.payload_plain_hash,
        )
        .unwrap();
        assert_eq!(dec, payload);
    }

    // -- Strict sign + encrypt end-to-end --

    #[test]
    fn strict_sign_and_encrypt_end_to_end() {
        let signing_kp = generate_strict_signing_keypair().unwrap();
        let recipient_kp = generate_strict_recipient_keypair(1).unwrap();
        let payload = json!({"order_id": "ORD-STRICT", "total": 500});

        let event_hash = [0xBB; 32];
        let sig = strict_sign_event_hash(&event_hash, &signing_kp.private).unwrap();
        assert!(strict_verify_event_signature(&event_hash, &sig, &signing_kp.public));

        let pph = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&pph);
        let enc = encrypt_payload_strict(&payload, &aad, &[recipient_kp.public.clone()]).unwrap();

        let dec_aad =
            PayloadAadParams { payload_plain_hash: &enc.payload_plain_hash, ..aad };
        let paad = compute_payload_aad(&dec_aad).unwrap();
        let dec = decrypt_payload_strict(
            &enc.payload_encrypted,
            &paad,
            1,
            &recipient_kp.private,
            &enc.payload_plain_hash,
        )
        .unwrap();
        assert_eq!(dec, payload);
    }

    // -- PoP integration --

    #[test]
    fn hybrid_pop_integrated_with_key_registration() {
        let kp = generate_hybrid_signing_keypair().unwrap();
        let pop = generate_hybrid_signing_pop(&kp).unwrap();

        // Serialize PoP (simulates key registration wire format)
        let pop_json = serde_json::to_string(&pop).unwrap();
        let pop_deserialized: HybridSignatureBundle =
            serde_json::from_str(&pop_json).unwrap();

        // Verify deserialized PoP
        assert!(verify_hybrid_signing_pop(&pop_deserialized, &kp.public));
    }

    #[test]
    fn strict_pop_integrated_with_serde() {
        let kp = generate_strict_signing_keypair().unwrap();
        let pop = generate_strict_signing_pop(&kp).unwrap();

        // Serialize keypair + PoP (simulates persistence)
        let kp_json = serde_json::to_string(&kp).unwrap();
        let kp_restored: StrictSigningKeypair = serde_json::from_str(&kp_json).unwrap();

        assert!(verify_strict_signing_pop(&pop, &kp_restored.public));
    }

    // -- Cross-language test vector helpers --

    #[test]
    fn test_vector_public_key_is_stable() {
        let pk = test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED);
        // The public key should be the same length every time (ML-DSA-65 = 1952 bytes)
        assert_eq!(pk.len(), 1952);

        // Running again should produce identical output (deterministic from seed)
        let pk2 = test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED);
        assert_eq!(pk, pk2);
    }

    #[test]
    fn test_vector_different_seeds_produce_different_keys() {
        let pk1 = test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED);
        let pk2 = test_vector_ml_dsa_public_key(&[0xFF; 32]);
        assert_ne!(pk1, pk2);
    }

    // -- Receipt signing integration --

    #[test]
    fn hybrid_receipt_signing_matches_event_signing() {
        let kp = generate_hybrid_signing_keypair().unwrap();
        let hash = [0xDD; 32];

        let event_sig = hybrid_sign_event_hash(&hash, &kp.private).unwrap();
        let receipt_sig = hybrid_sign_receipt_hash(&hash, &kp.private).unwrap();

        // Both produce valid signatures (may differ due to ML-DSA randomness)
        assert!(hybrid_verify_event_signature(&hash, &event_sig, &kp.public));
        assert!(hybrid_verify_receipt_signature(&hash, &receipt_sig, &kp.public));
    }

    // -- Serde key persistence simulation --

    #[test]
    fn hybrid_keypair_survives_json_persistence() {
        let kp = generate_hybrid_signing_keypair().unwrap();
        let json = serde_json::to_string_pretty(&kp).unwrap();

        // Simulate write to disk and read back
        let restored: HybridSigningKeypair = serde_json::from_str(&json).unwrap();

        // Original and restored produce verifiable signatures
        let hash = [0xEE; 32];
        let sig = hybrid_sign_event_hash(&hash, &restored.private).unwrap();
        assert!(hybrid_verify_event_signature(&hash, &sig, &restored.public));
    }

    #[test]
    fn strict_keypair_survives_json_persistence() {
        let kp = generate_strict_signing_keypair().unwrap();
        let json = serde_json::to_string_pretty(&kp).unwrap();
        let restored: StrictSigningKeypair = serde_json::from_str(&json).unwrap();

        let hash = [0xFF; 32];
        let sig = strict_sign_event_hash(&hash, &restored.private).unwrap();
        assert!(strict_verify_event_signature(&hash, &sig, &restored.public));
    }

    #[test]
    fn hybrid_recipient_keypair_survives_json_persistence() {
        let kp = generate_hybrid_recipient_keypair(42).unwrap();
        let json = serde_json::to_string_pretty(&kp).unwrap();
        let restored: HybridRecipientKeypair = serde_json::from_str(&json).unwrap();

        let dek = [0x11; 32];
        let wrapped = wrap_dek_hybrid(&dek, &restored.public, b"persist-test").unwrap();
        let recovered = unwrap_dek_hybrid(&wrapped, &restored.private, b"persist-test").unwrap();
        assert_eq!(recovered, dek);
    }
}
