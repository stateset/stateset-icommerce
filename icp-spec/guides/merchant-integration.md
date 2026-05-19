# Merchant Integration Guide

You sell things. You want to accept ICP-signed Intents from agent
buyers, hold escrow on a named Settler, and ship product. This guide is
the shortest path from "have a store" to "have a handler answering
`/icp/v1/intents`."

The normative spec is in [`ICP-1.0-DRAFT.md`](../ICP-1.0-DRAFT.md);
this is the operator-facing walkthrough.

## Two integration paths

| Path | Best when | Effort |
|---|---|---|
| **Deploy the reference handler** with a custom Backend adapter | You don't have an existing API gateway and want a turnkey ICP endpoint | An afternoon |
| **Mount the handler library** into your existing server | You already run a checkout API and want ICP as one more transport | A day |

Both end up at the same wire surface — the only thing that varies is
where the HTTP layer lives.

## Before you start

You need:

1. A **server with a stable HTTPS hostname** (e.g. `shop.example.com`).
   ICP clients fetch your `/.well-known/icp` capability document at this
   origin.
2. A way to **price line items** (existing pricing logic is fine; the
   handler calls into it).
3. A way to **decide what's "fulfilled"** (a shipment label, a digital
   delivery, a service activation — same as your existing checkout).
4. An allowlist of **Settlers you accept** (start with at least one;
   `settler:circle.usdc.base` is the reference Settler).

You do **not** need a wallet, a chain, or a treasury — value lives at
the Settler, not at the merchant.

## Step 1 — Generate your merchant identity

Every merchant is identified by an **AID** (Agent IDentifier) derived
from an Ed25519 signing key per [`ICP-1.0-DRAFT.md`](../ICP-1.0-DRAFT.md)
§4.2. Generate it once, store both keys in your secrets manager, never
rotate without a key-rotation event.

### JavaScript

```js
import { generateIdentity, identityFromSeeds } from '@stateset/icp-client';

const identity = generateIdentity();
console.log('AID:', identity.aid);
// Persist BOTH 32-byte seeds in your KMS — restore with identityFromSeeds().
const edSeedHex = identity.ed25519_seed.toString('hex');
const xSeedHex = identity.x25519_seed.toString('hex');
```

### Python

```python
from icp_client import generate_identity, identity_from_seeds

identity = generate_identity()
print('AID:', identity.aid)
# Persist BOTH 32-byte seeds in your KMS — restore with identity_from_seeds().
ed_seed_hex = identity.ed25519_seed.hex()
x_seed_hex = identity.x25519_seed.hex()
```

### Rust

```rust
use stateset_icp_client::Identity;

let identity = Identity::generate();
println!("AID: {}", identity.aid());
// Identity::generate() uses OsRng but does not expose the raw seeds — for
// persistence, generate seeds yourself (32 bytes each) and use
// Identity::from_seeds(&ed_seed, &x_seed) at startup.
```

The AID looks like `aid:v1:zMerchant…` and is the public identifier
you advertise on outreach pages, contracts, and the discovery document.

## Step 2 — Stand up the handler

Cheapest path: run [`icp-handler`](../../icp-handler/) directly with a
custom Backend.

```sh
git clone https://github.com/stateset/stateset-icommerce
cd stateset-icommerce/icp-handler
node src/server.mjs    # listens on 0.0.0.0:8787
```

That gives you a working handler with the stub Backend — useful for
the conformance suite, useless for real commerce. To wire it to your
catalog, implement the Backend interface (see
[`icp-spec/handler-design.md`](../handler-design.md) §"Default StateSet
backend adapter") and inject it:

```js
import { createServer } from './icp-handler/src/server.mjs';
import { ShopifyBackend } from './my-backends/shopify.mjs';   // your code

const server = createServer({
  backend: new ShopifyBackend({ shopifyDomain: 'shop.myshopify.com', adminToken: '...' }),
  merchantIdentity: identity,                // from Step 1
  allowedSettlers: ['settler:circle.usdc.base'],
});
server.listen(443);
```

The **Backend interface** has 5 methods (full Rust signatures in
[`handler-design.md`](../handler-design.md) §"The Backend trait"; the
JS reference handler uses the same shape):

| Method | Returns | When called |
|---|---|---|
| `quote(intent)` | `Quote` or `error` | On every `POST /icp/v1/intents` |
| `accept(quote_id, accept_envelope)` | `EscrowFunding` instructions | On `POST /icp/v1/quotes/:id/accept` |
| `fulfill(escrow_id, evidence)` | `FulfillmentReceipt` | On `POST /icp/v1/escrows/:id/fulfill` |
| `observe(intent_id)` | Stream of `EscrowEvent` | On `GET /icp/v1/escrows/:id/events` (SSE) |
| `dispute(escrow_id, dispute_intent)` | `DisputeOutcome` | On `POST /icp/v1/escrows/:id/dispute` |

`quote` and `fulfill` are the only ones you implement non-trivially.
`accept`, `observe`, and `dispute` can pass through to defaults.

## Step 3 — Pick your Settlers

Read [`icp-spec/SETTLERS.md`](../SETTLERS.md) §"Reference Settler
bindings" for the current allowlist. Each Settler operates on a
specific rail (USDC on Base, ACH via Stripe Treasury, etc.) and a
specific custodian (Circle, Stripe).

For each Settler you accept, you commit to:

- **Letting that Settler hold escrow on the buyer's behalf** between
  acceptance and release.
- **Trusting that Settler's SettlementReceipt** as proof of payment.

You do **not** custody value — the Settler does. You just publish the
allowlist on `/.well-known/icp` and the handler enforces it.

To start, allowlist one Settler. Add more as their conformance reports
land in [`icp-conformance/`](../../icp-conformance/).

## Step 4 — Advertise capabilities

The handler auto-publishes a discovery document at
`GET /icp/v1/.well-known/icp`. Verify it's reachable and signed:

```sh
curl https://shop.example.com/icp/v1/.well-known/icp | jq .
```

You should see your `merchant_aid`, `merchant_pubkey_hex`, accepted
Settlers, and supported verbs. If you don't, the handler isn't reachable
from the internet — fix DNS/TLS before continuing.

## Step 5 — Run the conformance suite

ICP-1.0 includes a black-box conformance suite that drives your handler
through the full Intent → Quote → Accept → Fulfill → SettlementReceipt
lifecycle plus negative cases (bad signature, disallowed Settler,
over-max-total). From a checkout of this repo:

```sh
cd icp-conformance
./runner/run.mjs --profile icp-1.0-core --iut https://shop.example.com
```

Three test categories run (`01-aid-derivation`, `02-canonical-json`,
`03-signature-verification`) plus the handler roundtrip. Green = your
handler is ICP-1.0 conformant.

Add this command to your CI so a regression fails the build, not your
buyers.

## Step 6 — Subscribe to push events (ICPIP-0005)

Once buyers start submitting Intents, you'll want webhooks instead of
polling. The handler already implements the publisher side; subscribe
agent-side per
[`icpip-0005-quickstart.md`](./icpip-0005-quickstart.md).

For your own ops dashboard, the handler also exposes Server-Sent Events
at `GET /icp/v1/escrows/:id/events` — useful for real-time fulfillment
UIs.

## Step 7 — Production checklist

Before flipping the DNS:

- [ ] Merchant signing key is in a KMS (not on disk in plaintext).
- [ ] Handler runs behind TLS — no plaintext HTTP for ICP traffic.
- [ ] At least one allowlisted Settler has a recent successful conformance
      report (`icp-conformance/runner` against that Settler's
      `.well-known/icp-settler`).
- [ ] Backend `fulfill()` is idempotent — the same `fulfill` POST must
      not double-ship.
- [ ] Backend `quote()` is deterministic for the same Intent — buyers
      verify quote hashes.
- [ ] Replay window enforced (handler default: 5 min).
- [ ] Max-total policy set (`policy.intent.max_total_usd`) so a buggy
      buyer can't auto-accept a $1M quote.
- [ ] OpenTelemetry traces emitted on every Intent (the reference handler
      does this by default; verify your custom Backend forwards them).
- [ ] Webhook delivery exponential-backoff configured (handler default:
      8 attempts).
- [ ] Recovery ring buffer sized for your expected event rate (handler
      default: 1000 events).

## Where to ask

- Spec questions → file an issue against
  [`stateset/stateset-icommerce`](https://github.com/stateset/stateset-icommerce)
  with label `icp-spec`.
- Conformance failures → label `icp-conformance` and attach the runner
  output.
- Settler operator questions → see
  [`icp-spec/SETTLERS.md`](../SETTLERS.md) and the per-Settler
  bindings in `icp-spec/settlers/`.

## Where to look next

- Full state machine — [`ICP-1.0-DRAFT.md`](../ICP-1.0-DRAFT.md) §7.
- Wire format — [`PACKET.md`](../PACKET.md).
- Error codes — [`schemas/error-codes.md`](../schemas/error-codes.md).
- End-to-end runnable demo —
  [`examples/02-end-to-end-flow/`](../examples/02-end-to-end-flow/).
- Push channels — [`icpip-0005-quickstart.md`](./icpip-0005-quickstart.md).
