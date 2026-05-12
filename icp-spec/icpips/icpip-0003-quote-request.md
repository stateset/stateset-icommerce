# ICPIP-0003: `quote.request` Verb Specification

```
ICPIP:        0003
Title:        quote.request Verb Specification (B2B Wholesale RFQ)
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/3 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-05-12
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Adds `quote.request` as the **sixth** ICP intent verb: a buyer Agent's
signed request for **pricing without commitment**. Unlike `purchase.create`
(which binds the buyer to a `max_total` ceiling and triggers escrow on
acceptance), `quote.request` returns a non-binding **PriceProposal** that
the buyer may evaluate, compare against competitors, and accept or
reject without protocol-level commitment.

This is the **B2B wholesale RFQ primitive** — the most common procurement
pattern in commercial purchasing. Without it, agents that operate
multi-vendor procurement flows have to use `purchase.create` for
discovery (committing themselves to a max_total before they know the
market), which forces them either to over-cap (risk over-paying) or
under-cap (risk rejection). Neither is acceptable for B2B at scale.

`quote.request` was deferred from ICP-1.0 because the four
value-transferring verbs covered ~99% of dollar volume. This ICPIP
proposes it as the first verb addition of ICP-1.1.

## Motivation

### What B2B procurement actually looks like

A procurement agent shopping for 500 units of a custom industrial
fastener runs roughly this flow today, **without** ICP:

1. Receive purchase order from internal requester (specs + budget)
2. Identify 5–10 candidate vendors
3. Send each vendor an RFQ (Request for Quote) — typically email,
   sometimes EDI, increasingly an API
4. Receive back per-vendor pricing with terms (lead time, volume
   discount, payment terms, return policy)
5. Compare quotes, optionally counter-offer the top 2–3 vendors
6. Pick a winner and issue a purchase order

ICP today (5 verbs) supports steps 4–6 of this flow IF the agent
already knows the price (via `inventory.query`). But step 3 — the RFQ
itself — has no protocol primitive. Agents resort to:
- Calling `inventory.query` and using its snapshot prices (works for
  catalog items, fails for custom orders or volume-discount tiers)
- Calling `purchase.create` with a placeholder `max_total` and
  cancelling (wastes merchant compute on Quote signing for transactions
  that won't happen)
- Going out-of-protocol via email/EDI (no signed audit record)

`quote.request` is the missing primitive. It says: *"For this
hypothetical purchase, what would your price be?"* — with no commitment
beyond paying for the merchant's compute (via fee-per-call infra
elsewhere in the stack, not part of ICP).

### Why this matters for ICP's adoption

B2B procurement is **the largest agentic commerce category by dollar
volume**. Global B2B e-commerce was ~$23T in 2024 (vs. ~$5.7T B2C).
Wholesale, manufacturing supply chains, healthcare procurement, and
government purchasing all depend on RFQ flows. A protocol without
quote.request cannot serve any of them.

Estimated adoption multiplier from this verb alone: 3–10× existing ICP
addressable volume, based on retail-vs-B2B dollar-volume ratios.

## Specification

### Wire format

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
      "target_unit_price": { "amount": "0.12", "currency": "USDC" },  // OPTIONAL
      "specifications": { "material": "316-SS", "finish": "passivated" } // OPTIONAL free-form
    }
  ],
  "ship_to": { ... },                                                   // OPTIONAL
  "expected_delivery_by": "2026-06-15T00:00:00Z",                       // OPTIONAL
  "purchase_window": "30d",                                              // how long buyer will hold the decision; merchant SHOULD honor pricing for this period
  "context": "annual-procurement-Q2-2026",                              // OPTIONAL — for merchant's analytics
  "principal_binding": <signed-binding>,                                 // MUST grant quote.request
  "nonce": "...",
  "iat": "...",
  "exp": "..."
}
```

The merchant signs a **PriceProposal** in response:

```json
{
  "type": "price.proposal",
  "v": "icp-1.0",
  "proposal_id": "icp_pp_01HXYZ...",
  "intent_id": "icp_int_01HXYZ...",
  "merchant": "aid:v1:zB...",
  "issued_at": "2026-05-12T17:55:00Z",
  "valid_until": "2026-06-11T17:55:00Z",                                // honors purchase_window if feasible
  "items": [
    {
      "sku": "FASTENER-M6X20",
      "quantity": 500,
      "unit_price": { "amount": "0.11", "currency": "USDC" },
      "line_total": { "amount": "55.00", "currency": "USDC" },
      "volume_tier": "500-999"                                          // OPTIONAL — explains how the price was computed
    }
  ],
  "total": { "amount": "55.00", "currency": "USDC" },
  "payment_terms": {                                                    // OPTIONAL — merchant's standard terms
    "net_days": 30,                                                     // payment due 30 days post-fulfillment
    "early_pay_discount": { "percent": "2", "if_paid_within_days": 10 }
  },
  "fulfillment_terms": {
    "lead_time_days": 7,
    "shipping_method": "ground",
    "estimated_delivery_by": "2026-06-22T00:00:00Z"
  },
  "return_policy_summary": "30 days, full refund, buyer pays return shipping",
  "non_binding_notice": "This proposal is informational and does not commit either party. To purchase, submit a purchase.create Intent referencing this proposal_id.",
  "signature": { "alg": "ed25519", "kid": "<merchant-aid>", "sig": "..." }
}
```

### From PriceProposal to purchase

To commit to a PriceProposal, the buyer submits a `purchase.create`
Intent with the additional field `from_proposal_id`:

```json
{
  "verb": "purchase.create",
  ...
  "from_proposal_id": "icp_pp_01HXYZ...",
  "max_total": { "amount": "55.00", "currency": "USDC" },                // MUST equal proposal.total when from_proposal_id is set
  ...
}
```

When `from_proposal_id` is present, the merchant MUST honor the
proposal's `unit_price`s in the resulting Quote IF the proposal is
still within `valid_until`. If the proposal has expired, the merchant
MAY treat the Intent as a fresh `purchase.create` and quote at current
prices (but MUST NOT silently — it MUST surface this via the new error
code `quote.proposal_expired` and require the buyer to re-submit
without `from_proposal_id`).

The `purchase.create` schema is extended to add the OPTIONAL
`from_proposal_id` field. This is a backward-compatible addition; ICP-1.0
clients ignore unknown fields.

### Validity semantics

- `valid_until` is the merchant's commitment to honor the prices. After
  this timestamp, all bets are off.
- The buyer MAY submit `purchase.create` with `from_proposal_id` at any
  time before `valid_until`. The merchant MUST honor the proposal's
  prices.
- The merchant MAY return a Quote with `total > proposal.total` if and
  only if the proposal has expired AND the buyer explicitly opted into
  fresh pricing by re-submitting without `from_proposal_id`. Silent
  divergence is non-conformant.

### Eligibility policy

Common rejection reasons (new error codes added to
`schemas/error-codes.md`):

| Code | When emitted |
|---|---|
| `policy.quote.not_available_for_quantity` | Quantity falls outside merchant's quotable range (e.g. >100k units) |
| `policy.quote.sku_not_quotable` | SKU is fixed-price catalog only; merchant doesn't quote it |
| `policy.quote.window_too_long` | Buyer's `purchase_window` exceeds merchant's policy ceiling |
| `quote.proposal_not_found` | `from_proposal_id` reference doesn't match any issued proposal |
| `quote.proposal_expired` | Proposal exists but `valid_until` is in the past |
| `quote.proposal_total_mismatch` | `max_total` in purchase.create doesn't match proposal.total when `from_proposal_id` is set |

### Schema updates

New JSON Schema: `schemas/intent.quote.request.schema.json` (forthcoming
with the reference implementation PR).

Modified schema: `schemas/intent.purchase.create.schema.json` adds
OPTIONAL `from_proposal_id` field with the pattern
`^icp_pp_[0-9A-HJKMNP-TV-Z]{26}$`.

### Capabilities advertisement

Merchants supporting `quote.request` MUST add it to their
`.well-known/icp` capabilities array. Merchants advertising
`quote.request` but consistently returning `policy.quote.not_available_for_quantity`
for every SKU SHOULD be flagged on the Foundation's conformance
dashboard as "advertised but unused" — this is mild non-conformance
that doesn't break the protocol but undermines buyer trust.

## Rationale

### Why a separate verb, not an `is_quote_only` flag on `purchase.create`

Rejected: making `purchase.create` carry a `is_quote_only: true` field.
Reasons:

1. **Semantic clarity**: `purchase.create` semantically commits to a
   transaction. Overloading it with a "but not really" flag is the
   kind of API mistake that produces subtle bugs (e.g. merchant
   accidentally treats a quote-only Intent as binding).
2. **Conformance hygiene**: a separate verb is independently
   conformance-testable. A flag is a flag-flip away from a security
   incident.
3. **Capability discovery**: clients can introspect verb support via
   `.well-known/icp`. Adding a flag would require advertising the
   flag's support separately, which is messier.
4. **Audit traceability**: an auditor reviewing a SettlementReceipt
   chain wants to see "this was an RFQ → quote → purchase chain" not
   "this purchase.create had a flag set 30 days ago and was retroactively
   quoted via another purchase.create."

### Why no escrow

PriceProposals don't trigger escrow because no value has been
committed. Escrow exists to protect a buyer from non-delivery and a
merchant from non-payment. A quote-only flow has no committed value, so
nothing needs to be held. This keeps quote.request cheap to serve and
fast to respond to.

### Why `from_proposal_id` instead of cloning the proposal into the Intent

Rejected: making the buyer copy `unit_price` and `payment_terms` from
the proposal into a fresh `purchase.create` Intent. Reasons:

1. **Signature integrity**: the merchant signed the PriceProposal. The
   buyer can't modify it without invalidating the signature. Forcing
   the buyer to copy fields invites tampering.
2. **Audit trail**: `from_proposal_id` creates an explicit pointer from
   the executed purchase back to the originating quote. Auditors can
   trace every Quote to its proposal (or determine no proposal
   existed) without inference.
3. **Merchant convenience**: with the proposal_id, the merchant can
   skip pricing logic on the resulting `purchase.create` and just
   reference the proposal.

### Threshold for hybrid PQC (ICPIP-0002 interaction)

PriceProposals contain pricing data with significant retroactive
information value (competitive intelligence, historical price curves).
The hybrid PQC mandate from ICPIP-0002 applies to PriceProposals at the
same value threshold as Intents: if the proposal's `total` is
≥$10,000 USD-equivalent, the merchant's signature MUST be hybrid
`ed25519+ml-dsa-65`.

This is non-controversial — proposals are signed, not held — but worth
specifying explicitly so implementers don't accidentally exclude them.

### Volume tier discounts

The `volume_tier` field in the proposal is OPTIONAL but RECOMMENDED.
When present, it explains the merchant's pricing logic:

```json
{
  "volume_tier": "500-999",
  "_tier_table_hint": "1-99: $0.15, 100-499: $0.13, 500-999: $0.11, 1000+: $0.09"
}
```

This is informational. Buyers can use it to evaluate whether quantity
adjustments would improve pricing. Production merchants SHOULD publish
their volume tier tables via `.well-known/icp/volume-tiers` (a future
endpoint extension), but this ICPIP doesn't require it.

## Backwards Compatibility

This is an **additive** verb. ICP-1.0 implementations that don't
support `quote.request`:

- MAY return `format.unknown_verb` if the Intent's verb field is
  `quote.request`. Buyer clients SHOULD fall back to using
  `inventory.query` for catalog discovery in this case.
- The new OPTIONAL `from_proposal_id` field in `purchase.create` is
  ignored by ICP-1.0 implementations. ICP-1.0 merchants will price
  fresh, which is the safe default.

The migration path is **opt-in**:

1. Day 0: ICP-1.1 ratified. `quote.request` is OPTIONAL for merchants.
2. Day 0+90: merchants who want B2B traffic advertise `quote.request`
   in their capabilities.
3. Day 0+180: buyer agents begin preferring merchants who advertise
   `quote.request` over those who don't (for high-volume orders).
4. Day 0+365: market pressure ratchets toward universal support;
   no protocol mandate is needed.

This is intentionally a soft transition. Unlike ICPIP-0002's PQC
mandate (which is a security-driven hard mandate at threshold), verbs
are features and the market decides their adoption.

## Security Considerations

### Quote farming attacks

A malicious buyer Agent could spam quote.request to harvest competitive
pricing intelligence at no cost. Mitigations:

1. **Per-AID rate limiting**: merchants MAY apply token-bucket rate
   limits to quote.request, charging the AID's principal for excess
   requests. The new error code `rate.quote_quota_exceeded` is
   already covered by ICP-1.0's `rate.aid_quota_exceeded`.
2. **PrincipalBinding authority**: the buyer's PrincipalBinding can
   restrict quote.request to specific merchants via the optional
   `allowed_counterparties` field (already in ICP-1.0).
3. **Sub-threshold inventory cost**: quote.request is cheaper than
   purchase.create to serve. Merchants offering it to anonymous
   parties accept the asymmetric cost as a customer-acquisition
   investment.
4. **Foundation-published bad-actor list**: AIDs that demonstrably abuse
   quote.request (Foundation-defined heuristics: >1000 RFQs per day
   with 0 conversions) MAY be flagged on a public bad-actor list.
   This is a future ICPIP (TBD), not part of 0003.

### Pricing tampering between proposal and purchase

An attacker who can intercept and modify a buyer's submitted
`purchase.create` Intent (despite TLS) might change `from_proposal_id`
to point at a higher-priced proposal. Mitigation: the Intent is signed
by the buyer; the merchant verifies the signature; modification breaks
the signature. The standard ICP-1.0 signature model already covers
this — no new mitigation needed.

### Information leakage via free-form `specifications` field

Buyers might include proprietary engineering specs in the
`specifications` field (e.g. tolerances that reveal product design).
Merchants who receive such RFQs have a fiduciary duty to handle the
data appropriately. This is a contracts/policy issue, not a protocol
issue. The ICP wire format doesn't classify which fields are
"sensitive" because the same field is sensitive for one buyer and not
for another. Buyers SHOULD apply their own data-classification logic
before submitting RFQs.

### Replay vs forks of pricing

PriceProposals are signed and time-bound (`valid_until`). A merchant
who issues a proposal MUST honor the prices for the validity window.
If market conditions change drastically (commodity price spike, supply
shock), the merchant's only recourse is to:

1. Refuse `purchase.create` referencing the proposal_id with a typed
   error code (TBD; perhaps `policy.market_emergency`), OR
2. Honor the proposal and absorb the loss as the cost of issuing
   binding-equivalent quotes.

Merchants who frequently invoke option 1 will lose trust. This ICPIP
deliberately doesn't make `valid_until` invalidatable by merchant
discretion — that would defeat the purpose. Merchants should set
shorter `valid_until` windows if they cannot commit to longer ones.

## Test Vectors

Conformance vector `06-quote-request-roundtrip` (to be added to
`icp-conformance/vectors/icp-1.0/` for IUTs that advertise
`quote.request` support):

```
inputs.json:
{
  "test": "06-quote-request-roundtrip",
  "agent": { ... deterministic seeds ... },
  "intent": {
    "v": "icp-1.0",
    "verb": "quote.request",
    "items": [{ "sku": "TEST-BOLT", "quantity": 500 }],
    ...
  },
  "merchant_pricing": {
    "TEST-BOLT": { "tier_500": "0.11" }
  }
}

expected.json:
{
  "intent_canonical_string": "...",
  "intent_signature_hex": "...",
  "proposal_total": "55.00",
  "proposal_signature_validates": true
}
```

A negative case: a `purchase.create` referencing an expired proposal_id
MUST yield `quote.proposal_expired`.

## Reference Implementation

The icp-handler / icp-mcp / @stateset/icp-client family supports
quote.request after this ICPIP advances to Last Call. Estimated changes:

- `icp-handler/src/backend-stub.mjs`: add `stubQuoteRequest()` (~80 LOC)
  matching the existing stub patterns.
- `icp-handler/src/server.mjs`: route `quote.request` to the stub.
- `icp-mcp/src/backend.mjs`: route via the same backend.
- `packages/icp-client/src/index.mjs`: add `.requestQuote()` method
  and extend `.purchase()` to accept `from_proposal_id`.
- 2 new handler tests (happy path + expired proposal rejection).

Following ICPIP-0001's gating: this ICPIP advances to Final only after
≥2 independent IUTs (e.g. handler + a non-StateSet implementation) pass
the conformance vector above.

## References

- ICP-1.0-DRAFT §6.1 (purchase.create) — the verb this composes with
- ICP-1.0-DRAFT §6.3 (inventory.query) — the discovery primitive
- ICPIP-0001 (Process) — the lifecycle this ICPIP follows
- ICPIP-0002 (Hybrid PQC) — threshold rules also apply to PriceProposals
- ISO 20022 RFQ message structure — informational comparison
- ANSI X12 EDI 840 (RFQ) — informational comparison

## Copyright

This ICPIP is licensed under CC-BY-4.0.
