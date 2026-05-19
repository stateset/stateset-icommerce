# Vector 03 — Signature Verification

**Spec sections covered:** ICP-1.0 §4.1 (Ed25519 signatures), §5.2 (signature
envelopes), `schemas/error-codes.md` `signature.*` namespace.

Exercises the **inverse** of vector 01 — instead of producing signatures,
the IUT verifies them. Every ICP merchant, Settler, and SDK runs this
verification path on every received message. If any IUT diverges on
verification semantics, signatures don't interop and the protocol breaks.

## Sub-cases

The vector contains **8 sub-cases**: 1 positive sanity control and 7
deliberate negative cases. For each, the IUT calls Ed25519 verify with
the supplied `(canonical, signature_hex, pubkey_hex)` triple and
returns the boolean result.

| # | Case | Description | Expected |
|---|---|---|---|
| 1 | `valid-roundtrip`        | Sign-and-verify with the same key + canonical                | **true** |
| 2 | `tampered-message`       | Genuine signature, but `canonical` is modified (one word changed) | false |
| 3 | `bit-flipped-signature`  | Genuine canonical + key, but the signature's last byte is flipped | false |
| 4 | `wrong-pubkey`           | Genuine signature + canonical, but verified under a different keypair's pubkey | false |
| 5 | `truncated-signature`    | Signature is 63 bytes (one byte removed)                     | false |
| 6 | `padded-signature`       | Signature is 65 bytes (one byte appended)                    | false |
| 7 | `all-zero-signature`     | 64 zero bytes                                                | false |
| 8 | `random-bytes-signature` | 64 bytes of deadbeef pattern (not a real signature)          | false |

The **positive control** ensures the IUT didn't accidentally reject
everything; the 7 negative cases ensure it didn't accidentally accept
everything.

## Determinism

All keypairs are derived from named seeds:
- Primary keypair: RFC 8032 §7.1 test vector seed
  (`9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60`)
- Secondary keypair (for `wrong-pubkey`): a second RFC 8032 test vector
  (`4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb`)

The canonical payload is a simple ICP-shaped test object. Any
implementation that uses a real Ed25519 library will produce the same
verification results for these inputs.

## Pass criteria

The IUT's stdout JSON MUST contain `verifications` matching
`expected.verifications` byte-by-byte: an array of 8 booleans where the
first is `true` and the next 7 are `false`. Any divergence is a FAIL
with the diverging case index reported by the runner.

## Why this matters

Vector 01 proves implementations can SIGN consistently. Vector 02
proves they CANONICALIZE consistently. Vector 03 proves they VERIFY
consistently — the third leg of the cross-language interop stool.

Without vector 03, an implementation could pass 01 and 02 while
silently accepting tampered signatures in production. This vector
catches that class of bug at the conformance gate.
