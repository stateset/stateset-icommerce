# Post-Quantum Security Initial Specification

Status: Draft
Date: 2026-03-30
Applies to: `stateset-icommerce`, `stateset-sequencer`, `stateset-stark`

## 1. Decision Summary

This stack should not choose between a Starknet-style hash-based system and lattice-based PQC as if they were interchangeable. They solve different parts of the problem.

The initial architecture is:

- keep the existing hash-based STARK proof system;
- migrate the event, receipt, key-registry, and payload-encryption layers to hybrid classical plus lattice PQC first;
- target lattice-only operation for signatures and KEM after a compatibility window;
- treat Ethereum L2 anchoring and ECDSA payment settlement as residual non-PQ dependencies until a separate migration exists.

The selected initial PQ algorithms are:

- signatures: `ML-DSA-65`, with migration profile `ed25519+mldsa65`;
- key establishment: `ML-KEM-768`, with migration profile `x25519+mlkem768`;
- symmetric encryption: retain `AES-256-GCM` in the initial phase;
- hashing and commitments: retain `SHA-256`, Merkle commitments, Rescue-Prime, and the current STARK construction.

## 2. Why This Decision Exists

Today the stack is split across two very different cryptographic planes:

- the control plane is classical: Ed25519 event signatures, Ed25519 receipt signatures, X25519 HPKE, TLS, secp256k1 anchoring, and ECDSA payment intents;
- the proof plane is already hash-based and largely post-quantum friendly: Merkle commitments, Rescue-Prime, and Winterfell STARK proofs.

Replacing the STARK system with a lattice-based proving system would not solve the immediate risk, because the immediate risk is concentrated in signatures, KEM, key registration, receipt signing, and transport-facing confidentiality.

Conversely, keeping only a Starknet-style hash-based proving layer does not solve agent authentication, receipt authenticity, or payload confidentiality by itself.

## 3. Problem Statement

The current system has the following quantum-sensitive surfaces:

- event authenticity depends on Ed25519 agent signatures;
- sequencer non-repudiation depends on Ed25519 receipt signatures;
- payload confidentiality depends on X25519-based HPKE;
- key registration assumes a single fixed-size 32-byte public key;
- multi-agent coordination depends on FROST over Ed25519;
- `stateset-sync` currently pushes plaintext envelopes and tolerates `http://` base URLs;
- Ethereum L2 anchoring depends on secp256k1;
- x402 payment intent validation depends on ECDSA.

The current STARK subsystem does not have the same failure mode. It is already built around transparent, hash-based proofs, but its payload-to-witness binding still depends on surrounding protocol artifacts.

## 4. Goals

- make event authenticity quantum-resistant;
- make receipt authenticity quantum-resistant;
- make payload key wrapping quantum-resistant;
- preserve compatibility during migration;
- avoid replacing the STARK proving stack unless strictly necessary;
- strengthen proof-to-payload binding around the existing STARK system;
- define a path from hybrid compatibility to strict PQ operation;
- isolate residual non-PQ surfaces explicitly instead of hiding them.

## 5. Non-Goals

- replacing Winterfell STARKs with a lattice-based proof system in phase 1;
- making Ethereum-compatible anchoring quantum-safe in this document;
- making historical Ed25519 or ECDSA artifacts retroactively quantum-safe;
- solving post-quantum threshold signatures in the first migration phase;
- redesigning every public input in the STARK AIR.

## 6. Threat Model

This specification assumes a future adversary with access to a cryptographically relevant quantum computer can:

- forge signatures on ECDLP-based schemes;
- break X25519-style key exchange and decrypt recorded ciphertexts later;
- exploit long-lived or reused public keys;
- target key registration and receipt flows, not just user-authored events.

This specification aims to prevent:

- forged agent events;
- forged sequencer receipts;
- harvest-now-decrypt-later compromise of encrypted payloads;
- false confidence caused by a PQ-friendly STARK layer wrapped in classical signatures.

This specification does not yet eliminate:

- dependence on EVM-compatible anchoring keys;
- dependence on ECDSA payment intent signatures;
- exposure of historical classical signatures already recorded;
- metadata leakage at transport boundaries before PQ-capable TLS is deployed.

## 7. Trade-Off Analysis

### 7.1 "Starknet-Style Hash-Based" Systems

In this document, "Starknet-style hash-based" means transparent proof and commitment systems built from hashes and FRI-style arguments, not a full replacement for signatures or KEM.

Strengths:

- conservative security assumptions;
- transparent proofs with no trusted setup;
- already aligned with `stateset-stark`;
- well-suited for computational integrity, commitments, and public verification;
- avoids lattice implementation risk in the proving layer.

Weaknesses:

- does not by itself provide a drop-in replacement for agent signatures or payload KEM;
- hash-based signature families such as XMSS or SPHINCS+ are a poor fit for the current agent model;
- stateful signatures are operationally risky for offline agents, retries, and multi-device agents;
- stateless hash-based signatures are significantly larger and slower than lattice signatures in a high-volume event log;
- threshold and aggregation ergonomics are weak for the current multi-agent design.

Conclusion:

- hash-based systems are the right answer for the proof plane;
- hash-based signatures are not the best answer for the event-authentication plane in this stack.

### 7.2 Lattice-Based PQC

In this document, "lattice-based PQC" means ML-DSA for signatures and ML-KEM for key establishment.

Strengths:

- directly addresses the current quantum-sensitive surfaces: signatures and KEM;
- NIST-standardized primitives exist for both functions;
- operationally closer to Ed25519 and X25519 than hash-based signature families are;
- better fit for key registries, proof-of-possession, receipt signing, and encrypted-payload workflows;
- materially smaller and more practical than hash-based signatures in the event plane.

Weaknesses:

- newer implementations and smaller operational history than Ed25519/X25519;
- side-channel hardening and constant-time discipline matter more;
- larger keys and signatures than classical elliptic-curve schemes;
- threshold, aggregation, and recursive-proof interactions remain immature;
- some lattice signature families have implementation complexity that should be avoided.

Conclusion:

- lattice PQC is the right answer for the control plane;
- it should be adopted first as a hybrid profile and later as a strict PQ profile.

### 7.3 Recommended Position

The correct architecture is not hash-based versus lattice-based. It is:

- hash-based STARKs for proofs and commitments;
- lattice-based PQC for signatures and KEM;
- hybrid compatibility during migration;
- explicit isolation of the remaining classical chain and settlement dependencies.

## 8. Selected Cryptographic Profile

### 8.1 Signature Algorithms

Target signature algorithm:

- `mldsa65`

Migration algorithm:

- `ed25519+mldsa65`

Rule:

- in a hybrid signature bundle, all required signature components must verify;
- a verifier must reject a partially valid bundle.

Rationale:

- `ML-DSA-65` is a practical first target;
- this avoids Falcon-style discrete-Gaussian implementation complexity in the initial spec;
- hybrid mode keeps classical interoperability while adding quantum resistance.

### 8.2 KEM Algorithms

Target KEM algorithm:

- `mlkem768`

Migration algorithm:

- `x25519+mlkem768`

Rule:

- in a hybrid recipient wrap, the wrapping key is derived from both shared secrets using HKDF-SHA256;
- failure of any required component invalidates the wrapped payload for that profile.

### 8.3 Symmetric and Hash Primitives

Retained in the initial migration:

- `AES-256-GCM` for payload encryption;
- `SHA-256` for event, payload, and receipt hashes;
- Merkle commitments;
- Rescue-Prime in the STARK layer.

Rationale:

- the immediate CRQC break is concentrated in asymmetric cryptography;
- AES-256 and SHA-256 remain acceptable in the initial migration window;
- replacing these primitives is lower priority than replacing Ed25519, X25519, secp256k1, and ECDSA.

### 8.4 Security Profiles

Three protocol profiles are defined:

- `legacy`: Ed25519 and X25519 only;
- `hybrid`: `ed25519+mldsa65` and `x25519+mlkem768`;
- `pqc-strict`: `mldsa65` and `mlkem768` only.

Profile rules:

- `legacy` remains read-compatible only during migration;
- `hybrid` is the default migration profile;
- `pqc-strict` rejects legacy-only keys, signatures, and recipient wraps.

## 9. Protocol Changes

This document proposes a new umbrella profile: `VES-PQC-1`.

`VES-PQC-1` covers signature bundles, key bundles, recipient wraps, receipts, and verification behavior. It does not replace the STARK proof system.

### 9.1 Event Signatures: `VES-SIG-2`

Current issue:

- `EventEnvelope.agent_signature` is a single raw Ed25519 signature.

Required change:

- replace raw signature bytes with a structured signature bundle;
- attach an explicit signature scheme identifier;
- preserve the existing event signing hash;
- verify all required signatures for the selected profile.

Suggested logical shape:

```text
SignatureBundle {
  scheme: "ed25519" | "mldsa65" | "ed25519+mldsa65"
  ed25519_signature?: bytes
  ml_dsa_65_signature?: bytes
}
```

Normative rules:

- the event signing hash remains the canonical signed message;
- the sequencer stores the scheme identifier alongside the signature bundle;
- `pqc-strict` envelopes must not contain an Ed25519-only bundle.

### 9.2 Payload Encryption: `VES-ENC-2`

Current issue:

- `EncryptedPayload` assumes X25519 HPKE and fixed recipient-wrapping fields.

Required change:

- generalize `HpkeParams` to a key-wrap profile;
- support hybrid recipient wraps;
- include all wrap components in recipient-manifest hashing.

Suggested logical shape:

```text
KeyWrapParams {
  scheme: "x25519-hkdf-sha256" | "mlkem768" | "x25519+mlkem768"
  kdf: "hkdf-sha256"
  aead: "aes-256-gcm"
}

RecipientWrap {
  recipient_kid: uint32
  wrap_scheme: string
  x25519_enc?: bytes
  ml_kem_ciphertext?: bytes
  wrap_nonce: bytes
  wrapped_key: bytes
}
```

Normative rules:

- the content ciphertext remains AES-256-GCM in this phase;
- the recipient-manifest hash must cover every KEM ciphertext and wrap field;
- `pqc-strict` payloads must not rely on X25519.

### 9.3 Key Registry: `VES-KEY-2`

Current issue:

- the registry assumes one fixed-size 32-byte public key and a coarse `KeyType`.

Required change:

- store algorithm-aware key bundles instead of raw fixed-width keys;
- support proof-of-possession bundles for all required signature algorithms;
- permit one logical `key_id` to represent a profile-specific bundle.

Suggested logical shape:

```text
PublicKeyBundle {
  scheme: "ed25519" | "mldsa65" | "ed25519+mldsa65" |
          "x25519"  | "mlkem768" | "x25519+mlkem768"
  ed25519_public_key?: bytes
  ml_dsa_65_public_key?: bytes
  x25519_public_key?: bytes
  ml_kem_768_public_key?: bytes
}

ProofOfPossessionBundle {
  scheme: string
  ed25519_pop?: bytes
  ml_dsa_65_pop?: bytes
}
```

Normative rules:

- the registry must record scheme, status, validity window, and bundle bytes;
- the sequencer must resolve verification keys by `key_id` and scheme, not by raw key length;
- `pqc-strict` registration requires ML-DSA or ML-KEM material as appropriate.

### 9.4 Sequencer Receipts: `VES-RECEIPT-2`

Current issue:

- the stack has multiple receipt-like artifacts, but not all are signature-bearing;
- soft finality is only meaningful if the receipt artifact itself is quantum-resistant.

Required change:

- any receipt relied upon for non-repudiation must carry a signature bundle;
- the sequencer must expose its receipt-signing scheme publicly;
- receipt verification must use the same profile rules as event verification.

Suggested logical shape:

```text
ReceiptSignatureBundle {
  scheme: "ed25519" | "mldsa65" | "ed25519+mldsa65"
  ed25519_signature?: bytes
  ml_dsa_65_signature?: bytes
}
```

Normative rules:

- the sequencer receipt hash stays domain-separated and stable;
- any unsigned batch receipt is informational only and must not be treated as PQ soft finality;
- production soft-finality receipts should be emitted under the hybrid profile before `pqc-strict` rollout.

### 9.5 STARK Binding: `VES-STARK-BIND-2`

Current issue:

- the STARK AIR proves policy compliance, but payload-to-witness linkage remains partly outside the proof.

Required change:

- keep the STARK proof system;
- require `witnessCommitment` and `amountBindingHash` in production compliance workflows;
- bind any protocol-level amount derivation artifacts to PQ or hybrid signatures outside the AIR if those artifacts are used for enforcement.

Normative rules:

- the initial migration does not replace the AIR with a lattice proof system;
- production compliance verification should require the secure STARK proof profile;
- a valid STARK proof is insufficient by itself unless the surrounding payload-binding artifacts are also validated.

### 9.6 Transport Profile

Current issue:

- `stateset-sync` can emit plaintext envelopes and tolerates `http://`.

Required change:

- `http://` must be rejected in `hybrid` and `pqc-strict`;
- sensitive event classes should require encrypted payload submission;
- hybrid or PQ-capable TLS should be preferred at the ingress boundary when available.

Normative rules:

- application-layer PQ signatures and PQ KEM remain mandatory even when TLS is present;
- PQ TLS is a hardening layer, not the only confidentiality boundary;
- plaintext payload submission should be disallowed for designated high-sensitivity domains in `pqc-strict`.

## 10. Repo-Specific Work Items

### 10.1 `stateset-icommerce`

- stabilize the new hybrid helpers in `stateset-crypto`;
- extend `ves.proto` to carry signature bundles and key bundles;
- update generated bindings across languages;
- teach `stateset-sync` to emit encrypted PQ-capable envelopes;
- reject `http://` in non-legacy profiles;
- add compatibility tests for legacy, hybrid, and strict PQ modes.

### 10.2 `stateset-sequencer`

- update key registration and verification paths for algorithm-aware bundles;
- accept hybrid event signatures and hybrid recipient wraps;
- issue hybrid receipt signatures;
- store scheme metadata for every signed or encrypted artifact;
- revise VES-MULTI-1 so that Ed25519 FROST is not the only path to multi-agent authorization;
- add a non-threshold fallback: `t-of-n` independent PQ or hybrid signature bundles.

### 10.3 `stateset-stark`

- keep the current STARK proof system;
- require secure proof options in production compliance flows;
- require `witnessCommitment` and `amountBindingHash` for higher-assurance verification;
- avoid framing the STARK layer as a replacement for PQ signatures or PQ KEM.

## 11. Finality Model After Migration

This spec distinguishes three finality levels:

- `legacy soft finality`: classical receipt or local acceptance only;
- `pq soft finality`: hybrid or PQ receipt signature plus local proof/binding validation;
- `classical hard finality`: EVM anchoring on Ethereum L2.

This document does not yet define `pq hard finality`.

`pq hard finality` will require one of:

- a PQ-friendly anchoring substrate;
- a second append-only transparency log with PQ signatures;
- a future chain bridge that can carry PQ attestations.

Until then, the system can achieve PQ soft finality before it achieves PQ hard finality.

## 12. Rollout Phases

### Phase 0: Inventory And Compatibility

- inventory every classical signature and KEM surface;
- classify flows into `legacy`, `hybrid`, and future `pqc-strict`;
- add metrics for profile usage.

### Phase 1: Library And Wire Format

- land hybrid cryptography in `stateset-crypto`;
- add bundle-aware messages to `ves.proto`;
- dual-encode and dual-verify in test environments.

### Phase 2: Sequencer Acceptance

- allow hybrid key registration;
- accept hybrid event signatures;
- accept hybrid recipient wraps;
- emit hybrid receipt signatures.

### Phase 3: Production Hybrid Default

- default new tenants and agents to hybrid mode;
- reject plaintext transport in hybrid mode;
- require signed receipt artifacts for soft finality.

### Phase 4: Strict PQ For New Tenants

- default new tenants to `pqc-strict`;
- continue read-only verification of legacy historical artifacts;
- disable Ed25519 and X25519 for new registrations.

### Phase 5: Classical Residual Migration

- define replacement or overlay for EVM anchoring;
- define PQ replacement for x402 ECDSA payment intents;
- define long-term story for multi-agent threshold authorization.

## 13. Open Questions

- whether `ML-DSA-65` is sufficient for every regulated workload or whether a higher-cost profile is needed;
- whether the key registry should keep `uint32 key_id` or move to opaque identifiers;
- whether hybrid receipts should be mandatory for all batches or only authoritative receipts;
- whether `stateset-stark` public inputs should eventually commit to a signed amount-binding attestation hash;
- whether x402 should migrate to a PQ signature scheme directly or be wrapped in a separate authorization layer;
- how to define `pq hard finality` while anchoring remains EVM-native.

## 14. Final Recommendation

For this stack, the right initial direction is:

- STARKs stay;
- lattice signatures and KEM replace the current elliptic-curve control plane;
- hybrid compatibility comes first;
- strict PQ comes next;
- EVM anchoring and ECDSA settlement are tracked as explicit residual risk, not hidden behind the STARK layer.

## 15. Implementation Status

Last updated: 2026-03-31

### Phase 0: Inventory and Compatibility

| Item | Status | Notes |
|------|--------|-------|
| Inventory classical signature surfaces | Done | Ed25519 events, X25519 KEM, secp256k1 anchoring identified |
| Classify flows into legacy/hybrid/pqc-strict | Done | `SecurityProfile` enum in Rust, `resolveSecurityProfile` in JS |
| Add metrics for profile usage | Done | Rust: `record_pqc_signature(profile)`, `record_pqc_encryption(profile)` in `stateset-observability`; JS: outbox `pqcMetrics` counters |

### Phase 1: Library and Wire Format

| Item | Status | Notes |
|------|--------|-------|
| Hybrid crypto in `stateset-crypto` | Done | `pqc` module: Ed25519+ML-DSA-65 signing, X25519+ML-KEM-768 wrapping, AES-256-GCM payload encryption |
| PQC-strict crypto in `stateset-crypto` | Done | ML-DSA-65-only signing, ML-KEM-768-only wrapping, strict payload encryption/decryption |
| Serde on all PQC types | Done | Hex-encoded JSON serialization for key persistence |
| Feature flag (`pqc`) | Done | `ml-kem` and `ml-dsa` behind optional `pqc` feature (default on) |
| Bundle-aware messages in `ves.proto` | Done | `SignatureBundle`, `PublicKeyBundle`, `ProofOfPossessionBundle`, `KeyWrapParams`, `RecipientKeyWrap`, receipt signature fields on `SequencedEvent` and `BatchReceipt` |
| Proof-of-possession | Done | Hybrid and strict PoP: `SHA-256("VES_POP_V1" \|\| pk)` challenge, Rust + NAPI + JS |
| Receipt signing | Done | `hybrid_sign_receipt_hash`, `strict_sign_receipt_hash` in Rust; `verifyReceiptSignature` in JS client |
| Node NAPI bindings | Done | 16 functions: hybrid keygen/sign/verify/encrypt/decrypt, strict keygen/sign/verify/encrypt/decrypt, hybrid PoP gen/verify, strict PoP gen/verify |
| Cross-language test vectors | Done | `TEST_VECTOR_SIGNING_SEED`, `test_vector_ml_dsa_public_key()`, 8 JS interop tests |
| `pqc-strict` config validation | Done | `createSyncConfig()` checks native PQC support at init time |
| `http://` rejection for non-legacy | Done | `assertSecureTransportForProfile()` enforced in REST + gRPC client constructors |

### Phase 2: Sequencer Acceptance (Client-Side Readiness)

| Item | Status | Notes |
|------|--------|-------|
| Hybrid key registration | Done | `exportSigningPublicKey()` includes PoP bundle; `registerAgentKey()` sends `public_key_bundle` + `proof_of_possession_bundle` |
| Hybrid event signatures | Done | Outbox `_signEventForProfile` emits `SIGNATURE_SCHEME_ED25519_ML_DSA_65` with dual-signature bundle |
| Hybrid recipient wraps | Done | Outbox `_encryptPayloadForProfile` encrypts with `X25519+ML-KEM-768` |
| Hybrid receipt verification | Done | `verifyReceiptSignature()` on `SequencerClient` supports legacy, hybrid, and strict receipt signatures |
| PQC-strict key registration | Done | Keys.js generates ML-DSA-65 / ML-KEM-768 keys with PoP |
| PQC-strict event signatures | Done | Outbox emits `SIGNATURE_SCHEME_ML_DSA_65` with ML-DSA-65-only bundle |
| PQC-strict payload encryption | Done | Outbox encrypts with `ML-KEM-768` only |
| PQC-strict decryption | Done | Engine dispatches to `decryptPayloadStrict` for `KEY_WRAP_SCHEME_ML_KEM_768` |

### Test Coverage

| Layer | Count | Notes |
|-------|-------|-------|
| Rust inline (`pqc::tests`) | 41 | Roundtrips, negatives, serde, strict, PoP, receipts, test vectors |
| Rust integration (`crypto_expanded`) | 11 | End-to-end sign+encrypt, PoP+serde, key persistence |
| Rust proptest | 10 | Hybrid/strict sign, wrap, encrypt roundtrips with random inputs |
| JS cross-language | 8 | NAPI binding verification, key size invariants |
| JS pqc-strict profile | 34 | Profile validation, transport, events, keys, receipts, NAPI roundtrips |
| JS sync-client | 96 | Push/pull, signature verification, key registration |
| **Total PQC-related** | **200+** | |

### Remaining for Phase 3+

- Default new tenants to hybrid mode (sequencer-side policy)
- Require signed receipt artifacts for soft finality (sequencer-side)
- Default new tenants to `pqc-strict` (Phase 4)
- Disable Ed25519/X25519 for new registrations (Phase 4)
- PQ replacement for EVM anchoring and x402 ECDSA (Phase 5)
- Multi-agent PQ threshold authorization (Phase 5)
