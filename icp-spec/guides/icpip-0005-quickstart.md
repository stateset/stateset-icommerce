# ICPIP-0005 Quickstart — Push Channels in 5 Minutes

This is the **integration guide** for ICPIP-0005 push channels.
The full spec is at [`icpips/icpip-0005-push-channels.md`](../icpips/icpip-0005-push-channels.md);
this guide shows how to use it.

Three lines per language and you have webhook subscribe + verify +
recovery wired end-to-end. No hand-rolled Ed25519, no hand-rolled
canonical JSON, no hand-rolled retry/dedupe semantics — every
first-party SDK ships this as one-call methods.

## The pattern

ICPIP-0005 has exactly **three** developer-facing calls on the client side:

| Call | Purpose | When you call it |
|---|---|---|
| `registerWebhook(...)` | Subscribe to event types on a channel | Once, at startup |
| `verifyWebhook(...)` | Validate each inbound POST | Per webhook delivery |
| `fetchChannelEvents(...)` | Backfill missed events | After observing a sequence gap |

Two flows on the server side:

| Flow | Trigger | What it does |
|---|---|---|
| **Live emit** | A state transition (e.g. fulfill → settled) | Signs an `EventEnvelope` and POSTs it to every subscribed webhook with 8-attempt exponential-backoff retry on failure |
| **Recovery** | Client GETs `/icp/v1/channels/:id/events?since=N` | Returns every retained signed envelope with `sequence > N` |

That's the whole protocol surface. The rest of this guide is the
side-by-side code.

## Subscribe

### JavaScript (`@stateset/icp-client`)

```js
import { ICPClient } from '@stateset/icp-client';

const client = await ICPClient.create({
  handlerUrl: 'https://shop.example.com',
  principal: 'did:web:agent-org.example',
});

const caps = await client.capabilities();
const reg = await client.registerWebhook({
  merchant: caps.merchant_aid,
  settler: caps.settler_allowlist[0],
  url: 'https://agent.example.com/icp/events',
  event_filters: ['settlement.released', 'dispute.opened'],
});
// reg.channel.channel_id — store this; you'll need it for recovery.
```

### Python (`icp-client`)

```python
from icp_client import ICPClient

client = ICPClient.create(
    handler_url='https://shop.example.com',
    principal='did:web:agent-org.example',
)

caps = client.capabilities()
reg = client.register_webhook(
    merchant=caps['merchant_aid'],
    settler=caps['settler_allowlist'][0],
    url='https://agent.example.com/icp/events',
    event_filters=['settlement.released', 'dispute.opened'],
)
channel_id = reg['channel']['channel_id']
```

### Rust (`stateset-icp-client`)

```rust
use stateset_icp_client::{Client, Identity};

let client = Client::new("https://shop.example.com", Identity::generate());
let caps = client.well_known()?;
let merchant = caps["merchant_aid"].as_str().unwrap();
let settler = caps["settler_allowlist"][0].as_str().unwrap();

let reg = client.register_webhook(
    merchant,
    settler,
    "webhook",
    Some("https://agent.example.com/icp/events"),
    &["settlement.released", "dispute.opened"],
)?;
let channel_id = reg.payload["channel_id"].as_str().unwrap().to_string();
```

## Verify each inbound POST

Receive the raw HTTP request, then:

### JavaScript

```js
import { verifyWebhook, ICPError } from '@stateset/icp-client';

app.post('/icp/events', (req, res) => {
  try {
    const envelope = verifyWebhook({
      body: req.rawBody,            // raw string, NOT pre-parsed!
      headers: req.headers,
      method: 'POST',
      path: req.originalUrl,
      merchantPubkeyRaw: merchantPubkey,  // from /.well-known/icp
    });
    handleEvent(envelope);  // your code
    res.status(202).end();
  } catch (err) {
    if (err instanceof ICPError) {
      console.warn(`rejected: ${err.code}`);
      res.status(401).end();
    }
  }
});
```

### Python

```python
from icp_client import verify_webhook, ICPError

@app.post('/icp/events')
def handle(request):
    try:
        envelope = verify_webhook(
            body=request.body.decode('utf-8'),  # raw string
            headers=dict(request.headers),
            method='POST',
            path=request.path,
            merchant_pubkey_raw=merchant_pubkey,
        )
        handle_event(envelope)
        return ('', 202)
    except ICPError as e:
        return ('', 401)
```

### Rust

```rust
use stateset_icp_client::{verify_webhook, VerifyWebhookOptions, Error};

async fn handle(req: Request) -> impl Responder {
    let opts = VerifyWebhookOptions::default();  // ±300s tolerance
    let headers: Vec<(&str, &str)> = req.headers().iter()
        .map(|(k, v)| (k.as_str(), v.to_str().unwrap()))
        .collect();
    match verify_webhook(
        &req.body,
        headers,
        "POST",
        req.path(),
        &merchant_pubkey_hex,
        opts,
    ) {
        Ok(envelope) => { handle_event(envelope); HttpResponse::Accepted().finish() }
        Err(Error::Icp { code, .. }) => {
            tracing::warn!(?code, "webhook rejected");
            HttpResponse::Unauthorized().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
```

All three implementations perform **the same four checks** per
ICPIP-0005 §6:

1. HTTP timestamp within ±300s of `now` (else `channel.replay`).
2. HTTP-layer `X-ICP-Signature: ed25519=<hex>` verifies against
   `<timestamp>.<method>.<path>.<body>`.
3. Body parses as `{envelope, signature}`.
4. Envelope signature verifies against the merchant pubkey over the
   envelope's canonical JSON bytes.

Any failure throws/returns a **typed** error with a `channel.*` code
so you can map directly to HTTP status.

## Recovery after a gap

If your receiver observed envelope `sequence=5` then later receives
`sequence=7`, you missed event 6. Backfill it:

### JavaScript

```js
const missed = await client.fetchChannelEvents(channelId, 5);
// missed = [verified envelope with sequence=6, verified envelope with sequence=7]
for (const env of missed) handleEvent(env);
```

### Python

```python
missed = client.fetch_channel_events(channel_id, 5)
for env in missed:
    handle_event(env)
```

### Rust

```rust
let missed = client.fetch_channel_events(&channel_id, 5)?;
for env in missed {
    handle_event(&env);
}
```

The recovery API returns events in ascending `sequence` order, and the
SDK **verifies each envelope signature** against the cached merchant
pubkey before returning. If `since` is before the retained window
(default 1000 events per channel), you get a typed
`channel.sequence_gap` error — re-register the channel and resync from
your last known good state.

## Server-side (for merchants implementing ICPIP-0005)

The reference handler at [`icp-handler/`](../../icp-handler/) wires this
together. The full loop is:

```
state transition           channel store           emitter
─────────────────          ─────────────           ───────
handleFulfill()      →     channel_id_1   ────→    POST → 200 → done
                     →     channel_id_2   ────→    POST → 500 → retry…
                                                        retry(2)…
                                                        retry(3)… exhausted → recovery log
handleDispute()      →     channel_id_1   ────→    POST → 200 → done
```

State transitions fan out via `publishToSubscribers(channelStore, eventType, payload, opts)`.
Every signed envelope is also written to the per-channel ring buffer
before the network attempt — so the recovery API serves the
`delivery_attempt: 1` form, which is the canonical dedupe key
receivers expect.

### Adding a new state transition

When a new merchant-side state change should fire a webhook:

```js
// in your state transition handler:
publishToSubscribers(
  channelStore,
  'subscription.canceled',
  {
    subscription_id, effective_at, refund_amount, /* ... */
  },
  { signingKey: merchantKp.privateKey, sourceAid: merchantAid },
).catch((err) => console.error(`publish failed: ${err}`));
```

That's it. Subscribed channels receive the signed envelope with retry
+ recovery wired automatically.

## Reliability guarantees summary

| Property | Mechanism |
|---|---|
| **Per-channel ordering** | Monotonic `sequence` field, allocated in emit order |
| **Per-channel chain** | Each envelope's `previous_event_id` references the previous successful delivery |
| **Cryptographic attestation** | Every envelope signed by the merchant; every webhook body also signed at the HTTP layer (`X-ICP-Signature`) |
| **Replay defense** | ±300s timestamp window enforced by `verifyWebhook` |
| **Live delivery** | 8-attempt exponential-backoff retry on 5xx/timeouts; 4xx (except 408/429) terminal |
| **Backfill** | Recovery API serves last 1000 envelopes per channel; same canonical bytes the live delivery would have produced |
| **Deduplication** | Receivers dedupe on `event_id`; recovery always returns `delivery_attempt: 1` form so the dedupe key is stable |

## Conformance

Every SDK's `verifyWebhook` implementation produces byte-identical
verification results against the same `(canonical_bytes, signature_hex,
pubkey_hex)` triples. This is enforced by **conformance vector 03**
([`icp-conformance/vectors/icp-1.0/03-signature-verification/`](../../icp-conformance/vectors/icp-1.0/03-signature-verification/))
which exercises JS, Rust, Go, and Python implementations against 8
sub-cases (valid roundtrip + 7 deliberate negative cases). All 4 IUTs
return identical boolean arrays — if a future implementation diverges,
CI catches it before merge.

## Where to next

- **Full spec**: [`icp-spec/icpips/icpip-0005-push-channels.md`](../icpips/icpip-0005-push-channels.md)
- **OpenAPI** (for codegen partners): [`icp-handler/openapi.yaml`](../../icp-handler/openapi.yaml)
- **Reference handler**: [`icp-handler/src/`](../../icp-handler/src/)
  (look at `channel-emitter.mjs` for the retry + recovery
  implementation and `server.mjs` for the state-transition wiring)
- **JS SDK**: [`packages/icp-client/src/index.mjs`](../../packages/icp-client/src/index.mjs)
  (now with `.d.ts` declarations for TypeScript consumers)
- **Python SDK**: [`packages/icp-python-client/`](../../packages/icp-python-client/)
- **Rust SDK**: [`crates/stateset-icp-client/`](../../crates/stateset-icp-client/)

## License

CC-BY-4.0. Reuse freely.
