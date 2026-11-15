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

Current vector set — ten families. `01`–`09` form the `icp-1.0-core`
profile; `10` is the `icp-1.0-commerce` profile, layered on top of it:

| Vector | Covers |
|---|---|
| `01-aid-derivation` | §4 key material, AID derivation, Intent signing |
| `02-canonical-json` | §5.1 Canonical JSON (RFC 8785 JCS) |
| `03-signature-verification` | §5.2 Ed25519 verification incl. negative cases |
| `04-escrow-lifecycle` | §8 escrow transition matrix and event replay |
| `05-intent-validation` | §6 intent envelope structural validation |
| `06-quote-binding` | §11.4 exact-decimal `max_total` ceiling |
| `07-settlement-receipts` | §9 co-signed SettlementReceipt verification |
| `08-timing` | §5.3 replay window (`iat`/`exp`) |
| `09-ceilings` | §6.2/§6.6 `max_refund` and `max_per_payout` ceilings |
| `10-commerce-invariants` | commerce-execution invariants (refund/capture bounds, returns, reservations, balanced journals, currency scale) |

The authoritative list is the `vectors` array of each file in
[`../../icp-conformance/profiles/`](../../icp-conformance/profiles/); each
family's `description.md` states exactly what it tests.

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
