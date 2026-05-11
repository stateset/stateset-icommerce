# ICP Test Vectors

Normative reference vectors. Any ICP-1.0 conformant implementation **MUST**
produce identical output for every positive vector and **MUST** reject every
negative vector with the documented error code.

## Layout

```
test-vectors/
├── icp-1.0/
│   ├── 01-aid-derivation/
│   ├── 02-signature-ed25519/
│   ├── 03-signature-hybrid-pqc/
│   ├── 04-canonical-cbor/
│   ├── 05-replay-protection/
│   ├── 06-intent-purchase-create/
│   ├── 07-quote-binding/
│   ├── 08-escrow-state-machine/
│   ├── 09-settlement-receipt/
│   ├── 10-principal-binding/
│   └── 99-negative-cases/
└── README.md
```

## Vector format

Each test case is a directory containing:

| File | Purpose |
|---|---|
| `description.md` | Human-readable description, citing spec section |
| `keys.json` | Test keypairs (DETERMINISTIC, derived from named seed) |
| `inputs/` | One CBOR file per input message |
| `expected.json` | Expected verification result, derived state, computed values |
| `negative.json` | (negative cases only) The error code and message |

## Determinism

All cryptographic material is derived from named seeds documented in
`keys.json`. **No random key generation.** This guarantees byte-for-byte
reproducibility across implementations.

Test seeds use the namespace `icp-test-vector:<vector-id>:<role>` fed to
HKDF-SHA256 with the salt `ICP-1.0-TEST-VECTORS-DO-NOT-USE-IN-PRODUCTION`.

## Status

Vectors are being authored alongside ICP-1.0-DRAFT. Initial set ships at
ICP-1.0 Last Call. Until then, this directory is a placeholder; do not treat
absence of a vector as protocol-conformant behavior.
