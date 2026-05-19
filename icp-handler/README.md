# icp-handler — Reference HTTP server for ICP-1.0

The minimum viable server that speaks the Intelligent Commerce Protocol.
**Zero external dependencies** (uses only `node:http` and `node:crypto`).
Runs on stock Node 20+.

## Why this exists

ICP-1.0 specifies a wire format and a state machine; `icp-spec/handler-design.md`
specifies an HTTP surface. This package implements that surface in the
smallest possible amount of working code so you can:

- `curl` real ICP endpoints in 30 seconds
- See the full Intent → Quote → Escrow → SettlementReceipt flow at HTTP
- Drop a real backend behind it via the `Backend` interface

The reference impl uses a **stub Backend** (in-memory state, auto-fulfillment).
A production deployment swaps in the real engine adapter described in
`icp-spec/handler-design.md` §"Default StateSet backend adapter".

## Run it

```sh
cd icp-handler
node src/server.mjs              # default port 8787
PORT=9000 node src/server.mjs    # custom port
```

You'll see:

```
icp-handler listening on http://127.0.0.1:8787
  merchant_aid: aid:v1:zMerchantHandlerInstance...
  merchant_pubkey_hex: ...
  allowed_settlers: settler:stateset.usdc.base-sepolia, settler:circle.usdc.base
```

## Test it

```sh
PORT=0 node --test test/roundtrip.test.mjs
```

The roundtrip test:
1. Generates a fresh buyer Agent (Ed25519 + X25519, AID per spec §4.2)
2. Builds and signs a real ICP-1.0 `purchase.create` Intent
3. POSTs it and asserts the Quote price (5% handling fee on $59.98 = $62.98)
4. Accepts the Quote, gets funding instructions
5. Fulfills the escrow, gets a co-signed SettlementReceipt
6. Re-fetches the SettlementReceipt by ID
7. Asserts bad signature, disallowed Settler, and over-max-total all fail
   with the correct spec-defined error codes

Result: **6/6 PASS** end-to-end.

## Surface

| Verb | Path | Purpose |
|---|---|---|
| GET  | `/icp/v1/.well-known/icp`         | Capability advertisement |
| GET  | `/icp/v1/settlers`                 | Accepted Settler allowlist |
| POST | `/icp/v1/intents`                  | Submit signed Intent, get Quote |
| POST | `/icp/v1/quotes/:id/accept`        | Accept a Quote, get funding instructions |
| POST | `/icp/v1/escrows/:id/fulfill`      | Submit fulfillment evidence (stub auto-releases) |
| POST | `/icp/v1/escrows/:id/dispute`      | Open a dispute |
| GET  | `/icp/v1/escrows/:id/events`       | Server-Sent Events stream of EscrowEvents |
| GET  | `/icp/v1/settlements/:id`          | Fetch a SettlementReceipt |
| GET  | `/healthz`                         | Liveness probe |

All bodies are JSON. Production handlers SHOULD additionally accept
`application/icp+cbor` per spec §5.1.

### OpenAPI

The full normative API surface is also published as
[`openapi.yaml`](./openapi.yaml) (OpenAPI 3.1.0). Use it to generate
clients in any language that lacks a hand-rolled SDK:

```sh
# Java
npx -y @openapitools/openapi-generator-cli generate -i openapi.yaml -g java -o /tmp/icp-java
# C#
npx -y @openapitools/openapi-generator-cli generate -i openapi.yaml -g csharp -o /tmp/icp-csharp
# Swift
npx -y @openapitools/openapi-generator-cli generate -i openapi.yaml -g swift5 -o /tmp/icp-swift
# Ruby / PHP / Kotlin / Dart / Elixir / Rust-server-stubs / etc.
```

The `test/openapi-sync.test.mjs` guard ensures the YAML can't drift
from the actual route registry — adding a route to `src/server.mjs`
without documenting it in `openapi.yaml` (or vice versa) fails CI.

## What the stub Backend does

- Quote pricing: sums `quantity × unit_price` per line, applies a flat 5%
  handling fee, rounds to 2dp.
- Settler allowlist: hardcoded `settler:stateset.usdc.base-sepolia` and
  `settler:circle.usdc.base`. Real deployments load this from a governance
  feed.
- Escrow: in-memory state. The `fulfill` path auto-funds and auto-releases
  for demo purposes — production waits for real on-chain confirmations
  from the Settler signing daemon.
- SettlementReceipt: co-signed by the merchant key (and the stub treats
  itself as the Settler too — production splits these).

The non-stub parts — signature verification, replay-window enforcement,
Settler allowlist gate, max_total ceiling, error codes — are **real**.
Drop in a real Backend and the security boundary is correct.

## Wire it up to the engine

The Backend interface (currently inline in `backend-stub.mjs`) is:

```js
quote(intent, signingKey) → { ok, quote, signatureHex } | { ok: false, error }
stubFundingInstructions(quote) → EscrowFunding
```

A real adapter calls `stateset-icommerce` crates instead:

```js
import { Engine } from '@stateset/embedded';
const engine = await Engine.open('store.db');

async function realQuote(intent, signingKey) {
  const draft = engine.orders.draftFromIntent(intent);
  const priced = await engine.pricing.price(draft);
  return engine.signing.signQuote(priced, signingKey);
}
```

The handler itself doesn't change — only the Backend.

## Where it sits in the stack

```
┌──────────────────────────────────────────────────┐
│ Agent (LangGraph, Anthropic SDK, OpenAI Agents)  │
└────────────────────┬─────────────────────────────┘
                     │ HTTP (this server)
                     ▼
┌──────────────────────────────────────────────────┐
│ icp-handler (THIS PACKAGE)                       │
│  - signature verification, allowlist, error model│
│  - HTTP / MCP / gRPC bindings                    │
└────────────────────┬─────────────────────────────┘
                     │ Backend trait
                     ▼
┌──────────────────────────────────────────────────┐
│ Commerce backend (stateset-icommerce, Saleor,    │
│   Medusa, in-house)                              │
└────────────────────┬─────────────────────────────┘
                     │ on-chain calls
                     ▼
┌──────────────────────────────────────────────────┐
│ Settler — Circle USDC on Base (ICPEscrow.sol)    │
└──────────────────────────────────────────────────┘
```

## Production checklist

- [ ] Replace `generateKeyPairSync` with a KMS-backed signing key
- [ ] Replace `_pubkey_hex` resolver with a real AID resolver
      (DNS-over-HTTPS, on-chain registry, or `.well-known/icp-agent`)
- [ ] Replace in-memory `state` module with the engine's persistent store
- [ ] Add per-AID rate limits (token bucket)
- [ ] OpenTelemetry traces (one span per protocol step)
- [ ] CBOR transport (`application/icp+cbor`) alongside JSON
- [ ] Connect to a real Settler signing daemon for actual chain events
      (the current implementation simulates funding and release)
- [ ] Replay nonce LRU cache, sized for `max(exp + 86400s)`

## Status

ICP-1.0 conformance: **partial** (purchase.create only). Tracks the spec.

This server demonstrates the protocol works end-to-end at HTTP. It is
NOT yet a production handler.
