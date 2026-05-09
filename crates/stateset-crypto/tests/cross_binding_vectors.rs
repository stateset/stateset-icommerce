//! Cross-binding compatibility test-vector verifier.
//!
//! This test reads `bindings/test-vectors/v1.json` and asserts every entry
//! matches what `stateset-crypto` produces. The same JSON is consumed by
//! every language binding (Node, Python, Go, Java, Kotlin, Swift, .NET,
//! Ruby, PHP, WASM) so any divergence between Rust ground truth and a
//! binding's implementation surfaces as a CI failure.
//!
//! Companion to `tests/test_vectors.rs`, which carries the older inline
//! Rust ↔ JS hardcoded vectors. New vectors should be added to the shared
//! JSON file, not duplicated inline.
//!
//! On placeholder values: a freshly-added vector typically has
//! `expected_hex` = `"00…00"`. Run with `STATESET_TEST_VECTORS_REGENERATE=1`
//! to print the actual computed hex for each mismatch (the test will still
//! fail; copy the printed values into the JSON, then re-run without the
//! env var).

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use stateset_crypto::canonicalize::canonicalize_json_bytes;
use stateset_crypto::hash::compute_payload_plain_hash;
use stateset_crypto::merkle::compute_merkle_root;

#[derive(Debug, Deserialize)]
struct VectorFile {
    #[serde(default)]
    version: u32,
    categories: Categories,
}

#[derive(Debug, Deserialize)]
struct Categories {
    #[serde(default)]
    canonical_json: Vec<CanonicalJsonVector>,
    #[serde(default)]
    payload_plain_hash: Vec<PayloadHashVector>,
    #[serde(default)]
    merkle_root: Vec<MerkleVector>,
}

#[derive(Debug, Deserialize)]
struct CanonicalJsonVector {
    id: String,
    input: Value,
    expected_hex: String,
}

#[derive(Debug, Deserialize)]
struct PayloadHashVector {
    id: String,
    input: Value,
    #[serde(default)]
    salt_hex: Option<String>,
    expected_hex: String,
}

#[derive(Debug, Deserialize)]
struct MerkleVector {
    id: String,
    leaves_hex: Vec<String>,
    expected_hex: String,
}

fn vectors_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is `crates/stateset-crypto`; the corpus is at
    // workspace-root `bindings/test-vectors/v1.json`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("bindings")
        .join("test-vectors")
        .join("v1.json")
}

fn load_vectors() -> VectorFile {
    let path = vectors_path();
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn regenerate_mode() -> bool {
    std::env::var("STATESET_TEST_VECTORS_REGENERATE")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

fn parse_hex(s: &str) -> Vec<u8> {
    let trimmed = s.strip_prefix("0x").unwrap_or(s);
    hex::decode(trimmed).expect("valid hex")
}

fn parse_hex_32(s: &str) -> [u8; 32] {
    let bytes = parse_hex(s);
    assert_eq!(bytes.len(), 32, "expected 32-byte hex, got {}", bytes.len());
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

#[test]
fn vector_file_is_present_and_versioned() {
    let f = load_vectors();
    assert_eq!(f.version, 1, "expected version 1; bump bindings if you change shape");
}

#[test]
fn canonical_json_vectors_match_ground_truth() {
    let file = load_vectors();
    let regen = regenerate_mode();
    let mut mismatches = 0_usize;
    for v in &file.categories.canonical_json {
        let canonical = canonicalize_json_bytes(&v.input)
            .unwrap_or_else(|e| panic!("canonicalize {}: {e}", v.id));
        let mut hasher = Sha256::new();
        hasher.update(&canonical);
        let actual: [u8; 32] = hasher.finalize().into();
        let actual_hex = hex::encode(actual);
        if actual_hex != v.expected_hex {
            mismatches += 1;
            if regen {
                eprintln!("[regenerate] canonical_json/{}: {}", v.id, actual_hex);
            } else {
                eprintln!(
                    "MISMATCH canonical_json/{}: expected {}, got {}",
                    v.id, v.expected_hex, actual_hex,
                );
            }
        }
    }
    assert_eq!(mismatches, 0, "{mismatches} canonical_json vector(s) mismatched");
}

#[test]
fn payload_plain_hash_vectors_match_ground_truth() {
    let file = load_vectors();
    let regen = regenerate_mode();
    let mut mismatches = 0_usize;
    for v in &file.categories.payload_plain_hash {
        let salt_arr = v.salt_hex.as_ref().map(|s| {
            let bytes = parse_hex(s);
            assert_eq!(bytes.len(), 16, "salt must be 16 bytes for {}", v.id);
            let mut out = [0_u8; 16];
            out.copy_from_slice(&bytes);
            out
        });
        let actual = compute_payload_plain_hash(&v.input, salt_arr.as_ref())
            .unwrap_or_else(|e| panic!("payload_plain_hash {}: {e}", v.id));
        let actual_hex = hex::encode(actual);
        if actual_hex != v.expected_hex {
            mismatches += 1;
            if regen {
                eprintln!("[regenerate] payload_plain_hash/{}: {}", v.id, actual_hex);
            } else {
                eprintln!(
                    "MISMATCH payload_plain_hash/{}: expected {}, got {}",
                    v.id, v.expected_hex, actual_hex,
                );
            }
        }
    }
    assert_eq!(mismatches, 0, "{mismatches} payload_plain_hash vector(s) mismatched");
}

#[test]
fn merkle_root_vectors_match_ground_truth() {
    let file = load_vectors();
    let regen = regenerate_mode();
    let mut mismatches = 0_usize;
    for v in &file.categories.merkle_root {
        let leaves: Vec<[u8; 32]> = v.leaves_hex.iter().map(|h| parse_hex_32(h)).collect();
        let actual = compute_merkle_root(&leaves);
        let actual_hex = hex::encode(actual);
        if actual_hex != v.expected_hex {
            mismatches += 1;
            if regen {
                eprintln!("[regenerate] merkle_root/{}: {}", v.id, actual_hex);
            } else {
                eprintln!(
                    "MISMATCH merkle_root/{}: expected {}, got {}",
                    v.id, v.expected_hex, actual_hex,
                );
            }
        }
    }
    assert_eq!(mismatches, 0, "{mismatches} merkle_root vector(s) mismatched");
}
