# Reference Settler: `settler:circle.usdc.base`

USDC on Base L2, custodied via Circle's smart-contract escrow. This is the
**first reference Settler** for ICP-1.0 because it is the lowest-friction
path to first-real-dollar-settled:

- Circle is a US-regulated MTL holder; legal exposure is well-understood.
- Base L2 is sub-cent fees and ~2-second blocktime, suitable for
  micro-intents up to seven-figure intents.
- Agent operators already hold Coinbase wallet stack that interoperates.
- USDC has a 1:1 fiat redemption story for off-ramp.

This document is normative for the `settler:circle.usdc.base` SettlerID.
It is a binding of the abstract Settler interface (`SETTLERS.md`) to a
specific rail. Other Circle-operated Settlers (`circle.usdc.ethereum`,
`circle.usdc.solana`, `circle.eurc.base`, etc.) follow the same pattern
with different chain anchors.

## On-chain anchor

The Settler is implemented as an upgradeable proxy contract on Base L2:

```
ICPEscrow proxy:        0x_______________ (set at allowlist publication)
ICPEscrow implementation: 0x_______________
USDC token:             0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
Settler signing key:    aid:v1:zCircleBaseUSDCQ22026
```

The proxy is owned by a 5-of-9 Circle-controlled Safe with a 48-hour
timelock on implementation upgrades, giving counterparties a fixed window
to opt out before any logic change takes effect.

## Lifecycle binding

| ICP state | Trigger on Base L2 | EscrowEvent emitted |
|---|---|---|
| `pending → funded` | `ICPEscrow.fund(escrowId, ...)` succeeds and is finalized (18 blocks) | `funded` event with `tx_hash` and `block_number` |
| `funded → fulfilled` | Merchant signs FulfillmentEvidence; Settler verifies and emits | `fulfilled` event (no on-chain tx; off-chain attestation) |
| `fulfilled → released` | Dispute window elapses; `ICPEscrow.release(escrowId, ...)` succeeds and is finalized | `released` event AND `SettlementReceipt` issued |
| `funded → disputed` | Either party calls `ICPEscrow.dispute(escrowId, ...)` | `disputed` event |
| `disputed → released` or `disputed → refunded` | Arbiter resolution + corresponding on-chain call | terminal event + `SettlementReceipt` |
| `funded → refunded` | Merchant cancellation OR fulfillment expiry, then `ICPEscrow.refund(escrowId, ...)` | `refunded` event + `SettlementReceipt` |

All state transitions that move USDC are on-chain transactions; all
state transitions that don't (`fulfilled` is the only one) are
off-chain Settler attestations signed by the Settler key.

## Funding

A buyer Agent (or a delegated funding service) calls:

```solidity
function fund(
    bytes32 escrowId,         // == keccak256(intent_id || quote_id)
    uint256 amount,           // exact match to Quote.total
    address merchantPayoutAddress,
    bytes32 fulfillmentDeadline,  // packed: deadline_unix || dispute_window_seconds
    bytes calldata icpQuoteSignatureBundle  // canonical CBOR of the accepted Quote
) external;
```

The contract verifies:
- `amount` equals the buyer-Agent-signed Quote.total
- Quote signature is valid against `merchant` AID's published key
- Quote is unexpired
- `escrowId` is not already in use

On success, USDC is transferred from `msg.sender` into the contract;
escrow state becomes `pending` until 18 confirmations, then `funded`.

## Release

After fulfillment AND dispute window elapsed:

```solidity
function release(
    bytes32 escrowId,
    bytes calldata icpFulfillmentReceiptCbor,
    bytes calldata merchantReleaseSignature
) external;
```

Contract verifies the merchant's signature over (escrowId || `release`),
then transfers the held USDC to the merchant's payout address. The
Settler off-chain service observes the on-chain event and emits the
co-signed SettlementReceipt.

## Dispute and arbitration

ICP-1.0 doesn't standardize the arbiter; this Settler binding uses a
simple two-tier arbitration:

1. **Tier 1 — Merchant override window** (24h): merchant can offer a
   refund; if buyer accepts, escrow auto-refunds.
2. **Tier 2 — Kleros-style arbitration**: if Tier 1 fails, the dispute
   is escalated to the configured arbitration contract
   (`0x_______________`, governed independently by Foundation).

Arbiter outcomes are on-chain calls to `release(...)` or `refund(...)`
with an additional `arbiterSignatureBundle` argument that the contract
verifies against the Foundation-published arbiter key set.

## Proof-of-reserves

`https://api.circle.com/icp/v1/proof-of-reserves/usdc-base/latest`

returns:

```json
{
  "as_of_block": 18342000,
  "as_of_unix": 1746820000,
  "open_escrow_count": 4128,
  "open_escrow_total": "1247389.42",
  "currency": "USDC",
  "contract_balance_attested": "1248501.10",
  "delta_buffer": "1111.68",
  "merkle_root": "0xabc...",
  "merkle_root_signature": "0xdef..."
}
```

`contract_balance_attested >= open_escrow_total` MUST hold at all times;
if it doesn't, the Settler is in violation and the allowlist entry is
auto-flagged within one POR cycle.

## Limits and fees

| Parameter | Value | Rationale |
|---|---|---|
| Min intent | $0.01 USDC | Demonstrates micro-intent path, useful for x402 metering |
| Max single intent | $1M USDC | Hard contract cap; raised case-by-case via Foundation vote |
| Max concurrent per AID | $10M USDC | Soft cap; Settler may decline new Intents above this |
| Settler fee | 0 bps for ICP-1.0 reference period | Subsidy in exchange for first-mover position; revisited at ICP-1.1 |
| Gas paid by | Buyer (fund), Merchant (release) | Standard pattern; sponsored gas via Coinbase Paymaster supported |

## Failure modes

| Failure | Behavior |
|---|---|
| Base L2 chain halt | Settler pauses new funding; in-flight escrows resume on chain restart; SettlementReceipts are emitted with `rail_finalized_at` reflecting the actual finalization, not the expected |
| USDC contract pause (Circle compliance action) | Settler emits `disputed` events for all funded escrows pending Circle resolution; receipts NOT issued until resolved |
| Settler signing key compromise (suspected) | Foundation rotates allowlist entry to revoked; Settler MUST publish a new key in discovery doc; in-flight escrows continue to verify against old key with timestamp gate |
| Counterparty wallet compromise | Out of scope — buyer/merchant key hygiene is their responsibility; ICP cannot recover |

## Production checklist (Settler operator)

- [ ] Deploy ICPEscrow proxy + implementation, verified on Basescan
- [ ] 5-of-9 Safe with 48h timelock owns proxy admin
- [ ] Settler signing key generated in HSM; never exported
- [ ] Discovery document published at `.well-known/icp-settler` with TLS
- [ ] Proof-of-reserves endpoint live, refresh interval ≤24h
- [ ] WebSocket `observe` endpoint load-tested to 1k concurrent subscribers
- [ ] On-call rotation with ≤15-minute response SLA
- [ ] Two Foundation co-attestations for allowlist inclusion

## Bootstrapping (StateSet-operated until Circle adopts)

Until Circle implements this binding natively, StateSet operates a
**testnet-only** Settler at `settler:stateset.usdc.base-sepolia` for
demos, conformance testing, and the first wave of design partners. It
holds testnet USDC only; SettlementReceipts are clearly marked
`network: base-sepolia` and tax/accounting systems MUST reject them.

The mainnet binding (`settler:circle.usdc.base`) goes live when Circle
signs the IP policy and operates the `.well-known/icp-settler`
endpoint. Until then, the field is reserved.
