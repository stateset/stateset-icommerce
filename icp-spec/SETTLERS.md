# ICP Settlers — Interface Specification

The **Settler** is the single point of value capture in ICP. Every Intent
names exactly one Settler. The Settler holds escrowed value between
acceptance and final release, signs every state transition, and produces
the canonical SettlementReceipt that downstream tax/accounting/audit
systems treat as authoritative.

This document specifies what a Settler MUST provide to be eligible for the
governance-published allowlist (`settlers.icp.dev/allowlist.json`,
forthcoming), and how new rails (USDC, Stripe Treasury, ACH, Bitcoin
Lightning, etc.) integrate with ICP.

## Status

ICP-1.0 normative. Read alongside `ICP-1.0-DRAFT.md` §8–§9, §11.1–§11.2.

## Roles

A Settler is an entity (legal person, smart contract, regulated
custodian, or some combination) that:

1. **Custodies** value posted to an Escrow between `funded` and one of
   `released`/`refunded`/`disputed→{released,refunded}`.
2. **Witnesses** state transitions and emits signed EscrowEvents.
3. **Executes** the rail-native value transfer at release/refund time.
4. **Issues** a SettlementReceipt as definitive proof of the transfer.

A Settler is identified by a hierarchical `settler:` URN (ICP-1.0 §6.1):

```
settler:<rail-family>.<asset>.<network>[.<custodian>]

settler:circle.usdc.base
settler:circle.usdc.ethereum
settler:stripe.treasury.ach
settler:stripe.treasury.wire.usd
settler:lightning.btc.mainnet
settler:fedwire.usd
settler:set.set.mainnet
```

The hierarchy is significant: counterparties may allowlist by prefix
(`settler:circle.*`) to opt into all Circle-operated Settlers without
naming each network individually.

## Required capabilities

Every Settler MUST implement, at minimum, the following operations.
Conformance is determined by the `icp-conformance-settler` test profile
(forthcoming).

### S.1 Identity

The Settler MUST publish:

- An **Ed25519 signing key** that signs all EscrowEvents and SettlementReceipts.
- An **OPTIONAL ML-DSA-65 hybrid key** (RECOMMENDED for any Settler holding
  >$10M aggregate escrow at rest).
- A **discovery document** at `https://<settler-domain>/.well-known/icp-settler`:

  ```json
  {
    "settler_id": "settler:circle.usdc.base",
    "operator": {
      "name": "Circle Internet Financial",
      "lei": "549300LFXJU8M0X8XV23",
      "jurisdiction": "US"
    },
    "signing_keys": [
      { "alg": "ed25519", "kid": "circle-usdc-base-2026-q2", "pub": "z..." }
    ],
    "endpoints": {
      "fund":     "https://api.circle.com/icp/v1/escrow/fund",
      "observe":  "wss://api.circle.com/icp/v1/escrow/events",
      "release":  "https://api.circle.com/icp/v1/escrow/release",
      "refund":   "https://api.circle.com/icp/v1/escrow/refund",
      "dispute":  "https://api.circle.com/icp/v1/escrow/dispute",
      "receipts": "https://api.circle.com/icp/v1/settlements"
    },
    "limits": {
      "min_intent": { "amount": "0.01", "currency": "USDC" },
      "max_intent": { "amount": "1000000.00", "currency": "USDC" },
      "max_pending_per_aid": { "amount": "10000000.00", "currency": "USDC" }
    },
    "finality": {
      "rail": "base-l2",
      "blocks_to_finality": 18,
      "expected_seconds_to_finality": 30
    },
    "proof_of_reserves": {
      "method": "chainlink-por",
      "endpoint": "https://api.circle.com/por/usdc-base/latest"
    },
    "version": "icp-1.0"
  }
  ```

### S.2 Escrow lifecycle

The Settler MUST honor the state machine in ICP-1.0 §8 and emit signed
EscrowEvents at every transition. Events are append-only with monotonic
`seq`. Replay of events from `seq=0` MUST reconstruct the exact current
state.

```json
{
  "type": "icp.escrow.event",
  "v": "icp-1.0",
  "escrow_id": "icp_esc_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "seq": 3,
  "from_state": "funded",
  "to_state": "fulfilled",
  "trigger": {
    "kind": "fulfillment-evidence-accepted",
    "evidence_id": "icp_ful_01HXYZ..."
  },
  "rail_event": {
    "rail": "base-l2",
    "block_number": 18342901,
    "tx_hash": "0xabc..."
  },
  "iat": "2026-05-09T17:55:12Z",
  "settler_signature": { "alg": "ed25519", "kid": "...", "sig": "..." }
}
```

Counterparties subscribe to events via the discovery-document `observe`
endpoint (WebSocket or SSE). The Settler MUST also accept retrieval by
`escrow_id` for cold replay.

### S.3 Settlement receipt

At terminal state (`released` or `refunded`), the Settler MUST produce a
SettlementReceipt and make it retrievable by `escrow_id` for at least
**7 years** (audit retention).

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
  "rail_finalized_at": "2026-05-09T17:55:42Z",
  "released_to": "0x742d35Cc...",
  "settler_signature": { "alg": "ed25519", "kid": "...", "sig": "..." },
  "merchant_signature": { "alg": "ed25519", "kid": "...", "sig": "..." }
}
```

The receipt is **co-signed** by the Settler and the receiving party. A
receipt with only one signature is INVALID and MUST NOT be treated as
proof of settlement.

### S.4 Proof-of-reserves

Any Settler holding >$1M aggregate USD-equivalent escrow at rest MUST
publish a proof-of-reserves attestation refreshed at least every 24 hours,
linked from the discovery document. Acceptable methods (ICP-1.0 normative):

| Method | Trust model |
|---|---|
| `chainlink-por` | Chainlink Proof of Reserves on-chain feed |
| `merkle-attestation` | Merkle tree of escrow balances, root signed by Settler |
| `audited-financial-statement` | Quarterly statement from a Big-4-equivalent auditor |
| `regulator-attestation` | Statement from Settler's prudential regulator |

The conformance suite verifies that a Settler's POR attestation
arithmetically covers its open EscrowEvents.

### S.5 Operational SLAs

To be eligible for the allowlist, a Settler MUST commit to:

- **Funding confirmation** within 4× rail-finality-time (e.g. ~2 minutes
  on Base L2; 1 business day on ACH).
- **Release execution** within 1× rail-finality-time after preconditions
  met.
- **Event observability** with end-to-end latency (Settler-detect-to-
  client-notify) under 60s in steady state.
- **99.9% uptime** for the `observe` and `receipts` endpoints,
  measured over rolling 90-day windows.

Settlers that fall below SLA for >5 days/quarter are flagged on the
public allowlist and may be removed by Foundation vote.

## Trust hardening

ICP gives counterparties three protocol-level ways to limit Settler
trust without removing the Settler from the protocol:

1. **Settler exposure cap.** A counterparty's policy may cap aggregate
   escrow exposure to any single Settler. The reference impl supports
   this via `policy.settler.max_concurrent_escrow_per_settler`.
2. **Settler diversity requirement.** A high-value Intent MAY name
   multiple Settlers across rails (`settler` becomes an array; payment
   is split). ICP-1.1 will formalize this; ICP-1.0 supports it via two
   parallel Intents.
3. **POR-gating.** A counterparty's policy MAY require an attestation
   timestamp ≤24h old before accepting any Intent naming a Settler.

## Allowlist governance

Until the ICP Foundation is incorporated, StateSet maintains the
canonical allowlist at `https://settlers.icp.dev/allowlist.json` (signed,
append-only, with a public Merkle log). Inclusion criteria:

1. All S.1–S.5 capabilities implemented and verified by a fresh
   conformance run.
2. Settler operator is a legal entity with a published LEI.
3. Settler operator agrees to ICP IP policy (royalty-free patent grant
   on operations covered by the spec).
4. Two existing allowlist members co-attest. (For the bootstrap period,
   StateSet attestation suffices.)

Removal triggers (any one):

- Conformance regression for >30 days.
- Settler operator becomes subject to a final regulatory action that
  prohibits custody operations.
- Two Foundation members vote for removal.

## Reference Settler bindings

| Settler | File | Status |
|---|---|---|
| `settler:circle.usdc.base` | `settlers/usdc-base.md` | Reference (this release) |
| `settler:stripe.treasury.ach` | `settlers/stripe-treasury-ach.md` | Drafting |
| `settler:lightning.btc.mainnet` | `settlers/lightning-btc.md` | Solicited (community) |
| `settler:set.set.mainnet` | `settlers/set-mainnet.md` | StateSet-operated |

## Open questions for ICP-1.1

- **Atomic cross-Settler swap.** HTLC-style atomic settlement across two
  Settlers (e.g. USDC out, ACH in). Requires a hash-lock primitive in the
  Escrow state machine.
- **Settler-of-last-resort.** A protocol-defined escrow custodian that
  triggers when the named Settler is unreachable for >7 days. Probably
  needs a cross-jurisdictional foundation vehicle.
- **Subsidized fees.** Should ICP define a fee-rebate primitive so a
  Settler can charge a small bps fee for protocol upkeep? Currently
  silent; community feedback wanted.
