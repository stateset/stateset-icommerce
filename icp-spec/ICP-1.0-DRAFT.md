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
the `icp-conformance` test suite tagged `icp-1.0`, including all signed test
vectors in `test-vectors/icp-1.0/`.

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

ICP messages are encoded as **Canonical CBOR** (RFC 8949 §4.2.2 deterministic
encoding). Every signed message is the CBOR encoding of a map with exactly
two top-level keys:

```cbor
{
  "p": <payload-bytes>,        // CBOR-encoded payload object
  "s": <signature-array>       // array of signatures, see §5.2
}
```

JSON representation is permitted at API boundaries (REST, MCP) but signatures
are computed over the canonical CBOR encoding only. JSON-to-CBOR
canonicalization rules are in `schemas/canonicalization.md`.

### 5.2 Signatures

The `s` array contains one or more signature objects:

```cbor
{
  "alg": "ed25519" | "ml-dsa-65" | "ed25519+ml-dsa-65",
  "kid": <AID-string>,
  "sig": <signature-bytes>
}
```

When `alg` is the hybrid form, `sig` is the concatenation
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

## 6. Intent objects

An **Intent** is an Agent's signed request for a commerce action. ICP-1.0
defines six core verbs:

| Verb | Description | §  |
|---|---|---|
| `purchase.create` | Buyer Agent requests goods/services | 6.1 |
| `purchase.return` | Buyer Agent requests return/refund | 6.2 |
| `inventory.query` | Agent requests inventory availability | 6.3 |
| `quote.request` | Agent requests pricing without commitment | 6.4 |
| `subscription.create` | Recurring purchase | 6.5 |
| `payout.request` | Agent requests release of held funds | 6.6 |

Each verb has a normative JSON Schema in `schemas/intent.<verb>.schema.json`.

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
Intent (verb specified in ICP-1.1).

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

**Cancellation.** Until `subscription.cancel` ships in ICP-1.1, buyers
MAY cancel out-of-band (e.g. by ceasing to fund occurrence escrows) and
merchants MUST honor a written cancellation. Buyers SHOULD NOT rely on
out-of-band cancellation for high-value subscriptions; wait for 1.1.

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

### 6.3 inventory.query

A buyer Agent's signed read-only query for inventory availability.
Specified in ICP-1.1 (forthcoming). Will be the highest-volume verb
by call count for B2B agentic commerce.

### 6.4 quote.request

A buyer Agent's signed request for pricing without commitment. Useful
for comparison shopping and B2B procurement. Specified in ICP-1.1.

### 6.6 payout.request

A merchant or marketplace participant Agent's signed request for release
of held funds (escrowed by the platform) to the participant's wallet.
Specified in ICP-1.1.

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

ICP errors are CBOR maps with normative shape:

```cbor
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

`test-vectors/icp-1.0/` contains the normative reference vectors. Each vector
file is a CBOR-encoded sequence of:

1. Input messages (Intent, Quote, Escrow events)
2. Expected output (verification result, computed AIDs, derived state)
3. Negative cases (tampered signatures, replayed nonces, expired Intents)

Implementations **MUST** produce identical output on the positive cases and
**MUST** reject the negative cases with the documented error code.

## 13. References

- RFC 2119 / RFC 8174 — Key words for use in RFCs
- RFC 8032 — Edwards-Curve Digital Signature Algorithm (Ed25519)
- RFC 7748 — Elliptic Curves for Security (X25519)
- RFC 8949 — Concise Binary Object Representation (CBOR)
- RFC 3339 — Date and Time on the Internet
- FIPS 203 — Module-Lattice-Based Key-Encapsulation Mechanism (ML-KEM)
- FIPS 204 — Module-Lattice-Based Digital Signature Algorithm (ML-DSA)
- AP2 — Agent Payments Protocol (Google, 2025)
- ACP — Agentic Commerce Protocol (OpenAI/Stripe, 2025)
- x402 — HTTP 402 Payment Required for Agents (Coinbase, 2025)
