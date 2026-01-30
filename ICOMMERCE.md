# StateSet iCommerce

## The Commerce Runtime for the Agent Economy

---

## Executive Summary

Commerce is undergoing its third major platform shift. The first wave moved transactions online (eCommerce). The second wave decoupled frontends from backends (headless commerce). The third wave—now beginning—hands commerce operations to autonomous AI agents.

StateSet is building the infrastructure for this shift: an embedded, deterministic commerce engine designed for AI agents. Where eCommerce was built for humans clicking buttons, iCommerce is built for agents making decisions.

We are creating the SQLite of commerce—portable, embeddable, zero-configuration, and universally deployable. Our open protocol (ACP) positions StateSet as the standard execution layer for any AI agent transacting in the real world.

---

## The Shift: From eCommerce to iCommerce

### The Problem with Current Infrastructure

Today's commerce platforms were designed for humans operating dashboards:

| Assumption | Reality in Agent Era |
|------------|---------------------|
| Users authenticate via OAuth | Agents need embedded access |
| Operations happen at human speed | Agents operate in milliseconds |
| State lives in vendor clouds | Agents need portable state |
| UIs surface information | Agents need structured data |
| Rate limits assume human pace | Agents hit limits instantly |

Every major commerce platform—Shopify, BigCommerce, Salesforce Commerce Cloud—shares these assumptions. They are fundamentally human-centric.

### The Agent Commerce Gap

AI agents from OpenAI, Anthropic, Google, and the open-source ecosystem are rapidly becoming capable of complex reasoning and task execution. Commerce is among the most common real-world actions these agents need to perform:

- Processing returns and refunds
- Managing subscriptions
- Handling customer inquiries
- Coordinating fulfillment
- Negotiating B2B procurement
- Managing inventory and reordering

Yet there is no commerce infrastructure designed for agents. Developers resort to brittle API integrations, screen scraping, or building custom backends. The result: agents that are slow, unreliable, and disconnected from real commerce state.

---

## The StateSet Solution

### iCommerce: A New Category

We define **iCommerce** (intelligent commerce) as the category of commerce infrastructure built for autonomous agents rather than human operators.

iCommerce is characterized by:

- **Embedded execution** — Commerce logic runs in-process, not over the network
- **Portable state** — A single file contains complete operational state
- **Deterministic operations** — Predictable outcomes that agents can reason about
- **Policy enforcement** — Business rules that constrain agent behavior
- **Protocol-first design** — Standardized interfaces across any AI system

### StateSet iCommerce Engine

StateSet is the reference implementation of iCommerce—an embedded commerce library written in Rust with bindings for every major runtime:

```
┌─────────────────────────────────────────────────────────────┐
│                     AI Agent Layer                          │
│            (ChatGPT, Claude, Gemini, LangChain)             │
└─────────────────────────────────────────────────────────────┘
                              │
                   Agentic Commerce Protocol (ACP)
                              │
┌─────────────────────────────────────────────────────────────┐
│                   StateSet iCommerce                        │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────┐  │
│  │  Policy (NSR) │  │ State (SQLite)│  │    Execution    │  │
│  │   Guardrails  │  │   Portable    │  │  Deterministic  │  │
│  └───────────────┘  └───────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Core capabilities:**

| Domain | Modules |
|--------|---------|
| Commerce | Customers, Orders, Products, Returns, Payments |
| Supply Chain | Purchase Orders, Inventory, Shipments, Receiving |
| Operations | Work Orders, BOM, Warranties, Invoices |
| Agent-Native | Policies, Approvals, Events, Conversations |

**Developer experience:**

```javascript
import { Commerce } from '@stateset/embedded';

const commerce = new Commerce('./store.db');

// Your agent now has a complete commerce engine
const order = await commerce.orders.create({
  customerId: customer.id,
  items: [{ sku: 'WIDGET-001', quantity: 2, unitPrice: 29.99 }]
});
```

No accounts. No API keys. No network dependency. Just a file path.

### Universal Runtime Support

One Rust core, every platform:

| Platform | Package | Install |
|----------|---------|---------|
| Node.js | `@stateset/embedded` | `npm install` |
| Python | `stateset-embedded` | `pip install` |
| Browser/Edge | `@stateset/embedded-wasm` | `import init` |
| Rust | `stateset-embedded` | `cargo add` |
| CLI | `@stateset/cli` | `npm install -g` |

---

## Agentic Commerce Protocol (ACP)

### Protocol as Wedge

StateSet's go-to-market strategy centers on the **Agentic Commerce Protocol**—an open standard for AI agents performing commerce operations.

ACP defines:

- **Capabilities** — Standardized operations (inventory.check, orders.create, returns.initiate)
- **Schemas** — Common data models (Order, Customer, Product, Fulfillment)
- **Policies** — Declarative business rules that constrain agent behavior
- **Trust boundaries** — What agents can do autonomously vs. requiring approval

### Protocol Network Effects

```
┌──────────────────────┐
│  Claude (Anthropic)  │──────┐
└──────────────────────┘      │
┌──────────────────────┐      │        ┌─────────────────────┐
│  ChatGPT (OpenAI)    │──────┼──ACP───│  StateSet iCommerce │
└──────────────────────┘      │        │  (reference impl)   │
┌──────────────────────┐      │        └─────────────────────┘
│  Gemini (Google)     │──────┤                  OR
└──────────────────────┘      │        ┌─────────────────────┐
┌──────────────────────┐      │        │  ACP adapters for   │
│  LangChain / AutoGPT │──────┘        │  Shopify, Stripe,   │
└──────────────────────┘               │  WooCommerce, etc   │
                                       └─────────────────────┘
```

As ACP adoption grows, StateSet benefits regardless of which implementation is used—but the embedded engine remains the fastest, most complete, and most portable option.

### ACP + MCP Integration

ACP is designed as a domain-specific layer atop Anthropic's Model Context Protocol (MCP), positioning StateSet within the emerging agent tool ecosystem:

```
MCP (generic tool protocol)
 └── ACP (commerce-specific)
      └── StateSet (reference implementation)
```

---

## Neuro-Symbolic Reasoning Engine

**Status:** Planned. Current releases provide deterministic core logic and CLI preview/apply gating; policy and approval workflows are on the roadmap.

### The Trust Problem

Large language models are non-deterministic. Letting GPT-4 directly control refund amounts, inventory allocations, or pricing decisions is operationally dangerous.

StateSet's **NSR (Neuro-Symbolic Reasoning) Engine** provides the trust boundary between agent intent and commerce execution:

```
Agent: "Give this customer a full refund"
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│                  StateSet Policy Engine                     │
│                                                             │
│   check_refund_eligibility(order)                           │
│   → order_age: 45 days                                      │
│   → policy: 30 day window                                   │
│   → result: DENIED                                          │
│                                                             │
│   suggest_alternatives()                                    │
│   → store_credit: eligible                                  │
│   → exchange: eligible                                      │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
Agent: "I can't process a full refund past 30 days, but I can 
        offer store credit or an exchange. Which would you prefer?"
```

### Symbolic + Neural

| Layer | Responsibility | Characteristics |
|-------|---------------|-----------------|
| LLM | Intent, conversation, judgment | Flexible, non-deterministic |
| NSR | Business rules, constraints | Rigid, auditable, deterministic |
| Core | State changes, transactions | ACID, reliable, portable |

This architecture gives enterprises confidence that agent-driven commerce will respect their policies while preserving the flexibility that makes AI valuable.

---

## Market Opportunity

### Addressable Markets

| Market | Size | StateSet Position |
|--------|------|-------------------|
| Commerce Platforms | $15B | Embedded alternative to SaaS |
| Order Management Systems | $3B | Agent-native OMS |
| Customer Service Automation | $12B | NSR-powered, deterministic |
| Inventory & Warehouse | $5B | Lightweight WMS |
| Procurement Software | $10B | Embedded procurement |
| AI Agent Infrastructure | $2B → $50B+ | Ground floor |

**Total addressable market: $40B+ in existing categories, with AI agent infrastructure representing uncapped greenfield opportunity.**

### The Wedge

Our initial focus: **AI agent infrastructure for commerce operations**.

Every company deploying customer service agents, procurement bots, or operational automation needs a commerce execution layer. StateSet is the only embedded, agent-native option.

### Expansion Path

```
Year 1: Agent commerce infrastructure (returns, support, ops)
Year 2: Embedded OMS for D2C and B2B
Year 3: Full operational ERP for SMB
Year 4+: Standard infrastructure for agent economy
```

---

## Business Model

### Open Core + Cloud

| Tier | Offering | Price |
|------|----------|-------|
| **Open Source** | Embedded engine, local-only | Free |
| **Pro** | Managed sync, cloud backup, hosted endpoints | $99-499/mo |
| **Enterprise** | Multi-tenant, SSO, audit logs, SLAs, NSR studio | Custom |

### Revenue Streams

1. **StateSet Cloud** — Hosted sync, backup, and multi-device coordination
2. **Enterprise Licenses** — On-premise deployment with support
3. **NSR Studio** — Visual policy builder for enterprise customers
4. **Marketplace** — Pre-built agent templates and integrations

### Unit Economics

The embedded model inverts traditional SaaS economics:

| Traditional SaaS | StateSet |
|------------------|----------|
| Host everything | Customer hosts compute |
| Scale infrastructure cost | Near-zero marginal cost |
| Per-seat pricing | Per-sync or GMV pricing |
| High CAC, high churn | Developer adoption, sticky infrastructure |

---

## Competitive Landscape

### Current Options for Agent Commerce

| Option | Limitation |# Set

[![Solidity](https://img.shields.io/badge/Solidity-0.8.20-blue)](https://soliditylang.org/)
[![Rust](https://img.shields.io/badge/Rust-2021-orange)](https://www.rust-lang.org/)
[![OP Stack](https://img.shields.io/badge/OP%20Stack-v1.8.0-red)](https://docs.optimism.io/)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

Set is an Ethereum Layer-2 (L2) network built on the **OP Stack**, designed for **commerce**. It offers faster, cheaper, and cryptographically verifiable transactions by leveraging optimistic rollups with Merkle root anchoring.

## Table of Contents

- [Architecture](#architecture)
- [Key Features](#key-features)
- [Chain Configuration](#chain-configuration)
- [Directory Structure](#directory-structure)
- [Technology Stack](#technology-stack)
- [Quick Start](#quick-start)
  - [Local Development (Anvil)](#local-development-anvil)
  - [Full Devnet](#full-devnet)
- [Smart Contracts](#smart-contracts)
  - [SetRegistry](#setregistry)
  - [SetPaymaster](#setpaymaster)
- [Anchor Service](#anchor-service)
- [Integration with stateset-sequencer](#integration-with-stateset-sequencer)
- [Docker Deployment](#docker-deployment)
- [Testing](#testing)
- [Deployment Checklist](#deployment-checklist)
- [Monitoring](#monitoring)
- [Security](#security)
- [Decentralization and Fault Proofs](#decentralization-and-fault-proofs)
- [Scorecard](#scorecard)
- [Troubleshooting](#troubleshooting)
- [Resources](#resources)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                             SET L2 (84532001)                           │
│                      (Commerce-Optimized OP Stack)                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │   op-geth    │  │   op-node    │  │  op-batcher  │  │ op-proposer │ │
│  │  (execution) │  │  (consensus) │  │   (batches)  │  │   (state)   │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────┘ │
│         │                │                  │                │         │
│         └────────────────┼──────────────────┴────────────────┘         │
│                          │                                              │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Smart Contracts                                │  │
│  │  ┌─────────────────────────┐  ┌────────────────────────────────┐ │  │
│  │  │      SetRegistry        │  │         SetPaymaster           │ │  │
│  │  │  (Merkle root anchoring │  │  (Gas abstraction for          │ │  │
│  │  │   from sequencer)       │  │   merchant transactions)       │ │  │
│  │  └─────────────────────────┘  └────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                          │                                              │
└──────────────────────────┼──────────────────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  │                  ▼
┌───────────────────┐      │      ┌─────────────────────────┐
│  Anchor Service   │      │      │  stateset-sequencer     │
│  (Rust)           │◄─────┴─────►│  (Off-chain commerce    │
│  - Health metrics │             │   event processing)     │
│  - Batch anchoring│             └─────────────────────────┘
└───────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      ETHEREUM SEPOLIA (L1) - 11155111                   │
│         OptimismPortal │ L2OutputOracle │ SystemConfig                  │
└─────────────────────────────────────────────────────────────────────────┘
```

## Key Features

| Feature | Description |
|---------|-------------|
| **2-second block times** | Fast confirmations optimized for commerce operations |
| **Low gas fees** | EIP-1559 parameters tuned for merchant transactions |
| **Merkle root anchoring** | Verifiable event commitments from stateset-sequencer |
| **Multi-tenant isolation** | Per-tenant/store state tracking via `keccak256(tenantId, storeId)` |
| **Inclusion proof verification** | On-chain verification of off-chain events |
| **Gas sponsorship** | Merchants can sponsor user transactions via SetPaymaster |
| **Strict mode verification** | State chain continuity checking to prevent gaps/forks |

## Chain Configuration

| Parameter | Value |
|-----------|-------|
| Chain ID | `84532001` |
| Block Time | 2 seconds |
| Gas Limit | 30M gas/block |
| L1 Settlement | Ethereum Sepolia (11155111) |
| Native Token | ETH |
| EVM Version | Cancun |
| OP Contracts Version | v1.8.0 |

## Directory Structure

```
set/
├── anchor/                     # Rust anchor service
│   ├── src/
│   │   ├── main.rs            # Entry point
│   │   ├── config.rs          # Configuration from env vars
│   │   ├── client.rs          # Sequencer API client
│   │   ├── service.rs         # Main anchor logic
│   │   ├── health.rs          # Health/metrics HTTP server
│   │   └── types.rs           # Data structures
│   └── tests/
│       └── integration.rs     # Integration tests
├── contracts/                  # Solidity smart contracts
│   ├── src/
│   │   ├── SetRegistry.sol    # Merkle root anchoring (433 lines)
│   │   └── commerce/
│   │       └── SetPaymaster.sol # Gas abstraction (558 lines)
│   ├── test/
│   │   ├── SetRegistry.t.sol  # Registry tests
│   │   └── SetPaymaster.t.sol # Paymaster tests
│   └── lib/                   # Dependencies (git submodules)
│       ├── forge-std/         # Foundry testing framework
│       └── openzeppelin-contracts/
├── op-stack/                   # OP Stack configuration
│   ├── deployer/              # op-deployer intent files
│   ├── batcher/               # Batch submission config
│   ├── proposer/              # State root submission config
│   ├── challenger/            # Dispute resolution config
│   └── sequencer/             # op-geth/op-node config
├── docker/                     # Docker Compose files
│   ├── docker-compose.yml     # Main local devnet
│   ├── docker-compose.sepolia.yml
│   ├── docker-compose.local.yml
│   └── config/                # JWT and node configs
├── scripts/                    # Deployment and management
│   ├── dev.sh                 # Local Anvil development helper
│   ├── anchor-devnet.sh       # Anchor service local helper
│   ├── deploy-set-contracts.sh
│   ├── deploy-l1.sh
│   ├── generate-genesis.sh
│   ├── reset-devnet.sh
│   ├── start-devnet.sh
│   ├── stop-devnet.sh
│   ├── quick-start-local.sh
│   └── install-op-stack.sh
├── config/                     # Chain configuration
│   ├── chain-config.toml     # L2 chain parameters
│   ├── local.env.example     # Local devnet env template
│   └── sepolia.env.example   # Sepolia env template
└── docs/                       # Documentation
    ├── README.md              # Architecture overview
    └── local_testing_guide.md # Anvil testing guide
```

## Technology Stack

### Languages & Frameworks

| Component | Technology | Version |
|-----------|------------|---------|
| Smart Contracts | Solidity | 0.8.20 |
| Contract Framework | Foundry (Forge) | Latest |
| Anchor Service | Rust | 2021 Edition |
| Async Runtime | Tokio | Full features |
| Ethereum Client | Alloy | 0.9 |
| HTTP Server | Axum | 0.8 |
| Scripting | Bash | - |

### OP Stack Components

| Component | Purpose |
|-----------|---------|
| op-geth | L2 execution client (EVM) |
| op-node | L2 consensus client |
| op-batcher | Submits transaction batches to L1 |
| op-proposer | Submits state roots to L1 |
| op-challenger | Dispute resolution |

### Dependencies

**Solidity:**
- OpenZeppelin Contracts (Upgradeable patterns)
- Forge-std (Testing)

**Rust:**
- `tokio` - Async runtime
- `alloy` - Ethereum interactions
- `axum` - HTTP server for health endpoints
- `tracing` - Structured logging
- `serde` - Serialization
- `reqwest` - HTTP client

## Quick Start

### Local Development (Anvil)

The fastest way to get started for development and testing:

```bash
# 1. Start local Anvil node (Chain ID: 84532001, 2s blocks)
./scripts/dev.sh start

# 2. Deploy contracts to local Anvil
./scripts/dev.sh deploy

# 3. Run contract tests
./scripts/dev.sh test

# 4. Check node status
./scripts/dev.sh status

# 5. Fund a test account
./scripts/dev.sh fund 0xYourAddress

# Other commands
./scripts/dev.sh accounts  # List pre-funded accounts
./scripts/dev.sh console   # Open Foundry console
```

**Pre-funded Test Accounts:**

| Account | Address | Private Key |
|---------|---------|-------------|
| Account 0 | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` | `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` |
| Account 1 | `0x70997970C51812dc3A010C7d01b50e0d17dc79C8` | `0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d` |
| ... | See `./scripts/dev.sh accounts` for all 10 accounts |

### Full Devnet

For a complete L2 environment with all OP Stack components:

**Prerequisites:**
- Go 1.21+
- Rust 1.70+
- Docker & Docker Compose
- 2+ ETH on Sepolia (for deployment)

```bash
# 1. Install OP Stack binaries
./scripts/install-op-stack.sh

# 2. Configure environment
cp config/sepolia.env.example config/sepolia.env
# Edit sepolia.env with your addresses and private keys

# 3. Deploy L1 contracts to Sepolia
./scripts/deploy-l1.sh

# 4. Generate L2 genesis
./scripts/generate-genesis.sh

# 5. Start the devnet
./scripts/start-devnet.sh

# Or use quick-start for minimal setup
./scripts/quick-start-local.sh
```

**Verify Chain is Running:**

```bash
# Check L2 block number
cast block-number --rpc-url http://localhost:8547

# Get chain ID (should return 84532001)
cast chain-id --rpc-url http://localhost:8547

# Check sync status
curl -s http://localhost:8547 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}'
```

## Smart Contracts

### SetRegistry

The SetRegistry contract stores batch commitments from the stateset-sequencer, enabling on-chain verification of off-chain commerce events.

**Key Features:**
- Multi-sequencer authorization
- State chain continuity verification
- Merkle inclusion proof verification
- Per-tenant/store isolation

**Core Functions:**

| Function | Description |
|----------|-------------|
| `commitBatch()` | Submit a batch commitment with Merkle roots |
| `verifyInclusion()` | Verify an event is included in a committed batch |
| `getLatestStateRoot()` | Get current state root for a tenant/store |
| `setSequencerAuthorization()` | Admin: authorize/revoke sequencers |
| `setStrictMode()` | Enable/disable state chain verification |

**Example Usage:**

```solidity
// Verify an order event was included in a batch
bool valid = registry.verifyInclusion(
    batchId,
    orderEventHash,
    merkleProof,
    leafIndex
);

// Get latest state root for a tenant/store
bytes32 stateRoot = registry.getLatestStateRoot(tenantId, storeId);
```

**Interact via CLI:**

```bash
# Check if a sequencer is authorized
cast call $REGISTRY_ADDRESS "authorizedSequencers(address)" $SEQUENCER_ADDRESS

# Get batch commitment
cast call $REGISTRY_ADDRESS "batchCommitments(bytes32)" $BATCH_ID
```

### SetPaymaster

Gas abstraction for sponsored commerce transactions, allowing merchants to pay for user gas fees.

**Sponsorship Tiers:**

| Tier | Monthly Limit | Per-Tx Limit |
|------|---------------|--------------|
| Starter | 0.1 ETH | 0.001 ETH |
| Growth | 1 ETH | 0.01 ETH |
| Enterprise | 10 ETH | 0.1 ETH |

**Supported Operation Types:**

| Operation | Description |
|-----------|-------------|
| `ORDER_CREATE` | Creating new orders |
| `ORDER_UPDATE` | Updating order status |
| `PAYMENT_PROCESS` | Processing payments |
| `INVENTORY_UPDATE` | Updating inventory |
| `RETURN_PROCESS` | Processing returns |
| `COMMITMENT_ANCHOR` | Anchoring commitments |
| `OTHER` | Other operations |

**Features:**
- Per-transaction and daily/monthly spend limits
- Automatic refund of unused gas
- Category-based sponsorship
- Merchant dashboards

## Anchor Service

The anchor service (`set-anchor`) is a Rust service that bridges the stateset-sequencer to the SetRegistry contract on-chain.

### Building

```bash
cd anchor
cargo build --release
```

### Running

```bash
# Set required environment variables
export SET_REGISTRY_ADDRESS=0x...
export SEQUENCER_PRIVATE_KEY=0x...
export SEQUENCER_API_URL=http://localhost:3000
export L2_RPC_URL=http://localhost:8547
export ANCHOR_INTERVAL_SECS=60  # seconds
export MIN_EVENTS_FOR_ANCHOR=100

# Run the service
./target/release/set-anchor
```

**Local devnet:**

```bash
./scripts/dev.sh anchor-start
./scripts/dev.sh anchor-smoke
```

Smoke overrides (optional):

```bash
EVENT_LEAF_0=0x... EVENT_LEAF_1=0x... TENANT_ID=0x... STORE_ID=0x... \
NEW_STATE_ROOT=0x... ./scripts/dev.sh smoke
```

### Health Endpoints

The anchor service exposes health and metrics endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Liveness probe (service is running) |
| `GET /ready` | Readiness probe (connected to chain and sequencer) |
| `GET /metrics` | Prometheus-format metrics |
| `GET /stats` | JSON statistics (anchored count, last anchor time, etc.) |

**Example:**

```bash
# Check if service is ready
curl http://localhost:9090/ready

# Get metrics
curl http://localhost:9090/metrics
```

## Integration with stateset-sequencer

Set integrates with the stateset-sequencer through a two-phase process:

```
stateset-sequencer                    Anchor Service                    SetRegistry
       │                                    │                                │
       │  1. Create BatchCommitment         │                                │
       │     with Merkle roots              │                                │
       │                                    │                                │
       │  2. GET /v1/commitments/pending    │                                │
       │◄───────────────────────────────────│                                │
       │     Return unanchored batches      │                                │
       │                                    │                                │
       │                                    │  3. commitBatch(...)           │
       │                                    │───────────────────────────────►│
       │                                    │     Returns tx hash            │
       │                                    │◄───────────────────────────────│
       │                                    │                                │
       │  4. POST /v1/commitments/{id}/anchored                              │
       │◄───────────────────────────────────│                                │
       │     with chain_tx_hash             │                                │
       │                                    │                                │
```

**API Endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/commitments/pending` | GET | List unanchored commitments |
| `/v1/commitments/{id}/anchored` | POST | Notify of successful anchoring |

## Docker Deployment

### Local Devnet

```bash
cd docker

# Start full local devnet (includes L1)
docker-compose up -d

# Check logs
docker-compose logs -f op-geth

# Stop
docker-compose down
```

### Sepolia Testnet

```bash
cd docker

# Connects to real Ethereum Sepolia
docker-compose -f docker-compose.sepolia.yml up -d
```

### With Optional Services

```bash
# With block explorer
docker-compose --profile explorer up -d

# With anchor service
docker-compose --profile anchor up -d
```

### Alternative L1 Clients

```bash
# Using Nethermind as L1
docker-compose -f docker-compose.nethermind.yml up -d

# Using Reth as L1
docker-compose -f docker-compose.reth.yml up -d
```

## Testing

### Contract Tests

```bash
cd contracts

# Run all tests
forge test

# Run with verbosity
forge test -vvv

# Run specific test
forge test --match-test testCommitBatch

# Run with gas reporting
forge test --gas-report

# Generate coverage
forge coverage
```

### Anchor Service Tests

```bash
cd anchor

# Run unit tests
cargo test

# Run integration tests (requires Anvil running)
cargo test --test integration

# Run with logs
RUST_LOG=debug cargo test
```

## Deployment Checklist

### Accounts Setup

1. [ ] Generate 5 Ethereum accounts:
   - Admin (owns contracts)
   - Batcher (submits batches to L1)
   - Proposer (submits state roots)
   - Challenger (dispute resolution)
   - Sequencer (L2 block production)

2. [ ] Fund each account with 0.5+ Sepolia ETH

### Infrastructure

3. [ ] Configure Sepolia RPC endpoint (Infura/Alchemy)
4. [ ] Set up JWT secret for engine API authentication
5. [ ] Prepare data directories for persistent storage

### Deployment

6. [ ] Run `deploy-l1.sh` - Deploy OP Stack contracts to Sepolia
7. [ ] Run `generate-genesis.sh` - Create L2 genesis block
8. [ ] Start L2 nodes (op-geth, op-node)
9. [ ] Start op-batcher and op-proposer
10. [ ] Deploy SetRegistry to L2
11. [ ] Deploy SetPaymaster to L2
12. [ ] Start anchor service

### Verification

13. [ ] Verify L2 is producing blocks (2s intervals)
14. [ ] Verify batches are being submitted to L1
15. [ ] Test anchor service connectivity
16. [ ] Verify contract deployments with `cast`

## Monitoring

See `docs/monitoring.md` for SLOs, alert suggestions, and metric definitions.

### Key Metrics

| Metric | Expected | Alert Threshold |
|--------|----------|-----------------|
| Block production | Every 2 seconds | > 10s gap |
| Batch submission | Every few minutes | > 30 min gap |
| Anchor lag | < 5 minutes | > 15 minutes |
| L2 safe head lag | < 10 blocks | > 100 blocks |

### Viewing Logs

```bash
# op-geth logs
tail -f logs/op-geth.log

# op-node logs
tail -f logs/op-node.log

# Anchor service logs
docker-compose logs -f set-anchor

# All OP Stack logs
./scripts/start-devnet.sh logs
```

### Anchor Service Metrics

```bash
# Prometheus metrics (HEALTH_PORT, default 9090)
curl http://localhost:9090/metrics

# JSON stats
curl http://localhost:9090/stats | jq
```

## Security

### Best Practices

- **Multi-sig admin**: Use a multisig wallet for admin/owner roles in production
- **Key management**: Never commit private keys; use environment variables or secret managers
- **Sequencer authorization**: Only authorize trusted sequencer addresses
- **Strict mode**: Enable strict mode in production to prevent state gaps
- **Threat model**: Review and maintain `docs/threat-model.md`
- **Operations runbook**: Keep `docs/runbook.md` current with incident response steps
- **Governance policy**: Maintain `docs/security.md` for upgrade and key management

### Pre-Production Checklist

- [ ] Smart contract audit completed
- [ ] Penetration testing of anchor service
- [ ] Key rotation procedures documented
- [ ] Incident response plan prepared
- [ ] Monitoring and alerting configured

## Decentralization and Fault Proofs

See `docs/decentralization.md` and `docs/fault-proofs.md` for the phased
decentralization plan and fault-proof operations. Validate production config with:

```bash
./scripts/validate-ops-config.sh --mode testnet --require-fault-proofs --require-admin-policy
```

Verify L1 settlement contracts:

```bash
./scripts/check-l1-settlement.sh --env-file config/sepolia.env --mode testnet --require-addresses
```

## Scorecard

See `docs/scorecard.md` for the 10/10 rubric and progress tracking. Supporting
docs include `docs/threat-model.md`, `docs/security.md`, `docs/runbook.md`, and
`docs/architecture.md`.

## Troubleshooting

### Common Issues

**L2 not producing blocks:**
```bash
# Check op-node sync status
curl -s http://localhost:9545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"optimism_syncStatus","params":[],"id":1}' | jq

# Verify L1 connection
curl -s http://localhost:8545 -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

**Anchor service not connecting:**
```bash
# Check health endpoint
curl http://localhost:9090/ready

# Verify environment variables
echo $SET_REGISTRY_ADDRESS
echo $L2_RPC_URL

# Check sequencer API
curl http://localhost:3000/v1/commitments/pending
```

**Contract deployment failing:**
```bash
# Ensure you have ETH
cast balance $DEPLOYER_ADDRESS --rpc-url http://localhost:8547

# Check gas prices
cast gas-price --rpc-url http://localhost:8547

# Verify RPC is responding
cast chain-id --rpc-url http://localhost:8547
```

**Tests failing:**
```bash
# Update dependencies
cd contracts && forge update

# Clean and rebuild
forge clean && forge build

# Run with more verbosity
forge test -vvvv
```

## Resources

### Documentation

- [OP Stack Documentation](https://docs.optimism.io/operators/chain-operators)
- [Optimism Monorepo](https://github.com/ethereum-optimism/optimism)
- [Foundry Book](https://book.getfoundry.sh/)
- [Alloy Documentation](https://alloy.rs/)

### Project Documentation

- [Local Testing Guide](docs/local_testing_guide.md)
- [Architecture Overview](docs/architecture.md)
- [Scorecard](docs/scorecard.md)
- [Toolchain Versions](docs/toolchain.md)
- [Monitoring and SLOs](docs/monitoring.md)
- [Security and Governance](docs/security.md)
- [Node Operator Guide](docs/node-operators.md)
- [Integration Example](docs/integration-example.md)
- [Block Explorer and Indexing](docs/explorer.md)
- [Bridge and Onramp Support](docs/bridge.md)
- [Operations History](docs/operations-history.md)
- [SDK](sdk/README.md)
- [Audit Report](docs/audit-report.md)
- [Governance Evidence](docs/governance-evidence.md)
- [Fault Proof Exercise Log](docs/fault-proof-exercise.md)
- [Decentralization Roadmap](docs/decentralization.md)
- [Fault Proof Operations](docs/fault-proofs.md)
- [Threat Model](docs/threat-model.md)
- [Operations Runbook](docs/runbook.md)

### Related Projects

- [StateSet Sequencer](../stateset-sequencer/) - Off-chain commerce event processing
- [StateSet Network](../) - Parent project documentation

## License

MIT

|--------|------------|
| Shopify/BigCommerce APIs | Human-centric, rate-limited, network-dependent |
| Custom backends | Expensive, slow, non-standard |
| Stripe | Payments only, not full commerce |
| Headless (Medusa, Saleor) | Still SaaS architecture, not embeddable |

### StateSet Differentiation

| Capability | StateSet | Alternatives |
|------------|----------|--------------|
| Embedded (no network) | ✓ | ✗ |
| Single-file portable state | ✓ | ✗ |
| Agent-native protocol | ✓ | ✗ |
| Policy/guardrail engine | ✓ | ✗ |
| Multi-runtime (Node, Python, WASM) | ✓ | ✗ |
| Offline-first | ✓ | ✗ |

**No one else is building agent-native commerce infrastructure.**

---

## Go-to-Market Strategy

### Phase 1: Protocol Establishment

- Publish ACP specification (open, MIT licensed)
- Ship MCP integration for Claude
- Ship function definitions for ChatGPT
- LangChain/LlamaIndex toolkit
- Developer documentation and tutorials

### Phase 2: Developer Adoption

- Presence in AI/agent communities
- Reference architectures for common use cases
- Integration with agent frameworks
- Open source community building

### Phase 3: Enterprise Expansion

- Customer service automation deployments
- B2B procurement agent implementations
- Manufacturing operations pilots
- NSR Studio for policy management

### Key Metrics

| Metric | Year 1 Target | Year 3 Target |
|--------|---------------|---------------|
| npm/PyPI downloads | 50K | 2M+ |
| Active deployments | 1,000 | 50,000+ |
| Cloud subscribers | 100 | 5,000+ |
| ARR | $500K | $15M+ |
| GMV through StateSet | $100M | $10B+ |

---

## Technical Architecture

### Crate Structure

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/        # Domain models, business logic
│   ├── stateset-db/          # SQLite persistence
│   ├── stateset-nsr/         # Neuro-symbolic reasoning
│   ├── stateset-sync/        # cr-sqlite CRDT sync
│   ├── stateset-acp/         # Protocol definitions
│   └── stateset-embedded/    # Unified interface
├── bindings/
│   ├── node/                 # N-API bindings
│   ├── python/               # PyO3 bindings
│   └── wasm/                 # WebAssembly
└── cli/                      # AI-powered CLI
```

### Design Principles

1. **Embedded-first** — Network is optional, not required
2. **Portable state** — Single file contains everything
3. **Deterministic execution** — Same input, same output, always
4. **Policy-enforced** — Business rules are code, not suggestions
5. **Protocol-native** — ACP is the interface, not an afterthought

---

## Why Now

### Convergence of Trends

1. **Agent capabilities** — GPT-4, Claude, Gemini can now handle complex reasoning
2. **Tool use protocols** — MCP, function calling standardizing agent-tool interaction
3. **Edge compute** — WASM, edge workers enable embedded execution anywhere
4. **Local-first movement** — Developers want control, not SaaS dependency
5. **Enterprise AI adoption** — Companies deploying agents need guardrails

### Window of Opportunity

The agent commerce infrastructure category is forming now. Standards are being set. The company that defines how agents do commerce will own the category for decades.

SQLite shipped in 2000 and remains the most deployed database 25 years later. Infrastructure categories, once won, are permanent.

---

## The Vision

### 2025
Agents use StateSet for customer service, returns, and basic operations.

### 2027
iCommerce becomes the standard protocol. Every major agent framework supports it. StateSet processes $100B+ GMV.

### 2030
iCommerce surpasses traditional eCommerce in transaction volume. Agents handle the majority of routine commerce operations. StateSet is infrastructure—invisible, ubiquitous, essential.

---

## Summary

**The opportunity:** Commerce infrastructure for the agent economy

**The product:** StateSet iCommerce—embedded, deterministic, portable

**The strategy:** Open protocol (ACP) + reference implementation + cloud services

**The outcome:** The SQLite of commerce—universal infrastructure for a new era

---

*LLMs reason. StateSet executes. Every agent needs a commerce engine.*

---

## Contact

StateSet Inc.
https://stateset.com
