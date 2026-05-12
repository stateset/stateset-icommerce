# ICPIP-0004: `payout.request` Verb Specification (Marketplace Payouts)

```
ICPIP:        0004
Title:        payout.request Verb Specification (Marketplace Payouts)
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/4 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-05-12
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Adds `payout.request` as the **seventh and final** ICP intent verb: a
seller Agent's signed request to receive a payout from a platform-held
balance. Unlike the other six ICP verbs, payout.request inverts the
principal-direction: the **recipient** (seller) is the signer, not the
**originator** (buyer). The platform signs a **PayoutAuthorization**
specifying the amount, the rail, and the release timing.

This is the **two-sided marketplace primitive** — Etsy, Stripe Connect,
Uber, Doordash, Shopify Marketplace, Amazon DSP, App Store, eBay, etc.
Without it, ICP-1.x cannot serve the platforms that hold seller funds
between buyer payment and seller payout. Adding it brings the protocol
to 100% commerce verb coverage.

## Motivation

### What marketplace settlement actually looks like

A modern two-sided marketplace operates as a **temporary fund custodian**:

1. Buyer pays the platform (purchase.create → escrow → settlement to platform).
2. Platform holds the funds for some period (often 1–7 days) to cover
   chargebacks, returns, fraud review, and compliance holds.
3. Platform deducts fees (platform commission, payment processor fees,
   compliance reserves).
4. Platform initiates payout to the seller's wallet or bank account.

Steps 1–3 are mostly covered by the existing 5 ICP verbs:
- `purchase.create` for the original transaction
- `purchase.return` for the refund path
- `settlement` already produces a SettlementReceipt to the platform's
  custody wallet

But step 4 — the **seller-side payout** — has no primitive. Sellers
today receive payouts via:
- Stripe Connect's proprietary API
- PayPal's payout endpoint
- ACH file uploads
- Crypto wallet transfers initiated by the platform

None of these produce a signed, audit-grade record on the SELLER'S
side. The platform records the disbursement; the seller has only the
rail-level transaction proof (e.g. a bank statement entry).

`payout.request` fixes this. The seller signs a request, the platform
signs an authorization, and the Settler executes the rail transfer.
Both parties have a cryptographic record of the payout, indexed by
`payout_id`, joinable to the original buyer transactions, and
independently auditable.

### Why this matters

Global marketplace GMV is conservatively $500B/year (US-only; ~$2T
globally). Etsy alone processed ~$13B in 2024. Stripe Connect processed
~$700B in 2024. Without `payout.request`, ICP serves only the
buyer-merchant slice of commerce. With it, ICP covers the seller-platform
slice too — the much larger and more compliance-sensitive half.

For agentic commerce specifically, autonomous seller agents are a real
near-term use case:
- Creator-economy agents managing print-on-demand stores
- AI-generated content publishers receiving micropayouts from
  ad-attribution platforms
- DAO treasury agents disbursing pro-rata revenue to contributors

All of these need an audit-grade payout record on the seller side. None
currently have one.

## Specification

### Wire format

```json
{
  "verb": "payout.request",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "seller": "aid:v1:zS...",                                              // NB: NOT `buyer`
  "platform": "aid:v1:zP...",                                            // NB: NOT `merchant`
  "settler": "settler:circle.usdc.base",
  "amount": { "amount": "1247.83", "currency": "USDC" },                 // amount requested
  "balance_window": {                                                     // OPTIONAL — period covered
    "from": "2026-05-01T00:00:00Z",
    "to": "2026-05-12T00:00:00Z"
  },
  "destination": {                                                        // where the payout should go
    "type": "wallet" | "bank-account",
    "wallet_address": "0x...",                                            // present when type=wallet
    "bank_routing": { ... }                                               // present when type=bank-account
  },
  "expedited": false,                                                     // OPTIONAL — request faster rail (typically a fee)
  "context": "weekly-payout-cycle-2026-W19",                              // OPTIONAL
  "principal_binding": <signed-binding>,                                  // MUST grant payout.request
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The platform signs a **PayoutAuthorization** in response:

```json
{
  "type": "payout.authorization",
  "v": "icp-1.0",
  "payout_id": "icp_pay_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "seller": "aid:v1:zS...",
  "platform": "aid:v1:zP...",
  "available_balance": { "amount": "1247.83", "currency": "USDC" },      // what the platform actually has held for this seller
  "approved_amount": { "amount": "1198.32", "currency": "USDC" },        // what the platform will release (after fees/reserves)
  "fees": [
    {
      "type": "platform_commission",
      "amount": { "amount": "37.43", "currency": "USDC" },
      "description": "Standard 3% platform commission"
    },
    {
      "type": "compliance_reserve",
      "amount": { "amount": "12.48", "currency": "USDC" },
      "description": "1% chargeback reserve (released after 90 days)",
      "release_at": "2026-08-10T00:00:00Z"                                // if reserve, when it becomes payout-eligible
    }
  ],
  "rail": "base-sepolia",
  "rail_initiated_at": "2026-05-12T18:00:00Z",                            // when the transfer hit the rail
  "expected_settlement_at": "2026-05-12T18:00:30Z",                       // when seller should see funds
  "source_transactions": [                                                // OPTIONAL — audit trail
    "icp_set_01HXYZ...AAA",
    "icp_set_01HXYZ...BBB"
  ],
  "issued_at": "2026-05-12T18:00:00Z",
  "signature": { "alg": "ed25519", "kid": "<platform-aid>", "sig": "..." }
}
```

### PrincipalBinding extension

The PrincipalBinding (ICP-1.0 §4.3) extends to support payout authority:

```json
{
  "principal": "did:web:seller-corp.example",
  "agent": "aid:v1:zS...",
  "authority": {
    "verbs": ["purchase.create", "payout.request"],
    "max_per_intent": { "amount": "10000", "currency": "USDC" },
    "max_per_payout": { "amount": "50000", "currency": "USDC" },         // NEW — payout-specific cap
    "max_per_period": {
      "amount": "100000",
      "currency": "USDC",
      "period": "7d"
    },
    "allowed_platforms": ["aid:v1:zP1...", "aid:v1:zP2..."]               // OPTIONAL — restrict which platforms can be paid out from
  },
  ...
}
```

`max_per_payout` is the per-call cap. `max_per_period` (already in
ICP-1.0) doubles as the rolling-window cap for payouts. A platform
issuing a PayoutAuthorization above either cap is non-conformant.

### Fee model

The PayoutAuthorization's `fees` array is **informational and binding**:
the platform commits that the listed fees are the only deductions
applied. A platform that withholds additional unlisted amounts is
non-conformant. Sellers may verify by comparing
`available_balance − sum(fees) == approved_amount`.

Each fee MUST specify:
- `type` — one of: `platform_commission`, `payment_processor`,
  `compliance_reserve`, `tax_withholding`, `chargeback_reserve`,
  `currency_conversion`, `expedited_payout`, `other`
- `amount` — the deducted Money
- `description` — human-readable explanation

Reserves (`compliance_reserve`, `chargeback_reserve`) MUST include
`release_at` indicating when the held amount becomes payout-eligible.

### Linking to source transactions

The `source_transactions` array is OPTIONAL but RECOMMENDED. It lists
the `settlement_id`s of the buyer transactions that contributed to the
seller's balance during `balance_window`. This makes payouts **fully
auditable**: a regulator or accountant can trace every dollar in the
payout back to its originating buyer transaction.

For platforms with very high transaction volume (e.g. Stripe Connect's
millions of daily transactions per seller), enumerating every
contributing settlement_id is impractical. In that case, the platform
MAY include a `source_transactions_merkle_root` instead — a Merkle root
over the contributing settlement_ids, with the inclusion proofs
available via the platform's API for verification.

### Eligibility policy

Common rejection reasons (new error codes added to `error-codes.md`):

| Code | When emitted |
|---|---|
| `policy.payout.insufficient_balance` | Requested amount exceeds available balance |
| `policy.payout.hold_period_active` | Funds still within compliance hold window |
| `policy.payout.exceeds_max_per_payout` | Request exceeds PrincipalBinding's `max_per_payout` |
| `policy.payout.exceeds_max_per_period` | Cumulative period exceeds `max_per_period` |
| `policy.payout.kyc_required` | Seller hasn't completed KYC; required for amounts above threshold |
| `policy.payout.destination_not_allowlisted` | Destination wallet/account not pre-registered |
| `policy.payout.rail_unavailable` | Named Settler doesn't support the requested rail |
| `policy.payout.expedited_unavailable` | Expedited payout not offered for this seller/amount |
| `policy.payout.compliance_hold` | Account under SAR/fraud review; payouts paused |
| `policy.payout.platform_not_allowed` | Platform AID not in `allowed_platforms` |

### Schema updates

New JSON Schema: `schemas/intent.payout.request.schema.json`.

Modified schema: `schemas/intent.purchase.create.schema.json` (and other
verb schemas) — the PrincipalBinding `authority` object gains OPTIONAL
`max_per_payout` and `allowed_platforms` fields. ICP-1.0 implementations
ignore unknown authority fields, so this is backward-compatible.

### Capabilities advertisement

Platforms supporting payouts add `payout.request` to their
`.well-known/icp/capabilities.verbs`. Sellers SHOULD only sign payout
requests against platforms that advertise support; otherwise the
platform may return `format.unknown_verb`.

Additionally, platforms SHOULD advertise their fee schedule and
typical hold periods at a NEW well-known sub-endpoint
`.well-known/icp/payout-policy` (specification of that endpoint is
left to a future ICPIP).

## Rationale

### Why a separate verb (not piggyback on settlement)

The existing SettlementReceipt represents a buyer→merchant flow
(forward direction). A payout is a platform→seller flow with very
different semantics:

- **Different principal**: the seller is requesting, not the buyer
- **No counterparty intent**: there's no buyer Intent on the seller side
- **Different timing**: payouts happen on platform-controlled cadence, not transaction-by-transaction
- **Different reservation**: payouts deduct from a pool, not from a specific buyer's escrow
- **Different compliance surface**: payouts trigger tax withholding, KYC, sanctions screening at different levels than buyer→merchant transactions

Conflating them would require so many conditional fields on SettlementReceipt
that the spec would become unreadable. A dedicated verb is cleaner and
keeps each verb's surface focused.

### Why the seller signs (not the platform requesting on their behalf)

Rejected: making the platform auto-initiate payouts. Reasons:

1. **Authorization audit trail**: the seller MUST be able to prove they
   authorized the payout. Platform-initiated payouts produce no
   seller-side signature, leaving the seller with only the platform's
   word that they "agreed."
2. **Programmable cadence**: an agentic seller can program their
   own payout cadence — "request payouts every Sunday at 10am if
   balance > $100." Platform-initiated forces the platform's cadence.
3. **Authority binding**: the PrincipalBinding mechanism naturally
   gates payout authority. Without seller signing, the binding doesn't
   apply to payouts.
4. **Symmetry with other verbs**: all other ICP verbs are
   buyer/originator-signed. Inverting one verb's signing direction is a
   one-time conceptual cost; doing it differently for payouts would
   inconsistent the protocol.

### Why fees are itemized rather than aggregated

A single "fees: $49.51" field would be simpler but inadequate:

1. **Tax compliance**: most tax authorities require fee breakouts for
   1099 / VAT / GST reporting. A single aggregate hides these.
2. **Dispute resolution**: when a seller queries why a payout was
   lower than expected, "the platform took $49.51" is uninformative.
   Itemized fees enable specific dispute.
3. **Regulatory trends**: EU PSD3 (effective 2026), CA SB-478, and
   similar require explicit fee disclosure to recipients. Itemized
   fees comply by default.

### Why optional `source_transactions`

Some platforms can enumerate; others cannot at scale. The OPTIONAL
field accommodates both. The Merkle-root fallback is a future
extension if needed (ICPIP-0006?), modeled on the audit-trail approach
in zkSNARK-anchored systems.

### Comparison to existing payout protocols

| Protocol | Seller-side signature? | Audit trail | Standardized fees |
|---|---|---|---|
| Stripe Connect | ❌ no | platform-side only | yes (per-account) |
| PayPal Payouts | ❌ no | platform-side only | yes (per-account) |
| ACH NACHA file | ❌ no | originator-side only | no (bank-specific) |
| **ICP `payout.request`** | **✅ yes** | **both sides** | **yes (per-Authorization)** |

ICP would be the first commerce protocol with seller-side payout
authorization records. This is a strict superset of existing capabilities.

### Comparison to crypto-native payouts

| Protocol | Authorization model | Audit |
|---|---|---|
| Direct wallet transfer | none (just a tx) | tx hash only |
| ERC-4337 user op | smart-contract wallet sig | tx hash + intent hash |
| EIP-712 typed data | offchain signed message | message + ondhain tx |
| **ICP `payout.request`** | **two-party signed authorization** | **payout_id + ondhain rail tx** |

The closest analog is EIP-712 typed-data signing for Permit2-style
flows. ICP's payout.request adds platform countersigning and structured
fee disclosure — capabilities that EIP-712 doesn't provide.

## Backwards Compatibility

This is an additive verb. ICP-1.0 platforms that don't support it
return `format.unknown_verb` and sellers fall back to platform-specific
payout APIs. The optional `max_per_payout` and `allowed_platforms`
PrincipalBinding fields are ignored by ICP-1.0 verifiers (per the
existing rule: verifiers MUST ignore unknown fields in `authority`).

Migration is opt-in per platform. There's no protocol mandate; market
pressure determines adoption pace.

## Security Considerations

### Replay protection

A replayed `payout.request` could trigger a duplicate payout. Standard
ICP nonce + iat/exp protection applies. Additionally, the platform
SHOULD track issued PayoutAuthorizations by `intent_id` and reject any
duplicate intent_id (returning the existing PayoutAuthorization
idempotently, similar to subscription.cancel idempotency).

### Destination authorization

A compromised seller Agent key could redirect payouts to an attacker's
wallet. Mitigations:

1. **Pre-registered destinations**: the platform MAY require sellers
   to pre-register destination wallets/accounts. The
   `policy.payout.destination_not_allowlisted` error code surfaces
   when a payout requests an unregistered destination.
2. **Cooling-off period on new destinations**: platforms SHOULD apply
   a 48-hour delay before paying out to a newly-registered destination.
3. **Multi-party authorization for high-value payouts**: the
   `max_per_payout` cap in PrincipalBinding is the protocol-level
   mitigation. Platforms can additionally require human approval above
   a threshold.

### Fee manipulation

A malicious platform could inflate fees to skim seller funds. The
`fees` array's binding nature (combined with the
`available_balance − sum(fees) == approved_amount` invariant) makes
this **observable** — sellers can audit. Repeated fee manipulation
would be flagged on the Foundation's conformance dashboard and lead
to certification revocation.

### Compliance surface

Payouts often trigger regulatory obligations the platform must
satisfy:
- **AML/KYC**: large payouts (>$10k cumulative annual) require
  verified KYC on the seller. The `policy.payout.kyc_required` code
  surfaces this.
- **Tax withholding**: US 1099 filing, EU VAT, Australia ATO — many
  require platforms to withhold and report. Encoded as a fee type
  `tax_withholding`.
- **Sanctions**: payouts to OFAC-sanctioned addresses are illegal.
  Platforms MUST run OFAC screening before issuing a PayoutAuthorization.
  Failures surface as `policy.payout.compliance_hold`.

These are platform obligations, not protocol obligations. The protocol
provides the cryptographic record so the platform can prove compliance;
the protocol doesn't enforce compliance itself.

### Reserve handling

Compliance reserves (chargeback or otherwise) MUST be released
predictably. The `release_at` field commits the platform to a release
date. A platform that withholds reserves past `release_at` is
non-conformant. The seller can submit a follow-up
`payout.request` for the reserve amount on or after `release_at`; the
platform's PayoutAuthorization for the reserve cycle references the
original payout's `source_transactions` (or the reserve_id chain).

## Test Vectors

Conformance vector `07-payout-roundtrip` (to be added to
`icp-conformance/vectors/icp-1.0/` for IUTs that advertise
`payout.request` support):

```
inputs.json:
{
  "test": "07-payout-roundtrip",
  "seller_agent": { ... deterministic seeds ... },
  "platform_balance": { "amount": "1247.83", "currency": "USDC" },
  "platform_fee_schedule": {
    "commission_percent": "3.0",
    "reserve_percent": "1.0",
    "reserve_release_days": 90
  },
  "intent": {
    "verb": "payout.request",
    "amount": { "amount": "1247.83", "currency": "USDC" },
    "destination": { "type": "wallet", "wallet_address": "0xABC..." },
    ...
  }
}

expected.json:
{
  "intent_canonical_string": "...",
  "intent_signature_hex": "...",
  "approved_amount": "1198.32",         // 1247.83 - 3% - 1%
  "fees_total": "49.51",
  "fees_count": 2,
  "release_at_for_reserve": "<iat + 90d>"
}
```

Negative cases:
- `payout.request` for amount > balance → `policy.payout.insufficient_balance`
- `payout.request` for amount > `max_per_payout` → `policy.payout.exceeds_max_per_payout`
- `payout.request` to non-allowlisted destination → `policy.payout.destination_not_allowlisted`

## Reference Implementation

Estimated effort (after this ICPIP reaches Last Call):

- `icp-handler/src/backend-stub.mjs`: `stubPayoutRequest()` (~120 LOC)
  including fee computation + reserve scheduling
- `icp-handler/src/server.mjs`: route payout.request
- `icp-mcp/src/backend.mjs`: route the verb
- `packages/icp-client/src/index.mjs`: `.payout()` method (~40 LOC)
- 3 handler tests (happy path + insufficient balance + KYC required)
- Conformance vector + IUT updates (Rust, JS, Go, Python all extended
  to handle the PrincipalBinding authority changes)

Following ICPIP-0001: ≥2 independent IUTs must pass the conformance
vector before Final promotion.

## References

- ICP-1.0-DRAFT §6.6 (payout.request stub — superseded by this ICPIP)
- ICPIP-0001 (Process)
- ICPIP-0002 (Hybrid PQC) — applies to payouts at threshold ($10k)
- ICPIP-0003 (quote.request) — companion verb specification
- Stripe Connect documentation (informational comparison)
- EIP-712 (informational comparison for typed-data authorization)
- US 1099-K, EU PSD3, CA SB-478 (regulatory context)

## Copyright

This ICPIP is licensed under CC-BY-4.0.
