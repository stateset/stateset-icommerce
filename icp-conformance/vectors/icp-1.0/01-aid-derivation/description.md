# Vector 01 — AID derivation and Intent signing

**Spec sections covered:** ICP-1.0 §4.1 (Key material), §4.2 (AID derivation),
§5.3 (Replay protection), §6.1 (purchase.create Intent), Ed25519 signing.

This is the **smallest end-to-end vector** in the suite. It exercises the
full identity-and-signing path that every higher-level vector depends on:

1. Construct Ed25519 + X25519 keypairs from deterministic 32-byte seeds.
2. Derive the AID per §4.2 (`aid:v1:` + Base58btc of `SHA-256(ed_pk || 0x00 || x_pk)`).
3. Build a canonical purchase.create Intent with all deterministic fields
   (intent_id, nonce, iat, exp, items, etc.).
4. Sign the canonical encoding with Ed25519.

If an implementation fails this vector, every other vector in the suite
will fail downstream.

## Seed derivation

The seeds in `inputs.json` were derived via HKDF-SHA256 with the
test-vector namespace from `icp-spec/test-vectors/README.md`:

```
salt = "ICP-1.0-TEST-VECTORS-DO-NOT-USE-IN-PRODUCTION"
ikm  = "icp-test-vector:01-aid-derivation:agent-a"
info = "ed25519-seed"  → ed25519_seed_hex
info = "x25519-seed"   → x25519_seed_hex
length = 32 bytes each
```

Implementers MAY re-derive the seeds from the namespace and verify they
match the bytes in `inputs.json`. The runner does NOT do this — it just
feeds the bytes from `inputs.json` to the adapter.

## Pass criteria

The adapter's stdout JSON MUST equal `expected.json` byte-for-byte after
key reordering and whitespace normalization. The runner compares:

- `ed25519_pubkey_hex`
- `x25519_pubkey_hex`
- `aid`
- `intent_canonical_string`
- `intent_signature_hex`

Any divergence is a FAIL with diff output for debugging.

## Negative case

`tamper_rejected` is checked when `params.verify_tamper_rejected` is
true: the adapter MUST verify that a tampered payload fails Ed25519
verification under the public key. A `false` here indicates a critical
security bug in the implementation.
