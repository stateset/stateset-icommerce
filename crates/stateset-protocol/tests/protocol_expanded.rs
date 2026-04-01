//! Expanded integration tests for stateset-protocol.
//!
//! Covers envelope serialization, batch construction, Merkle proofs,
//! version negotiation, handshake validation, canonical JSON, domain hashing,
//! and protocol/schema version newtypes.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use stateset_protocol::canonical::{canonical_json, domain_hash};
use stateset_protocol::merkle::{
    ZERO_HASH, compute_merkle_proof, compute_merkle_root, verify_merkle_proof,
};
use stateset_protocol::{
    BatchSignature, EventEnvelope, MerkleLeafHashMode, MerkleProof, PayloadCodec, ProtocolError,
    ProtocolVersion, SchemaVersion, SignatureAlgorithm, SyncBatch,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_envelope(event_type: &str, entity_id: &str, payload: &[u8]) -> EventEnvelope {
    EventEnvelope::builder()
        .event_type(event_type)
        .entity_type("order")
        .entity_id(entity_id)
        .payload(payload.to_vec())
        .build()
        .unwrap()
}

fn make_envelope_with_codec(payload: &[u8], codec: PayloadCodec) -> EventEnvelope {
    EventEnvelope::builder()
        .event_type("test.event")
        .entity_type("test")
        .entity_id("t_1")
        .payload(payload.to_vec())
        .codec(codec)
        .build()
        .unwrap()
}

// ===========================================================================
// Envelope serialization tests
// ===========================================================================

#[test]
fn envelope_json_serde_roundtrip_full() {
    let corr = Uuid::new_v4();
    let cause = Uuid::new_v4();
    let envelope = EventEnvelope::builder()
        .event_type("order.created")
        .entity_type("order")
        .entity_id("ord_42")
        .payload(br#"{"total": 100}"#.to_vec())
        .correlation_id(corr)
        .causation_id(cause)
        .sequence(7)
        .schema_version(2)
        .build()
        .unwrap();

    let json = serde_json::to_string(&envelope).unwrap();
    let deserialized: EventEnvelope = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, envelope.id);
    assert_eq!(deserialized.sequence, 7);
    assert_eq!(deserialized.event_type, "order.created");
    assert_eq!(deserialized.entity_type, "order");
    assert_eq!(deserialized.entity_id, "ord_42");
    assert_eq!(deserialized.correlation_id, Some(corr));
    assert_eq!(deserialized.causation_id, Some(cause));
    assert_eq!(deserialized.payload_codec, PayloadCodec::Json);
    assert_eq!(deserialized.protocol_version, 1);
    assert_eq!(deserialized.schema_version, 2);
    assert_eq!(deserialized.payload_hash, envelope.payload_hash);
}

#[test]
fn envelope_different_codecs_produce_different_leaf_hashes() {
    let payload = b"test data".to_vec();
    let json_env = make_envelope_with_codec(&payload, PayloadCodec::Json);
    let cbor_env = make_envelope_with_codec(&payload, PayloadCodec::Cbor);
    let msgpack_env = make_envelope_with_codec(&payload, PayloadCodec::MessagePack);

    assert_ne!(json_env.merkle_leaf_hash(), cbor_env.merkle_leaf_hash());
    assert_ne!(json_env.merkle_leaf_hash(), msgpack_env.merkle_leaf_hash());
    assert_ne!(cbor_env.merkle_leaf_hash(), msgpack_env.merkle_leaf_hash());
}

#[test]
fn envelope_payload_hash_integrity() {
    let envelope = make_envelope("order.created", "ord_1", br#"{"total": 42}"#);
    let expected = EventEnvelope::compute_payload_hash(&envelope.payload);
    assert_eq!(envelope.payload_hash, expected);
    assert!(envelope.validate().is_ok());
}

#[test]
fn envelope_tampered_payload_fails_validation() {
    let mut envelope = make_envelope("order.created", "ord_1", b"original");
    envelope.payload = b"tampered".to_vec();
    assert!(envelope.validate().is_err());
}

#[test]
fn envelope_ordering_by_sequence_then_timestamp() {
    let ts1 = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
    let ts2 = DateTime::parse_from_rfc3339("2026-01-02T00:00:00Z").unwrap().with_timezone(&Utc);

    let mut e1 = make_envelope("a", "1", b"x");
    let mut e2 = make_envelope("b", "2", b"y");
    e1.sequence = 1;
    e1.timestamp = ts1;
    e2.sequence = 1;
    e2.timestamp = ts2;

    assert!(e1 < e2); // same sequence, ts1 < ts2
}

#[test]
fn envelope_sort_by_sequence() {
    let mut envelopes: Vec<EventEnvelope> = (0..10)
        .rev()
        .map(|i| {
            let mut e = make_envelope(&format!("evt_{i}"), &format!("id_{i}"), b"p");
            e.sequence = i;
            e
        })
        .collect();

    envelopes.sort();
    for (i, e) in envelopes.iter().enumerate() {
        assert_eq!(e.sequence, i as u64);
    }
}

// ===========================================================================
// Batch construction and validation
// ===========================================================================

#[test]
fn batch_single_event_merkle_root_matches_leaf() {
    let env = make_envelope("order.created", "ord_1", b"data");
    let batch = SyncBatch::new("node_a", vec![env.clone()]);

    assert_eq!(batch.merkle_root, env.merkle_leaf_hash());
    assert!(batch.verify_merkle_root());
    assert!(batch.validate().is_ok());
}

#[test]
fn batch_multiple_events_valid() {
    let envs: Vec<EventEnvelope> = (0..5)
        .map(|i| {
            make_envelope(
                &format!("evt_{i}"),
                &format!("id_{i}"),
                format!("payload_{i}").as_bytes(),
            )
        })
        .collect();

    let batch = SyncBatch::new("node_b", envs);
    assert_eq!(batch.len(), 5);
    assert!(!batch.is_empty());
    assert!(batch.verify_merkle_root());
    assert!(batch.validate().is_ok());
}

#[test]
fn batch_validate_rejects_empty() {
    let batch = SyncBatch::new("node_c", vec![]);
    assert!(batch.validate().is_err());
}

#[test]
fn batch_validate_rejects_empty_source_node() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.source_node_id = String::new();
    assert!(batch.validate().is_err());
}

#[test]
fn batch_validate_rejects_unsupported_protocol_version() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.protocol_version = 99;
    assert!(matches!(batch.validate(), Err(ProtocolError::UnsupportedVersion(_))));
}

#[test]
fn batch_validate_detects_tampered_envelope() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.leaves[0].event_type = "tampered".to_string();
    assert!(!batch.verify_merkle_root());
}

#[test]
fn batch_validate_detects_tampered_root() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.merkle_root = [0xFF; 32];
    assert!(matches!(batch.validate(), Err(ProtocolError::MerkleVerificationFailed(_))));
}

#[test]
fn batch_serde_roundtrip() {
    let envs = vec![make_envelope("a", "1", b"data1"), make_envelope("b", "2", b"data2")];
    let batch = SyncBatch::new("node_x", envs);

    let json = serde_json::to_string(&batch).unwrap();
    let deserialized: SyncBatch = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.batch_id, batch.batch_id);
    assert_eq!(deserialized.source_node_id, batch.source_node_id);
    assert_eq!(deserialized.merkle_root, batch.merkle_root);
    assert_eq!(deserialized.leaves.len(), 2);
    assert!(deserialized.verify_merkle_root());
}

// ===========================================================================
// Batch signature and proof validation
// ===========================================================================

#[test]
fn batch_without_signatures_validates() {
    let envs = vec![make_envelope("t", "1", b"d"), make_envelope("t", "2", b"e")];
    let batch = SyncBatch::new("signer_node", envs);
    // A batch without signatures should validate (signatures are optional)
    assert!(batch.validate().is_ok());
}

#[test]
fn batch_with_wrong_public_key_length_fails() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.add_signature(BatchSignature {
        signer_id: "signer".into(),
        algorithm: SignatureAlgorithm::Ed25519,
        signature: vec![0u8; 64],
        public_key: vec![0u8; 16], // wrong length, should be 32
    });

    assert!(matches!(batch.validate(), Err(ProtocolError::InvalidSignature(_))));
}

#[test]
fn batch_with_invalid_signature_bytes_fails() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.add_signature(BatchSignature {
        signer_id: "signer".into(),
        algorithm: SignatureAlgorithm::Ed25519,
        signature: vec![0xDE, 0xAD],
        public_key: vec![0u8; 32],
    });

    assert!(matches!(batch.validate(), Err(ProtocolError::InvalidSignature(_))));
}

#[test]
fn batch_with_empty_signer_id_fails() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.add_signature(BatchSignature {
        signer_id: "  ".into(),
        algorithm: SignatureAlgorithm::Ed25519,
        signature: vec![0u8; 64],
        public_key: vec![0u8; 32],
    });

    assert!(matches!(batch.validate(), Err(ProtocolError::InvalidSignature(_))));
}

#[test]
fn batch_proof_validates_for_each_leaf() {
    let envs: Vec<EventEnvelope> = (0..4)
        .map(|i| make_envelope(&format!("t_{i}"), &format!("{i}"), format!("d{i}").as_bytes()))
        .collect();
    let mut batch = SyncBatch::new("node", envs);

    // Compute leaf hashes and add proofs for each leaf
    let leaf_hashes: Vec<[u8; 32]> = batch.leaves.iter().map(|e| e.merkle_leaf_hash()).collect();
    for i in 0..4 {
        let proof = compute_merkle_proof(&leaf_hashes, i).unwrap();
        batch.add_proof(proof);
    }

    assert!(batch.validate().is_ok());
}

#[test]
fn batch_proof_with_wrong_leaf_hash_fails() {
    let envs = vec![make_envelope("t", "1", b"d"), make_envelope("t", "2", b"e")];
    let mut batch = SyncBatch::new("node", envs);

    let leaf_hashes: Vec<[u8; 32]> = batch.leaves.iter().map(|e| e.merkle_leaf_hash()).collect();
    let mut proof = compute_merkle_proof(&leaf_hashes, 0).unwrap();
    proof.leaf_hash = [0xAB; 32]; // tamper
    batch.add_proof(proof);

    assert!(matches!(batch.validate(), Err(ProtocolError::MerkleVerificationFailed(_))));
}

// ===========================================================================
// Merkle tree tests
// ===========================================================================

#[test]
fn merkle_root_empty_is_zero() {
    assert_eq!(compute_merkle_root(&[]), ZERO_HASH);
}

#[test]
fn merkle_root_single_leaf_is_identity() {
    let leaf = [42u8; 32];
    assert_eq!(compute_merkle_root(&[leaf]), leaf);
}

#[test]
fn merkle_root_order_sensitive() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    assert_ne!(compute_merkle_root(&[a, b]), compute_merkle_root(&[b, a]));
}

#[test]
fn merkle_proof_roundtrip_for_various_sizes() {
    for size in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32] {
        let leaves: Vec<[u8; 32]> = (0..size).map(|i| [i as u8; 32]).collect();
        let root = compute_merkle_root(&leaves);

        for idx in 0..size {
            let proof = compute_merkle_proof(&leaves, idx).unwrap();
            assert_eq!(proof.root, root);
            assert_eq!(proof.leaf_hash, leaves[idx]);
            assert!(verify_merkle_proof(&proof));
        }
    }
}

#[test]
fn merkle_proof_tampered_sibling_fails() {
    let leaves: Vec<[u8; 32]> = (0..8).map(|i| [i as u8; 32]).collect();
    let mut proof = compute_merkle_proof(&leaves, 3).unwrap();
    proof.siblings[0] = [0xFF; 32];
    assert!(!verify_merkle_proof(&proof));
}

#[test]
fn merkle_proof_out_of_bounds_returns_error() {
    let leaves = vec![[1u8; 32], [2u8; 32]];
    assert!(compute_merkle_proof(&leaves, 5).is_err());
}

#[test]
fn merkle_proof_serde_roundtrip() {
    let leaves: Vec<[u8; 32]> = (0..4).map(|i| [i as u8; 32]).collect();
    let proof = compute_merkle_proof(&leaves, 2).unwrap();
    let json = serde_json::to_string(&proof).unwrap();
    let deserialized: MerkleProof = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, proof);
    assert!(deserialized.verify());
}

// ===========================================================================
// Version negotiation and newtypes
// ===========================================================================

#[test]
fn protocol_version_current_is_one() {
    assert_eq!(ProtocolVersion::CURRENT.as_u16(), 1);
}

#[test]
fn protocol_version_serde_transparent() {
    let v = ProtocolVersion::new(3);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "3");
    let deserialized: ProtocolVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, v);
}

#[test]
fn protocol_version_ordering() {
    let v1 = ProtocolVersion::new(1);
    let v2 = ProtocolVersion::new(2);
    let v3 = ProtocolVersion::new(3);
    assert!(v1 < v2);
    assert!(v2 < v3);
    assert_eq!(v1, ProtocolVersion::new(1));
}

#[test]
fn protocol_version_from_and_into_u16() {
    let v: ProtocolVersion = 5u16.into();
    assert_eq!(v.as_u16(), 5);
    let n: u16 = v.into();
    assert_eq!(n, 5);
}

#[test]
fn schema_version_current_is_one() {
    assert_eq!(SchemaVersion::CURRENT.as_u16(), 1);
}

#[test]
fn schema_version_serde_transparent() {
    let v = SchemaVersion::new(7);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "7");
    let deserialized: SchemaVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, v);
}

#[test]
fn schema_version_ordering() {
    let v1 = SchemaVersion::new(1);
    let v2 = SchemaVersion::new(2);
    assert!(v1 < v2);
}

#[test]
fn version_newtypes_are_distinct() {
    // Cannot compare ProtocolVersion to SchemaVersion at compile time,
    // but verify they are distinct types with the same inner value
    let pv = ProtocolVersion::new(1);
    let sv = SchemaVersion::new(1);
    assert_eq!(pv.as_u16(), sv.as_u16());
    assert_eq!(format!("{pv}"), format!("{sv}"));
}

// ===========================================================================
// Handshake / validation flow tests
// ===========================================================================

#[test]
fn envelope_validate_rejects_unsupported_protocol_version() {
    let mut env = make_envelope("t", "1", b"d");
    env.protocol_version = 2;
    assert!(matches!(env.validate(), Err(ProtocolError::UnsupportedVersion(_))));
}

#[test]
fn envelope_validate_rejects_zero_schema_version() {
    let mut env = make_envelope("t", "1", b"d");
    env.schema_version = 0;
    assert!(matches!(env.validate(), Err(ProtocolError::InvalidEnvelope(_))));
}

#[test]
fn envelope_validate_accepts_high_schema_version() {
    let mut env = make_envelope("t", "1", b"d");
    env.schema_version = 999;
    assert!(env.validate().is_ok());
}

#[test]
fn envelope_builder_requires_all_mandatory_fields() {
    // Missing event_type
    assert!(
        EventEnvelope::builder()
            .entity_type("t")
            .entity_id("1")
            .payload(b"d".to_vec())
            .build()
            .is_err()
    );

    // Missing entity_type
    assert!(
        EventEnvelope::builder()
            .event_type("t")
            .entity_id("1")
            .payload(b"d".to_vec())
            .build()
            .is_err()
    );

    // Missing entity_id
    assert!(
        EventEnvelope::builder()
            .event_type("t")
            .entity_type("t")
            .payload(b"d".to_vec())
            .build()
            .is_err()
    );

    // Missing payload
    assert!(
        EventEnvelope::builder().event_type("t").entity_type("t").entity_id("1").build().is_err()
    );
}

#[test]
fn envelope_validate_rejects_empty_payload() {
    let mut env = make_envelope("t", "1", b"d");
    env.payload = Vec::new();
    assert!(env.validate().is_err());
}

#[test]
fn envelope_validate_rejects_whitespace_strings() {
    let mut env = make_envelope("t", "1", b"d");

    let original_type = env.event_type.clone();
    env.event_type = "  ".to_string();
    assert!(env.validate().is_err());
    env.event_type = original_type;

    let original_entity = env.entity_type.clone();
    env.entity_type = "\t".to_string();
    assert!(env.validate().is_err());
    env.entity_type = original_entity;

    env.entity_id = "  ".to_string();
    assert!(env.validate().is_err());
}

// ===========================================================================
// Canonical JSON tests
// ===========================================================================

#[test]
fn canonical_json_key_sorting() {
    let v = json!({"z": 1, "a": 2, "m": 3});
    let result = canonical_json(&v).unwrap();
    assert_eq!(result, r#"{"a":2,"m":3,"z":1}"#);
}

#[test]
fn canonical_json_nested_key_sorting() {
    let v = json!({"outer": {"z": 1, "a": 2}});
    let result = canonical_json(&v).unwrap();
    assert_eq!(result, r#"{"outer":{"a":2,"z":1}}"#);
}

#[test]
fn canonical_json_deterministic() {
    let v = json!({"b": [1, 2, 3], "a": {"nested": true}});
    let r1 = canonical_json(&v).unwrap();
    let r2 = canonical_json(&v).unwrap();
    assert_eq!(r1, r2);
}

#[test]
fn canonical_json_preserves_arrays() {
    let v = json!([3, 1, 2]);
    let result = canonical_json(&v).unwrap();
    assert_eq!(result, "[3,1,2]"); // arrays are not sorted
}

#[test]
fn canonical_json_primitives() {
    assert_eq!(canonical_json(&json!(null)).unwrap(), "null");
    assert_eq!(canonical_json(&json!(true)).unwrap(), "true");
    assert_eq!(canonical_json(&json!(false)).unwrap(), "false");
    assert_eq!(canonical_json(&json!(42)).unwrap(), "42");
    assert_eq!(canonical_json(&json!("hello")).unwrap(), r#""hello""#);
}

// ===========================================================================
// Domain hash tests
// ===========================================================================

#[test]
fn domain_hash_deterministic() {
    let h1 = domain_hash("TEST_DOMAIN", b"payload");
    let h2 = domain_hash("TEST_DOMAIN", b"payload");
    assert_eq!(h1, h2);
}

#[test]
fn domain_hash_different_domains() {
    let h1 = domain_hash("DOMAIN_A", b"same");
    let h2 = domain_hash("DOMAIN_B", b"same");
    assert_ne!(h1, h2);
}

#[test]
fn domain_hash_different_data() {
    let h1 = domain_hash("DOMAIN", b"data1");
    let h2 = domain_hash("DOMAIN", b"data2");
    assert_ne!(h1, h2);
}

#[test]
fn domain_hash_not_ambiguous_under_concatenation() {
    assert_ne!(domain_hash("ab", b"cd"), domain_hash("a", b"bcd"));
    assert_ne!(domain_hash("abc", b"d"), domain_hash("ab", b"cd"));
}

#[test]
fn domain_hash_empty_inputs() {
    let h1 = domain_hash("", b"");
    assert_eq!(h1.len(), 32);
    assert_ne!(h1, [0u8; 32]);
}

// ===========================================================================
// Error type tests
// ===========================================================================

#[test]
fn protocol_error_category_labels() {
    assert_eq!(ProtocolError::InvalidEnvelope("x".into()).category(), "invalid_envelope");
    assert_eq!(ProtocolError::InvalidBatch("x".into()).category(), "invalid_batch");
    assert_eq!(
        ProtocolError::MerkleVerificationFailed("x".into()).category(),
        "merkle_verification_failed"
    );
    assert_eq!(ProtocolError::InvalidSignature("x".into()).category(), "invalid_signature");
    assert_eq!(ProtocolError::UnsupportedVersion("x".into()).category(), "unsupported_version");
    assert_eq!(ProtocolError::SerializationError("x".into()).category(), "serialization_error");
}

#[test]
fn protocol_error_message_extraction() {
    let err = ProtocolError::InvalidEnvelope("test message".into());
    assert_eq!(err.message(), "test message");
}

#[test]
fn protocol_error_display() {
    let err = ProtocolError::InvalidBatch("missing leaves".into());
    assert_eq!(err.to_string(), "invalid batch: missing leaves");
}

#[test]
fn protocol_error_from_serde_json() {
    let json_err = serde_json::from_str::<serde_json::Value>("{{bad}}").unwrap_err();
    let proto_err: ProtocolError = json_err.into();
    assert!(matches!(proto_err, ProtocolError::SerializationError(_)));
}

#[test]
fn protocol_error_is_clone() {
    let err = ProtocolError::InvalidEnvelope("clone me".into());
    let cloned = err.clone();
    assert_eq!(err.to_string(), cloned.to_string());
}

// ===========================================================================
// Leaf hash mode tests
// ===========================================================================

#[test]
fn merkle_leaf_hash_mode_default_is_v2() {
    assert_eq!(MerkleLeafHashMode::default(), MerkleLeafHashMode::EnvelopeHashV2);
}

#[test]
fn merkle_leaf_hash_mode_serde_roundtrip() {
    for mode in [MerkleLeafHashMode::EnvelopeHashV2, MerkleLeafHashMode::PayloadHashV1] {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: MerkleLeafHashMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);
    }
}

#[test]
fn legacy_payload_hash_mode_rejected_by_validate() {
    let envs = vec![make_envelope("t", "1", b"d")];
    let mut batch = SyncBatch::new("node", envs);
    batch.merkle_leaf_hash_mode = MerkleLeafHashMode::PayloadHashV1;
    // Recompute root with legacy mode
    let legacy_hashes: Vec<[u8; 32]> = batch.leaves.iter().map(|e| e.payload_hash).collect();
    batch.merkle_root = compute_merkle_root(&legacy_hashes);
    assert!(batch.verify_merkle_root()); // root matches
    assert!(matches!(batch.validate(), Err(ProtocolError::UnsupportedVersion(_)))); // but v1 rejected
}
