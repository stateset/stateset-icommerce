# Example 01 — AID derivation and Intent signing

The smallest possible end-to-end demonstration of ICP-1.0's identity and
signing model. **Zero dependencies.** Runs on stock Node.js 18+ using only
`node:crypto`.

## Run it

```sh
node demo.mjs
```

Expected output (values vary because keys are freshly generated):

```
=== Identity ===
Ed25519 pub (hex): 6f3...
X25519  pub (hex): 09a...
AID:                aid:v1:zQ3sh4Y8...

=== Intent ===
{"buyer":"aid:v1:...","exp":"2026-05-09T18:01:42Z", ...

=== Signature ===
Bytes (hex): a4f...
Length: 64 bytes

=== Verification ===
Round-trip verify: PASS ✓
Tampered payload reject: PASS ✓
```

Exit code is 0 on success, 1 if the round-trip verify fails or if
tampering is silently accepted (which would be a security bug).

## What this proves

| Spec section | Demonstrated by |
|---|---|
| §4.1 Key material — Ed25519 + X25519 | `generateKeyPairSync` calls |
| §4.2 AID derivation — `SHA-256(ed_pk \|\| 0x00 \|\| x_pk)` + Base58btc | `aidPayload`, `aidDigest`, `aid` |
| §5.3 Replay protection — nonce, iat, exp ≤ 600s | `nonce`, `iat`, `exp` fields |
| §6.1 purchase.create Intent shape | `intent` object |
| Signature scheme — Ed25519 over canonical bytes | `sign(...)` + verify round-trip |
| Negative case — tampered payload MUST fail | `tamperedOk` check |

## What this does NOT do

- **Canonical CBOR.** ICP-1.0 signs canonical JSON (RFC 8785 JCS) — which
  is exactly what the demo does (with a dependency-free JCS subset). The
  binary CBOR profile (RFC 8949 §4.2.2) is reserved for icp-1.1; when it
  ships, the signing primitive (Ed25519 over canonical bytes) stays
  identical and only the serialization changes.
- **PrincipalBinding signing.** The demo uses a placeholder
  `<would-be-signed-by-principal-key>`. A real PrincipalBinding is signed
  by the Principal's key, not the Agent's. See
  `schemas/intent.purchase.create.schema.json` → `PrincipalBinding`.
- **ML-DSA-65 hybrid signatures.** ICP-1.0 makes hybrid PQC OPTIONAL.
  When supported, the `s` array in the signed envelope contains both
  signatures and verifiers MUST verify both. Stock `node:crypto` doesn't
  ship ML-DSA-65 yet; the reference Rust implementation in
  `crates/stateset-crypto/` does.
- **Settler interaction.** The Intent names a Settler but the demo doesn't
  submit it. See `examples/02-escrow-roundtrip/` (forthcoming) for the
  full lifecycle including a stub Settler.

## Why a Node demo, not a Rust demo

The reference implementation is in Rust. But the audience for this spec
is also engineers writing TypeScript/Node clients, Python integrations,
and curious reviewers from Anthropic / OpenAI / Stripe / Coinbase who
want to grok the model in 30 seconds. Node + zero deps is the format
that runs everywhere with no build step.

The Rust equivalent (using `crates/stateset-crypto`) ships in
`examples/icp_roundtrip.rs` and is the seed for the conformance suite.

## Becoming test vector 01

This demo is the human-readable seed for
`icp-conformance/vectors/icp-1.0/01-aid-derivation/`.
The conformance vector replaces the random keypair with a deterministic
HKDF-derived seed (so output is byte-identical across implementations)
and asserts the resulting AID matches the recorded fixture.

Conversion plan:
1. Replace `generateKeyPairSync('ed25519')` with key-from-seed using
   PKCS#8 envelope `30 2e 02 01 00 30 05 06 03 2b 65 70 04 22 04 20 || seed`.
2. Use named seed `icp-test-vector:01-aid-derivation:agent-a` per
   `test-vectors/README.md` HKDF rules.
3. Record expected AID, expected signature bytes, expected canonical
   payload bytes in `expected.json`.
4. Add a negative case fixture with a single bit-flip in the payload.
