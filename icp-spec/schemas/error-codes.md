# ICP-1.0 — Error Codes

Normative enumeration of ICP-1.0 error codes. Implementations **MUST**
use these codes in the `code` field of `icp.error` payloads. Codes are
dot-separated namespaces: `<top-level>.<specific>`.

A handler or backend that emits an error **MUST**:
1. Use one of the codes below.
2. Include a non-empty human-readable `message`.
3. OPTIONALLY include `intent_id`, `escrow_id`, `settlement_id` for
   correlation when known.
4. OPTIONALLY include `remediation` (a hint for the caller).
5. OPTIONALLY include `retry_after` (in seconds) when the error is
   transient.

Counterparties **MAY** rely on `code` for programmatic branching.
Counterparties **MUST NOT** rely on `message` for branching (it is for
humans).

## Format

```json
{
  "type": "icp.error",
  "code": "<namespace>.<specific>",
  "message": "<human-readable>",
  "intent_id":      <optional, string>,
  "escrow_id":      <optional, string>,
  "settlement_id":  <optional, string>,
  "remediation":    <optional, string>,
  "retry_after":    <optional, integer seconds>
}
```

## Namespaces

| Namespace   | Meaning                                                |
|-------------|--------------------------------------------------------|
| `auth`      | Identity/AID resolution and principal-binding errors  |
| `delegation`| PrincipalBinding verification (reference-handler refinement of `auth.principal_binding_*`) |
| `signature` | Cryptographic signature validation errors             |
| `replay`    | Nonce reuse and timestamp window violations           |
| `policy`    | Counterparty's policy rejection (allowlist, ceilings) |
| `quote`     | PriceProposal referencing errors (`from_proposal_id`, ICPIP-0006); the codes are listed with `policy` below |
| `format`    | Wire format / schema validation errors                |
| `version`   | Spec version incompatibility                          |
| `escrow`    | Escrow state machine errors                           |
| `inventory` | Merchant stock availability at reservation time       |
| `settlement`| Settlement / rail errors                              |
| `dispute`   | Dispute and arbitration errors                        |
| `rate`      | Rate-limit and quota errors                           |
| `settler`   | Settler-side operational errors                       |
| `arbiter`   | Arbiter authorization / decision errors               |
| `conformance` | Conformance test errors                             |
| `channel`   | Push-channel registration / delivery errors (ICPIP-0005) |
| `idempotency` | `purchase.create` idempotency-key conflicts (ICPIP-0006) |
| `pagination` | `inventory.query` cursor pagination errors (ICPIP-0006) |
| `internal`  | Counterparty-side failure with no protocol cause; the request was well-formed and authorized |

## Codes (ICP-1.0 normative)

### auth

| Code                          | When emitted                                         |
|-------------------------------|------------------------------------------------------|
| `auth.aid_resolution_failed`  | Cannot resolve AID to a public key                   |
| `auth.aid_key_mismatch`       | Supplied public key differs from the key an operator-owned source already binds to this AID |
| `auth.aid_unregistered`       | Signer AID is neither operator-registered nor previously pinned; an operator-keyed handler does not admit caller-minted AIDs |
| `auth.buyer_mismatch`         | `purchase.create` signer is not the Intent's `buyer` |
| `auth.acting_party_mismatch`  | Signer of any other verb is not the Intent's acting party — its `buyer`, or its `seller` for the inverted-direction `payout.request` |
| `auth.acceptance_invalid`     | Quote acceptance is not signed by the original buyer |
| `auth.signer_capacity`        | Handler is at its signer-admission bound (503, not 401/403 — it is capacity, not authorization) |
| `auth.principal_binding_invalid` | PrincipalBinding signature does not verify. *The reference handler emits the finer-grained `delegation.signature_invalid` / `delegation.signature_missing`.* |
| `auth.principal_binding_expired` | PrincipalBinding `expiry` is in the past. *The reference handler emits `delegation.expired`.* |
| `auth.principal_binding_revoked` | PrincipalBinding revocation endpoint says revoked |
| `auth.authority_insufficient` | Intent value exceeds PrincipalBinding's `max_per_intent` or `max_per_period` |
| `auth.verb_not_authorized`    | Intent verb not in PrincipalBinding's `authority.verbs` |
| `auth.counterparty_not_allowed` | Counterparty AID not in `allowed_counterparties` |

### delegation

Principal delegation (§4.4) as the reference handler enforces it. These
refine `auth.principal_binding_*`: they distinguish *no binding* from *a
binding this handler cannot verify* from *a binding that does not cover this
request*, which callers need in order to know whether to re-delegate, ask the
operator to register a key, or stop. Implementations that emit only the
ICP-1.0 `auth.principal_binding_*` codes remain conformant; a caller
encountering a `delegation.*` code SHOULD treat it as an `auth.*` failure.

A handler resolves the principal's public key from **operator configuration
only** (`ICP_PRINCIPAL_KEYS_JSON` in the reference handler). Key material in
the request body is never trusted — that is what `untrusted_key_material`
refuses.

| Code                              | When emitted                                              |
|-----------------------------------|-----------------------------------------------------------|
| `delegation.required`             | Handler requires a `principal_binding` and the Intent carries none (or none naming a principal) |
| `delegation.principal_unknown`    | No operator-configured key for the named principal        |
| `delegation.untrusted_key_material` | Request nominated the principal's key (e.g. `_principal_pubkey_hex`); principal keys come from operator configuration only |
| `delegation.signature_missing`    | Binding carries no `signature.sig`                        |
| `delegation.signature_invalid`    | Binding signature fails under the operator-configured principal key |
| `delegation.scope_mismatch`       | Binding does not name the Intent's acting Agent (`buyer`, or `seller` for the inverted-direction `payout.request`), or its `authority.verbs` omits the Intent's verb |
| `delegation.expired`              | Binding's `expiry` is in the past                         |

The signing input is the canonical JSON of the binding with its `signature`
field removed, so `expiry` and `authority` are covered: widening either after
signing yields `delegation.signature_invalid`.

### signature

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `signature.invalid`        | Ed25519 verification failed                              |
| `signature.algorithm_unsupported` | Signature `alg` not supported by this implementation |
| `signature.hybrid_incomplete` | Hybrid (`ed25519+ml-dsa-65`) signature missing one component |
| `signature.malformed`      | Signature bytes not the expected length or shape         |
| `signature.kid_unknown`    | Signature `kid` does not match a known key for the AID  |

### replay

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `replay.nonce_seen`        | Nonce already used within the protocol window            |
| `replay.intent_seen`       | `intent_id` already carries state from an earlier submission; Intent identity is immutable, so a fresh nonce cannot rebind it (409) |
| `replay.expired`           | `exp` is in the past                                     |
| `replay.window_too_long`   | `exp - iat` exceeds the spec maximum (600s for Intents, 86400s otherwise) |
| `replay.iat_in_future`     | `iat` is more than allowed clock skew in the future      |
| `replay.timestamp_malformed` | `iat`/`exp` is not RFC 3339                            |

### policy

| Code                            | When emitted                                          |
|---------------------------------|-------------------------------------------------------|
| `policy.settler.not_allowed`    | Named Settler not on counterparty's allowlist        |
| `policy.settler.exposure_exceeded` | Aggregate escrow with this Settler exceeds policy cap |
| `policy.quote.exceeds_max_total` | Quote total exceeds Intent's `max_total`            |
| `policy.por_required`           | Settler proof-of-reserves attestation too old        |
| `policy.value_above_kyc_floor`  | Intent value > $10k without KYC on principal         |
| `policy.value_above_kyb_floor`  | Intent value > $10k without KYB on principal         |
| `policy.cross_border_restricted` | Buyer and merchant jurisdictions are incompatible   |
| `policy.return.window_expired`  | Original settlement older than merchant return window (per §6.2) |
| `policy.return.not_eligible`    | Item category not eligible for return per merchant policy |
| `policy.return.already_returned` | Line item already covered by a prior ReturnAuthorization |
| `policy.return.exceeds_max_refund` | Refund amount exceeds Intent's `max_refund` ceiling |
| `policy.return.original_disputed` | Original settlement is in a disputed state — resolve dispute first |
| `policy.subscription.not_found` | Subscription_id not recognized by merchant |
| `policy.subscription.not_cancellable` | Merchant policy prohibits mid-cycle cancellation |
| `policy.subscription.outside_window` | Cancellation request after merchant_terms.cancellation_notice_period |
| `policy.subscription.already_cancelled` | (Informational; conformant merchants return the existing CancellationAuthorization instead of erroring) |
| `policy.quote.not_available_for_quantity` | quote.request quantity falls outside merchant's quotable range |
| `policy.quote.sku_not_quotable` | SKU is fixed-price catalog only; merchant doesn't quote it |
| `policy.quote.window_too_long` | Buyer's `purchase_window` exceeds merchant's policy ceiling |
| `quote.proposal_not_found` | `from_proposal_id` in purchase.create doesn't match any issued proposal |
| `quote.proposal_expired` | Proposal exists but `valid_until` is in the past |
| `quote.proposal_total_mismatch` | `max_total` in purchase.create doesn't match proposal.total when `from_proposal_id` is set |
| `policy.payout.insufficient_balance` | payout.request amount exceeds seller's available balance |
| `policy.payout.hold_period_active` | Funds still within compliance hold window |
| `policy.payout.exceeds_max_per_payout` | Request exceeds PrincipalBinding's `max_per_payout` cap |
| `policy.payout.exceeds_max_per_period` | Cumulative period payouts exceed `max_per_period` |
| `policy.payout.kyc_required` | Seller hasn't completed KYC; required above threshold |
| `policy.payout.destination_not_allowlisted` | Destination wallet/account not pre-registered |
| `policy.payout.rail_unavailable` | Named Settler doesn't support the requested rail |
| `policy.payout.expedited_unavailable` | Expedited payout not offered for this seller/amount |
| `policy.payout.compliance_hold` | Account under SAR/fraud review; payouts paused |
| `policy.payout.platform_not_allowed` | Platform AID not in `allowed_platforms` |

### format

| Code                          | When emitted                                          |
|-------------------------------|-------------------------------------------------------|
| `format.bad_json`             | Request body is not valid JSON                       |
| `format.bad_cbor`             | Request body is not valid CBOR (reserved icp-1.1 binary profile) |
| `format.canonicalization_failed` | Canonicalization rule violated (e.g. unsorted keys, indefinite-length CBOR) |
| `format.missing_field`        | Required field absent                                |
| `format.unknown_verb`         | Intent `verb` not implemented or not recognized      |
| `format.unknown_intent`       | `intent_id` not found                                |
| `format.unknown_quote`        | `quote_id` not found or wrong intent_id              |
| `format.unknown_escrow`       | `escrow_id` not found                                |
| `format.unknown_settlement`   | `settlement_id` not found                            |
| `format.unknown_route`        | HTTP path not recognized                             |
| `format.bad_timestamp`        | Timestamp not RFC 3339                               |
| `format.bad_aid`              | AID does not match `aid:v1:z…` regex or fails Base58btc decode |
| `format.bad_settler_id`       | SettlerID does not match `settler:<rail>.<asset>.<network>` |
| `format.bad_money`            | `Money.amount` is not a valid decimal string         |
| `format.invalid_money`        | Money in the request cannot be priced exactly — the reference handler's spelling of the `format.bad_money` case it detects while quoting (422, since it is the backend refusing a well-formed request) |
| `format.bad_query_param`      | Query-string value is not of the required type or range (e.g. `?since=` is not a non-negative integer) |
| `format.unknown_channel_type` | `channel.type` is neither `webhook` nor `sse` (422 — the merchant understood the value and refused it, rather than failing to route the request) |
| `format.bad_currency`         | Unknown currency / not ISO 4217 + canonical tickers |
| `format.bad_schema`           | Value does not match its JSON Schema                 |

### version

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `version.unsupported`      | `v` field not supported by this implementation           |
| `version.deprecated`       | `v` field supported but in deprecation window            |
| `version.too_new`          | Counterparty advertises a newer spec version             |

### escrow

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `escrow.wrong_state`       | Operation not valid from current escrow state            |
| `escrow.already_funded`    | Escrow already in `funded` state, cannot re-fund        |
| `escrow.expired`           | Escrow's fulfillment deadline + dispute window elapsed without action |
| `escrow.amount_mismatch`   | On-chain funded amount does not match Quote total       |
| `escrow.not_found`         | Escrow ID exists in no Settler's ledger                 |
| `escrow.seq_out_of_order`  | EscrowEvent `seq` is not strictly monotonic              |

### settlement

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `settlement.receipt_signature_missing` | Receipt has fewer than 2 signatures             |
| `settlement.settler_signature_invalid` | Receipt's `settler_signature` fails verification (the `merchant_signature` failure case is `signature.invalid` from the generic namespace) |
| `settlement.rail_failed`   | Settlement rail returned failure or never finalized      |
| `settlement.rail_unsupported` | Rail named in Intent not supported by this Settler    |
| `settlement.not_found`     | `settlement_id` exists nowhere in this Settler's records |
| `settlement.amount_mismatch` | Receipt amount does not match escrow amount            |
| `settlement.escrow_mismatch` | Receipt names an escrow that does not belong to its `intent_id` |
| `settlement.already_settled` | Escrow already settled under a different `settlement_id` — one escrow settles once |
| `settlement.not_final`     | Receipt's `final_state` is not a terminal state, so it cannot be co-signed |

### inventory

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `inventory.insufficient`   | Quoted stock is no longer available at reservation time — the Quote was valid when issued and someone else took the units first (409) |

### internal

Emitted when the counterparty failed for a reason the protocol does not model:
the request was well-formed, authenticated and authorized, and the failure is
the counterparty's own. It carries no diagnostic detail on purpose. A caller
SHOULD retry with the same identity and the same Intent, which is safe because
`intent_id` is idempotent.

| Code                          | When emitted                                            |
|-------------------------------|---------------------------------------------------------|
| `internal.transaction_failed` | A state transition failed or was rolled back; nothing was committed (500) |

### dispute

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `dispute.window_closed`    | Dispute window has elapsed                               |
| `dispute.already_resolved` | Escrow already moved to a terminal state                 |
| `dispute.unauthorized_party` | Caller is neither buyer nor merchant                   |
| `dispute.evidence_required` | Tier 2 arbitration requires evidence not provided      |

### arbiter

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `arbiter.unauthorized`     | Caller does not hold ARBITER_ROLE                        |
| `arbiter.beneficiary_invalid` | Arbiter directed funds to a non-party address          |
| `arbiter.quorum_not_met`   | Multi-arbiter decision lacks required signatures         |

### rate

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `rate.aid_quota_exceeded`  | Per-AID rate limit exceeded                              |
| `rate.global_quota_exceeded` | Global handler rate limit exceeded                     |
| `rate.settler_tps_exceeded` | Settler-side TPS cap                                    |

### settler

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `settler.unavailable`      | Settler endpoint unreachable                             |
| `settler.paused`           | Settler in compliance-pause state                        |
| `settler.por_stale`        | Settler proof-of-reserves older than allowed             |
| `settler.por_failed`       | POR arithmetic check failed (reserves < open escrows)    |
| `settler.key_unavailable`  | Counterparty holds no operator-configured key for the named Settler and so cannot co-sign; the key is never read from the receipt being authorized (503) |
| `settler.key_invalid`      | Operator-configured Settler key is present but unusable (wrong length, or not Ed25519) |

### conformance

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `conformance.unsupported`  | IUT cannot perform this test (returns SKIP)              |
| `conformance.divergence`   | Output differs from expected                             |

### channel (ICPIP-0005)

| Code                              | When emitted                                              |
|-----------------------------------|-----------------------------------------------------------|
| `channel.not_found`               | `channel_id` unknown                                      |
| `channel.expired`                 | Channel TTL elapsed; re-registration required             |
| `channel.signature_invalid`       | HTTP-layer or envelope-layer signature failed             |
| `channel.replay`                  | Webhook timestamp outside ±5 min window                   |
| `channel.sequence_gap`            | Recovery `?since=` is ahead of server's last `sequence`   |
| `channel.token_expired`           | SSE subscription token TTL elapsed                        |
| `channel.event_type_unsupported`  | Filter requested an unknown event type                    |
| `channel.url_unverified`          | Webhook URL failed verification challenge                 |
| `channel.durable_delivery_unavailable` | Handler is running durable state with no durable delivery backend, so it refuses a channel it could not replay (503) |

### idempotency (ICPIP-0006)

| Code                       | When emitted                                              |
|----------------------------|-----------------------------------------------------------|
| `idempotency.key_reused`   | `purchase.create` `intent_id` already seen with a different canonical fingerprint |

### pagination (ICPIP-0006)

| Code                        | When emitted                                             |
|-----------------------------|----------------------------------------------------------|
| `pagination.cursor_invalid` | `inventory.query` `cursor` is malformed, not issued by this merchant, or presented with a different filter set |
| `pagination.cursor_expired` | `inventory.query` `cursor` was valid but its retention window elapsed |

## HTTP status mapping

For HTTP transports, error codes map to status codes as follows:

| Code prefix    | HTTP status |
|----------------|-------------|
| `auth.*`       | 401 / 403 (503 for `auth.signer_capacity`) |
| `delegation.*` | 401 (signature) / 403 (missing, unknown, scope, expiry) / 400 (`untrusted_key_material`) |
| `signature.*`  | 401         |
| `replay.*`     | 400 (or 410 for `replay.expired`) |
| `policy.*`     | 422 (semantic policy reject) or 403 (authorization) |
| `quote.*`      | 422         |
| `format.*`     | 400 (or 404 for `format.unknown_*`; 422 where the merchant understood the value and refused it — `format.invalid_money`, `format.unknown_channel_type`) |
| `version.*`    | 400         |
| `escrow.*`     | 409 (state conflict) or 404 |
| `inventory.*`  | 409 (another buyer took the units) |
| `settlement.*` | 404 / 500   |
| `dispute.*`    | 409 / 403   |
| `arbiter.*`    | 403         |
| `rate.*`       | 429         |
| `settler.*`    | 503         |
| `channel.*`    | 401 / 404 / 409 / 410 / 422 (see per-code table) |
| `idempotency.*` | 409 |
| `pagination.*` | 400 (or 410 for `pagination.cursor_expired`) |
| `internal.*`   | 500         |

## Stability

This enumeration is **frozen for the ICP-1.0 major version**. New codes
MAY be added in ICP-1.1+; existing codes' meanings **MUST NOT** change
within a major version. Removing or renaming a code is a major bump.

Implementations encountering an unknown `code` SHOULD log the error,
treat it as best they can based on the namespace prefix, and continue.
Unknown codes are a forward-compatibility hazard but not a hard failure.
