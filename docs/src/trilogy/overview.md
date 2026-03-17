# The StateSet Trilogy

The StateSet Trilogy is a vertically integrated, three-layer protocol stack for autonomous AI agent commerce. Each layer is independently verifiable — no layer trusts the one above it.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 3: StateSet iCommerce (Application)                         │
│                                                                     │
│  AI Agents · 520+ MCP Tools · A2A Protocol · Policy Engine         │
│  Platform Adapters (Stripe, Shopify, WooCommerce) · 11 Bindings    │
│                                                                     │
│  Emits: VES-signed commerce events (Ed25519 + AES-256-GCM)        │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ VES v1.0 events
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 2: StateSet Sequencer (Ordering)                            │
│                                                                     │
│  Deterministic event ordering · Gap-free sequences                  │
│  Merkle tree commitments · Agent key registry                       │
│  STARK compliance proofs · Batch settlement                         │
│  x402 payment intent processing                                     │
│                                                                     │
│  Produces: Merkle roots, sequencer receipts, compliance proofs     │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Merkle commitments
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 1: SET Chain L2 (Settlement)                                │
│                                                                     │
│  OP Stack · 2-second blocks · Sub-cent fees                        │
│  SetRegistry (on-chain anchoring) · SetPaymaster (gas abstraction) │
│  ssUSD stablecoin (yield-bearing, T-Bill backed)                   │
│                                                                     │
│  Settles to: Ethereum L1 (data availability + finality)            │
└─────────────────────────────────────────────────────────────────────┘
```

## Why Three Layers?

Each layer solves a distinct problem:

| Layer | Problem | Solution |
|-------|---------|----------|
| **iCommerce** | AI agents need a commerce engine, not a REST API | Embeddable, deterministic, policy-governed |
| **Sequencer** | Multiple agents need a canonical ordering of events | Gap-free sequencing with cryptographic proofs |
| **SET Chain** | Commerce needs on-chain settlement without gas friction | Purpose-built L2 with gas abstraction and yield-bearing stablecoins |

The layers compose additively. You can run iCommerce standalone (Tier 1), add the Sequencer for enterprise multi-agent coordination (Tier 2), or add SET Chain for on-chain settlement (Tier 3). See [Product Tiers](../tiers.md).

## End-to-End Flow

Here is a complete transaction lifecycle across all three layers:

```
1. Agent creates an order                   [iCommerce]
   └─ Policy engine validates               [iCommerce]
   └─ Event signed with Ed25519             [iCommerce, VES v1.0]

2. Event submitted to sequencer             [Sequencer]
   └─ Agent signature verified              [Sequencer]
   └─ Sequence number assigned (gap-free)   [Sequencer]
   └─ Sequencer receipt issued              [Sequencer]

3. Payment intent created (x402)            [iCommerce]
   └─ Intent signed and queued              [Sequencer]
   └─ Batch accumulated                     [Sequencer]

4. STARK compliance proof generated         [Sequencer + STARK Prover]
   └─ Proves amount satisfies policy        [STARK Prover]
   └─ Without revealing the amount          [Zero-knowledge]

5. Merkle commitment batched                [Sequencer]
   └─ Events organized into Merkle tree     [Sequencer]
   └─ State root chained to previous batch  [Sequencer]

6. Anchor service submits to chain          [SET Chain]
   └─ commitBatch() on SetRegistry          [SET Chain]
   └─ Payment batch settled in ssUSD        [SET Chain]
   └─ Gas sponsored by SetPaymaster         [SET Chain]

7. Any third party can verify               [SET Chain]
   └─ verifyInclusion() on SetRegistry      [SET Chain]
   └─ No access to sequencer or DB needed   [Trustless]
```

## Verification Model

The trilogy enables a verification chain where each layer can be independently audited:

**Application layer**: Events are signed with Ed25519 — any agent can verify that a specific agent authored a specific event.

**Ordering layer**: The sequencer assigns monotonic sequence numbers and issues signed receipts — any observer can verify that no events were dropped, reordered, or duplicated.

**Settlement layer**: Merkle commitments are anchored on-chain — any third party (auditor, logistics partner, lender) can verify event inclusion without accessing any private database.

**Compliance layer**: STARK proofs demonstrate that private transaction amounts satisfy regulatory policies — without revealing the amounts themselves.

## Key Protocols

| Protocol | Layer | Purpose |
|----------|-------|---------|
| [VES v1.0](../security/ves.md) | Application + Sequencer | Cryptographic event signing, encryption, Merkle proofs |
| [A2A](../a2a/overview.md) | Application | Agent-to-agent commerce (quotes, escrow, splits, disputes) |
| [x402](../payments/x402.md) | Application + Sequencer | Signed payment intents with batch settlement |
| [STARK Proofs](stark-proofs.md) | Sequencer | Zero-knowledge compliance verification |

## Smart Contracts

| Contract | Address | Purpose |
|----------|---------|---------|
| **SetRegistry** | Deployed on SET Chain | Merkle commitment anchoring, inclusion verification |
| **SetPaymaster** | Deployed on SET Chain | ERC-4337 gas sponsorship for commerce transactions |
| **ssUSD / wSSDC** | Deployed on SET Chain | Yield-bearing stablecoin backed by T-Bills |
| **SetTimelock** | Deployed on SET Chain | 24-hour governance delay for contract upgrades |

## Repositories

| Repository | Language | Purpose |
|------------|----------|---------|
| `stateset-icommerce` | Rust + Node.js | Commerce engine, CLI, MCP server, A2A protocol |
| `stateset-sequencer` | Rust | VES sequencer, commitment engine, x402 processing |
| `stateset-stark` | Rust | STARK proof system for ZK compliance |
| `set` | Solidity + Rust | SET Chain L2 contracts, anchor service, SDK |

## Further Reading

- [Sequencer](sequencer.md) — Deterministic event ordering and commitment engine
- [SET Chain](set-chain.md) — Commerce-optimized L2 with gas abstraction
- [STARK Proofs](stark-proofs.md) — Zero-knowledge compliance proofs
- [ssUSD Stablecoin](ssusd.md) — Yield-bearing settlement asset
- [Anchor Service](anchor.md) — Bridge between sequencer and on-chain
