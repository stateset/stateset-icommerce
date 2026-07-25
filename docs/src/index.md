# StateSet iCommerce

**The Embedded Commerce Engine for Autonomous AI Agents**

StateSet iCommerce is a portable, AI-native commerce engine that runs in-process with zero external dependencies. Built on a type-safe Rust core with multi-language bindings, it provides a broad commerce and ERP surface area as deterministic operations that AI agents can safely invoke.

Think of it as the **SQLite of Commerce**: embed a full commerce engine in any application, in any language, with a single dependency.

Current release: **1.22.0**

Before depending on this stack for regulated or infrastructure-grade workloads, read the
[Trust Foundation](trust-foundation.md). It states the current guarantees, residual trust
assumptions, and the boundary between implemented, adjacent-repo, and planned capabilities.

## See It In Action

An AI agent receives the instruction: *"Process a return for order #1234."*

```
1. Agent calls list_orders to find order #1234              → read-only, no --apply
2. Agent calls evaluate_policy to check the return window   → policy says: allowed
3. Agent calls create_return with --apply                   → preview: "Would create RMA, refund $29.99"
4. User confirms → return created, refund issued, inventory adjusted
5. Event signed with Ed25519, synced to sequencer           → tamper-proof audit trail
```

Every step is deterministic, policy-governed, and cryptographically verifiable. No dashboard needed.

## Why iCommerce?

Traditional commerce platforms assume a human in the loop. iCommerce assumes the operator is an AI agent:

| Traditional Platform | iCommerce |
|---------------------|-----------|
| REST APIs with opaque `400 Bad Request` errors | Structured errors with per-field explanations LLMs can reason about |
| Webhook pipelines for integration | Cryptographically signed event streams with replay |
| Dashboard for monitoring | Machine-readable health checks, heartbeat alerts |
| Manual approval workflows | Policy engine with programmatic deny/allow and auto-remediation |
| Separate server to deploy | Embeddable library that runs in-process, zero dependencies |
| $0.30 minimum Stripe charge | Sub-cent agent-to-agent payments via x402 protocol |

## What You Get

| Layer | What It Provides |
|-------|-----------------|
| **Rust Core** | Type-safe domain models, typed IDs, explicit state machines |
| **Commerce Engine** | Orders, inventory, payments, returns, subscriptions, manufacturing, tax, analytics, loyalty, reviews, and more |
| **MCP Tools** | Commerce operations exposed via the Model Context Protocol |
| **A2A Protocol** | Agent-to-agent payments, quotes, escrow, splits, reputation, disputes |
| **x402 Protocol** | Cryptographically signed payment intents with budget governance |
| **VES v1.0** | Ed25519 signatures, AES-256-GCM encryption, Merkle proofs |
| **Policy Engine** | Declarative YAML rules with explainable denials |
| **Platform Adapters** | Stripe, WooCommerce, Shopify sync with real-time webhooks |
| **Language Bindings** | Rust, Node.js, Python, Ruby, PHP, Java, Kotlin, Swift, .NET, Go, WASM |
| **[Sequencer](trilogy/sequencer.md)** (Tier 2) | Gap-free event ordering, Merkle commitments, agent key registry |
| **[SET Chain L2](trilogy/set-chain.md)** (Tier 3) | On-chain anchoring, gas abstraction, 2-second blocks |
| **[STARK Proofs](trilogy/stark-proofs.md)** (Tier 3) | Zero-knowledge compliance (AML, order caps) without revealing amounts |
| **[ssUSD](trilogy/ssusd.md)** (Tier 3) | Yield-bearing stablecoin (~5% APY, T-Bill backed) |

## The StateSet Trilogy

iCommerce is the application layer of a vertically integrated, three-layer protocol stack:

```
┌──────────────────────────────────────────────────────────────────┐
│  Layer 3: iCommerce (Application)                                │
│  AI Agents · MCP Tools · A2A Protocol · Policy Engine           │
└──────────────────────────────┬───────────────────────────────────┘
                               │ VES v1.0 signed events
┌──────────────────────────────┼───────────────────────────────────┐
│  Layer 2: Sequencer (Ordering)                                   │
│  Gap-free sequencing · Merkle commitments · STARK proofs        │
└──────────────────────────────┬───────────────────────────────────┘
                               │ Merkle roots
┌──────────────────────────────┼───────────────────────────────────┐
│  Layer 1: SET Chain L2 (Settlement)                              │
│  On-chain anchoring · Gas abstraction · ssUSD stablecoin        │
└──────────────────────────────────────────────────────────────────┘
```

Each layer is independently verifiable — no layer trusts the one above it. See [The StateSet Trilogy](trilogy/overview.md).

## Three Tiers

```
Tier 1: Standalone       Free, open source. SQLite + CLI + adapters.
   └─ Add sync.json →
Tier 2: Sequencer        Enterprise. Adds VES sync, multi-agent coordination, audit trails.
   └─ Add chain RPC →
Tier 3: Full Trilogy     Settlement. Adds SET Chain L2, ssUSD stablecoin, on-chain anchoring.
```

No data migration between tiers. See [Product Tiers](tiers.md).

## Where to Start

### I'm building AI agents that handle commerce

1. [What is iCommerce?](concepts/icommerce.md) — Understand the paradigm shift
2. [Getting Started](getting-started.md) — Install and run in 60 seconds
3. [AI Agent Quickstart](ai-agents.md) — OpenAI, Vercel AI SDK, LangChain, MCP
4. [Policy Engine](policy/engine.md) — Safety guardrails for autonomous agents
5. [MCP Tools](guides/mcp-tools.md) — hundreds of operations your agent can call

### I'm connecting an existing store (Shopify, Stripe, WooCommerce)

1. [Standalone Quickstart](standalone-quickstart.md) — 5-minute setup
2. [Adapter Overview](adapters/overview.md) — Which adapter to use
3. [Stripe](adapters/stripe.md) / [Shopify](adapters/shopify.md) / [WooCommerce](adapters/woocommerce.md)

### I'm building agent-to-agent commerce

1. [A2A Protocol Overview](a2a/overview.md) — How agents transact
2. [Case Studies](concepts/case-studies.md) — Real-world scenarios
3. [x402 Payments](payments/x402.md) — Cryptographic payment intents
4. [Budget Governance](payments/budget.md) — Spending controls

### I want to understand the full protocol stack

1. [The StateSet Trilogy](trilogy/overview.md) — Three-layer architecture overview
2. [Sequencer](trilogy/sequencer.md) — Deterministic event ordering and commitments
3. [SET Chain L2](trilogy/set-chain.md) — Commerce-optimized L2 with gas abstraction
4. [STARK Proofs](trilogy/stark-proofs.md) — Zero-knowledge compliance verification
5. [ssUSD Stablecoin](trilogy/ssusd.md) — Yield-bearing settlement asset
6. [Trust Foundation](trust-foundation.md) — exact guarantees, gaps, and residual trust assumptions

### I'm deploying to production

1. [Architecture](architecture.md) — System design
2. [Deployment](advanced/deployment.md) — Docker, Kubernetes, PostgreSQL
3. [Security Architecture](security/architecture.md) — Hardening guide
4. [Compliance & Audit](advanced/compliance.md) — GDPR, SOC 2, audit trails
5. [Testing Strategy](advanced/testing.md) — quality gates across the Rust, CLI, admin, and binding surfaces

Current workspace and binding counts live in [Workspace Inventory](appendix/workspace-inventory.md).
6. [Trust Foundation](trust-foundation.md) — current trust posture and open gaps

Current release: **1.22.0**
