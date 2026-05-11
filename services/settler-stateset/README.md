# settler-stateset

Reference Settler daemon for ICP-1.0. Implements the Settler side of
`icp-spec/SETTLERS.md`: signs ICP EscrowEvents at every state transition,
issues SettlementReceipts at terminal states, publishes the
`.well-known/icp-settler` discovery document, and serves the
proof-of-reserves attestation.

Operates the **bootstrap testnet Settler** at
`settler:stateset.usdc.base-sepolia` for demos, conformance testing,
and the first wave of design partners. Mainnet operation transfers to
Circle (or an equivalent regulated custodian) upon allowlist inclusion
of `settler:circle.usdc.base`.

## Modes

| Mode | Trigger | Behavior |
|---|---|---|
| **mock** (default) | unset `SETTLER_CHAIN_RPC` | Events injected via `POST /admin/escrow/event`. Useful for tests, demos, conformance dry-runs. |
| **chain** (future) | set `SETTLER_CHAIN_RPC` | Subscribes to `ICPEscrow.sol` events on Base Sepolia via JSON-RPC. Mock injection disabled. |

Only mock mode is implemented in this release. Chain mode hooks are in
place; finishing them is a future tick. (Roughly: replace the
`POST /admin/escrow/event` handler with a `viem`-based log subscriber.)

## Run

```sh
cd services/settler-stateset
node src/server.mjs              # default port 8788
PORT=9000 node src/server.mjs    # custom port
```

You'll see:

```
settler-stateset listening on http://127.0.0.1:8788
  settler_id:   settler:stateset.usdc.base-sepolia
  settler_kid:  settler-stateset-12345
  settler_pub:  d75a980182b10ab7...
  mode:         mock
```

## Test

```sh
PORT=0 node --test test/settler.test.mjs
```

9 tests covering discovery doc shape, signed event emission, **independent
signature verification with tampered-payload rejection**, full
lifecycle (fund → fulfill → release with SettlementReceipt), refund
path, dispute state-machine enforcement, and signed proof-of-reserves.
**9/9 PASS.**

## HTTP surface

| Verb | Path | Purpose |
|---|---|---|
| GET  | `/healthz`                                          | Liveness probe (+ snapshot counts) |
| GET  | `/.well-known/icp-settler`                          | Discovery doc per SETTLERS.md §S.1 |
| POST | `/admin/escrow/event`                               | Mock-mode event injection (chain-mode: 403) |
| GET  | `/icp/v1/escrows/:id`                               | Escrow state + full event log |
| GET  | `/icp/v1/escrows/:id/events`                        | Server-Sent Events stream of EscrowEvents |
| GET  | `/icp/v1/settlements/:id`                           | Co-signed SettlementReceipt |
| GET  | `/icp/v1/settlers/:id/proof-of-reserves`            | Signed POR attestation |

## Mock-mode event injection

The `POST /admin/escrow/event` endpoint accepts one of five `kind`
values, each of which corresponds to a rail-level event the daemon
would otherwise observe on-chain:

```sh
# fund event — creates the escrow record
curl -s -X POST http://127.0.0.1:8788/admin/escrow/event \
  -H 'content-type: application/json' \
  -d '{
    "escrow_id": "0xabc...",
    "kind": "fund",
    "init": {
      "intent_id": "icp_int_DEMO",
      "amount": {"amount":"100.00","currency":"USDC"}
    },
    "rail_event": {"rail":"base-sepolia","tx_hash":"0x..."}
  }'

# fulfill — merchant submitted fulfillment evidence
curl -s -X POST http://127.0.0.1:8788/admin/escrow/event \
  -H 'content-type: application/json' \
  -d '{"escrow_id":"0xabc...","kind":"fulfill","evidence_id":"icp_ful_xyz"}'

# release — dispute window elapsed, funds move on-chain to merchant
curl -s -X POST http://127.0.0.1:8788/admin/escrow/event \
  -H 'content-type: application/json' \
  -d '{"escrow_id":"0xabc...","kind":"release"}'
```

Each call returns the signed EscrowEvent. The daemon enforces the
state machine: invalid transitions (release before fulfill, fund
already-funded escrow) return `409 Conflict` with the proper
`escrow.wrong_state` error code per `error-codes.md`.

## How the Settler signs

1. Build the EscrowEvent payload (no `settler_signature` field yet).
2. Canonicalize via RFC-8785-compatible JSON ordering (same rule as
   the conformance suite vector 02).
3. Sign canonical bytes with Ed25519 using the Settler signing key.
4. Attach the signature: `{alg: "ed25519", kid: <settler_kid>, sig: <hex>}`.
5. Persist + fan-out to SSE subscribers + return to the caller.

A receiver can verify by:
1. Strip `settler_signature` from the received event.
2. Canonicalize the rest.
3. Verify the signature against the public key from the discovery doc.

The test suite includes a tampered-payload negative case proving this
property: change the `seq` field and verification fails.

## Production checklist (operator of a real Settler)

- [ ] Replace `generateKeyPairSync('ed25519')` with HSM-backed key
- [ ] Implement chain-mode log subscriber (replace mock POST endpoint)
- [ ] Persist events + receipts to durable storage (Postgres recommended)
- [ ] Hook real `ICPEscrow.sol` contract addresses
- [ ] Implement real Merkle proof-of-reserves with on-chain balance attestation
- [ ] Add per-AID rate limiting (token bucket)
- [ ] OpenTelemetry traces tagging every span with `escrow_id`
- [ ] WebSocket transport in addition to SSE for browser-incompatible clients
- [ ] Compliance pause endpoint with `PAUSER_ROLE` auth (mirrors ICPEscrow contract)
- [ ] Two-Foundation-member co-attestation before allowlist inclusion

## Status

**Reference implementation, mock mode complete.** Production deployment
gated on the production checklist above. Suitable today for:
- Conformance testing of any ICP implementation
- End-to-end demos
- Settler interface design reviews
- Testnet design-partner integrations
