# Anchor Service

The anchor service is a Rust daemon that bridges the StateSet Sequencer to the SetRegistry smart contract on SET Chain. It polls for pending commitments and submits them on-chain, converting soft-final sequencer receipts into hard-final on-chain commitments.

## How It Works

```
Sequencer                  Anchor Service              SET Chain
    │                           │                          │
    │◄─ 1. Poll /commitments/  ─│                          │
    │      pending              │                          │
    │                           │                          │
    │─ 2. Return pending ──────►│                          │
    │    batches                 │                          │
    │                           │─ 3. commitBatch() ──────►│
    │                           │   (SetRegistry)          │
    │                           │                          │
    │                           │◄─ 4. tx receipt ────────│
    │                           │                          │
    │◄─ 5. Mark as anchored ───│                          │
    │   (tx hash)               │                          │
```

The anchor service runs on a 60-second polling interval, batching commitments that meet the minimum event threshold (default: 100 events per batch).

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SEQUENCER_API_URL` | — | Sequencer API endpoint |
| `L2_RPC_URL` | — | SET Chain RPC endpoint |
| `SET_REGISTRY_ADDRESS` | — | SetRegistry contract address |
| `SEQUENCER_PRIVATE_KEY` | — | Authorized sequencer Ed25519 key |
| `ANCHOR_INTERVAL_SECS` | `60` | Polling interval |
| `MIN_EVENTS_PER_BATCH` | `100` | Minimum events before anchoring |

## Circuit Breaker

The anchor service includes a circuit breaker for resilience:

```
Closed (normal) ──[5 consecutive failures]──► Open (halt)
                                                  │
                                             [60s timeout]
                                                  │
                                                  ▼
                                             Half-Open (probe)
                                                  │
                                          [success] → Closed
                                          [failure] → Open
```

The circuit breaker monitors:
- Gas price spikes (pauses anchoring if gas exceeds threshold)
- RPC connectivity (retries with exponential backoff)
- Contract authorization (verifies sequencer is still authorized)
- Contract uptime (checks SetRegistry is responding)

## Health Endpoints

| Endpoint | Description |
|----------|-------------|
| `/health` | Basic liveness check |
| `/ready` | DB + RPC connectivity |
| `/metrics` | Prometheus metrics |
| `/stats` | Anchoring statistics (batches submitted, events anchored, gas spent) |

## Gas Economics

| Metric | Value |
|--------|-------|
| `commitBatch` gas | 60–80k per batch |
| Cost per batch | ~$0.08 |
| Events per batch | 100–1,000 |
| Cost per event | ~$0.0008–$0.00008 |

Gas costs are amortized across all tenants in a batch. Higher event throughput reduces per-event anchoring cost.

## Running the Anchor Service

### Local Development

```bash
cd /path/to/set/anchor
cargo run --release
```

### Production (Docker)

```bash
docker run -e SEQUENCER_API_URL=https://sequencer.stateset.com \
           -e L2_RPC_URL=https://rpc.stateset.zone \
           -e SET_REGISTRY_ADDRESS=0x... \
           -e SEQUENCER_PRIVATE_KEY=0x... \
           stateset/anchor:latest
```

### Kubernetes

The anchor service includes liveness and readiness probes:

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8081
readinessProbe:
  httpGet:
    path: /ready
    port: 8081
```

## Finality Progression

After the anchor service submits a commitment:

```
Soft finality (sequencer receipt)     → milliseconds
Anchor submitted (L2 transaction)     → ~2 seconds (SET Chain block time)
L2 finality (confirmed on SET Chain)  → ~4 seconds
L1 finality (posted to Ethereum)      → ~12 minutes (OP Stack batch interval)
Challenge period complete             → ~7 days (OP Stack security window)
```

For most commerce use cases, L2 finality (~4 seconds) provides sufficient settlement guarantees. The 7-day challenge period is a background security property inherited from the OP Stack.
