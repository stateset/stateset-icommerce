# VES v1.0 Specification

Verifiable Encrypted Signatures (VES) v1.0 is a cryptographic specification for tamper-proof event synchronization. It combines JSON canonicalization, domain-separated hashing, Ed25519 signatures, AES-256-GCM encryption, and Merkle tree proofs.

## Components

| Component | Standard | Purpose |
|-----------|----------|---------|
| Canonicalization | RFC 8785 (JCS) | Deterministic JSON serialization |
| Hashing | SHA-256 (domain-separated) | Content-addressable identification |
| Signing | Ed25519 | Tamper-proof event signatures |
| Encryption | AES-256-GCM | Confidential event payloads |
| Key Wrapping | X25519 ECDH + HKDF | Multi-recipient encryption |
| Proofs | Merkle Trees | Efficient subset verification |

## How It Works

### 1. Canonicalize

Every event payload is serialized to a deterministic byte representation using RFC 8785 JSON Canonicalization Scheme (JCS). This ensures that the same logical payload always produces the same bytes, regardless of property ordering or whitespace.

### 2. Hash (Domain-Separated)

The canonical bytes are hashed with SHA-256 using domain separation tags to prevent cross-protocol collisions:

| Tag | Purpose |
|-----|---------|
| `VES_PAYLOAD_PLAIN_V1` | Unsigned payload hash |
| `VES_PAYLOAD_AAD_V1` | Additional authenticated data |
| `VES_PAYLOAD_CIPHER_V1` | Encrypted payload hash |
| `VES_RECIPIENTS_V1` | Recipient list hash |
| `VES_EVENTSIG_V1` | Event signature domain |
| `VES_LEAF_V1` | Merkle tree leaf hash |
| `VES_NODE_V1` | Merkle tree node hash |
| `VES_PAD_LEAF_V1` | Merkle padding leaf |
| `VES_STREAM_V1` | Event stream identifier |
| `VES_RECEIPT_V1` | Receipt hash |

### 3. Sign

The hash is signed with Ed25519. Keys are zeroized from memory immediately after use (via the `zeroize` crate).

### 4. Organize into Merkle Trees

Signed events are organized into Merkle trees for efficient proof generation. Any subset of events can be independently verified without downloading the full history.

### 5. Encrypt (Optional)

For confidential events, the payload is encrypted with AES-256-GCM. Key wrapping uses X25519 ECDH with HKDF key derivation, enabling multi-recipient encryption where each recipient can decrypt with their own key.

## Rust API

```rust
use stateset_crypto::{sign, verify, canonicalize, hash_domain};

// Canonicalize
let canonical = canonicalize(&payload)?;

// Hash with domain separation
let hash = hash_domain(b"VES_PAYLOAD_PLAIN_V1", &canonical);

// Sign
let signature = sign(&signing_key, &hash);

// Verify
assert!(verify(&public_key, &hash, &signature));
```

## Merkle Proofs

```rust
use stateset_crypto::merkle::{MerkleTree, verify_proof};

// Build a tree from event hashes
let tree = MerkleTree::from_leaves(&event_hashes);

// Generate a proof for a specific event
let proof = tree.proof(event_index);

// Verify the proof against the root
assert!(verify_proof(&tree.root(), &event_hash, &proof));
```

## Security Properties

- **Tamper-proof**: Changing any event invalidates its signature and all Merkle proofs that include it
- **Forward integrity**: Each event references its predecessor's hash, creating a chain
- **Selective disclosure**: Merkle proofs allow verifying specific events without revealing others
- **Memory safety**: All signing keys are zeroized after use; constant-time comparison prevents timing attacks
- **Deterministic**: Same input always produces the same canonical form, hash, and signature

## CLI Integration

```bash
# Generate signing keys
stateset-sync keys:generate

# Register public key with sequencer
stateset-sync keys:register

# Rotate keys
stateset-sync keys:rotate --all --register
```

## End-to-End Example

Signing an order event from canonicalization through Merkle inclusion:

```rust
use stateset_crypto::{canonicalize, hash_domain, sign, verify};
use stateset_crypto::merkle::MerkleTree;

// 1. Construct the event payload
let event = serde_json::json!({
    "type": "order.created",
    "orderId": "ord_abc123",
    "customerId": "cust_xyz",
    "total": "59.98",
    "currency": "USD",
    "timestamp": "2026-03-16T10:30:45Z"
});

// 2. Canonicalize (RFC 8785 JCS)
//    Keys are sorted, whitespace removed, numbers normalized
let canonical = canonicalize(&event)?;
//  → {"currency":"USD","customerId":"cust_xyz","orderId":"ord_abc123",
//     "timestamp":"2026-03-16T10:30:45Z","total":"59.98","type":"order.created"}

// 3. Domain-separated hash
let event_hash = hash_domain(b"VES_EVENTSIG_V1", &canonical);
//  → SHA-256("VES_EVENTSIG_V1" || canonical_bytes)

// 4. Sign with Ed25519
let signature = sign(&agent_signing_key, &event_hash);

// 5. Verify
assert!(verify(&agent_public_key, &event_hash, &signature));

// 6. Include in Merkle tree
let tree = MerkleTree::from_leaves(&[event_hash, other_event_hash, ...]);
let proof = tree.proof(0); // proof for this event

// 7. Anyone can verify inclusion
assert!(tree.verify_proof(&event_hash, &proof));
```

## Cross-Language Consistency

VES operations produce identical output across Rust and JavaScript. The test suite includes cross-language test vectors:

```
Input:  {"b": 2, "a": 1}
JCS:    {"a":1,"b":2}
Hash:   SHA-256("VES_PAYLOAD_PLAIN_V1" || jcs_bytes) = 0xabc123...
Sign:   Ed25519(key, hash) = 0xdef456...
```

Both the Rust `stateset-crypto` crate and the JavaScript `cli/src/x402/crypto.js` module are intended to produce byte-identical results for the same inputs, including optional `resourceUri` and `resourceMethod` binding for x402 payment intents.

## Encryption Flow (VES-ENC-1)

For confidential events between agents:

```
1. Generate ephemeral X25519 key pair
2. ECDH: shared_secret = X25519(ephemeral_private, recipient_public)
3. HKDF: encryption_key = HKDF-SHA256(shared_secret, salt, "VES-ENC-1")
4. Encrypt: AES-256-GCM(encryption_key, nonce, plaintext, AAD)
5. Bundle: { ephemeral_public, nonce, ciphertext, tag }
6. Zeroize: ephemeral_private, shared_secret, encryption_key
```

Multi-recipient: repeat steps 2-3 for each recipient's public key. Each gets their own wrapped key but shares the same ciphertext.

## Implementation

The VES v1.0 specification is implemented in the `stateset-crypto` crate:

| Module | Lines | Purpose |
|--------|-------|---------|
| `canonicalize.rs` | RFC 8785 JSON Canonicalization | Deterministic serialization |
| `sign.rs` | Ed25519 signing | Domain-separated signatures |
| `encrypt.rs` | AES-256-GCM (VES-ENC-1) | Confidential event encryption |
| `hash.rs` | Domain-separated SHA-256 | 10 domain separation tags |
| `merkle.rs` | Merkle tree construction | O(log n) proof generation and verification |
| `encoding.rs` | Hex/base64/UUID encoding | Cross-language byte representation |

All modules use the `zeroize` crate to wipe sensitive data from memory after use. Signature comparison uses the `subtle` crate for constant-time operations.
