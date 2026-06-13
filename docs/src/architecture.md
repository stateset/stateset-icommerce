# Architecture

StateSet iCommerce is the application layer of the StateSet Trilogy — a vertically integrated, three-layer protocol stack for autonomous AI agent commerce. Within iCommerce itself, the architecture is organized into three internal layers: a Rust core, language bindings, and the CLI + MCP server.

## Trilogy Stack

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Layer 3: StateSet iCommerce (this project)                              │
│  AI Agents · MCP Tools · A2A Protocol · Policy Engine                   │
│  Platform Adapters (Stripe, Shopify, WooCommerce) · Multi-language Bindings│
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

The repository has three distinct rings:

1. A layered Rust kernel
2. Language bindings that package the kernel for different runtimes
3. Operator-facing Node and web surfaces (`cli/` and `admin/`)

The current workspace manifests describe this dependency direction:

```text
stateset-primitives | stateset-crypto | stateset-pricing | stateset-observability
stateset-policy | stateset-authz | stateset-a2a | stateset-jobs
stateset-migrations | stateset-macros
        ->
stateset-core | stateset-sync
        ->
stateset-db
        ->
stateset-embedded
        ->
stateset-http | stateset-sdk | bindings/*
        ->
admin | cli
```

That graph matters more than the directory count, because it tells you where changes flow.

## Layer Responsibilities

| Layer | Primary crates/surfaces | Purpose |
|-------|--------------------------|---------|
| Foundation | `stateset-primitives`, `stateset-crypto`, `stateset-pricing`, `stateset-observability`, `stateset-policy`, `stateset-authz`, `stateset-a2a`, `stateset-jobs`, `stateset-migrations`, `stateset-macros` | Narrow capabilities and cross-cutting building blocks |
| Domain kernel | `stateset-core`, `stateset-sync` | Commerce models, wire contracts, and sync/runtime logic |
| Persistence + product API | `stateset-db`, `stateset-embedded` | Storage and the main embeddable commerce surface |
| Edge adapters | `stateset-http`, `stateset-sdk`, `stateset-ffi`, `bindings/*` | Transport, Rust facade, C-style interop, and runtime packaging |
| Operator surfaces | `cli/`, `admin/` | MCP, agents, automation, and operations UI |

## High-Leverage Crates

- `stateset-core` is the main fan-in point for the product/runtime graph. It is the cleanest place to understand the commerce model, and the easiest place to create wide blast radius if you change it carelessly.
- `stateset-db` is where backend-specific behavior and feature parity become concrete.
- `stateset-embedded` is the main productized Rust API and the key fan-out point into HTTP and the binding layer.
- `stateset-http` is an important edge adapter, but it is not the architectural center of gravity.

## Binding Topology

The binding story is more direct than a generic SDK wrapper model:

- `bindings/node` links directly to `stateset-embedded`, `stateset-core`, `stateset-db`, and `stateset-crypto`.
- The admin app and CLI both consume `@stateset/embedded` directly.
- `bindings/python` links directly to `stateset-embedded`, `stateset-core`, `stateset-primitives`, `stateset-db`, and `stateset-sdk`.
- Go, Swift, Java, Kotlin, and .NET also link directly to `stateset-embedded` and `stateset-core`.
- `stateset-sdk` is the Rust-facing facade crate for feature-gated re-exports.
- `stateset-ffi` is an optional C-ABI oriented interop surface. It is useful for explicit C-style integration, but it is not the mandatory substrate for every binding in this repository.
- Ruby and PHP remain in the repo, but they are intentionally excluded from default workspace membership because they depend on host runtimes or headers.

## CLI and Admin Surfaces

The outer ring is large enough to treat as its own product surface:

- `cli/` is a Node 20.20+ ES module runtime with the MCP servers, tool registry, sync/x402 flows, agent routing, messaging channels, and scaffolding logic.
- `admin/` is a Next.js + TypeScript operations surface that loads the local Node binding at runtime.
- The root quality pipeline validates release hygiene, Rust fmt/tests/lints/feature-matrix checks, shell scripts, the Node binding, the admin app, and the CLI together.

## Recommended Onboarding Order

Read the codebase in this order:

1. `stateset-core`
2. `stateset-db`
3. `stateset-embedded`
4. `stateset-sync`, `stateset-policy`, `stateset-authz`, `stateset-pricing`
5. `stateset-http`
6. `bindings/node`
7. `admin/`
8. `cli/`

That sequence follows the actual dependency direction and keeps the largest operator-facing surfaces for last.

For a more explicit walkthrough, see [Dependency Direction](guides/dependency-direction.md).

## External Integrations

### StateSet Sequencer (Tier 2+)

When configured with a sequencer URL, iCommerce syncs commerce events for multi-agent coordination:

- Events signed with Ed25519 and submitted via the [VES v1.0](security/ves.md) protocol
- Local event-log and outbox ordering stay provisional; only the sequencer assigns canonical distributed sequence numbers
- Sequencer assigns gap-free sequence numbers, returns canonical acknowledgements, and issues signed receipts
- The Rust sync layer now includes a concrete `SequencerHttpTransport`; canonical remote cursor state, latest sequencer commitment metadata, retained push confirmations, inspectable dead-lettered rejections, and core VES envelope metadata on `SyncEvent` are all preserved separately from the local outbox
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
