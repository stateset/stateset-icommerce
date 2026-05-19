# ICPIP-0005: Push Channels — Webhooks and Server-Sent Events

```
ICPIP:        0005
Title:        Push Channels (Webhooks + Server-Sent Events)
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions/5 (forthcoming)
Status:       Draft
Type:         Standards Track
Category:     Core
Created:      2026-05-12
Requires:     ICPIP-0001
Supersedes:   —
```

## Abstract

Adds **two normative push-channel mechanisms** to ICP-1.x for
merchant→Agent and Settler→Agent out-of-band event delivery:

1. **Webhooks** — HTTP POSTs from the merchant (or Settler) to an
   Agent-controlled URL, used when the Agent has a stable public
   endpoint.
2. **Server-Sent Events (SSE)** — long-lived streams the Agent opens
   to the merchant, used when the Agent is behind NAT, in a sandbox,
   or otherwise cannot expose an inbound URL.

Both channels carry the same **EventEnvelope** shape: a signed,
canonical, replay-resistant message describing a single state
transition. The two are wire-equivalent — Agents choose between them
based on deployment constraints, not protocol semantics.

Without push channels, every Agent has to poll, which is expensive,
high-latency, and operationally fragile. With them, ICP becomes
truly event-driven and reaches Stripe-tier reliability for
out-of-band notifications.

## Motivation

ICP-1.0 ships **seven** intent verbs covering buyer↔merchant
request/response flows. But many real commerce events happen
**out-of-band**, after the original Intent has returned its
synchronous response:

| Event class | Example trigger | Today's only path |
|---|---|---|
| Inventory | Price drops below Agent's target | Agent polls `inventory.query` |
| Settlement | Escrow `released` / `refunded` | Agent polls `/settlements/:id` |
| Dispute | Counterparty files dispute | Agent polls — or never finds out |
| Subscription | Card expiring, plan price change | No mechanism today |
| Compliance | KYB re-verification due | No mechanism today |
| Risk | Anomaly flagged on Agent's account | No mechanism today |
| Payout | Reserve released, balance available | No mechanism today |

For each of these, polling is wasteful and slow; without webhooks,
agents cannot economically participate at scale. ICPIP-0005 closes
that gap.

## Specification

### 1. Channel registration

#### 1.1 Webhook registration

Agents register a webhook URL by submitting a signed
`channel.register` Intent (new sub-verb under the existing intent
envelope shape):

```jsonc
{
  "v": "icp-1.0",
  "verb": "channel.register",
  "intent_id": "icp_int_…",
  "buyer": "aid:v1:zAgentXYZ",
  "merchant": "aid:v1:zMerchantABC",
  "settler": "settler:stateset.usdc.base-sepolia",
  "channel": {
    "type": "webhook",
    "url": "https://agent.example.com/icp/events",
    "event_filters": [
      "settlement.released",
      "escrow.refunded",
      "dispute.opened"
    ],
    "delivery": {
      "max_attempts": 8,
      "backoff": "exponential",
      "initial_delay_seconds": 5
    },
    "auth": {
      "scheme": "ed25519",        // or "hmac-sha256"
      "verifying_key_hex": "…32B"
    }
  },
  "expiry": "2027-05-12T00:00:00Z",
  "principal_binding": { /* … */ },
  "nonce": "…",
  "iat": "…",
  "exp": "…"
}
```

Merchant returns a signed `ChannelRegistration`:

```jsonc
{
  "channel": {
    "channel_id": "icp_ch_…",
    "type": "webhook",
    "registered_at": "…",
    "expires_at": "…",
    "events_registered": [ /* echoed filters */ ]
  },
  "signature": { "alg": "ed25519", "kid": "aid:v1:zMerchantABC", "sig": "…" }
}
```

#### 1.2 SSE registration

For Agents that cannot expose a public URL (browser extensions,
mobile apps, sandboxes), the same `channel.register` Intent is sent
with `channel.type = "sse"`. The merchant responds with a
short-lived **subscription token**:

```jsonc
{
  "channel": {
    "channel_id": "icp_ch_…",
    "type": "sse",
    "sse_endpoint": "https://shop.example.com/icp/v1/events/sse",
    "subscription_token": "…opaque 32B…",
    "token_ttl_seconds": 3600
  },
  "signature": { /* … */ }
}
```

The Agent then opens an SSE stream with the token in the
`Authorization: Bearer <token>` header (token MUST be rotated every
`token_ttl_seconds` via a refresh sub-verb).

### 2. EventEnvelope

Every pushed event — webhook payload OR SSE `data:` line —
carries this canonical envelope:

```jsonc
{
  "envelope": {
    "v": "icp-1.0",
    "event_id": "icp_evt_01HVZX…",
    "event_type": "settlement.released",
    "channel_id": "icp_ch_…",
    "sequence": 4719,                 // monotonic per channel
    "originated_at": "2026-05-12T15:22:09.000Z",
    "source": "aid:v1:zMerchantABC",  // or Settler AID for settler events
    "target": "aid:v1:zAgentXYZ",
    "payload": { /* event-type-specific */ },
    "previous_event_id": "icp_evt_01HVZW…",  // null on first event in channel
    "delivery_attempt": 1
  },
  "signature": { "alg": "ed25519", "kid": "aid:v1:zMerchantABC", "sig": "…" }
}
```

The signature is over the canonical JSON of `envelope` (RFC 8785).
The Agent MUST:

1. Verify `signature.sig` against the source's published Ed25519
   pubkey (resolved via `.well-known/icp` or DID document).
2. Verify `target` matches the Agent's own AID.
3. Verify `previous_event_id` matches the Agent's stored last-seen
   event for `channel_id` (gap → reconcile via the recovery API; see §5).
4. Reject duplicates (same `event_id` already seen).

### 3. Event types (initial set)

| Event type | Source | Trigger | Payload shape |
|---|---|---|---|
| `settlement.released` | Settler | Escrow released to merchant | `{settlement_id, amount, final_state: "released"}` |
| `settlement.refunded` | Settler | Escrow refunded to buyer | `{settlement_id, amount, final_state: "refunded", reason}` |
| `escrow.opened` | Settler | Funds locked in escrow | `{escrow_id, amount, opens_at}` |
| `dispute.opened` | Settler | Counterparty filed dispute | `{dispute_id, reason, evidence_required_by}` |
| `dispute.resolved` | Settler/arbiter | Dispute decided | `{dispute_id, resolution, winner}` |
| `subscription.charge_pending` | Merchant | T-72h before recurring charge | `{subscription_id, amount, charge_at}` |
| `subscription.canceled` | Merchant | Subscription terminated | `{subscription_id, effective_at, final_charge}` |
| `inventory.price_changed` | Merchant | SKU price changed | `{sku, old_price, new_price, effective_at}` |
| `inventory.stock_depleted` | Merchant | SKU sold out | `{sku, last_available_at}` |
| `payout.released` | Platform | Reserve released to seller | `{payout_id, amount, released_at}` |
| `compliance.kyb_due` | Merchant | Re-verification required | `{due_by, required_documents}` |
| `risk.flag` | Merchant | Risk anomaly on Agent account | `{flag_type, severity, recommended_action}` |

Future events MUST be added by ICPIPs that update this registry; the
registry itself lives at `icp-spec/registries/event-types.md` once
this ICPIP is Final.

### 4. Delivery semantics

#### 4.1 Webhook delivery

- Merchants MUST retry on transport failure (network error, TLS
  failure, 5xx response, or no 2xx within 10 seconds).
- Retry schedule: exponential backoff per Agent registration (default:
  5s, 10s, 20s, 40s, 80s, 160s, 320s, 640s — 8 attempts, ~20 min total).
- Retries reuse the same `event_id` and `delivery_attempt`
  increments by one — Agent dedupes on `event_id`.
- 4xx responses (except 408/429) are terminal: merchant marks the
  delivery as failed and emits `delivery.failed` to a configurable
  fallback channel (or DLQ).
- The merchant MUST sign each retry afresh — `delivery_attempt`
  changes the canonical bytes.

#### 4.2 SSE delivery

- Server emits `data: <EventEnvelope JSON>\n\n` lines.
- Server emits `:heartbeat\n\n` every 30 seconds to keep idle
  proxies from closing the connection.
- On reconnect, the Agent sends `Last-Event-ID: <event_id>` header;
  the server resumes from that point in the channel's event log.
- The subscription token MUST be refreshed before expiry via a
  separate POST to `/icp/v1/channels/:channel_id/refresh` (signed
  Intent).

### 5. Recovery and ordering

- Every channel has a **monotonic sequence**. Agents observing a gap
  MUST fetch missing events via
  `GET /icp/v1/channels/:channel_id/events?since=<sequence>`.
- Servers MUST retain the last N events per channel (recommended:
  N = 1000) for recovery; older events may require a different
  recovery path (audit log).
- Agents MUST treat out-of-order events as a protocol violation;
  the merchant is required to emit events in `sequence` order.

### 6. Security

- Every EventEnvelope is signed by the source's Ed25519 key. Agents
  MUST verify before processing.
- Webhook bodies MUST also be signed at the **HTTP layer** via either:
  - Ed25519: `X-ICP-Signature: ed25519=<base64-signature>` over
    `timestamp.method.path.body` (mirrors Stripe's webhook signing
    pattern but with Ed25519 instead of HMAC).
  - HMAC-SHA256: `X-ICP-Signature: hmac-sha256=<base64-mac>`, key
    rotation via a separate `channel.rotate_key` Intent.
- HTTP timestamps MUST be within ±5 minutes; older signatures are
  rejected as replay attempts.
- The body signature is REDUNDANT with the envelope signature but
  provides defense-in-depth at the HTTP layer and is what most
  webhook libraries already expect.

### 7. Error codes

This ICPIP adds the following codes to the `error-codes.md`
namespace:

| Code | HTTP | Meaning |
|---|---|---|
| `channel.not_found` | 404 | `channel_id` unknown |
| `channel.expired` | 410 | Channel TTL elapsed; re-register |
| `channel.signature_invalid` | 401 | HTTP-layer signature failed |
| `channel.replay` | 409 | Timestamp outside ±5 min window |
| `channel.sequence_gap` | 409 | Agent's `since` ahead of server's last `sequence` |
| `channel.token_expired` | 401 | SSE subscription token TTL elapsed |
| `channel.event_type_unsupported` | 422 | Filter requested an unknown event type |

## Rationale

### Why both webhooks AND SSE?

Different Agent deployment topologies have different inbound-connectivity
constraints:

- A **backend Agent** running on its own infrastructure can host
  a webhook URL — webhooks are cheaper for the merchant (no
  long-lived connections) and natural for high-throughput.
- A **browser-extension Agent** (Claude Desktop, ChatGPT Atlas)
  has no public URL — SSE is the only practical option.
- A **mobile Agent** typically uses webhooks via a push-notification
  proxy or SSE if foregrounded.

Specifying both means ICP works across the full Agent surface,
from datacenter to browser tab.

### Why per-event signing, not just HTTP signing?

HTTP layer signing protects against **transport tampering**
(MITM, replay). Envelope signing protects against **server-of-record
tampering** — even an Agent that stores received events for audit
can prove later that the merchant attested to each one. This is
crucial for dispute resolution and regulatory audit, where Agents
need to demonstrate they acted on signed merchant directives.

### Why monotonic `sequence` per channel, not global?

Per-channel monotonicity:
- Keeps the merchant's emit path lock-free per Agent.
- Lets Agents detect gaps without needing global cross-Agent ordering.
- Maps naturally to the recovery API (`?since=`).

### Why Ed25519 OR HMAC for HTTP signing?

Ed25519 is the natural choice given the rest of ICP, but HMAC-SHA256
is what most existing webhook libraries (Stripe, GitHub, Twilio)
expect — supporting both eases integration with existing webhook
infrastructure.

## Reference implementation (planned)

This ICPIP defines the wire spec. Reference implementations to be
shipped post-Final:

- **Handler-side**: extend `icp-handler/src/server.mjs` with
  `/icp/v1/channels` route group + emit on every state transition.
- **SDK-side**: extend `@stateset/icp-client`, `icp-client` (PyPI),
  and `stateset-icp-client` (cargo) with:
  - `registerWebhook(opts)` — submits the channel.register Intent
  - `subscribeSSE(opts)` — opens the SSE stream + handles reconnect
  - `verifyEvent(env)` — verifies the envelope + HTTP signature
- **Conformance vector 05**: `channel.event_verification` —
  fixed (channel_id, sequence, payload) tuples → expected canonical
  bytes + expected verification result per IUT.

## Test plan

When the reference impl lands:

| Test | Coverage |
|---|---|
| Webhook registration → signed Intent → ChannelRegistration | wire shape |
| Webhook delivery → 2xx → no retry | happy path |
| Webhook delivery → 500 → 8 retries with exponential backoff | failure path |
| Webhook delivery → 403 → terminal failure → DLQ | non-retryable |
| Webhook body signature verification (Ed25519) | security |
| Webhook body signature verification (HMAC) | security |
| SSE connection → 5 events → graceful disconnect | happy path |
| SSE reconnect with Last-Event-ID → resume from sequence | recovery |
| SSE token refresh → new token → continue stream | rotation |
| Sequence gap → recovery API call → backfill | recovery |
| Replay attack (re-deliver old `event_id`) → rejected | security |
| HTTP timestamp ±6min → rejected with `channel.replay` | security |
| Cross-IUT envelope verification (vector 05) | conformance |

## Backwards compatibility

This ICPIP is **strictly additive**. The existing SSE escrow event
stream (`GET /icp/v1/escrows/:id/events`) remains as-is for
backwards compatibility; it can be deprecated in ICP-1.2 once all
deployed handlers support the new channel-based path.

## Security considerations

- **Replay**: HTTP timestamp + event_id dedup gives layered
  protection.
- **MITM**: Both HTTP-layer and envelope-layer signatures must hold.
- **Webhook URL hijack**: If an Agent's webhook URL is compromised,
  events leak. Mitigation: short channel TTLs (recommend ≤ 30 days)
  + Agent-initiated re-registration with new URL signed by the same
  Agent identity.
- **Denial-of-Service**: Servers MUST rate-limit per-channel event
  emission (recommend 100 events/sec/channel default cap) and
  reject excess with `rate.too_many_requests`.
- **Trust transitivity**: An Agent that delegates webhook handling
  to a third party (e.g., serverless function) MUST ensure that
  function re-verifies the envelope signature before acting. The
  HTTP-layer signature alone is insufficient if the third party
  itself is in the trust boundary.

## Open issues

1. **Webhook authentication during channel.register**: should the
   merchant verify the URL is controlled by the Agent before
   delivering anything (e.g., ACME-style challenge)? Proposal: yes,
   require a `verification_token` GET round-trip before the channel
   is active. Open for discussion.
2. **Event-type registry governance**: who can add event types?
   Proposal: anyone via a sub-ICPIP that updates
   `registries/event-types.md`; merchants advertise supported types
   in `.well-known/icp`. Open for discussion.
3. **Cross-channel ordering**: do we need a global event sequence
   across channels per Agent? Probably not — per-channel is
   sufficient for state reconciliation. Open for discussion.

## Copyright

This ICPIP is licensed under CC-BY-4.0.
