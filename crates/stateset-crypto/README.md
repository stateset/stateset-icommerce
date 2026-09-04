# stateset-crypto

[![crates.io](https://img.shields.io/crates/v/stateset-crypto.svg)](https://crates.io/crates/stateset-crypto)
[![docs.rs](https://docs.rs/stateset-crypto/badge.svg)](https://docs.rs/stateset-crypto)

VES v1.0 cryptographic operations for verifiable commerce events: canonical JSON,
domain-separated hashing, Ed25519 signatures, authenticated payload encryption, and
Merkle commitments — with optional hybrid post-quantum signing and key wrapping.

Every hash in this crate is domain-separated, so a leaf hash can never be confused
with a node hash or a receipt hash. Canonicalization is byte-exact with JS and Go
implementations of RFC 8785, which is what makes a signature produced by a Rust
agent verifiable by a TypeScript one.

## Features

- **RFC 8785 JSON Canonicalization (JCS)** — byte-identical output across Rust, JS, and Go
- **Domain-separated hashing** — leaf, node, receipt, stream, payload, and AAD hashes
- **Ed25519 signing** — private keys wrapped in `Zeroizing`, scrubbed on drop
- **AES-256-GCM payload encryption** (VES-ENC-1) with X25519 ECDH key wrapping
- **Hybrid post-quantum** — Ed25519 + ML-DSA-65 signatures, X25519 + ML-KEM-768 key wrapping
- **Merkle trees** — root computation with explicit pad-leaf semantics
- **Zero unsafe code**, `#[deny(unsafe_code)]`

## Usage

Canonicalize, sign, and verify:

```rust
use stateset_crypto::{canonicalize::canonicalize_json, sign::{generate_keypair, sign_event_hash, verify_event_signature}};
use serde_json::json;

// Canonical JSON is byte-stable regardless of key order
let a = canonicalize_json(&json!({ "b": 2, "a": 1 }))?;
let b = canonicalize_json(&json!({ "a": 1, "b": 2 }))?;
assert_eq!(a, b);
assert_eq!(a, r#"{"a":1,"b":2}"#);

// Sign a 32-byte hash and verify it
let (private_key, public_key) = generate_keypair();
let event_hash = [7u8; 32];

let signature = sign_event_hash(&event_hash, &private_key)?;
assert!(verify_event_signature(&event_hash, &signature, &public_key));

// A different hash does not verify against that signature
assert!(!verify_event_signature(&[8u8; 32], &signature, &public_key));
# Ok::<(), stateset_crypto::CryptoError>(())
```

Merkle commitments:

```rust
use stateset_crypto::merkle::{compute_merkle_root, compute_node_hash};

let leaves = [[1u8; 32], [2u8; 32]];
let root = compute_merkle_root(&leaves);

// A two-leaf root is exactly the node hash of its two leaves
assert_eq!(root, compute_node_hash(&leaves[0], &leaves[1]));
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `pqc` | Hybrid post-quantum signing (ML-DSA-65) and key wrapping (ML-KEM-768) | Yes |

Disable default features for a classical-only build:

```toml
stateset-crypto = { version = "1.31.0", default-features = false }
```

## Post-Quantum Scope

The `pqc` feature provides *hybrid* constructions — classical and post-quantum
primitives composed so that breaking either alone is insufficient. It does not make
the whole system post-quantum: settlement finality, transport, and the x402 payment
path retain classical assumptions. See
[`TRUST_FOUNDATION.md`](https://github.com/stateset/stateset-icommerce/blob/master/TRUST_FOUNDATION.md)
for what is and is not claimed.

## Part of StateSet iCommerce

Used by [`stateset-sync`](https://crates.io/crates/stateset-sync) for event
attestation and available through
[`stateset-sdk`](https://crates.io/crates/stateset-sdk)'s `crypto` feature.

## License

MIT OR Apache-2.0
