# ICP-1.0 — Intelligent Commerce Protocol

**Status:** Draft
**Editors:** StateSet, Inc. (interim steward)
**Date:** 2026-05-09

## 1. Abstract

ICP defines a wire format, cryptographic identity model, state machine, and
error vocabulary for autonomous agents to conduct commerce transactions
across organizational boundaries. It composes with payment-rail protocols
(x402, AP2) and checkout protocols (ACP) and supplies the missing layer:
the operational lifecycle of an order, including negotiation, escrow,
fulfillment, settlement, and dispute.

## 2. Conformance

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in
this document are to be interpreted as described in BCP 14 (RFC 2119,
RFC 8174) when, and only when, they appear in all capitals.

An implementation is **ICP-1.0 conformant** if and only if it passes 100% of
the `icp-conformance` suite for spec version `icp-1.0` — every vector in
`icp-conformance/vectors/icp-1.0/`, executed via the `icp-conformance`
runner (`node runner/run.mjs --profile icp-1.0-core`).

## 3. Architecture

ICP defines four roles:

- **Principal** — a legal entity (human, business, DAO) that authorizes one
  or more Agents to act on its behalf within bounded authority.
- **Agent** — a process holding signing keys, capable of producing and
  verifying ICP messages. Agents act on behalf of exactly one Principal.
- **Counterparty** — an Agent on the other side of an Intent (e.g. merchant
  Agent to buyer Agent).
- **Settler** — a third party (custodian, stablecoin issuer, payment
  processor) that holds escrowed value and releases it on protocol-defined
  triggers. The Settler is named in every Intent and is part of the
  signature payload.

A complete ICP transaction proceeds through a deterministic state machine:

```
INTENT_PROPOSED ─▶ QUOTED ─▶ ACCEPTED ─▶ ESCROWED ─▶ FULFILLED ─▶ SETTLED
                                  │             │             │
                                  ▼             ▼             ▼
                              REJECTED      DISPUTED      DISPUTED
                                                              │
                                                              ▼
                                                      RESOLVED / REFUNDED
```

State transitions are described normatively in §8.

## 4. Cryptographic identity

### 4.1 Key material

Every Agent **MUST** have a stable identity composed of:

- One **Ed25519** signing key (RFC 8032), used for all ICP message signatures.
- An **OPTIONAL** ML-DSA-65 signing key (FIPS 204, ML-DSA), used for hybrid
  post-quantum signatures. When present, the Agent **MUST** sign with both
  keys; verifiers **MUST** require both signatures to validate.
- One **X25519** key-agreement key (RFC 7748), used for confidential payload
  encryption.
- An **OPTIONAL** ML-KEM-768 KEM key (FIPS 203), used for hybrid
  post-quantum confidentiality.

### 4.2 Agent identifier (AID)

An Agent's identifier is the Multibase-Base58btc encoding of:

```
SHA-256( ed25519_pubkey || 0x00 || x25519_pubkey )
```

prefixed with `aid:` and a 4-character version tag. ICP-1.0 AIDs use the tag
`v1`:

```
aid:v1:zQ3sh...
```

AIDs are stable for the lifetime of the keypair. Key rotation produces a new
AID and **MUST** be accompanied by a signed rotation event from the prior AID.

### 4.3 Principal binding

An Agent **MUST** carry a signed **PrincipalBinding** statement issued by its
Principal, asserting:

- the Principal's legal identifier (LEI for businesses, Verifiable Credential
  for humans, or ICP-issued PrincipalID for DAOs);
- the Agent's AID;
- bounded authority (max-value-per-intent, max-value-per-period, allowed
  intent verbs, allowed counterparties);
- expiry timestamp;
- revocation endpoint (HTTPS or IPFS).

Counterparties **MUST** verify the PrincipalBinding before accepting an
Intent above their configured trust-floor value.

## 5. Wire format

### 5.1 Canonicalization

ICP messages are encoded as **Canonical JSON** (RFC 8785 JSON
Canonicalization Scheme, JCS). Every signed message is a JSON object with
exactly two top-level keys:

```json
{
  "p": <payload-object>,       // the payload object (§6–§9)
  "s": <signature-array>       // array of signatures, see §5.2
}
```

Signatures are computed over the RFC 8785 JCS encoding of the payload
object only; verifiers **MUST** re-canonicalize the received payload and
verify against those bytes. The canonicalization rules are in
`schemas/canonicalization.md` §1.

A binary **Canonical CBOR** encoding (RFC 8949 §4.2.2 deterministic
encoding, `application/icp+cbor`) is **reserved** for a future binary
profile, planned for icp-1.1. Implementations **MUST NOT** emit or accept
CBOR-signed messages under `v: "icp-1.0"`. The reserved CBOR rules are
specified in `schemas/canonicalization.md` §2 so implementations can
prepare without a breaking re-specification.

### 5.2 Signatures

The `s` array contains one or more signature objects:

```json
{
  "alg": "ed25519" | "ml-dsa-65" | "ed25519+ml-dsa-65",
  "kid": <AID-string>,
  "sig": <signature-hex>
}
```

`sig` carries the signature bytes as lowercase hex. When `alg` is the
hybrid form, the signature bytes are the concatenation
`ed25519_sig || ml_dsa_65_sig`. Verifiers **MUST** verify both components.

### 5.3 Replay protection

Every payload **MUST** include:

- `nonce` — 16 random bytes, unique per signing key per protocol-lifetime;
- `iat` — RFC 3339 timestamp with second precision;
- `exp` — RFC 3339 timestamp, **MUST** be ≤ `iat + 600s` for Intents,
  ≤ `iat + 86400s` for long-running state transitions.

Verifiers **MUST** reject any message with an already-seen nonce within the
`exp` window, and **SHOULD** maintain a nonce cache for at least
`max(iat + 86400s)` seconds.

> **Non-normative.** A byte-identical retry of a `purchase.create` Intent would
> otherwise trip this nonce check; ICPIP-0006 specifies how idempotent replay
> (same `intent_id` + identical canonical bytes) takes precedence over the
> nonce guard and returns the original Quote. See that ICPIP — this section's
> normative replay rule is unchanged.

## 6. Intent objects

An **Intent** is an Agent's signed request for a commerce action. ICP-1.0
defines seven core verbs:

| Verb | Description | §  |
|---|---|---|
| `purchase.create` | Buyer Agent requests goods/services | 6.1 |
| `purchase.return` | Buyer Agent requests return/refund | 6.2 |
| `inventory.query` | Agent requests inventory availability | 6.3 |
| `quote.request` | Agent requests pricing without commitment | 6.4 |
| `subscription.create` | Recurring purchase | 6.5 |
| `subscription.cancel` | Cancel an existing subscription authorization | 6.5.1 |
| `payout.request` | Agent requests release of held funds | 6.6 |

Each verb has a normative JSON Schema in `schemas/intent.<verb>.schema.json`.

Extension verbs MAY be defined through the ICPIP process (e.g.
`channel.register`, ICPIP-0005); they follow the same envelope, signature,
and replay rules as the core verbs.

### 6.1 purchase.create

```json
{
  "verb": "purchase.create",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",      // ULID
  "buyer": "aid:v1:zQ3shA...",
  "merchant": "aid:v1:zQ3shB...",
  "settler": "settler:circle.usdc.base",
  "items": [
    {
      "sku": "WIDGET-001",
      "quantity": 2,
      "unit_price": { "amount": "29.99", "currency": "USD" },
      "metadata": { ... }
    }
  ],
  "ship_to": { ... },                     // optional
  "max_total": { "amount": "65.00", "currency": "USD" },
  "expiry": "2026-05-09T18:00:00Z",
  "principal_binding": <signed-binding>,
  "nonce": "01HXYZ...",
  "iat": "2026-05-09T17:50:00Z",
  "exp": "2026-05-09T18:00:00Z"
}
```

`max_total` is the buyer's authoritative price ceiling. Merchants **MUST NOT**
return a Quote with `total > max_total`. This binds the merchant's price-quote
to the buyer's authority and prevents Agents from being upsold beyond their
mandate.

> **Non-normative.** Retry idempotency for `purchase.create` (a byte-identical
> retry returns the original Quote; a reused `intent_id` with different terms is
> rejected) is specified in ICPIP-0006. It is not part of ICP-1.0's normative
> text; see that ICPIP for the proposed semantics and error codes.

### 6.5 subscription.create

A buyer Agent's signed request to establish a recurring purchase
authorization with a merchant. Unlike `purchase.create` (which authorizes
a single transaction), `subscription.create` authorizes the merchant to
initiate purchase occurrences on a stated cadence until the subscription
is cancelled or its `max_occurrences` limit is reached.

Each subsequent billing cycle produces an **occurrence**: an automatic
`purchase.create` Intent referencing the `subscription_id`. The merchant
SHALL NOT initiate an occurrence whose Quote total exceeds
`max_total_per_period` (the ceiling rule from §11.4 generalized to the
recurring case). Buyers MAY cancel by signing a `subscription.cancel`
Intent (§6.5.1).

```json
{
  "verb": "subscription.create",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "buyer": "aid:v1:zA...",
  "merchant": "aid:v1:zB...",
  "settler": "settler:circle.usdc.base",
  "service_id": "premium-monthly",
  "cadence": "30d",                            // ISO-style period: "1d","7d","30d","1y" or RFC-3339 duration
  "max_total_per_period": { "amount": "29.99", "currency": "USDC" },
  "max_occurrences": 12,                       // optional; null = until cancelled
  "first_charge_at": "2026-05-15T00:00:00Z",
  "principal_binding": <signed-binding>,
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs a **SubscriptionAuthorization** in response:

```json
{
  "type": "subscription.authorization",
  "v": "icp-1.0",
  "subscription_id": "icp_sub_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "merchant": "aid:v1:zB...",
  "service_id": "premium-monthly",
  "cadence": "30d",
  "max_total_per_period": { "amount": "29.99", "currency": "USDC" },
  "max_occurrences": 12,
  "first_charge_at": "2026-05-15T00:00:00Z",
  "merchant_terms": {
    "cancellation_notice_period": "0d",        // immediate cancellation OK
    "refund_policy": "pro-rated",              // pro-rated | none | full
    "service_grant_per_period": "premium-tier-30d"
  },
  "expiry": "2027-05-15T00:00:00Z",            // when subscription auto-expires
  "iat": "...",
  "signature": { "alg": "ed25519", "kid": "<merchant-aid>", "sig": "..." }
}
```

The buyer Agent retains the SubscriptionAuthorization and presents it
on subsequent occurrences so the merchant can prove the authorization
was granted. Occurrence Intents reference the `subscription_id` in their
`metadata` field.

**Per-period cap enforcement.** The protocol-level invariant is that any
occurrence-Quote's `total` MUST be ≤ `max_total_per_period`. Merchants
that return Quotes exceeding this cap are non-conformant; conformant
buyer Agents MUST refuse to accept such Quotes.

**Cancellation.** Buyers cancel a subscription by signing a
`subscription.cancel` Intent (§6.5.1). Out-of-band cancellation (ceasing
to fund occurrence escrows) is permitted as a fallback, but produces no
audit-grade record — buyers handling high-value subscriptions SHOULD
always cancel via the protocol so the cancellation is signed,
non-repudiable, and dated.

### 6.5.1 subscription.cancel

A buyer Agent's signed request to cancel an existing subscription
authorization. Cancellation takes effect at the next billing boundary
or immediately, per the merchant's terms. The merchant signs a
**CancellationAuthorization** confirming the effective date and any
pro-rated refund.

```json
{
  "verb": "subscription.cancel",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "buyer": "aid:v1:zA...",
  "merchant": "aid:v1:zB...",
  "settler": "settler:circle.usdc.base",
  "subscription_id": "icp_sub_01HXYZ...",        // ID from the SubscriptionAuthorization
  "effective": "immediate",                       // "immediate" | "end-of-period"
  "reason": "no-longer-needed",                   // OPTIONAL — same enum as purchase.return reasons
  "principal_binding": <signed-binding>,          // MUST grant subscription.cancel
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs a **CancellationAuthorization** in response:

```json
{
  "type": "subscription.cancellation",
  "v": "icp-1.0",
  "cancellation_id": "icp_can_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "subscription_id": "icp_sub_01HXYZ...",
  "merchant": "aid:v1:zB...",
  "effective_at": "2026-05-12T18:00:00Z",         // when the cancellation takes effect
  "final_occurrences": 0,                          // remaining billing cycles before effective_at
  "pro_rated_refund": {                            // OPTIONAL — present iff merchant_terms.refund_policy = "pro-rated"
    "amount": { "amount": "15.00", "currency": "USDC" },
    "rail": "base-sepolia",
    "release_to": "<buyer-wallet-address>",
    "expected_settlement_within": "5d"
  },
  "iat": "...",
  "signature": { "alg": "ed25519", "kid": "<merchant-aid>", "sig": "..." }
}
```

**Effective semantics.**
- `effective: "immediate"` — no further occurrences are generated;
  pro-rated refund (if applicable) is issued for the current period.
- `effective: "end-of-period"` — the current period's occurrence remains
  in force; the next scheduled occurrence is NOT generated.

The merchant SHOULD honor whichever the buyer requested, but MAY downgrade
`immediate` to `end-of-period` per its policy (e.g. for annual
subscriptions with non-refundable prepayment). The merchant's chosen
`effective_at` is authoritative; conformant buyers reading the
CancellationAuthorization MUST treat `effective_at` as the binding date.

**Idempotency.** Cancellation of an already-cancelled subscription
returns the existing CancellationAuthorization, NOT an error. This makes
client retry logic safe.

**Eligibility.** Common rejection reasons:
- `policy.subscription.not_found` — subscription_id not recognized
- `policy.subscription.already_cancelled` — would return existing
  CancellationAuthorization, not a rejection
- `policy.subscription.not_cancellable` — merchant policy prohibits
  mid-cycle cancellation (e.g. annual non-refundable plans)
- `policy.subscription.outside_window` — cancellation request after
  the cancellation_notice_period ended

These error codes are added to `schemas/error-codes.md`.

### 6.2 purchase.return

A buyer Agent's signed request to return goods or services from a prior
completed settlement. The merchant evaluates against its return policy
and either signs a ReturnAuthorization (success) or returns an
`icp.error` (`policy.return.not_eligible` for policy reject; standard
codes otherwise).

Unlike `purchase.create`, this verb does NOT trigger a fresh escrow —
the rail-level refund happens within the existing settlement chain. The
ReturnAuthorization is the cryptographic record that the merchant
accepted the return and committed to the refund path. Auditors trace
both the original SettlementReceipt and the ReturnAuthorization to
reconstruct the complete commerce event.

```json
{
  "verb": "purchase.return",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "buyer": "aid:v1:zA...",
  "merchant": "aid:v1:zB...",
  "settler": "settler:circle.usdc.base",
  "original_settlement_id": "icp_set_01HXYZ...",      // SettlementReceipt of the original transaction
  "items": [                                            // subset of original line items
    { "sku": "WIDGET-001", "quantity": 1, "reason": "defective" }
  ],
  "desired_outcome": "refund",                          // refund | replacement | credit
  "max_refund": { "amount": "29.99", "currency": "USDC" },  // ceiling for protection
  "narrative": "Item arrived damaged, photo attached via metadata",
  "principal_binding": <signed-binding>,                // MUST grant purchase.return
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs a **ReturnAuthorization** in response:

```json
{
  "type": "return.authorization",
  "v": "icp-1.0",
  "return_id": "icp_ret_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "original_settlement_id": "icp_set_01HXYZ...",
  "merchant": "aid:v1:zB...",
  "outcome": "refund",                                  // refund | replacement | credit | partial-refund
  "refund": {
    "amount": { "amount": "29.99", "currency": "USDC" },
    "rail": "base-sepolia",
    "release_to": "<buyer-wallet-address>",
    "expected_settlement_within": "5d"                  // duration; SHOULD be honored
  },
  "merchant_terms": {
    "return_shipping_label_url": "https://...",         // OPTIONAL
    "rma_code": "RMA-2026-...",                         // OPTIONAL, merchant-internal
    "must_return_by": "2026-06-01T00:00:00Z"            // OPTIONAL deadline for physical return
  },
  "iat": "...",
  "signature": { "alg": "ed25519", "kid": "<merchant-aid>", "sig": "..." }
}
```

**Refund ceiling enforcement.** `max_refund` in the Intent caps the
refund. A merchant signing a ReturnAuthorization with
`refund.amount > max_refund` is non-conformant; conformant buyer Agents
MUST reject such authorizations.

**Refund settlement event.** The actual rail-level refund (USDC
transferred back, ACH credit, etc.) produces a follow-up
SettlementReceipt with `final_state: "refunded"` (or
`"partially-refunded"`). That receipt's `settlement_id` references the
`return_id` in its `parent_return_id` field for audit traceability.
ICP-1.0 specifies the Intent + Authorization; the Settler observes the
refund tx and signs the follow-up SettlementReceipt as it does for
forward settlements (§S.3).

**Eligibility policy.** Common rejection reasons:
- `policy.return.window_expired` — original settlement >30d (or merchant-defined window)
- `policy.return.not_eligible` — item category is non-returnable per merchant policy
- `policy.return.already_returned` — line item already in a prior return
- `policy.return.exceeds_max_refund` — requested refund > Intent's `max_refund`
- `policy.return.original_disputed` — original settlement is in a disputed state

These error codes are added to `schemas/error-codes.md` under the
`policy.return.*` namespace.

### 6.3 inventory.query

A buyer Agent's signed read-only query for inventory availability. Unlike
the value-transferring verbs (`purchase.create`, `subscription.create`,
`purchase.return`), `inventory.query` does NOT trigger an escrow or
settlement — it returns a signed **InventorySnapshot** that the buyer can
use to plan subsequent value-transferring Intents.

The query is **signed by the buyer** for non-repudiation and rate-limit
accounting. The response is **signed by the merchant** so the buyer can
later prove what prices and availability were advertised at a given
moment — a critical primitive for dispute resolution when a subsequent
`purchase.create` Quote diverges from the queried inventory.

```json
{
  "verb": "inventory.query",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "buyer": "aid:v1:zA...",
  "merchant": "aid:v1:zB...",
  "settler": "settler:circle.usdc.base",        // names the rail the buyer expects to settle in
  "skus": [                                      // OPTIONAL — empty means "advertise everything"
    { "sku": "WIDGET-001", "quantity": 2 },
    { "sku": "WIDGET-002", "quantity": 5 }
  ],
  "filters": {                                   // OPTIONAL — merchant-defined free-form
    "category": "electronics",
    "in_stock_only": true
  },
  "max_results": 100,                            // OPTIONAL — cap the snapshot size
  "principal_binding": <signed-binding>,         // MUST grant inventory.query
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs an **InventorySnapshot** in response:

```json
{
  "type": "inventory.snapshot",
  "v": "icp-1.0",
  "snapshot_id": "icp_inv_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "merchant": "aid:v1:zB...",
  "snapshot_taken_at": "2026-05-12T17:50:00Z",
  "valid_until": "2026-05-12T17:55:00Z",          // typically 5min; merchant policy
  "items": [
    {
      "sku": "WIDGET-001",
      "available_quantity": 47,
      "unit_price": { "amount": "29.99", "currency": "USDC" },
      "metadata": { "lead_time_days": 2, "weight_g": 250 }
    },
    {
      "sku": "WIDGET-002",
      "available_quantity": 0,
      "unit_price": { "amount": "49.99", "currency": "USDC" },
      "metadata": { "restock_eta": "2026-05-19T00:00:00Z" }
    }
  ],
  "total_matching_skus": 2,                       // for pagination signaling
  "iat": "...",
  "signature": { "alg": "ed25519", "kid": "<merchant-aid>", "sig": "..." }
}
```

> **Non-normative.** Opaque cursor pagination over `total_matching_skus`
> (request `cursor`, response `next_cursor`, page-size bounds, and
> stability-under-mutation guarantees) is specified in ICPIP-0006. The
> `max_results`/`total_matching_skus` fields above are normative ICP-1.0;
> the cursor mechanism that walks them is proposed in that ICPIP.

**Snapshot validity.** The `valid_until` field tells the buyer how long
the prices/availability are guaranteed. After expiry, prices are stale.
Buyers MAY use a stale snapshot to inform a subsequent
`purchase.create`, but the merchant is NOT bound to honor stale prices.

**Snapshot-quote consistency.** If a `purchase.create` Quote returns a
`unit_price` that differs from a still-valid InventorySnapshot's
`unit_price` for the same `sku`, the merchant SHOULD include
`snapshot_id` in the Quote's metadata explaining the divergence
(e.g. dynamic surge pricing). Conformant buyer Agents MAY refuse such
Quotes as non-binding price-walk attempts.

**Rate limiting.** Read-only queries are cheaper to serve than
value-transferring Intents but still consume merchant resources.
Merchants MAY rate-limit `inventory.query` per buyer AID. Excess
queries return `rate.aid_quota_exceeded` with a `retry_after` hint.

**Why this verb matters for B2B.** B2B agentic commerce is dominated
by discovery: an agent on a procurement system runs hundreds of
`inventory.query` calls across vendors for every `purchase.create`. By
volume, inventory.query is the highest-call-count verb in the
protocol. ICP-1.0 ships it as a first-class verb (not deferred to 1.1)
because B2B adoption is gated on it.

### 6.4 quote.request

A buyer Agent's signed request for pricing **without commitment** —
the B2B wholesale RFQ primitive. Unlike `purchase.create` (which binds
to a `max_total` ceiling and triggers escrow on acceptance),
`quote.request` returns a non-binding **PriceProposal** that the buyer
may evaluate, compare against competitors, and accept or reject
without protocol-level commitment.

Full normative specification: see [ICPIP-0003](./icpips/icpip-0003-quote-request.md).

```json
{
  "verb": "quote.request",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "buyer": "aid:v1:zA...",
  "merchant": "aid:v1:zB...",
  "settler": "settler:circle.usdc.base",
  "items": [
    {
      "sku": "FASTENER-M6X20",
      "quantity": 500,
      "target_unit_price": { "amount": "0.12", "currency": "USDC" },
      "specifications": { "material": "316-SS" }
    }
  ],
  "ship_to": { ... },
  "expected_delivery_by": "2026-06-15T00:00:00Z",
  "purchase_window": "30d",
  "principal_binding": <signed-binding>,
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs a **PriceProposal** in response with `valid_until`,
line-item pricing, payment terms, fulfillment terms, and an explicit
`non_binding_notice`. To commit, the buyer submits a subsequent
`purchase.create` Intent referencing the proposal via the OPTIONAL
`from_proposal_id` field — the merchant MUST honor proposal prices while
the proposal is valid, MUST surface `quote.proposal_expired` otherwise.

Six new error codes added to §10: `policy.quote.not_available_for_quantity`,
`policy.quote.sku_not_quotable`, `policy.quote.window_too_long`,
`quote.proposal_not_found`, `quote.proposal_expired`,
`quote.proposal_total_mismatch`.

### 6.6 payout.request

A seller Agent's signed request to receive a payout from a platform-held
balance. The **only ICP verb with inverted signing direction**: the
recipient (seller) signs the Intent; the originator (platform) signs the
response.

Full normative specification: see [ICPIP-0004](./icpips/icpip-0004-payout-request.md).

```json
{
  "verb": "payout.request",
  "v": "icp-1.0",
  "intent_id": "icp_int_01HXYZ...",
  "seller": "aid:v1:zS...",
  "platform": "aid:v1:zP...",
  "settler": "settler:circle.usdc.base",
  "amount": { "amount": "1247.83", "currency": "USDC" },
  "destination": {
    "type": "wallet",
    "wallet_address": "0x..."
  },
  "expedited": false,
  "principal_binding": <signed-binding>,
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The platform signs a **PayoutAuthorization** in response with itemized
binding fees (`platform_commission`, `compliance_reserve`, etc.),
`available_balance`, `approved_amount = available_balance - sum(fees)`,
and the rail finalization timing.

PrincipalBinding `authority` is extended with OPTIONAL `max_per_payout`
and `allowed_platforms` fields (backward-compatible — ICP-1.0
implementations ignore unknown authority fields).

10 new error codes added to §10 under `policy.payout.*` namespace:
`insufficient_balance`, `hold_period_active`, `exceeds_max_per_payout`,
`exceeds_max_per_period`, `kyc_required`, `destination_not_allowlisted`,
`rail_unavailable`, `expedited_unavailable`, `compliance_hold`,
`platform_not_allowed`.

After this verb ships, ICP-1.0 covers **100% of the commerce verb
surface** — ~$31T in addressable annual commerce flow across retail,
SaaS, returns, B2B procurement, and marketplaces.

## 7. Quote objects

A **Quote** is a Counterparty's signed response binding terms for a bounded
acceptance window:

```json
{
  "type": "quote",
  "v": "icp-1.0",
  "quote_id": "icp_qt_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",      // matches the Intent
  "merchant": "aid:v1:zQ3shB...",
  "total": { "amount": "62.49", "currency": "USD" },
  "lines": [ ... ],                       // line-item breakdown
  "settler": "settler:circle.usdc.base",
  "escrow_terms": {
    "release_on": "fulfilled+24h",
    "dispute_window": "168h"
  },
  "expiry": "2026-05-09T17:55:00Z",       // tighter than Intent expiry
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

Quote acceptance is signaled by the Buyer Agent signing an `Accept` envelope
referencing `quote_id`. Acceptance after `expiry` is invalid.

## 8. Escrow state machine

After Acceptance, the Settler creates an **Escrow** record and reports its
state via signed **EscrowEvent** messages. States and transitions:

| From | To | Trigger | Required signature |
|---|---|---|---|
| `pending` | `funded` | Buyer payment confirmed by Settler | Settler |
| `funded` | `released` | Fulfillment confirmed AND dispute window elapsed | Settler + Merchant |
| `funded` | `disputed` | Buyer or Merchant raises Dispute | Disputing party |
| `disputed` | `released` | Dispute resolution favors Merchant | Settler + arbiter |
| `disputed` | `refunded` | Dispute resolution favors Buyer | Settler + arbiter |
| `funded` | `refunded` | Merchant cancels OR fulfillment expires | Settler + Merchant |

Every transition emits a signed EscrowEvent with monotonic `seq`. Implementations
**MUST** persist EscrowEvents and **MUST** be able to reconstruct the current
escrow state by replaying events from `seq=0`.

## 9. Settlement

Settlement is the irrevocable transfer of value from Escrow to Merchant
(`released`) or back to Buyer (`refunded`). Settlement **MUST** produce a
**SettlementReceipt** signed jointly by the Settler and the receiving party,
containing:

- `escrow_id`
- `final_state`: `"released"` or `"refunded"`
- `amount` and `currency`
- `rail`: settlement rail identifier (e.g. `circle.usdc.base`,
  `stripe.treasury.ach`, `solana.spl.usdc`)
- `rail_txid`: rail-native transaction identifier (chain hash, ACH trace
  number, etc.)
- `settled_at`

SettlementReceipts are the canonical proof of payment. Tax, accounting, and
audit systems **MUST** treat the SettlementReceipt as authoritative.

## 10. Error model

ICP errors are JSON objects with normative shape:

```json
{
  "type": "icp.error",
  "code": "<error-code>",          // see error-codes.md
  "message": "<human-readable>",
  "intent_id": <optional>,
  "remediation": <optional>,
  "retry_after": <optional-seconds>
}
```

Error codes are namespaced with dot-separated segments. ICP-1.0 reserves the
following top-level namespaces: `auth`, `signature`, `replay`, `policy`,
`escrow`, `settlement`, `dispute`, `rate`, `format`, `version`. The complete
enumeration is in `schemas/error-codes.md`.

## 11. Security considerations

### 11.1 Settler trust

The Settler is the **single point of value-capture** in ICP. A malicious
Settler can refuse to honor a release. Counterparties **MUST**:

- Use only Settlers from a vetted allowlist (governance-maintained for
  `aid:v1` AIDs);
- Prefer Settlers that publish on-chain proof-of-reserves;
- Cap individual escrow exposure to a Settler at a configured limit.

### 11.2 Replay across rails

A SettlementReceipt is bound to a specific rail. Implementations **MUST NOT**
treat a SettlementReceipt from rail A as proof of settlement on rail B, even
if the amounts match.

### 11.3 PrincipalBinding revocation

PrincipalBindings include a revocation endpoint. Counterparties **SHOULD**
check revocation for any Intent whose value exceeds a configured threshold
(suggested: $1,000 or local equivalent), and **MUST** check for any Intent
above $10,000.

### 11.4 Quote-binding attack

Without `max_total`, a merchant could return an inflated Quote and rely on a
sloppy Buyer Agent to auto-accept. The `max_total` field in the Intent is a
**MUST NOT** ceiling for Quotes; this is the protocol-level mitigation.

## 12. Test vectors

`icp-conformance/vectors/icp-1.0/` contains the normative reference vectors,
executed via the `icp-conformance` runner (§2). Each vector is a directory
of JSON files (`description.md`, `inputs.json`, `expected.json`) covering:

1. Input messages (deterministic key material, payloads, signatures)
2. Expected output (verification result, computed AIDs, canonical bytes)
3. Negative cases (tampered payloads, bit-flipped or truncated signatures)

Implementations **MUST** produce identical output on the positive cases and
**MUST** reject the negative cases with the documented error code.

## 13. References

- RFC 2119 / RFC 8174 — Key words for use in RFCs
- RFC 8032 — Edwards-Curve Digital Signature Algorithm (Ed25519)
- RFC 7748 — Elliptic Curves for Security (X25519)
- RFC 8785 — JSON Canonicalization Scheme (JCS)
- RFC 8949 — Concise Binary Object Representation (CBOR)
- RFC 3339 — Date and Time on the Internet
- FIPS 203 — Module-Lattice-Based Key-Encapsulation Mechanism (ML-KEM)
- FIPS 204 — Module-Lattice-Based Digital Signature Algorithm (ML-DSA)
- AP2 — Agent Payments Protocol (Google, 2025)
- ACP — Agentic Commerce Protocol (OpenAI/Stripe, 2025)
- x402 — HTTP 402 Payment Required for Agents (Coinbase, 2025)
