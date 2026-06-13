# ICPIP-0002: Hybrid Ed25519 + ML-DSA-65 Signature Mandate for High-Value Intents

```
ICPIP:        0002
Title:        Hybrid Ed25519 + ML-DSA-65 Signature Mandate for High-Value Intents
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/2 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-05-12
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Tightens ICP's existing OPTIONAL hybrid signature provision (ICP-1.0 §4.1) to
a **MANDATORY** requirement for Intents whose `max_total` (or equivalent
value field) equals or exceeds **$10,000 USD-equivalent**. Below the
threshold, signatures remain Ed25519-only per ICP-1.0. Above the threshold,
the wire format `alg` field MUST be `ed25519+ml-dsa-65`, and verifiers MUST
require both signature components to validate.

This proposal addresses the **harvest-now-decrypt-later (HNDL) threat
model**: signatures captured today on classical-only ciphers will become
forgeable once cryptographically-relevant quantum computers (CRQCs) arrive.
NIST PQC migration guidance (May 2025), CNSA 2.0 (NSA, Sept 2022), and
ENISA's PQC roadmap (Oct 2024) all converge on the same deadline range:
hybrid by 2030, pure-PQC by 2035. ICP's transaction lifecycles —
particularly long-running SettlementReceipts retained for 7+ years per
SETTLERS.md §S.3 — fall squarely inside the HNDL exposure window.

Tightening at the value threshold preserves throughput for small
transactions (where the cryptographic overhead would dominate latency)
while protecting the high-value $-flow that's most attractive to a
retroactive attacker.

## Motivation

### The harvest-now-decrypt-later threat

A patient adversary in 2026 cannot break Ed25519 today. But they can
**record signed payloads today** and verify them once CRQCs arrive. The
window of practical CRQC viability is publicly debated, but the
consensus point estimates from authoritative sources align:

| Source | CRQC arrival window |
|---|---|
| Mosca (Quantum Risk Assessment 2022) | 2030–2040 (1-in-7 by 2030, 1-in-2 by 2040) |
| NIST PQC Migration (NIST IR 8547, 2025) | "by 2035" — hard deprecation of classical KEMs and signatures |
| CNSA 2.0 (NSA, Sept 2022) | NSS systems: hybrid by 2030, pure PQC by 2035 |
| BSI (Germany) | "CRQC plausible 2030+, plan for 2025–2030 migration" |

ICP transactions retained beyond ~2030 are within the HNDL exposure
window. The most-valuable-target subset (large settlements, recurring
SaaS billing aggregations) is the most attractive harvest target.

### Why $10,000 USD-equivalent

Tightening at a value threshold rather than universally:

1. **Preserves throughput**. ML-DSA-65 signatures are ~3293 bytes vs
   Ed25519's 64 bytes (51× larger) and ~10× slower to verify on a CPU.
   For a $0.01 micro-purchase, the overhead is disproportionate. For a
   $100,000 wholesale order, the overhead is rounding.
2. **Matches existing regulatory thresholds**. The Bank Secrecy Act
   Currency Transaction Report threshold is $10,000. FinCEN's SAR
   guidance follows. ICP-1.0 already uses $10,000 as the
   `policy.value_above_kyc_floor` boundary.
3. **Encourages graduated adoption**. Implementers can ship Ed25519-only
   for their initial integration and add ML-DSA-65 when they cross the
   threshold — no big-bang migration.

### Why hybrid (not pure PQC)

A pure-PQC mandate is premature in 2026:

- ML-DSA-65 is **young**: standardized in FIPS 204 (Aug 2024). Real-world
  attack surface is not yet well-explored. A future cryptanalytic break
  of ML-DSA would invalidate every signature produced under a pure-PQC
  mandate.
- Ed25519 has **15+ years of survival** against active attack. The
  probability of a classical break is now low.
- Hybrid is provably no weaker than the stronger component: an attacker
  must break BOTH Ed25519 AND ML-DSA-65 to forge a hybrid signature.
- NIST's own CNSA 2.0 guidance specifies **hybrid for 2030–2035**, then
  pure PQC after — explicitly because of ML-DSA's adolescence.

Pure-PQC migration is the topic of a future ICPIP (likely ~2032), conditional
on cryptographic maturity at that point.

### Why mandate at the protocol level

A merchant or Settler that "prefers" hybrid signatures cannot enforce
them — Intents arrive over the wire and the merchant's only choice is
to accept or reject. Wire-format mandates are the **only** enforcement
mechanism that survives a malicious counterparty.

## Specification

### Threshold calculation

An Intent is **high-value** if its `max_total` (for `purchase.create`),
`max_total_per_period` (for `subscription.create`), `max_refund` (for
`purchase.return`), or `max_per_intent` (in PrincipalBinding) — whichever
applies to the verb — equals or exceeds **$10,000 USD-equivalent** at the
Intent's `iat` timestamp.

USD-equivalent conversion uses one of the following oracle sources, in
order of preference:

1. **Chainlink** USD price feeds for the named currency, fetched within
   60 seconds of `iat` (preferred for crypto-rail Settlers).
2. **OANDA** end-of-day mid-market FX rates for fiat (preferred for
   fiat-rail Settlers).
3. **Settler-published exchange rate**: any Settler in the
   `settler.discovery_doc.exchange_rates[]` array MUST publish a signed
   rate; merchants MAY use the Settler's own rate if no independent
   oracle is reachable, with the trade-off that the rate is then trusted.

A Settler choosing option 3 MUST advertise the rate freshness window in
its discovery document. Rates older than 24 hours MUST NOT be used for
threshold determination.

### Wire format change

For high-value Intents, the signature envelope MUST be:

```json
{
  "alg": "ed25519+ml-dsa-65",
  "kid": <AID-string>,
  "sig": <ed25519_sig (64 bytes) || ml_dsa_65_sig (3293 bytes)>
}
```

The `sig` field is the concatenation `ed25519_sig || ml_dsa_65_sig`
(64 + 3293 = 3357 bytes total). Verifiers MUST:

1. Split `sig` at offset 64.
2. Verify the first 64 bytes as an Ed25519 signature over the canonical
   payload.
3. Verify the remaining 3293 bytes as an ML-DSA-65 signature over the
   same canonical payload.
4. Both signatures MUST validate for the Intent to be accepted.

If `alg` is `ed25519` (legacy) on a high-value Intent, verifiers MUST
reject with `signature.hybrid_required` (new error code, see below).

### Key binding

The PrincipalBinding (ICP-1.0 §4.3) MUST carry the ML-DSA-65 public key
when the Agent is authorized for high-value Intents:

```json
{
  "principal": "did:web:big-buyer.example",
  "agent": "aid:v1:zA...",
  "ml_dsa_65_pubkey_hex": "<base64url-encoded ML-DSA-65 public key>",
  "authority": {
    "max_per_intent": { "amount": "100000", "currency": "USDC" },
    ...
  },
  ...
}
```

The PrincipalBinding signature itself MUST be hybrid (`ed25519+ml-dsa-65`)
if the bound `max_per_intent` exceeds the threshold.

The AID derivation (ICP-1.0 §4.2) is **unchanged** — the AID hash
remains `SHA-256(ed_pk || 0x00 || x_pk)`. ML-DSA-65 keys are bound at
the PrincipalBinding level, not the AID level, so:

1. Existing AIDs remain stable across the ICP-1.x → ICP-2.0 boundary.
2. Agents can rotate to PQC by issuing a new PrincipalBinding without
   changing AID.
3. AID-based routing, reputation, and indexing don't break.

This is a deliberate trade-off vs. embedding the ML-DSA-65 key in the
AID hash: the latter would yield true PQC identities but would break
all existing AIDs at migration. The PrincipalBinding-level binding
preserves continuity at the cost of one extra hop in verification.

### Error codes

New normative error codes (added to `schemas/error-codes.md`):

| Code | When emitted |
|---|---|
| `signature.hybrid_required` | High-value Intent (over threshold) signed with `alg: ed25519` only |
| `signature.hybrid_incomplete` | High-value Intent `sig` field is wrong length (not exactly 3357 bytes) |
| `signature.ml_dsa_invalid` | Ed25519 component verified but ML-DSA-65 component failed |
| `signature.ml_dsa_pubkey_missing` | PrincipalBinding for high-value Intent omits `ml_dsa_65_pubkey_hex` |
| `auth.ml_dsa_pubkey_mismatch` | Submitted Intent's pubkey doesn't match PrincipalBinding |
| `policy.por_oracle_stale` | All oracle sources for USD-equivalent conversion >24h old |

### Migration window

- **ICP-1.x** (current line, including 1.2): hybrid is SHOULD for
  high-value Intents. Implementations are encouraged to support hybrid
  but not required.
- **ICP-2.0**: hybrid is MUST for high-value Intents. The 12 months
  preceding ICP-2.0 ratification serve as the **soft enforcement
  window**: verifiers SHOULD log Ed25519-only high-value Intents as
  policy warnings but SHOULD NOT reject them.
- **ICP-2.0 + 1 year**: hard enforcement. Ed25519-only high-value
  Intents MUST be rejected.

The Foundation board ratifies the ICP-2.0 promotion date based on:

1. At least **3 independent implementations** with verified hybrid
   support passing the conformance vector (this proposal's §"Test
   vectors").
2. ML-DSA-65 library availability across Node.js, Rust, Go, and Python
   (the four current ICP IUT languages).
3. No known cryptanalytic compromise of ML-DSA-65 in the 12 months
   prior to promotion.

### Settler obligations

Settlers (per SETTLERS.md) MUST verify both signature components on
every received Intent and EscrowEvent above threshold. A Settler that
silently accepts Ed25519-only high-value Intents post-ICP-2.0 is
non-conformant; conformance certification (§icp-conformance/) will fail
for any Settler that doesn't enforce hybrid above threshold.

## Rationale

### Why this design, not alternatives

**Alternative 1: Hybrid for all transactions, regardless of value.**
Rejected — disproportionate overhead for micro-transactions where the
3.3 KB signature dominates a $0.01 Intent's payload. Throughput
matters, especially for high-call-volume verbs like `inventory.query`.

**Alternative 2: Pure PQC at threshold (no Ed25519 fallback).**
Rejected — ML-DSA-65 is too young; a future cryptanalytic break would
catastrophically invalidate every signature. Hybrid is strictly safer
during ML-DSA-65's maturation period.

**Alternative 3: ML-KEM-768 instead of ML-DSA-65.** Rejected — ML-KEM
is a KEM (encryption), not a signature scheme. The ML-DSA family is
NIST's signature counterpart. ML-KEM-768 is in scope for a SEPARATE
future ICPIP addressing confidential Intent payloads (currently
ICP-1.0 uses X25519 for ECDH; the solicited ICPIP-0007 will mandate
hybrid X25519+ML-KEM-768 for confidential intents).

**Alternative 4: Different threshold ($1k? $100k?).** Rejected —
$10,000 matches the existing BSA CTR threshold and ICP's existing
`policy.value_above_kyc_floor` boundary. Lower thresholds would
disproportionately burden mid-market commerce; higher thresholds would
leave too much HNDL surface unprotected.

**Alternative 5: SLH-DSA (formerly SPHINCS+) instead of ML-DSA-65.**
Rejected — SLH-DSA signatures are ~10× larger than ML-DSA-65 (~30 KB
vs 3.3 KB), and ~100× slower. The security margin is higher but the
throughput cost is impractical for ICP's transaction volume.
SLH-DSA may become viable for very-high-value transactions
(>$10M) in a future ICPIP.

### Comparison to other agentic-commerce protocols

| Protocol | Signature scheme | PQC plan |
|---|---|---|
| AP2 (Google) | secp256r1 (ECDSA) | Not yet specified |
| ACP (OpenAI/Stripe) | secp256k1 (ECDSA) | Not yet specified |
| x402 (Coinbase) | secp256k1 | Not yet specified |
| **ICP-1.0** | **Ed25519** (+optional ML-DSA-65) | This ICPIP |
| **ICP-2.0** (proposed) | **Ed25519 + ML-DSA-65 hybrid at threshold** | Pure PQC ~2032+ |

ICP would be the **first agentic-commerce protocol to mandate PQC** at any
value threshold. This is a deliberate competitive positioning: high-value
B2B procurement and SaaS billing are the deepest pockets in agentic
commerce, and they're the customers most concerned about long-term
audit-grade signature retention. Mandating PQC for these flows is a
direct sales benefit.

### Comparison to traditional finance precedent

- **SWIFT**: announced ISO 20022 PQC roadmap in 2024; soft mandate by 2027.
- **Fedwire**: pilot PQC integration with NIST CSF reference 2025.
- **SEC EDGAR**: planning hybrid signature support for filings 2027+.

Tightening ICP's signature scheme matches the regulated-finance
trajectory, not exceeds it.

## Backwards Compatibility

### Below-threshold Intents (no change)

All Intents with values below $10,000 USD-equivalent continue to use
Ed25519-only signatures with `alg: ed25519`. ICP-1.0 implementations
remain valid for this traffic.

### Above-threshold Intents

ICP-1.x implementations that haven't adopted hybrid will see two
behavior changes:

1. **Outgoing**: their above-threshold Intents will be rejected by
   ICP-2.0+ verifiers. The merchant Backend SHOULD detect this on the
   first rejection and (a) downgrade the Intent to below threshold
   (split into multiple smaller Intents), or (b) upgrade the Agent's
   signing software.
2. **Incoming**: above-threshold Intents will be sent in
   `ed25519+ml-dsa-65` format. ICP-1.x verifiers will see an
   unrecognized `alg` and SHOULD reject with `signature.algorithm_unsupported`,
   which the sender SHOULD recognize as a signal to retry with
   Ed25519-only IF the receiver is known to be ICP-1.x.

Production migration playbook:

1. **Month 0** (ICP-1.x): existing flows continue unchanged.
2. **Month 1**: all ICP-1.x implementations begin supporting hybrid as
   an OPTIONAL alternative (this is already true per ICP-1.0 §4.1).
3. **Month 6**: most merchants advertise hybrid support in their
   discovery document.
4. **Month 12**: ICP-2.0 ratified. Hybrid becomes MUST above threshold.
   Soft warnings begin.
5. **Month 24**: hard enforcement.

### Schema versioning

The wire `v` field stays at `"icp-1.0"` through the soft-enforcement
period. ICP-2.0 changes `v` to `"icp-2.0"`. Implementations MAY
support both versions simultaneously and SHOULD prefer the higher one.

## Security Considerations

### Hybrid attack surface

Three failure modes:

1. **Ed25519 break (classical or quantum)**: Ed25519 component
   invalidates, but ML-DSA-65 still holds. Total signature still
   considered valid → **security degraded to ML-DSA-65 alone**.
   Mitigation: ML-DSA-65 selected because no known classical attack;
   it's NIST-vetted.
2. **ML-DSA-65 break**: ML-DSA component invalidates, but Ed25519
   still holds. Total signature still valid → **security degraded to
   Ed25519 alone**. Mitigation: 15+ years of Ed25519 maturity.
3. **Both break**: hybrid provides no protection. Mitigation: vanishingly
   unlikely simultaneously; if it happens, the entire ICP signing layer
   needs a fast revocation + re-keying.

Crucially, hybrid is **provably no weaker** than the stronger component.
A break of one does not enable forgery; only a break of BOTH does. This
is the central security argument for hybrid over either pure scheme.

### Key management complexity

Each ICP Agent above threshold must maintain TWO signing keys (Ed25519
+ ML-DSA-65). Practical implications:

- **Generation**: independent key generation; agents MUST NOT derive
  ML-DSA-65 keys from Ed25519 seeds via HKDF or any other reduction.
- **Storage**: HSMs supporting Ed25519 are mature (AWS KMS, Google
  Cloud KMS, Azure Key Vault); HSMs supporting ML-DSA are nascent
  (HashiCorp Vault since 1.18; AWS KMS announced support Q3 2025).
- **Rotation**: rotating either key produces a new AID (since the AID
  hashes the Ed25519 pubkey). Rotating ML-DSA-65 without changing
  Ed25519: not possible at the AID level; achieved by reissuing the
  PrincipalBinding with a new ML-DSA-65 key while keeping the AID.

### Side-channel resistance

ML-DSA-65 implementations have known side-channel concerns (rejection
sampling can leak timing information). This proposal MANDATES that
production implementations use constant-time ML-DSA-65 code (e.g. NIST
reference implementation v3.1 or later, or `pqcrystals-dilithium`
hardened builds). Conformance certification will include side-channel
spot-checks.

### Oracle manipulation

The USD-equivalent threshold determination is oracle-driven. An attacker
who can manipulate the oracle to report a sub-threshold rate at `iat`
could downgrade a high-value Intent to Ed25519-only. Mitigations:

1. **Multi-source consensus**: implementations SHOULD consult ≥2
   oracle sources and reject Intents where they disagree by >5%.
2. **Settler-published rates** (option 3) are signed by the Settler;
   manipulation requires Settler compromise, which is a much higher
   bar than oracle compromise.
3. **24-hour freshness window**: stale rates are rejected.

### Quantum-safe revocation

PrincipalBinding revocation URLs are currently HTTPS endpoints. Post-CRQC,
HTTPS itself is broken (TLS 1.3 uses classical KEMs). The revocation
infrastructure MUST migrate to PQC-secured transport (TLS 1.3 with
hybrid X25519+ML-KEM-768, or future TLS 1.4) on the same timeline as
this signature mandate. A separate ICPIP (forthcoming, ICPIP-0007)
will address confidential transport for revocation and PrincipalBinding.

## Test Vectors

Conformance vector `03-hybrid-pqc-signing` (to be added to
`icp-conformance/vectors/icp-2.0/`):

```
inputs.json:
{
  "test": "03-hybrid-pqc-signing",
  "agent": {
    "ed25519_seed_hex": "<32-byte hex>",
    "ml_dsa_65_seed_hex": "<32-byte hex>"   // deterministic ML-DSA per FIPS 204 §3.6
  },
  "intent": {
    "v": "icp-2.0",
    "verb": "purchase.create",
    "intent_id": "icp_int_...",
    "max_total": { "amount": "15000.00", "currency": "USDC" },  // above threshold
    ...
  }
}

expected.json:
{
  "ed25519_pubkey_hex": "...",
  "ml_dsa_65_pubkey_hex": "...",
  "aid": "aid:v1:z...",                     // unchanged from ICP-1.0
  "intent_canonical_string": "...",
  "intent_signature_hex": "<3357 bytes hex>" // ed25519_sig || ml_dsa_65_sig
}
```

The vector will use **NIST CAVP test vectors for ML-DSA-65** (FIPS 204
Appendix B) for the seed material, so implementations can
independently verify their ML-DSA-65 layer against the NIST reference
output before composing with Ed25519.

A negative case: a high-value Intent signed with Ed25519-only MUST
produce verification failure with code `signature.hybrid_required`.

## Reference Implementation

The Rust IUT (`crates/stateset-icp-iut`) currently has ML-DSA-65 access
via the `ml-dsa` crate, gated behind the `pqc` feature in
`crates/stateset-crypto`. Updating it to produce hybrid signatures
above threshold: estimated ~120 LOC + tests.

The other three IUTs need additions:

| IUT | Status | Path to compliance |
|---|---|---|
| JS (`reference-demo.mjs`) | No ML-DSA | Use `@noble/post-quantum` (npm) or wasm shim |
| Go (`stateset-icp-iut-go`) | No ML-DSA | Use `github.com/cloudflare/circl/sign/dilithium/mode3` |
| Python (`stateset-icp-iut-py`) | No ML-DSA | Use `oqs-python` or `cryptography` once ML-DSA lands |

All four implementations passing the hybrid-PQC conformance vector is a
hard precondition for ICP-2.0 ratification per the migration window §.

## References

- FIPS 204 — Module-Lattice-Based Digital Signature Algorithm
  (https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)
- NIST IR 8547 — Considerations for Achieving Crypto Agility (2025)
- CNSA 2.0 — Commercial National Security Algorithm Suite 2.0 (NSA, 2022)
- ENISA Post-Quantum Cryptography report (2024)
- RFC 9180 — Hybrid Public Key Encryption (HPKE)
- ICP-1.0-DRAFT §4.1 (Key material), §4.3 (PrincipalBinding)
- ICPIP-0001 (this ICPIP follows the lifecycle defined there)
- ICP Foundation Charter §3.4 (spec stewardship veto)

## Copyright

This ICPIP is licensed under CC-BY-4.0 (the same license as the ICP
specification prose).
