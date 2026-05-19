# Settler Implementation Guide

You operate a custodian (regulated financial institution, on-chain
escrow contract, or some combination) and want to become a named
Settler on the ICP allowlist. This guide is the shortest path from
"have an escrow capability" to "appear in
`settlers.icp.dev/allowlist.json`."

The normative spec is [`SETTLERS.md`](../SETTLERS.md); this is the
operator-facing walkthrough. The reference binding for the first
production Settler is [`settlers/usdc-base.md`](../settlers/usdc-base.md) —
read it alongside this guide.

## What you're signing up for

Every Intent names exactly one Settler. The Settler holds escrowed
value between **acceptance** and **release/refund**, signs every state
transition, and produces the canonical **SettlementReceipt** that
downstream tax/accounting/audit systems treat as authoritative.

This makes you the single point of value capture for every ICP
transaction routed through you. The reward is volume; the cost is the
SLA commitments in [`SETTLERS.md`](../SETTLERS.md) §S.5 and the
proof-of-reserves obligation in §S.4.

## Eligibility

To be considered for the allowlist you must be:

1. A **legal entity with a published LEI** (Legal Entity Identifier).
2. **Regulated to custody value on the rail you operate** (e.g.
   money-transmitter licenses for USD rails, a smart contract under
   audit-attested governance for on-chain rails).
3. Willing to grant ICP's **royalty-free patent license** on operations
   covered by the spec.

Two existing allowlist members co-attest your inclusion. During the
bootstrap period, the StateSet attestation alone suffices.

## Pick a Settler URN

Your Settler ID is a hierarchical URN. Pick one that's specific enough
to disambiguate but generic enough to allowlist by prefix:

```
settler:<rail-family>.<asset>.<network>[.<custodian>]
```

Examples:

| URN | Meaning |
|---|---|
| `settler:circle.usdc.base` | Circle's USDC custody on Base L2 |
| `settler:stripe.treasury.ach` | Stripe Treasury operating ACH |
| `settler:fedwire.usd` | Fedwire (the rail itself, custodian-neutral) |

Counterparties allowlist by prefix (`settler:circle.*`), so the URN
hierarchy doubles as your namespace claim.

## The 5 capabilities you must implement

| ID | What | Spec |
|---|---|---|
| S.1 | Identity + discovery document | [`SETTLERS.md`](../SETTLERS.md) §S.1 |
| S.2 | Escrow lifecycle (fund/release/refund/dispute) | §S.2 |
| S.3 | SettlementReceipt issuance | §S.3 |
| S.4 | Proof-of-reserves (if you hold >$1M aggregate) | §S.4 |
| S.5 | Operational SLAs | §S.5 |

The next 5 sections walk through each.

## S.1 — Publish identity

Generate an **Ed25519 signing key** (recommended: an HSM-backed key for
production; key rotation is a separate process described in
[`SETTLERS.md`](../SETTLERS.md) §"Key rotation"). If you'll hold more
than $10M aggregate escrow at rest, also generate an **ML-DSA-65 hybrid
key** per [`ICPIP-0002`](../icpips/icpip-0002-hybrid-pqc.md).

Publish a discovery document at
`https://<your-domain>/.well-known/icp-settler`:

```json
{
  "settler_id": "settler:yourco.usdc.base",
  "operator": {
    "name": "Your Co., Inc.",
    "lei": "549300LFXJU8M0X8XV23",
    "jurisdiction": "US"
  },
  "signing_keys": [
    { "alg": "ed25519", "kid": "yourco-usdc-base-2026-q2", "pub": "z..." }
  ],
  "endpoints": {
    "fund":     "https://api.yourco.com/icp/v1/escrow/fund",
    "observe":  "wss://api.yourco.com/icp/v1/escrow/events",
    "release":  "https://api.yourco.com/icp/v1/escrow/release",
    "refund":   "https://api.yourco.com/icp/v1/escrow/refund",
    "dispute":  "https://api.yourco.com/icp/v1/escrow/dispute",
    "receipts": "https://api.yourco.com/icp/v1/settlements"
  },
  "rails": ["base"],
  "assets": ["USDC"],
  "policy": {
    "min_escrow_usd": "1.00",
    "max_escrow_usd": "100000.00",
    "supported_dispute_paths": ["arbiter-redirect", "merchant-refund"]
  }
}
```

The doc itself does **not** need to be signed (CORS-friendly,
clients fetch it directly); the keys it advertises sign every
EscrowEvent and SettlementReceipt downstream.

## S.2 — Implement the escrow lifecycle

You expose 5 endpoints (the URLs in the discovery document above). At
minimum each one must:

| Endpoint | Input | Output | Side effect |
|---|---|---|---|
| `POST fund` | `{ intent_id, payer_aid, amount, asset, currency, deadline }` | `{ escrow_id, funding_instructions }` | Open an escrow row in your ledger; return rail-specific funding instructions (USDC: a vault address + memo; ACH: a virtual account number) |
| `POST release` | `{ escrow_id, fulfillment_evidence }` | `{ settlement_receipt }` | Execute the rail transfer to the named recipient; return a signed SettlementReceipt |
| `POST refund` | `{ escrow_id, reason }` | `{ settlement_receipt }` | Execute the rail transfer back to the payer; return a signed refund SettlementReceipt |
| `POST dispute` | `{ escrow_id, reason, evidence }` | `{ disposition }` | Mark escrow disputed; halt auto-release |
| `WS observe` | n/a | Stream of signed `EscrowEvent` | Push every state transition for the escrow IDs the subscriber is authorized to see |

Every state transition is also reported through `observe`, **and** is
authoritative — the merchant handler may have its own cache, but in
case of disagreement the Settler's signed EscrowEvent wins.

The full state machine is in
[`ICP-1.0-DRAFT.md`](../ICP-1.0-DRAFT.md) §7.

## S.3 — Issue SettlementReceipts

A SettlementReceipt is a small signed document that says "I, the named
Settler, executed this transfer at this time on this rail and the rail
confirms it." Its canonical form is in
[`SETTLERS.md`](../SETTLERS.md) §S.3 and must be retrievable by
`escrow_id` for at least **7 years** (audit retention):

```json
{
  "type": "icp.settlement.receipt",
  "v": "icp-1.0",
  "settlement_id": "icp_set_01HXYZ...",
  "escrow_id": "icp_esc_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "final_state": "released",
  "amount": { "amount": "62.49", "currency": "USDC" },
  "rail": "base-l2",
  "rail_txid": "0xabc...",
  "rail_block_number": 18342919,
  "iat": "2026-05-09T17:55:18Z",
  "settler_signature": { "alg": "ed25519", "kid": "yourco-usdc-base-2026-q2", "sig": "..." }
}
```

Sign the canonicalized form (RFC 8785 JCS, per
[`schemas/canonicalization.md`](../schemas/canonicalization.md)) with
the Ed25519 key advertised at that `kid`. The first-party SDKs
(`@stateset/icp-client` → `verifySettlementReceipt`,
`icp_client.verify_settlement_receipt`,
`stateset_icp_client::verify_settlement_receipt`) all consume this
exact shape — keep it byte-identical or counterparties will reject.

## S.4 — Proof-of-reserves

Required if aggregate USD-equivalent escrow at rest exceeds **$1M** at
any point in a 24h window. Refresh at least every 24h. Acceptable
attestation methods:

| Method | Trust model |
|---|---|
| `chainlink-por` | On-chain PoR feed (e.g. Chainlink) |
| `merkle-attestation` | Merkle tree of escrow balances, root signed by you |
| `audited-financial-statement` | Quarterly statement from a Big-4-equivalent auditor |
| `regulator-attestation` | Statement from your prudential regulator |

Publish the attestation URL on your discovery document under
`proof_of_reserves`:

```json
{
  "proof_of_reserves": {
    "method": "merkle-attestation",
    "url": "https://api.yourco.com/icp/v1/por/latest",
    "refreshed_at": "2026-05-20T14:00:00Z"
  }
}
```

The conformance suite verifies your POR attestation arithmetically
covers all open EscrowEvents observable through `observe` — i.e. it
catches under-collateralization.

## S.5 — Operational SLAs

Allowlist eligibility commits you to:

| SLA | Threshold |
|---|---|
| **Funding confirmation** | within 4× rail-finality-time (≈ 2 min on Base L2, 1 business day on ACH) |
| **Release execution** | within 1× rail-finality-time after preconditions met |
| **Event observability** | Settler-detect-to-client-notify under 60s in steady state |
| **Uptime** | 99.9% for `observe` and `receipts`, rolling 90-day window |

The Foundation tracks these via the conformance suite + the public
allowlist registry. Falling below for >5 days/quarter flags you on the
public allowlist; persistent failure leads to removal vote.

## Run the conformance suite

The black-box settler conformance profile drives a synthetic Intent
through your endpoints and verifies every signature, state transition,
and receipt against the spec:

```sh
git clone https://github.com/stateset/stateset-icommerce
cd stateset-icommerce/icp-conformance
./runner/run.mjs --profile icp-1.0-settler --iut https://api.yourco.com
```

The runner exercises:

1. Discovery document fetch + key validation
2. Fund → release happy path (signed EscrowEvents at each transition)
3. Fund → refund path
4. Fund → dispute → resolve path
5. SettlementReceipt signature + canonicalization
6. Replay rejection (re-POSTing a release fails after first success)
7. Allowlist enforcement (Intent naming a different Settler is rejected)
8. POR coverage (where applicable)

Green = you can submit to the allowlist.

## Submit to the allowlist

Open a PR against [`stateset/stateset-icommerce`](https://github.com/stateset/stateset-icommerce)
adding:

1. `icp-spec/settlers/<your-urn>.md` — operator-specific binding (use
   [`settlers/usdc-base.md`](../settlers/usdc-base.md) as the template).
2. The conformance runner's PASS log as a CI artifact.
3. Co-attestations from two existing allowlist members (or StateSet for
   the bootstrap period).
4. Your LEI + jurisdiction + signed patent-grant addendum.

The Foundation reviews and merges. Inclusion in
`settlers.icp.dev/allowlist.json` follows the next allowlist signing
round (currently weekly).

## Production checklist

- [ ] Signing key in an HSM (never on disk in plaintext).
- [ ] Discovery document served over TLS, CORS-enabled for
      `https://*.icp.dev`.
- [ ] All 5 endpoints idempotent — re-POSTing the same call yields the
      same response, never double-executes.
- [ ] EscrowEvent stream is **append-only** and gap-free; replay log is
      durable across restarts.
- [ ] SettlementReceipt issuance is transactional with the rail
      execution — receipt and tx_hash exist together or not at all.
- [ ] POR attestation refresh on a cron, not on first POR query.
- [ ] Allowlist of merchant AIDs that can call `fund` — anonymous
      `fund` is a denial-of-service vector.
- [ ] Rate limits per merchant AID and per source IP.
- [ ] Funding-instruction expiry — funding addresses/memos that go
      unused for >Intent deadline are recycled.
- [ ] Observability: OpenTelemetry traces on every state transition,
      tagged with `escrow_id`, `intent_id`, `settler_id`.

## Where to look next

- Reference binding — [`settlers/usdc-base.md`](../settlers/usdc-base.md).
- Full settler spec — [`SETTLERS.md`](../SETTLERS.md).
- On-chain reference contract —
  [`contracts/usdc-base/`](../contracts/usdc-base/) (Foundry, 15/15 tests).
- Event envelope + observe stream — [`PACKET.md`](../PACKET.md).
- Foundation governance — [`governance/`](../governance/).
