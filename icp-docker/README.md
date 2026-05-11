# icp-docker — Production deployment package

One-command Docker Compose stack that brings up the full ICP protocol layer
as it would run in production: merchant Backend (`icp-handler`) and Settler
operator (`settler-stateset`) as separate containers with separate signing
keys, separate ports, isolated network, and proper healthchecks.

## Run it

```sh
# From the repo root
docker compose -f icp-docker/docker-compose.yml up -d

# Verify both services healthy
docker compose -f icp-docker/docker-compose.yml ps

# Hit the endpoints
curl http://127.0.0.1:8787/icp/v1/.well-known/icp | jq
curl http://127.0.0.1:8788/.well-known/icp-settler | jq

# Run the integration test
node icp-docker/integration-test.mjs
```

**Expected**: 17/17 PASS on the integration test, covering health
checks, discovery, the full purchase flow with independent signature
verification, and signature tampering rejection.

```sh
# Tear down
docker compose -f icp-docker/docker-compose.yml down
```

## Services

| Service | Port | Image | Role |
|---|---|---|---|
| `handler` | 8787 | `stateset/icp-handler:dev` | Merchant Backend (HTTP) |
| `settler` | 8788 | `stateset/settler-stateset:dev` | Settler operator (HTTP) |
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
- `restart: unless-stopped` policy for both services

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

## Chain mode (forthcoming)

The Settler service runs in mock mode by default — events injected via
`POST /admin/escrow/event`. To run in chain mode (when implemented):

```yaml
services:
  settler:
    environment:
      SETTLER_CHAIN_RPC: "https://sepolia.base.org"
      ICPESCROW_ADDRESS: "0x..."
      SETTLER_KEY_KMS_REF: "alias/icp-settler-prod"
```

The mock-mode admin endpoint disables itself in chain mode.

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

## Integration with the real chain

The `services/settler-stateset` Settler daemon is currently zero-dep and
runs in mock mode (events injected via admin endpoint). To wire it to
a real chain, add a separate `chain-watcher` service that:

1. Subscribes to `ICPEscrow.sol` events on Base Sepolia via JSON-RPC
2. Translates each on-chain event to the Settler's
   `POST /admin/escrow/event` format
3. POSTs to the Settler

This keeps the Settler daemon dep-free; viem (or any chain library) lives
in the watcher. Suggested implementation: 200 LOC TypeScript using `viem`,
deployed as a sidecar in this compose file.

## Why minimum complexity matters

The protocol-layer Docker image is intentionally simple:
- Single Dockerfile, single base image
- No npm install, no build step
- Only the directories the runtime actually needs
- One healthcheck script shared across services

This is a deliberate choice. Production operators reviewing this stack
should see "small attack surface, easy to audit, easy to rebuild from
source." Every byte of dependency added to the image is a byte they
have to vet.
