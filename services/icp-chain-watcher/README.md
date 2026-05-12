# icp-chain-watcher

Zero-dep Node.js service that closes the **chain-mode** gap for the ICP
Settler.

The settler-stateset daemon by default runs in **mock mode** — events
are injected via `POST /admin/escrow/event`. In production, those events
should reflect what actually happened on-chain. This watcher is the
bridge:

```
   Base Sepolia               settler-stateset
   ICPEscrow.sol               (mock or chain mode)
        │                              │
        │  EscrowFunded /              │
        │  EscrowDisputed /            │
   ┌────┤  EscrowReleased / ...        │
   │    │                              │
   │  ┌─┴────────────────┐             │
   │  │ icp-chain-       │ POST /admin │
   │  │   watcher        │─────────────▶
   │  │                  │ /escrow/event
   │  │  - eth_getLogs   │             │
   │  │  - ABI decode    │             │
   │  │  - finality lag  │             │
   │  │  - state file    │             │
   │  └──────────────────┘             │
   └───── poll every 12s ──────────────┘
```

The daemon stays zero-dep; the watcher is its own process with its own
deps (which today are also zero — pure `fetch` + manual ABI decoding).

## Run

```sh
RPC_URL=https://sepolia.base.org \
CONTRACT_ADDRESS=0x... \
SETTLER_URL=http://127.0.0.1:8788 \
node src/server.mjs
```

Configuration (env vars):

| Var | Default | Purpose |
|---|---|---|
| `RPC_URL` | _(required)_ | EVM JSON-RPC endpoint |
| `CONTRACT_ADDRESS` | _(required)_ | Deployed ICPEscrow.sol address |
| `SETTLER_URL` | _(required)_ | settler-stateset HTTP base URL |
| `START_BLOCK` | latest − 1000 | First block to scan |
| `POLL_INTERVAL_MS` | 12000 | Polling cadence (~one Base block) |
| `FINALITY_BLOCKS` | 18 | Wait this many blocks before forwarding |
| `LOG_BATCH_MAX_BLOCKS` | 500 | Max range per `eth_getLogs` call |
| `STATE_FILE` | `./.icp-chain-watcher-state.json` | Last-processed block persistence |
| `PORT` | 8789 | Health endpoint port |

## Test

```sh
node --test test/watcher.test.mjs
```

**8/8 PASS** covering:
- ABI decoding for the 5 ICPEscrow events (Funded, Disputed, Released,
  Refunded, Resolved), including indexed bytes32, indexed address,
  uint128, uint64, bytes32, and variable-length string fields
- Forwarder mapping (each decoded event → correct Settler admin payload
  with proper currency-decimal handling for USDC's 6 decimals)
- End-to-end: mock JSON-RPC server + real settler-stateset daemon +
  ChainWatcher pulling logs and forwarding to Settler
- State persistence (last_processed_block advances correctly)
- Range awareness (no double-processing already-confirmed blocks)

## How decoding works

The 5 ICPEscrow events have known topic[0] hashes (computed via
`cast keccak '<sig>'` and verified against the Foundry tooling).
Hardcoded in `src/abi-decoder.mjs`:

```
EscrowFunded   0x5c5e9cbd002f416577cd999eb1297865013aecaf0f8c6f593e56c9c334d4644f
EscrowDisputed 0x85df63e82b1c4b692591e851fd05ac7c87d4dd28557d780c47c462a11f64e0c8
EscrowReleased 0x95d522762e04d28e21709344963474d18d6d8c19cea99865cf53029a3c25ec54
EscrowRefunded 0xa3a9c68367292ca26571c2c1b730c525eb110a42666b162ac6ceeb25ffa461f0
EscrowResolved 0x86e741358ba245b5ec9be2af9edd5f3c7be4399b7701dc5e4b009ea0aeac0302
```

For non-indexed fields, the watcher decodes Solidity ABI words manually:

- **uintN**: right-aligned in a 32-byte word; take last N/8 bytes as
  big-endian unsigned integer (using `BigInt` to avoid precision loss
  for `uint128`).
- **bytes32**: the 32-byte word directly.
- **string**: 32-byte offset → 32-byte length → UTF-8 bytes padded to
  the next 32-byte boundary.

For indexed fields (in `topics[1..3]`):
- **bytes32**: the topic value directly.
- **address**: 32-byte topic with the address right-aligned; take
  last 20 bytes.

This is enough for the 5 events. Adding a new event with different
types would require extending `decodeLog()` in `src/abi-decoder.mjs`.

## Currency handling

The Solidity contract stores amounts in token base units (6 decimals
for USDC). The watcher's forwarder converts to decimal-formatted
strings before POSTing to the Settler so the Settler doesn't have to
know which token's at which decimals. Example:

```
on-chain uint128: 100000000  (raw USDC base units)
forwarded as: { "amount": "100.000000", "currency": "USDC" }
```

This means the watcher needs to know the token decimals per Settler
binding. Currently hardcoded to USDC 6 decimals; a future enhancement
would read decimals from the contract or from the Settler discovery
doc.

## Finality + reorg handling

The watcher reads `eth_blockNumber`, subtracts `FINALITY_BLOCKS` (default
18 — matches Base L2 finality), and only processes logs up to the
finalized head. This protects against reorgs at the cost of latency
(~3-5 minutes on Base).

In the event of a reorg deeper than `FINALITY_BLOCKS`, the watcher
would forward duplicate events and the Settler would respond with
`escrow.already_funded` for the second occurrence. The watcher logs the
error but does NOT roll back — the on-chain state is authoritative.
Production deployments should monitor for repeated `already_funded`
responses as a reorg signal.

## State persistence

The watcher writes `last_processed_block` to `STATE_FILE` after each
successful tick. On restart, it resumes from `last_processed_block + 1`.
This file is the only persistent state — losing it means the watcher
re-scans from `latest − 1000` and re-forwards events. Idempotent
forwarding (the Settler's `already_funded` check) protects against
duplicate state from a reset.

## Health endpoint

```
GET /healthz
{
  "ok": true,
  "events_seen": 1247,
  "events_forwarded": 1247,
  "errors": 0,
  "last_block": 1018,
  "last_processed_block": 1018
}
```

Suitable for Kubernetes liveness probes or any HTTP healthcheck.

## What's NOT implemented (production gaps)

- **WebSocket subscriptions** instead of polling. Sub-second latency
  would require `eth_subscribe` with logsFilter; this version polls
  every 12s to minimize RPC cost on free-tier endpoints. Switching to
  WebSocket is ~50 LOC.
- **Multi-chain support**. The current watcher is single-chain. For
  multi-rail Settlers (USDC on Base + USDC on Ethereum + USDC on
  Solana) you'd run multiple watcher processes.
- **Solana / non-EVM rails**. The ABI decoder is EVM-specific. Solana
  uses Borsh; a future Solana watcher would need a separate decoder.
- **Discovery document updates**. The Settler's `.well-known/icp-settler`
  document advertises `operating_mode: "mock"`. When chain mode is
  active, the daemon should be reconfigured to advertise
  `operating_mode: "chain"` and refuse `/admin/escrow/event` calls
  from any source other than the watcher's authenticated identity.
  Currently the admin endpoint is open in mock mode and disabled in
  chain mode; a chain-watcher-authenticated middle ground is on the
  ICP-1.1 roadmap.

## Deployment example (Kubernetes)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: icp-chain-watcher
spec:
  replicas: 1  # exactly one watcher per chain to avoid duplicate forwarding
  template:
    spec:
      containers:
      - name: watcher
        image: stateset/icp-chain-watcher:1.2.0
        env:
        - name: RPC_URL
          valueFrom: { secretKeyRef: { name: rpc-url, key: base-sepolia } }
        - name: CONTRACT_ADDRESS
          value: "0x_PROD_DEPLOYED_ICPESCROW_"
        - name: SETTLER_URL
          value: "http://settler-stateset.icp:8788"
        - name: FINALITY_BLOCKS
          value: "18"
        volumeMounts:
        - name: state
          mountPath: /state
        env:
        - name: STATE_FILE
          value: /state/watcher.json
      volumes:
      - name: state
        persistentVolumeClaim: { claimName: chain-watcher-state }
```

## Status

Reference implementation. Tested against a mock JSON-RPC server + the
real settler-stateset daemon. Ready for production deployment once:

1. ICPEscrow.sol is deployed to Base Sepolia (or a real RPC URL is
   configured)
2. settler-stateset is running in chain-mode (currently mock-only)
3. The watcher process is supervised (systemd / Kubernetes / Nomad)

The watcher itself has no external dependencies and runs anywhere
Node 20+ runs.
