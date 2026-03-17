# Architecture

StateSet iCommerce is the application layer of the StateSet Trilogy — a vertically integrated, three-layer protocol stack for autonomous AI agent commerce. Within iCommerce itself, the architecture is organized into three internal layers: a Rust core, language bindings, and the CLI + MCP server.

## Trilogy Stack

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Layer 3: StateSet iCommerce (this project)                              │
│  AI Agents · 520+ MCP Tools · A2A Protocol · Policy Engine              │
│  Platform Adapters (Stripe, Shopify, WooCommerce) · 11 Language Bindings│
└──────────────────────────────┬───────────────────────────────────────────┘
                               │ VES v1.0 signed events (Ed25519 + AES-256-GCM)
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  Layer 2: StateSet Sequencer (stateset-sequencer repo)                   │
│  Gap-free sequencing · Merkle commitments · Agent key registry          │
│  STARK compliance proofs · x402 payment batch processing                │
└──────────────────────────────┬───────────────────────────────────────────┘
                               │ Merkle roots + STARK proofs
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  Layer 1: SET Chain L2 (set repo, Chain ID 84532001)                    │
│  OP Stack · 2s blocks · SetRegistry (anchoring) · SetPaymaster (gas)   │
│  ssUSD stablecoin (yield-bearing, T-Bill backed) · Ethereum L1 DA      │
└──────────────────────────────────────────────────────────────────────────┘
```

Each layer is independently verifiable — no layer trusts the one above it. See [The StateSet Trilogy](trilogy/overview.md) for the full protocol architecture.

## iCommerce Internal Architecture

## System Diagram

```
                          ┌──────────────────────────────────┐
                          │       Admin Dashboard            │
                          │     (Next.js + TypeScript)       │
                          └───────────────┬──────────────────┘
                                          │
┌─────────────────────────────────────────┼──────────────────────────────────────┐
│                            CLI + MCP Server                                    │
│                                                                                │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│   │ 18 Agent │  │  520+MCP │  │  Policy  │  │   Sync   │  │Autonomous│      │
│   │ Configs  │  │  Tools   │  │  Engine  │  │  Engine  │  │  Engine  │      │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
│        └──────────────┼──────────────┼──────────────┼──────────────┘           │
│                       │              │              │                           │
│   ┌───────────────────┴──────────────┴──────────────┴───────────────────────┐  │
│   │                    Thin MCP Orchestrator (470 lines)                     │  │
│   │     adaptTool() → permission → telemetry → handler → response           │  │
│   └───────────────────┬──────────────┬──────────────────────────────────────┘  │
│                       │              │                                          │
│   ┌───────────────────┴──────────────┴──────────────────────────────────────┐  │
│   │                     A2A + x402 Protocols                                 │  │
│   │     Payments · Quotes · Escrow · Splits · Subscriptions                  │  │
│   │     Payment Intents · Ed25519 · Budget · Settlement                      │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────┬─────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼─────────────────────────────────────────────┐
│                        Language Bindings (11 platforms)                         │
│   Node (NAPI) · Python (PyO3) · Ruby (Magnus) · PHP (ext-php-rs)              │
│   Go (cgo) · Java (JNI) · Kotlin · Swift · .NET (P/Invoke) · WASM            │
└──────────────────────────────────┬─────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼─────────────────────────────────────────────┐
│                          Rust Core (21 Crates)                                 │
│                                                                                │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                       │
│   │  Primitives  │  │     Core     │  │    Crypto    │                       │
│   │  (IDs, Money │  │ (50+ models, │  │  (VES v1.0)  │                       │
│   │   Sku, Curr) │  │  25 domains) │  │              │                       │
│   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                       │
│          └─────────────────┼─────────────────┘                                │
│                      ┌─────┴─────┐                                            │
│                      │    DB     │                                            │
│                      │ SQLite +  │                                            │
│                      │ PostgreSQL│                                            │
│                      └───────────┘                                            │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐          │
│   │ Policy  │  │   A2A   │  │ Pricing │  │  Authz  │  │  Sync   │          │
│   └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘          │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐          │
│   │  Jobs   │  │  HTTP   │  │Protocol │  │Observ.  │  │  FFI    │          │
│   └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘          │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Rust Core (21 Crates)

| Crate | Purpose | Key Characteristic |
|-------|---------|-------------------|
| `stateset-primitives` | Strongly-typed IDs and value objects | Zero dependencies, `Copy + Eq + Hash` |
| `stateset-core` | Domain models, repository traits, errors | Pure logic, no I/O |
| `stateset-crypto` | VES v1.0 cryptographic operations | Memory-safe, keys zeroized after use |
| `stateset-db` | SQLite + PostgreSQL implementations | Trait-based backend switching |
| `stateset-embedded` | Unified high-level API surface | Primary binding target |
| `stateset-policy` | Declarative rule engine | YAML/JSON rule definitions |
| `stateset-a2a` | Agent-to-Agent commerce primitives | Split payments, escrow, subscriptions |
| `stateset-pricing` | Deterministic pricing engine | Pure functions, WASM-compatible |
| `stateset-authz` | Authorization, RBAC, rate limiting | IO-free, framework-agnostic |
| `stateset-observability` | Metrics, tracing, OpenTelemetry | Lock-free atomic counters |
| `stateset-protocol` | Wire-format types for sync | IO-free, WASM-compatible |
| `stateset-sync` | Event-sourcing sync engine | Outbox pattern, conflict resolution |
| `stateset-http` | Axum REST + SSE server | Auth, CORS, tracing middleware |
| `stateset-jobs` | Background job scheduler | Cron, intervals, retries |
| `stateset-ffi` | Stable C ABI for bindings | `#[repr(C)]`, ABI versioning |
| `stateset-macros` | Procedural macros | Code generation for domain models |
| `stateset-migrations` | Database schema migrations | Checksummed, rollback support |
| `stateset-sdk` | Facade with feature gates | Single entry point |
| `stateset-test-utils` | Shared test fixtures | Builder pattern, assertion macros |
| `stateset-benches` | Criterion benchmarks | Performance regression detection |
| `stateset-integration-tests` | Cross-crate tests | End-to-end validation |

## CLI + MCP Server

The CLI layer is written in JavaScript (ES modules, Node 18+) and consists of:

- **MCP Orchestrator** (`mcp-server.js`, 470 lines) — thin router that delegates to tool modules
- **48 tool modules** (`tools/`) — 520+ MCP tool definitions with Zod validation
- **44 A2A modules** (`a2a/`) — agent-to-agent commerce protocol implementation
- **3 platform adapters** (`adapters/`) — Stripe, WooCommerce, Shopify sync
- **x402 client** (`x402/`) — payment intent signing, budget governance, circuit breaker
- **18 agent configurations** (`agent-definitions.js`) — specialized commerce agents
- **Policy engine** — YAML rule evaluation with explainable denials

## Language Bindings

Bindings are generated from a single declarative spec (see [ADR-0005](adr/0005-binding-generation.md)). Each binding provides the same 41 domain API surface in language-idiomatic style:

| Binding | Technology | Platform |
|---------|-----------|----------|
| Node.js | NAPI-RS | Linux, macOS, Windows |
| Python | PyO3 | Linux, macOS, Windows |
| Ruby | Magnus | Linux, macOS |
| PHP | ext-php-rs | Linux, macOS, Windows |
| Go | cgo | Linux, macOS, Windows |
| Java | JNI | JVM (all) |
| Kotlin | JNI | JVM + Android |
| Swift | C FFI | macOS, iOS |
| C# / .NET | P/Invoke | Windows, Linux, macOS |
| WASM | wasm-bindgen | Browsers, Node, Deno, Workers |

## Admin Dashboard

A Next.js + TypeScript web application providing:

- Order, product, customer, and inventory management views
- Analytics dashboards with revenue and inventory metrics
- Subscription and return processing workflows
- Agent status monitoring
- Gateway and settings configuration
- 261 tests (Vitest + jsdom)

## External Integrations

### StateSet Sequencer (Tier 2+)

When configured with a sequencer URL, iCommerce syncs commerce events for multi-agent coordination:

- Events signed with Ed25519 and submitted via the [VES v1.0](security/ves.md) protocol
- Sequencer assigns gap-free sequence numbers and issues signed receipts
- Merkle commitments enable efficient event verification
- See [Sequencer](trilogy/sequencer.md) for details

### SET Chain (Tier 3)

When configured with a chain RPC, iCommerce gains on-chain settlement:

- Merkle commitments anchored to [SetRegistry](trilogy/set-chain.md) for trustless verification
- Gas abstracted via [SetPaymaster](trilogy/set-chain.md) — agents never hold ETH
- Settlement in [ssUSD](trilogy/ssusd.md) yield-bearing stablecoin
- [STARK proofs](trilogy/stark-proofs.md) for zero-knowledge compliance

### StateSet STARK (Tier 3)

Zero-knowledge compliance proofs enable privacy-preserving regulatory verification:

- Prove transaction amounts satisfy AML thresholds without revealing amounts
- Batch proofs aggregate 64–128 events into a single STARK proof
- See [STARK Compliance Proofs](trilogy/stark-proofs.md)

## Key Design Decisions

For the rationale behind major structural choices, see the [Architecture Decision Records](adr/README.md).
