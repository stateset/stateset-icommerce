# ICPIP-0008: Pre-Settlement Compliance Checkpoints

```
ICPIP:        0008
Title:        Pre-Settlement Compliance Checkpoints
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/8 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-08-31
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Adds a protocol-native **compliance checkpoint** between escrow funding and
settlement: a point in the §8 state machine where the Settler **MUST**
evaluate sanctions screening, jurisdiction rules, and KYC attestation
references before value becomes irrevocable, and **MUST** record the
result in the signed EscrowEvent chain. Compliance evidence enters the
protocol as **signed intent fields and Settler obligations, not
middleware**: the parties' attestation references travel inside the
Intent's signature payload, and the checkpoint verdict is a first-class,
replayable event.

ICP-1.0 already names the failure modes (`policy.value_above_kyc_floor`,
`policy.cross_border_restricted`, `policy.payout.compliance_hold`,
`settler.paused`) but says nothing about *when* screening happens, *what*
evidence it consumes, or *how* a verdict is recorded. This ICPIP closes
that gap for ICP-1.1.

## Motivation

Every custodial Settler operating in a real jurisdiction must screen
parties before releasing value — this is not optional and does not become
optional because the parties are autonomous agents. Today each Settler
would bolt screening onto its internal pipeline, invisibly to the
protocol. That has three failure modes:

1. **Unverifiable holds.** A Settler that pauses settlement for review
   emits nothing normative; the buyer agent sees a stuck escrow and
   cannot distinguish compliance review from operator failure, so it
   cannot decide whether to dispute, wait, or write the Settler off.
2. **Non-portable evidence.** A principal that has completed KYC with one
   Settler re-attests from scratch with every other Settler, because the
   protocol defines no way to reference an attestation.
3. **Retrofitted audit trails.** When a regulator asks why a settlement
   was released, the answer lives in a Settler's private logs rather than
   in the same signed, replayable event chain that proves everything else
   about the transaction.

The kernel roadmap calls this workstream the moat "precisely because it is
unglamorous": the commerce protocol that wins will be the one where
compliance hooks were native before regulators forced retrofits.

## Specification

### 1. `compliance` Intent field

A new **OPTIONAL** top-level payload field on every value-transferring
verb (`purchase.create`, `purchase.return`, `subscription.create`,
`payout.request`):

```json
"compliance": {
  "buyer_jurisdiction": "US-CA",
  "merchant_jurisdiction": "DE",
  "attestations": [
    {
      "kind": "kyc",
      "issuer": "aid:v1:zQ3shIssuer...",
      "subject": "aid:v1:zQ3shBuyer...",
      "ref": "https://attest.example.com/kyc/01HXYZ...",
      "hash": "<sha256-hex of the attestation document>",
      "expires": "2027-08-31T00:00:00Z"
    }
  ]
}
```

- Jurisdictions are ISO 3166-1 alpha-2, optionally suffixed with a
  subdivision per ISO 3166-2. Parties **SHOULD** declare the jurisdiction
  of their Principal, not of their infrastructure.
- `attestations[].kind` is one of `kyc`, `kyb`, `sanctions_screen`,
  `tax_residency`; the set is extensible by later ICPIPs.
- `ref` resolves to the attestation document (HTTPS or IPFS); `hash`
  binds the reference so a swapped document is detectable. The document
  format is out of scope for this ICPIP (Verifiable Credentials are the
  expected carrier).
- The field sits inside the signed payload: parties cannot repudiate the
  evidence they presented.
- Backward compatibility: ICP-1.0 verifiers ignore unknown payload
  fields; a missing `compliance` field means "no evidence presented" and
  is evaluated exactly as today.

### 2. The checkpoint in the escrow state machine

One new escrow state, `compliance_hold`, and three transitions extend the
§8 table:

| From | To | Trigger | Required signature |
|---|---|---|---|
| `funded` | `compliance_hold` | Settler opens a compliance review | Settler |
| `compliance_hold` | `released` | Review passes AND the §8 release conditions hold | Settler (+ Merchant per §8) |
| `compliance_hold` | `refunded` | Review fails terminally | Settler |

- The checkpoint is **mandatory in effect, optional in mechanics**: a
  Settler **MUST** evaluate its compliance policy before any transition
  to `released` or `refunded`, but MAY do so instantaneously without
  entering `compliance_hold`. The state exists for reviews that take
  time; sub-second screening needs no extra event.
- An escrow **MUST NOT** remain in `compliance_hold` longer than the
  Settler's published `max_review_window` (see §4); on expiry the Settler
  **MUST** transition to `refunded` with `policy.review_window_expired`.
- Dispute rights are unaffected: `compliance_hold` does not suspend the
  dispute window, and `disputed` still takes precedence.

### 3. CheckpointResult event

The EscrowEvent emitted on leaving `compliance_hold` (or on instantaneous
evaluation, attached to the `released`/`refunded` event) **MUST** carry a
`checkpoint` object:

```json
"checkpoint": {
  "verdict": "pass" | "fail" | "expired",
  "evaluated_at": "2026-08-31T12:00:00Z",
  "evidence": [ { "kind": "kyc", "hash": "<sha256-hex>", "result": "accepted" } ],
  "codes": []
}
```

`evidence[].hash` echoes the attestation hashes actually consumed, so the
audit trail proves *which* documents the verdict rested on without
embedding their contents. On `fail`, `codes` carries the normative error
codes below. The checkpoint object is inside the signed EscrowEvent:
the verdict is as tamper-evident as the money movement.

### 4. Settler obligations (SETTLERS.md §S.6)

A conformant Settler:

- **MUST** publish in its capability document: supported jurisdictions,
  required attestation kinds by value tier, trusted attestation issuers,
  and `max_review_window` (RFC 3339 duration).
- **MUST** evaluate its published policy before every settlement, and
  **MUST NOT** apply unpublished criteria (no secret policy — a verdict
  must be explainable by the published rules plus the presented
  evidence).
- **MUST** emit the CheckpointResult event per §3.
- **MUST NOT** use `compliance_hold` as a liquidity instrument; the
  proof-of-reserves obligations of §S.4 apply to held funds throughout.

### 5. Error codes

Four additions to the `policy` namespace in `schemas/error-codes.md`:

| Code | Emission condition |
|---|---|
| `policy.sanctions_screen_failed` | A party matched a sanctions list the Settler screens against |
| `policy.jurisdiction_unsupported` | Declared jurisdiction outside the Settler's published set |
| `policy.attestation_expired` | Referenced attestation past `expires` at evaluation time |
| `policy.attestation_issuer_untrusted` | Attestation issuer not in the Settler's published issuer set |
| `policy.review_window_expired` | `compliance_hold` exceeded the published `max_review_window` |

### 6. Conformance vectors

Promotion past Review requires a new vector family
`11-compliance-checkpoints` covering: pass-through (no `compliance`
field, low value), instantaneous pass with evidence echo, hold → release,
hold → refund on each failure code, review-window expiry, and a tampered
attestation hash rejected with `format.invalid_field`.

## Rationale

**Why intent fields + Settler obligations, not middleware?** A screening
API in front of the Settler (the middleware design) leaves the verdict
outside the signed event chain — exactly the retrofitted-audit-trail
failure this ICPIP exists to prevent. Putting references in the signed
Intent and verdicts in signed EscrowEvents means the entire compliance
story replays from `seq=0` like everything else.

**Why references, not embedded credentials?** Attestation documents are
large, privacy-sensitive, and jurisdiction-shaped. Embedding them would
put personal data on every wire hop and inside every archive. A
`ref` + `hash` pair keeps the protocol payload clean while making the
evidence binding.

**Why a state, not a flag?** Reviews take hours to days. Without a
normative state, a held escrow is indistinguishable from a stalled one
(motivation #1). With one, agents can branch on it, SLAs can bound it,
and the timing vectors of family 08 extend naturally.

**Why allow instantaneous evaluation?** Requiring a `compliance_hold`
round-trip on every sub-second screen would double event volume for the
overwhelmingly common case. The obligation is the evaluation and the
recorded verdict, not the intermediate state.

## Backwards compatibility

ICP-1.0 implementations ignore the `compliance` payload field (unknown
fields are ignored) and never see `compliance_hold` (they do not talk to
ICP-1.1 Settlers). For mixed fleets, a 1.1 Settler facing a 1.0
counterparty MAY still hold and screen — it just cannot expect presented
attestations, and its checkpoint events will be unknown to the 1.0 party;
the escrow still resolves through the 1.0-visible terminal states.

## Security considerations

- The attestation `hash` prevents reference swapping but not issuer
  compromise; the trusted-issuer set is the Settler's published,
  governance-visible choice.
- `compliance` fields leak jurisdiction information to counterparties.
  This is deliberate — cross-border applicability is exactly what the
  parties need to evaluate before committing — but implementations
  **SHOULD NOT** put more than the declared fields there, and the
  confidential-payload encryption of §4.1 applies to the Intent as usual.
- A malicious Settler could `fail` verdicts to expropriate via refund
  routing. Refunds return value to the Buyer, so the attack yields the
  Settler nothing; slow-hold griefing is bounded by `max_review_window`
  and visible in the §S.5 SLA metrics.

## Open questions (for Review)

- Should `checkpoint.evidence` results distinguish `accepted` /
  `ignored` / `rejected` per attestation, or is the aggregate verdict
  enough?
- Does `tax_residency` belong here or in the SettlementReceipt tax
  extension (roadmap 3.2, a companion ICPIP)?
- Outside counsel review is required before this ICPIP advances to
  Last Call (kernel roadmap 3.1 exit criterion).

## Copyright

CC-BY-4.0, consistent with the ICP specification license.
