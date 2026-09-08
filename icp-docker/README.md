# icp-docker — Production deployment package

One-command Docker Compose stack that brings up the full ICP protocol layer
as it would run in production: merchant Backend (`icp-handler`), Settler
operator (`settler-stateset`) and chain watcher (`icp-chain-watcher`) as
separate containers with separate signing keys, separate ports, isolated
network, and proper healthchecks.

## Run it

```sh
# From the repo root
docker compose -f icp-docker/docker-compose.yml up -d

# Verify all four long-running services healthy
docker compose -f icp-docker/docker-compose.yml ps

# Hit the endpoints
curl http://127.0.0.1:8787/icp/v1/.well-known/icp | jq
curl http://127.0.0.1:8788/.well-known/icp-settler | jq
curl http://127.0.0.1:8789/healthz | jq   # chain-watcher forwarding metrics
curl http://127.0.0.1:8790/healthz | jq   # mock EVM RPC

# Run the integration test
node icp-docker/integration-test.mjs
```

**Expected**: all checks PASS, covering health checks, discovery, the full
purchase flow with independent signature verification, signature tampering
rejection, and an escrow driven from funded to settled entirely by on-chain
events observed through the watcher.

```sh
# Tear down
docker compose -f icp-docker/docker-compose.yml down
```

## Services

| Service | Port | Image | Role |
|---|---|---|---|
| `handler` | 8787 | `stateset/icp-handler:dev` | Merchant Backend (HTTP) |
| `settler` | 8788 | `stateset/settler-stateset:dev` | Settler operator (HTTP) |
| `chain-watcher` | 8789 | `stateset/icp-chain-watcher:dev` | Observes ICPEscrow logs, forwards to the Settler |
| `mock-rpc` | 8790 | `stateset/icp-mock-rpc:dev` | Test double for the chain (see below) |
| `mcp`     | stdio (on-demand) | `stateset/icp-mcp:dev` | MCP transport (stdio) |

The `mcp` service is built but not started by default (MCP uses stdio,
not network sockets). To use it:

```sh
docker compose -f icp-docker/docker-compose.yml run --rm mcp
```

## Image properties

- Base: `node:20-slim` (~180 MB after layers)
- Zero `npm install` — every protocol-layer component uses only Node
  stdlib (`node:http`, `node:crypto`, `node:readline`).
- Non-root user (`node`, UID 1000)
- Reproducible: same source → same image hash (no random timestamps in
  the build)
- Healthcheck on `/healthz` every 10s after 5s grace period
- `restart: unless-stopped` policy for every long-running service

The watcher builds from its own `services/icp-chain-watcher/Dockerfile`
(same base, same non-root user, same healthcheck script) because it is the
one component with an outbound dependency — an EVM RPC endpoint — and a
different operational shape: exactly one replica per chain, with a cursor
file it must not share.

## Architecture

```
                ┌──────────────────┐
   port 8787 ──▶│  icp-handler     │  merchant Backend
                │  /icp/v1/...      │
                │  signing key A    │
                └──────────────────┘
                ┌──────────────────┐
   port 8788 ──▶│  settler-stateset │  Settler operator
                │  /.well-known/... │
                │  /admin/escrow/...│
                │  signing key B    │
                └──────────────────┘
```

Two independent processes. Two independent keys. Communication between
them happens via the buyer Agent (the integration test, or a real
client). Neither needs to trust the other; both keys are independently
verifiable via their `.well-known/` discovery documents.

## Chain mode

The Settler daemon itself stays zero-dep and mock-mode: it only ever accepts
lifecycle transitions on `POST /admin/escrow/event`. Chain mode is the
`chain-watcher` sidecar deciding what gets posted there — it polls
`eth_getLogs`, decodes `ICPEscrow.sol` events, waits `FINALITY_BLOCKS` before
forwarding, and persists a cursor so a restart does not re-scan.

In this stack the watcher points at `mock-rpc` rather than Base Sepolia:

```
  ┌───────────┐  eth_getLogs   ┌─────────────────┐  POST /admin/     ┌──────────┐
  │ mock-rpc  │◀───────────────│  chain-watcher  │──escrow/event────▶│ settler  │
  │  :8790    │  (real ABI-    │     :8789       │                   │  :8788   │
  └───────────┘   encoded logs)└─────────────────┘                   └──────────┘
```

`mock-rpc` is a test double for the **chain only** — it serves genuinely
ABI-encoded `ICPEscrow` logs, which the watcher decodes with the same
decoder it would use against Base Sepolia, and the settler that receives
them is the real daemon. It seeds one finalized `EscrowFunded` log at
startup and accepts further events on `POST /admin/emit`:

```sh
curl -sS -X POST http://127.0.0.1:8790/admin/emit \
  -H 'content-type: application/json' \
  -d '{"event":"EscrowReleased","amount":"100000000"}' | jq
```

`icp-docker/integration-test.mjs` uses exactly that to drive an escrow from
`funded` (observed on chain) through `fulfilled` (off-chain merchant
evidence — `ICPEscrow.sol` emits no event for it) to `released`, then
verifies the settler's signed release event against the key in its
discovery document. `node --test icp-docker/test/mock-rpc.test.mjs` runs the
same path in-process, without Docker.

To point the watcher at a real chain, swap the two env vars in
`docker-compose.yml`:

```yaml
services:
  chain-watcher:
    environment:
      RPC_URL: "https://sepolia.base.org"
      CONTRACT_ADDRESS: "0x_DEPLOYED_ICPESCROW_"
```

and drop the `mock-rpc` service.

## Production-readiness checklist

This compose file is a **development-grade** deployment. For production:

- [ ] Replace `generateKeyPairSync` calls with KMS/HSM-backed keys
- [ ] Pin image tags to specific digests, not `:dev`
- [ ] Add an ingress / reverse proxy (Caddy / Traefik) with TLS
- [ ] Configure per-AID rate limits
- [ ] Add Prometheus metrics endpoint (each service exposes `/metrics`
      when `NODE_ENV=production` — TODO)
- [ ] Persist state to durable storage (Postgres) rather than in-memory
- [ ] Set up backups for SettlementReceipt records (7-year retention
      per SETTLERS.md §S.3)
- [ ] Run two-party signing for SettlementReceipts (merchant + Settler
      co-signature — currently the stub treats merchant as Settler too)
- [ ] Configure liveness/readiness probes for Kubernetes deployments
- [ ] Set resource limits (`mem_limit`, `cpus`) per service
- [ ] Enable Docker content trust + image signing
- [ ] Set up centralized logging (each service writes structured JSON to stderr)
- [ ] Configure cosign-signed releases

## Why minimum complexity matters

The protocol-layer Docker image is intentionally simple:
- Single Dockerfile, single base image
- No npm install, no build step
- Only the directories the runtime actually needs
- One healthcheck script shared across services

This is a deliberate choice. Production operators reviewing this stack
should see "small attack surface, easy to audit, easy to rebuild from
source." Every byte of dependency added to the image is a byte they
have to vet. The watcher's second Dockerfile follows the same rules — no
npm install, no build step, same base image and healthcheck script — and
`mock-rpc` ships in the shared image rather than pulling in a chain
simulator.
