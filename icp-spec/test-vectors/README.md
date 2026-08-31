# ICP Test Vectors

**The live, normative vectors have moved.** They are maintained in the
conformance suite at
[`../../icp-conformance/vectors/icp-1.0/`](../../icp-conformance/vectors/icp-1.0/)
and executed by the conformance runner:

```sh
cd icp-conformance
node runner/run.mjs --profile icp-1.0-core --iut reference-demo
```

Any ICP-1.0 conformant implementation **MUST** produce identical output
for every positive vector and **MUST** reject every negative vector with
the documented error code.

Current vector set (`icp-1.0-core` profile):

| Vector | Covers |
|---|---|
| `01-aid-derivation` | §4 key material, AID derivation, Intent signing |
| `02-canonical-json` | §5.1 Canonical JSON (RFC 8785 JCS) |
| `03-signature-verification` | §5.2 Ed25519 verification incl. negative cases |

## Vector format

Each test case is a directory containing:

| File | Purpose |
|---|---|
| `description.md` | Human-readable description, citing spec sections |
| `inputs.json` | Deterministic inputs (seeds, payloads, signatures) |
| `expected.json` | Expected verification results, derived state, computed values |

The vector set reached full normative coverage (ten families, four IUTs
byte-identical) at ICP-1.0 Last Call — see `../LAST-CALL.md`. Within the
1.0 major it now only grows by suite-patch additions.

## Determinism

All cryptographic material is derived from named seeds. **No random key
generation.** This guarantees byte-for-byte reproducibility across
implementations.

Test seeds use the namespace `icp-test-vector:<vector-id>:<role>` fed to
HKDF-SHA256 with the salt `ICP-1.0-TEST-VECTORS-DO-NOT-USE-IN-PRODUCTION`.
