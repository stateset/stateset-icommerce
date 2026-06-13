# ICPIP-0006: Idempotency Keys and Cursor Pagination

```
ICPIP:        0006
Title:        Idempotency Keys (purchase.create) and Cursor Pagination (inventory.query)
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/6 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-06-12
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Adds two normative operational guarantees that ICP-1.0 leaves implicit:

1. **Idempotent `purchase.create`** — a retried Intent (same `intent_id`,
   identical canonical payload) MUST return the *same* Quote rather than
   minting a second one, while a *different* payload reusing an
   already-seen `intent_id` MUST be rejected. This makes the buyer→merchant
   request path safe to retry across network failures without double-quoting
   or double-charging.

2. **Cursor pagination for `inventory.query`** — formalizes the
   `total_matching_skus`/`max_results` signaling that ICP-1.0 §6.3 mentions
   but does not specify, by adding an *opaque cursor* with stable, bounded,
   resumable semantics. This lets a procurement Agent page through a large
   catalog deterministically instead of guessing offsets.

Both changes are strictly additive: existing single-shot callers that never
retry and never paginate are unaffected. The ICPIP also registers the new
error codes these guarantees require.

## Motivation

ICP-1.0 ships a request/response protocol over an unreliable transport
(HTTP), but says nothing about what happens when a caller *retries*.

**`purchase.create` retries.** A buyer Agent POSTs a signed
`purchase.create` Intent, the merchant prices and returns a Quote, but the
TCP connection drops before the response arrives. The Agent's only safe
move today is to give up or to re-POST and hope. Re-POSTing currently mints
a *fresh* Quote with a new `quote_id`, a new `nonce`, and a new `exp` — two
live Quotes now exist for one buyer decision, and (worse) the §5.3 nonce
guard added in the reference handler will *reject* the retry outright as
`replay.nonce_seen` if the Agent re-sends byte-identical bytes. Neither
outcome is acceptable: a retry of a non-effecting price request should be
*idempotent*, returning the original Quote.

This is the same problem Stripe solved with `Idempotency-Key`; ICP already
carries a natural idempotency key — the ULID `intent_id` — but never
specified its retry semantics.

**`inventory.query` pagination.** §6.3 calls `inventory.query` "the
highest-call-count verb in the protocol" and the B2B adoption gate, and the
InventorySnapshot already carries `total_matching_skus` "for pagination
signaling." But there is no cursor. A procurement Agent querying a
50,000-SKU distributor catalog with `max_results: 100` gets the first 100
matches and no defined way to fetch SKUs 101–200. Offset pagination
(`?offset=100`) is fragile: if the catalog mutates between pages, items
shift and the Agent silently skips or double-counts SKUs. ICP needs a
*stable, opaque cursor* so multi-page catalog walks are correct.

## Specification

The keywords MUST, MUST NOT, SHOULD, SHOULD NOT, MAY are to be interpreted
per BCP 14.

### 1. Idempotent `purchase.create`

#### 1.1 Idempotency key

The idempotency key for a `purchase.create` Intent is its `intent_id`. The
idempotency *fingerprint* is the SHA-256 of the RFC 8785 (JCS) canonical
encoding of the Intent object — i.e. the exact bytes the signature already
covers (ICP-1.0 §5.1).

> Note: the fingerprint deliberately covers the *whole* Intent, including
> `nonce`, `iat`, and `exp`. A byte-identical retry therefore has an
> identical fingerprint; any field change (even just a fresh `nonce`)
> produces a different fingerprint and is treated as a new, conflicting
> request — see §1.3.

#### 1.2 Idempotent replay (same key, same fingerprint)

A merchant that has already produced a Quote for `intent_id = X` and later
receives another `purchase.create` Intent with `intent_id = X` whose
fingerprint matches the stored one MUST return the **originally issued
Quote** verbatim (same `quote_id`, `total`, `nonce`, `exp`, and
`signature`), with HTTP status `200`. The merchant MUST NOT mint a second
Quote and MUST NOT re-run pricing.

The idempotent-replay path takes precedence over the §5.3 nonce-replay
guard: when `intent_id` + fingerprint match a stored Quote, the request is
an idempotent retry, not a replay attack, and MUST NOT be rejected with
`replay.nonce_seen`. (Conversely, a *different* `intent_id` reusing a stale
`nonce` is still a `replay.nonce_seen` violation per §5.3.)

#### 1.3 Idempotency conflict (same key, different fingerprint)

If a `purchase.create` Intent reuses an `intent_id` the merchant has seen
but its fingerprint differs from the stored one, the merchant MUST reject it
with the new error code `idempotency.key_reused` (§3) and MUST NOT issue a
Quote. This prevents an Agent (or an attacker who captured an `intent_id`)
from silently mutating the terms of an in-flight purchase.

#### 1.4 Retention window

Merchants MUST honor idempotent replay for at least as long as the issued
Quote is valid (`Quote.exp`). Merchants SHOULD retain the
`intent_id → (fingerprint, Quote)` mapping for at least `Quote.exp + 86400s`
so that a late retry observes the conflict in §1.3 rather than being treated
as a brand-new Intent after the entry is evicted. After retention expires, a
re-sent `intent_id` MAY be treated as new; the §5.3 nonce window
(`max(iat + 86400s)`) bounds the practical exposure.

#### 1.5 Scope

Idempotency in this ICPIP is specified for `purchase.create` only, because
it is the canonical value-decision verb where a duplicate Quote is most
costly. The other read-only/non-effecting verbs (`inventory.query`,
`quote.request`) are naturally safe to repeat. The value-effecting verbs
`subscription.create`, `purchase.return`, and `payout.request` SHOULD adopt
the same `intent_id`-keyed idempotency in a follow-up ICPIP; their
authorization objects are already `intent_id`-stamped, so the extension is
mechanical. This ICPIP does not yet mandate it for them.

### 2. Cursor pagination for `inventory.query`

#### 2.1 Request

`inventory.query` gains one OPTIONAL field:

```jsonc
{
  "verb": "inventory.query",
  "v": "icp-1.0",
  // … existing fields …
  "max_results": 100,        // OPTIONAL, see §2.3 for bounds
  "cursor": "icpcur_…",       // OPTIONAL opaque cursor from a prior snapshot
  "nonce": "…", "iat": "…", "exp": "…"
}
```

- `cursor` is an **opaque** string. Buyers MUST treat it as a blob and MUST
  NOT parse, construct, or mutate it. Only a value returned by the same
  merchant in a prior InventorySnapshot's `next_cursor` (§2.2) is valid.
- When `cursor` is absent, the query returns the **first page**.
- When `cursor` is present, the query resumes **immediately after** the last
  item of the page that produced it.

#### 2.2 Response

The InventorySnapshot gains two OPTIONAL fields:

```jsonc
{
  "type": "inventory.snapshot",
  // … existing fields …
  "items": [ /* up to max_results items */ ],
  "total_matching_skus": 5031,   // total across ALL pages for this filter set
  "next_cursor": "icpcur_…",      // present iff more pages remain; else omitted/null
  "iat": "…",
  "signature": { /* merchant signature over the canonical snapshot */ }
}
```

- `next_cursor` is present (non-null) **iff** at least one more matching SKU
  exists beyond this page. Its absence (or `null`) signals the final page.
- `total_matching_skus` is the count across the entire result set for the
  query's filters, independent of pagination, so an Agent can size its walk
  up front. It MAY be approximate for very large catalogs; if approximate,
  the merchant SHOULD set it to a value `≥` the true count.

#### 2.3 Page-size bounds

- `max_results` MUST be a positive integer. A merchant MUST clamp it to its
  policy ceiling (the reference handler caps at **100**) and MUST NOT return
  more than `max_results` items in a single snapshot.
- `max_results <= 0` or a non-integer MUST be rejected with
  `format.bad_schema`.
- A merchant MAY return *fewer* items than `max_results` on a non-final page
  (e.g. an internal shard boundary); `next_cursor` — not item count —
  signals whether more pages remain.

#### 2.4 Stability and opacity

- A cursor encodes the merchant's resume position (e.g. the last
  `(sort_key, sku)` pair) plus the query's filter set. The merchant MUST
  reject a cursor presented with a *different* filter set
  (`skus`/`filters`) than the one that produced it, using
  `pagination.cursor_invalid` (§3).
- Pagination is **stable under insertion/deletion**: walking with cursors
  MUST NOT skip a SKU that was present for the entire walk, and MUST NOT
  return the same SKU twice, even if the catalog mutates between pages.
  Merchants achieve this with a keyset (seek) cursor over a stable sort
  (e.g. `sku` ascending), NOT an integer offset.
- Each page is an independently signed InventorySnapshot with its own
  `snapshot_id`, `snapshot_taken_at`, and `valid_until`. Prices on page *N*
  reflect the moment page *N* was taken, not the moment page 1 was taken;
  Agents MUST NOT assume cross-page price consistency.

#### 2.5 Cursor expiry

A merchant MAY expire cursors (recommended: `≥ 1h`). A presented cursor that
has expired MUST be rejected with `pagination.cursor_expired` (§3); the Agent
restarts the walk from the first page.

### 3. Error codes

This ICPIP adds the following codes to `schemas/error-codes.md` under two
new namespaces, `idempotency` and `pagination` (registered as frozen per the
error-codes registry conventions):

| Code | HTTP | When emitted |
|---|---|---|
| `idempotency.key_reused` | 409 | `intent_id` already seen with a *different* canonical fingerprint (§1.3) |
| `pagination.cursor_invalid` | 400 | `cursor` is malformed, not issued by this merchant, or presented with a different filter set (§2.4) |
| `pagination.cursor_expired` | 410 | `cursor` was valid but its server-side retention window elapsed (§2.5) |

The HTTP status mapping table in `error-codes.md` is extended with the
`idempotency.*` (→ 409) and `pagination.*` (→ 400, or 410 for
`*_expired`) prefixes.

## Rationale

### Why `intent_id` as the idempotency key, not a separate header?

ICP already mints a ULID `intent_id` per Intent and the signature already
binds it. Reusing it avoids a redundant `Idempotency-Key` header that could
drift from the signed body. The fingerprint check (§1.1) closes the obvious
attack: an `intent_id` alone is not enough; the *terms* must also match, or
the request is a conflict, not a replay.

### Why does idempotent replay outrank the §5.3 nonce guard?

A byte-identical retry is, by construction, a re-send of the same signed
nonce — which the §5.3 guard would otherwise reject. Without the §1.2
precedence rule, the very thing that makes a retry safe (identical bytes)
would make it fail. The precedence is safe because idempotent replay returns
the *already-issued* Quote: no new value decision is made, so no replay
window is bypassed. A nonce reused under a *different* `intent_id` (the
actual replay-attack shape) is unaffected and still rejected.

### Why opaque keyset cursors instead of `?offset=`/`?page=`?

Offset pagination is `O(n)` to seek and unstable: an insert before the
current offset shifts every later item, so the Agent skips a SKU; a delete
double-serves one. Keyset (seek) cursors over a stable sort are `O(log n)`
to resume and stable under concurrent mutation — the property §2.4 requires.
Making the cursor opaque lets merchants change their internal cursor
encoding (shard id, sort key, HMAC tag) without breaking clients, and lets
them bind the cursor to its originating filter set (§2.4) and sign/expire it
(§2.5).

### Why `total_matching_skus` MAY be approximate?

Exact counts over a 50k-SKU catalog with live filters can be expensive
(a full scan per query). Allowing an upper-bound approximation lets a
merchant serve the count cheaply while still letting the Agent size its
walk; `next_cursor` remains the authoritative "more pages?" signal, so an
approximate total never causes a missed or phantom page.

## Backwards Compatibility

Strictly additive:

- A `purchase.create` caller that never retries never observes idempotency
  behavior. A caller that *does* retry byte-identically now gets the
  original Quote instead of a `replay.nonce_seen` rejection — strictly
  better.
- An `inventory.query` caller that omits `cursor` gets the first page
  exactly as before. The new `next_cursor`/`total_matching_skus` fields are
  additive snapshot fields; per ICP-1.0's unknown-field rules, older buyers
  ignore them.
- No wire-breaking change; no major version bump required. Merchants that do
  not yet implement pagination simply never emit `next_cursor` (one page,
  capped at `max_results`), which is already the ICP-1.0 behavior.

## Security Considerations

- **Idempotency-key squatting.** An attacker who observes a victim's
  `intent_id` cannot mutate the order: a different fingerprint under the same
  `intent_id` is rejected with `idempotency.key_reused` (§1.3), and a
  byte-identical resend only ever returns the *victim's own* already-issued
  Quote — it reveals nothing new and effects nothing. The Quote was already
  disclosed to whoever holds the bytes.
- **Cursor as a capability.** An opaque cursor can leak query intent if
  guessable or forgeable. Merchants SHOULD make cursors unforgeable (e.g.
  HMAC the encoded position with a server key) so a cursor cannot be crafted
  to enumerate a catalog the buyer's filter set did not match. Binding the
  cursor to its filter set (§2.4) prevents filter-swap escalation.
- **Pagination DoS.** Deep walks consume merchant resources. Merchants
  SHOULD apply the existing §6.3 `rate.aid_quota_exceeded` per-AID limit to
  paginated walks and MAY cap total pages per cursor lineage.
- **Replay-window interaction.** §1.2's precedence is scoped to *matching
  fingerprints*; it does not widen the §5.3 nonce window for any
  non-identical message, so the replay surface is unchanged for attacks.

## Test Vectors

To be added to `icp-conformance/vectors/icp-1.0/06-idempotency-pagination/`
upon Final. Planned cases:

| Case | Input | Expected |
|---|---|---|
| idem-replay | two byte-identical `purchase.create` Intents | second returns the first's exact Quote (same `quote_id`) |
| idem-conflict | same `intent_id`, one field changed (fresh `nonce`) | second → `idempotency.key_reused` (409) |
| page-walk | `max_results: 2` over a 5-SKU catalog | 3 pages; `next_cursor` present on pages 1–2, absent on page 3; union = all 5 SKUs, no dupes |
| page-stable | delete a not-yet-returned SKU mid-walk | walk still returns each surviving SKU exactly once |
| cursor-filter-swap | valid cursor presented with a changed `filters` | `pagination.cursor_invalid` (400) |
| max-results-bad | `max_results: 0` | `format.bad_schema` (400) |

Two prototype implementations passing these vectors is the hard gate for
Final promotion.

## Reference Implementation

- **Handler-side**: `icp-handler` already clamps `max_results` at 100 and
  emits `total_matching_skus` (`backend-stub.mjs` `stubInventoryQuery`); the
  cursor + `next_cursor` and the `intent_id` idempotency store are the
  remaining work, tracked for the post-Final reference branch.
- **SDK-side**: `@stateset/icp-client` / `icp-client` (PyPI) /
  `stateset-icp-client` (cargo) gain a `queryInventoryAll(opts)` helper that
  transparently follows `next_cursor`, and a retry wrapper that resends the
  same signed `purchase.create` bytes on transport failure.

## References

- ICPIP-0001 (process)
- ICPIP-0005 (push channels — companion operational ICPIP; same structure)
- ICP-1.0 §5.1 (canonicalization / JCS), §5.3 (replay protection),
  §6.1 (`purchase.create`), §6.3 (`inventory.query`)
- RFC 8785 — JSON Canonicalization Scheme (JCS)
- Stripe `Idempotency-Key` semantics (prior art for retry idempotency)

## Copyright

This ICPIP is licensed under CC-BY-4.0.
