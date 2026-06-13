# StateSet iCommerce: An Embedded Commerce Engine for Autonomous AI Agents

**Technical Whitepaper v0.7.15**
**March 2026**

---

## Abstract

StateSet iCommerce is an embedded, zero-dependency commerce engine designed for autonomous AI agents. Built on a Rust core with language bindings for 10 platforms, it provides a complete commerce and ERP surface area — orders, inventory, payments, returns, subscriptions, manufacturing, and more — as deterministic, locally executable operations that AI agents can safely invoke. The system introduces three novel protocols: the **Agent-to-Agent (A2A) Commerce Protocol** for autonomous economic transactions between agents, the **x402 Payment Protocol** for cryptographically verifiable payment intents, and the **Verifiable Encrypted Signatures (VES) v1.0** specification for tamper-proof event synchronization. iCommerce exposes 700+ tools via the Model Context Protocol (MCP), governed by a declarative policy engine with explainable denials, and is backed by comprehensive automated tests across all layers. The result is a portable, embeddable commerce runtime — the "SQLite of Commerce" — that enables AI agents to reason about, decide on, and execute commerce operations independently.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Use Cases: Agents in Production](#2-use-cases-agents-in-production)
3. [Design Principles](#3-design-principles)
4. [System Architecture](#4-system-architecture)
5. [The Agentic Reasoning Loop](#5-the-agentic-reasoning-loop)
6. [Rust Core: Domain Model & Type System](#6-rust-core-domain-model--type-system)
7. [Database Layer: Local-First Persistence](#7-database-layer-local-first-persistence)
8. [MCP Tool Surface: 365 Deterministic Operations](#8-mcp-tool-surface-365-deterministic-operations)
9. [Agent-to-Agent (A2A) Commerce Protocol](#9-agent-to-agent-a2a-commerce-protocol)
10. [x402 Payment Protocol](#10-x402-payment-protocol)
11. [Verifiable Encrypted Signatures (VES) v1.0](#11-verifiable-encrypted-signatures-ves-v10)
12. [Policy Engine: Declarative Safety Guardrails](#12-policy-engine-declarative-safety-guardrails)
13. [Autonomous Engine: Self-Governing Commerce](#13-autonomous-engine-self-governing-commerce)
14. [Multi-Agent System: Specialized Commerce Agents](#14-multi-agent-system-specialized-commerce-agents)
15. [Sync Architecture: Eventually Consistent Multi-Agent State](#15-sync-architecture-eventually-consistent-multi-agent-state)
16. [Observability & Telemetry](#16-observability--telemetry)
17. [Security Architecture](#17-security-architecture)
18. [Language Bindings & Portability](#18-language-bindings--portability)
19. [Admin Dashboard](#19-admin-dashboard)
20. [Testing & Quality Assurance](#20-testing--quality-assurance)
21. [Performance](#21-performance)
22. [Related Work](#22-related-work)
23. [Roadmap](#23-roadmap)
24. [Conclusion](#24-conclusion)

---

## 1. Introduction

### 1.1 From eCommerce to iCommerce

The commerce software stack has remained structurally unchanged for two decades: a centralized server exposes REST APIs, human operators manage state through dashboards, and integration is achieved through webhook pipelines and manual orchestration. This architecture assumes a human in the loop at every decision point.

The emergence of autonomous AI agents — systems capable of reasoning, planning, and executing multi-step operations — demands a fundamentally different commerce runtime. Agents do not need dashboards; they need deterministic APIs. They do not need webhook pipelines; they need cryptographically verifiable event streams. They do not need manual integration; they need portable, embeddable libraries they can carry with them.

**iCommerce** (Intelligent Commerce) is the paradigm shift from human-operated commerce platforms to agent-native commerce engines. StateSet iCommerce is the reference implementation of this paradigm.

### 1.2 Design Goals

1. **Embeddable**: Run in-process with zero external dependencies, like SQLite for databases
2. **Deterministic**: Same inputs produce identical outputs — safe for automated execution
3. **Portable**: Consistent APIs across 10 language bindings from a single Rust core
4. **Agent-Native**: First-class primitives for agent-to-agent payments, escrow, reputation, and trust
5. **Verifiable**: Every state mutation is cryptographically signed and Merkle-provable
6. **Safe**: Policy-governed execution with explainable denials and audit trails

### 1.3 Contributions

This paper presents:

- A **type-safe domain model** with 24 strongly-typed entity IDs, domain-specific error hierarchies, and explicit state machines for every aggregate
- The **A2A Commerce Protocol** enabling autonomous economic transactions between AI agents, including direct payments, quote negotiation, escrow, split payments, subscriptions, and dispute resolution
- The **x402 Payment Protocol** for off-chain payment intents with Ed25519 signatures and gas-abstracted on-chain settlement across multiple blockchain networks
- **VES v1.0**, a cryptographic specification combining RFC 8785 JSON Canonicalization, domain-separated SHA-256 hashing, Ed25519 signatures, AES-256-GCM encryption, and Merkle tree proofs
- A **declarative policy engine** with deny-override semantics, per-condition explainability, and transform audit trails
- An **MCP tool surface** of 700+ commerce operations, the largest known domain-specific MCP server

---

## 2. Use Cases: Agents in Production

To ground the abstract architecture in concrete business scenarios, we present three representative agentic workflows that iCommerce enables today.

### 2.1 Autonomous Supply Chain Procurement

An **Inventory Agent** monitors stock levels via the heartbeat system and detects that Widget-A has fallen below its reorder threshold. Without human intervention, it:

1. Queries the supplier registry and identifies three qualified vendor agents
2. Issues an `a2a_request_quote` to each vendor with the required SKU, quantity, and delivery window
3. Receives competing quotes (the RFQ protocol caps negotiation at 5 rounds to prevent infinite loops)
4. Evaluates quotes against procurement policy rules (price ceiling, lead time, supplier reputation score)
5. Accepts the best quote via `a2a_accept_quote`, which creates a purchase order and an x402 payment intent
6. Funds are held in escrow with a `seller_fulfilled` condition — released only when the vendor agent confirms shipment
7. Upon delivery confirmation, inventory is automatically adjusted, and the VES sync system propagates the event to all subscribed agents

**Total human involvement: zero.** The entire flow executes within the policy guardrails set by the operations team and is fully auditable through cryptographically signed event logs.

### 2.2 Micro-Payment API Economy

A **Research Agent** needs real-time pricing data from a **Market Data Agent** that charges $0.02 per API call. The interaction is:

1. The Research Agent discovers the Market Data Agent via its ERC-8004 Agent Card, which declares capabilities and pricing
2. It calls `x402Fetch()`, which automatically attaches a signed payment header to each HTTP request
3. The Market Data Agent verifies the payment signature, serves the data, and returns a receipt
4. Budget governance caps the Research Agent at $5/day — if the budget is exhausted, a `BudgetExceededError` halts further requests rather than silently overspending
5. At end-of-day, the x402 sequencer batch-settles all accumulated micro-intents on-chain in a single transaction

This pattern enables an ecosystem where agents pay agents for services at machine speed, with sub-cent granularity and cryptographic accountability.

### 2.3 End-to-End Order Fulfillment

A **Customer Service Agent** receives a natural language order request via the messaging gateway. It:

1. Creates a cart, applies a promotional discount (validated by the policy engine), and calculates tax
2. Processes payment via x402, receiving a signed receipt
3. The **Fulfillment Agent** picks up the order event via SSE streaming, reserves inventory, and creates a shipment
4. Tracking events propagate in real-time to the customer via webhook notification
5. If the customer initiates a return, the **Returns Agent** evaluates the return policy (window, condition, reason), creates an RMA, and issues a refund — all within policy-defined guardrails

Each agent operates with its own tool permissions, budget limits, and policy constraints, yet they collaborate seamlessly through the shared event log and A2A protocol.

---

## 3. Design Principles

### 3.1 Local-First Execution

iCommerce runs entirely in-process using SQLite as its default storage backend. No network calls, no external services, no containers. An agent can `npm install @stateset/embedded` and have a full commerce engine running in the same process. This eliminates latency, reduces failure modes, and enables offline-first operation.

### 3.2 Deterministic Operations

Every operation in the commerce engine is a pure function of its inputs and the current database state. There are no hidden side effects, no background timers affecting computation, and no non-deterministic behavior. This property is critical for AI agents: it means operations can be safely replayed, simulated, and reasoned about.

### 3.3 Type Safety Through Newtypes

The Rust core uses strongly-typed newtypes for all entity identifiers. An `OrderId` cannot be accidentally passed where a `CustomerId` is expected — the compiler rejects it. This prevents an entire class of bugs that are common in stringly-typed commerce systems.

### 3.4 Explicit State Machines

Every domain aggregate (Order, Payment, Return, Subscription, WorkOrder) has an explicit state machine with validated transitions. The `can_transition_to()` method returns whether a transition is valid, and `is_terminal()` indicates whether further transitions are possible. Invalid transitions produce typed errors rather than silently corrupting state.

### 3.5 Preview Before Execute

All write operations are blocked by default. The `--apply` flag must be explicitly provided to enable mutations. Without it, every operation returns a preview of what would happen — how many records would be affected, what state changes would occur — without actually executing. This safety model is essential for autonomous agents operating at scale.

---

## 4. System Architecture

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
│   │ 18 Agent │  │ 700+ MCP │  │  Policy  │  │   Sync   │  │Autonomous│      │
│   │ Configs  │  │  Tools   │  │  Engine  │  │  Engine  │  │  Engine  │      │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
│        └──────────────┼──────────────┼──────────────┼──────────────┘           │
│                       │              │              │                           │
│   ┌───────────────────┴──────────────┴──────────────┴───────────────────────┐  │
│   │                    Thin MCP Orchestrator                                 │  │
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
│                        Language Bindings                                        │
│   Node (NAPI) · Python (PyO3) · Ruby (Magnus) · PHP (ext-php-rs)              │
│   Go (cgo) · Java (JNI) · Kotlin · Swift · .NET (P/Invoke) · WASM            │
└──────────────────────────────────┬─────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼─────────────────────────────────────────────┐
│                          Rust Core (21 Crates)                                 │
│                                                                                │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                       │
│   │  Primitives  │  │     Core     │  │    Crypto    │                       │
│   │  (IDs, Money │  │ (25 domains, │  │  (VES v1.0)  │                       │
│   │   Sku, Curr) │  │  50 repos)   │  │              │                       │
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

The system is organized into three layers:

1. **Rust Core** (21 crates): Pure domain models, database abstraction, cryptographic primitives, policy evaluation, and pricing calculations — all with zero I/O side effects in the core logic
2. **Language Bindings**: FFI layer exposing the Rust core to 10 programming languages, each with idiomatic APIs
3. **CLI + MCP Server**: The agent-facing interface, providing 700+ tools via the Model Context Protocol, 18 specialized agents, and the A2A/x402 protocol implementations

---

## 5. The Agentic Reasoning Loop

Understanding how an LLM interacts with iCommerce is critical to understanding the system's design. Every agent operation follows a structured reasoning loop that combines LLM intelligence with deterministic execution:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         LLM Reasoning Engine                             │
│                    (Claude, GPT-4, Gemini, Ollama)                       │
└────────┬───────────────────────────────┬────────────────────────────────┘
         │                               │
    1. Natural Language              6. Observe Result
    Intent / Context                 & Reason About
         │                           Next Step
         ▼                               ▲
┌────────────────┐              ┌────────────────┐
│  2. Select     │              │  5. Execute    │
│  MCP Tool      │              │  State Change  │
│  (from 700+)   │              │  (if --apply)  │
└───────┬────────┘              └───────┬────────┘
        │                               ▲
        ▼                               │
┌────────────────┐              ┌────────────────┐
│  3. Preview    │──[allowed]──►│  4. Policy     │
│  (dry run)     │              │  Evaluation    │
│                │              │                │
│  Returns what  │              │  Deny → return │
│  would change  │              │  structured    │
│                │◄─[denied]────│  explanation   │
└────────────────┘              │  with remedy   │
                                └────────────────┘
                                        │
                                        ▼
                                ┌────────────────┐
                                │  7. Sign &     │
                                │  Sync Event    │
                                │  (VES v1.0)    │
                                └────────────────┘
```

**Why preview-first matters for LLMs:** When an agent calls a tool without `--apply`, it receives a structured preview showing exactly what would change — affected record counts, before/after states, and validation results. The LLM can reason about this preview, confirm it matches the user's intent, and only then issue the mutating call. This eliminates the "fire and forget" pattern that makes autonomous agents dangerous.

**Why explainable denials matter for LLMs:** Traditional APIs return opaque error codes (`400 Bad Request`) that cause LLMs to retry the same failing request in a loop. iCommerce's policy engine returns structured denials with per-condition breakdowns: which field failed, what was expected vs. actual, and a human-readable remediation string. This explanation flows directly into the LLM's context window, enabling the agent to autonomously correct its parameters and retry without human intervention.

---

## 6. Rust Core: Domain Model & Type System

### 6.1 Crate Organization

| Crate | Purpose | Key Characteristic |
|-------|---------|-------------------|
| `stateset-primitives` | Strongly-typed IDs and value objects | Zero dependencies, `Copy + Eq + Hash` |
| `stateset-core` | Domain models, repository traits, errors | Pure logic, no I/O |
| `stateset-crypto` | VES v1.0 cryptographic operations | Memory-safe, keys zeroized after use |
| `stateset-db` | SQLite + PostgreSQL implementations | Trait-based backend switching |
| `stateset-embedded` | Unified high-level API surface | Primary binding target |
| `stateset-policy` | Declarative rule engine | YAML/JSON rule definitions |
| `stateset-a2a` | Agent-to-Agent commerce | Split payments, escrow, subscriptions |
| `stateset-pricing` | Deterministic pricing engine | Pure functions, WASM-compatible |
| `stateset-authz` | Authorization, RBAC, rate limiting | IO-free, framework-agnostic |
| `stateset-observability` | Metrics, tracing, OpenTelemetry | Lock-free atomic counters |
| `stateset-sync` | Event-sourcing sync engine + wire types | Outbox pattern, conflict resolution |
| `stateset-http` | Axum REST + SSE server | Auth, CORS, tracing middleware |
| `stateset-jobs` | Background job scheduler | Cron, intervals, retries |
| `stateset-ffi` | Stable C ABI | `#[repr(C)]`, ABI versioning |
| `stateset-macros` | Procedural macros | Code generation for domain models |
| `stateset-migrations` | Database schema migrations | Checksummed, rollback support |
| `stateset-sdk` | Facade with feature gates | Single entry point |
| `stateset-test-utils` | Shared test fixtures | Builder pattern, assertion macros |
| `stateset-benches` | Criterion benchmarks | Performance regression detection |
| `stateset-integration-tests` | Cross-crate tests | End-to-end validation |

### 6.2 Strongly-Typed Entity Identifiers

All entity identifiers are newtype wrappers around `Uuid`, providing compile-time safety:

```rust
// These are distinct types — the compiler prevents mixing them
pub struct OrderId(Uuid);
pub struct CustomerId(Uuid);
pub struct ProductId(Uuid);
pub struct PaymentId(Uuid);
pub struct InventoryItemId(Uuid);
pub struct SubscriptionId(Uuid);
pub struct CartId(Uuid);
pub struct ShipmentId(Uuid);
pub struct ReturnId(Uuid);
pub struct InvoiceId(Uuid);
pub struct AgentId(Uuid);
pub struct PromotionId(Uuid);
// ... 24 total ID types

// Compile-time error: cannot pass OrderId where CustomerId expected
fn get_customer(id: CustomerId) -> Customer { ... }
get_customer(order_id); // ERROR: mismatched types
```

All ID types derive `Copy`, `Eq`, `Hash`, `Serialize`, `Deserialize`, and `Display`. The `#[must_use]` attribute ensures that constructed IDs are always consumed.

### 6.3 Value Types

```rust
/// Monetary amount with currency safety
#[must_use]
pub struct Money {
    pub amount: Decimal,
    pub currency: CurrencyCode,
}

/// ISO 4217 currency code
#[must_use]
pub struct CurrencyCode([u8; 3]);

/// Validated SKU string
#[must_use]
pub struct Sku(String);
```

`Money` arithmetic operations (`checked_add`, `checked_sub`, `checked_mul_scalar`, `checked_div_scalar`) enforce currency matching at runtime and return `None` on overflow. The `is_negative()` method correctly handles negative zero.

### 6.4 Domain Models

iCommerce covers 25+ commerce domains:

| Domain | Key Types | Status States |
|--------|-----------|---------------|
| Orders | `Order`, `OrderItem`, `Address` | Pending → Confirmed → Processing → Shipped → Delivered |
| Payments | `PaymentTransaction`, `RefundRecord` | Pending → Processing → Completed / Failed / Disputed |
| Inventory | `InventoryItem`, `InventoryReservation` | 8 transaction types (Receipt, Shipment, Adjustment, ...) |
| Returns | `Return`, `ReturnItem` | Requested → Approved → InTransit → Received → Completed |
| Subscriptions | `SubscriptionPlan`, `Subscription` | Trial → Active → Paused → PastDue → Cancelled |
| Manufacturing | `BillOfMaterials`, `WorkOrder` | Draft → Active → InProgress → Completed |
| Customers | `Customer`, `CustomerAddress` | Active, Inactive, Suspended |
| Products | `Product`, `ProductVariant` | Active, Draft, Archived |
| Shipments | `Shipment`, `TrackingEvent` | Pending → Packed → Shipped → Delivered |
| x402 | `X402PaymentIntent` | Pending → Submitted → Completed / Failed |
| A2A | `A2APayment`, `A2AQuote` | Pending → Submitted → Completed / Failed |
| ERC-8004 | `AgentIdentity`, `AgentCard` | Agent registration and capability discovery |

All status enums are annotated with `#[non_exhaustive]` for forward compatibility and derive `strum::Display` for zero-allocation string conversion.

### 6.5 State Machine Enforcement

Every aggregate with a lifecycle implements explicit state transitions:

```rust
impl OrderStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Confirmed)
            | (Self::Confirmed, Self::Processing)
            | (Self::Processing, Self::Shipped)
            | (Self::Shipped, Self::Delivered)
            | (Self::Pending | Self::Confirmed, Self::Cancelled)
            // ...
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Delivered | Self::Cancelled | Self::Refunded)
    }
}
```

Invalid transitions produce a `StateTransitionError<S>` carrying the attempted and expected states, enabling clear error messages that LLMs can interpret and act on.

### 6.6 Error Architecture

Errors form a two-level hierarchy designed around one principle: **every error must tell the caller what to do next.**

```
CommerceError
├── OrderError
├── InventoryError
├── PaymentError
├── ReturnError
├── ShippingError
├── CustomerError
├── ProductError
├── DbError
├── StateTransitionError<S>
├── ValidationError
└── BatchResult<T>  (partial success tracking)
```

Every `CommerceError` exposes categorization methods: `is_not_found()`, `is_validation()`, `is_conflict()`, `is_database()`, `is_retryable()`. The `is_retryable()` method is critical for agent retry logic — only transient failures (deadlocks, connection timeouts) return `true`, preventing agents from endlessly retrying permanent failures.

### 6.7 Repository Trait System

Data access is abstracted through a generic repository pattern:

```rust
pub trait Repository<Entity, Id, CreateInput, UpdateInput, Filter> {
    fn create(&self, input: CreateInput) -> Result<Entity, CommerceError>;
    fn get(&self, id: Id) -> Result<Entity, CommerceError>;
    fn update(&self, id: Id, input: UpdateInput) -> Result<Entity, CommerceError>;
    fn delete(&self, id: Id) -> Result<(), CommerceError>;
    fn list(&self, filter: Filter) -> Result<Vec<Entity>, CommerceError>;
    fn count(&self, filter: Filter) -> Result<u64, CommerceError>;
    fn get_batch(&self, ids: &[Id]) -> Result<Vec<Entity>, CommerceError>;
    fn create_batch(&self, inputs: Vec<CreateInput>) -> Result<Vec<Entity>, CommerceError>;
}
```

50 domain-specific repository traits extend this base. The `auto_impl` macro provides blanket implementations for `&T`, `Box<T>`, and `Arc<T>`, enabling flexible ownership patterns. Async variants (`AsyncRepository`, `AsyncTransactional`) support PostgreSQL backends.

---

## 7. Database Layer: Local-First Persistence

### 7.1 Dual-Backend Strategy

iCommerce supports two storage backends:

- **SQLite** (default): In-process, zero-configuration, ideal for embedded agents and development. Connection pooling via `r2d2`.
- **PostgreSQL**: Server-grade, async via `sqlx`, ideal for production deployments with concurrent access.

The `Database` trait provides a unified interface with 32 repository accessors. Backend switching is achieved at configuration time — no code changes required:

```rust
let db = match config.backend {
    Backend::Sqlite => SqliteDatabase::new(path)?,
    Backend::Postgres => PostgresDatabase::new(url).await?,
};
```

### 7.2 Transaction Support

ACID transactions are supported at multiple isolation levels (Read Uncommitted through Serializable). Critical payment operations use `with_immediate_transaction()` for atomicity, ensuring that multi-step financial operations either fully complete or fully roll back.

### 7.3 Migration System

Schema migrations are checksummed for integrity verification and support rollback. The A2A module extends the core schema with 13 additional tables for agent-to-agent commerce operations.

---

## 8. MCP Tool Surface: 365 Deterministic Operations

### 8.1 Architecture

The MCP server is a minimal orchestrator that loads tools from 39 domain-specific modules. Every tool invocation passes through a standard pipeline:

```
Tool Call → Permission Gate → Telemetry Span → Handler → Response Envelope
```

The `adaptTool()` function wraps each raw handler with permission checking, treasury charging (for metered operations), span-based telemetry, and consistent error formatting. All error responses follow the shape `{ success: false, error: '...' }`.

### 8.2 Tool Categories

| Category | Module Count | Tool Count | Description |
|----------|-------------|------------|-------------|
| Core Commerce | 18 | ~140 | Orders, inventory, customers, products, carts, returns, payments, shipments, manufacturing, invoices, suppliers, warranties, subscriptions, promotions, tax, currency, analytics, reviews |
| A2A Commerce | 1 | 53 | Direct payments, quotes, escrow, splits, subscriptions, disputes, reputation, webhooks, events |
| x402 Protocol | 1 | 13 | Payment intents, signing, settlement, nonces, credit ledger |
| Search & Discovery | 2 | 21 | Vector semantic search, agent card registry |
| Platform Operations | 6 | 40 | Sync, import/export, custom objects, connectors, treasury, ERC-8004 |
| Specialized | 6 | ~40 | Fraud detection, gift cards, store credits, loyalty, segments, shipping zones, wishlists |
| Blockchain | 1 | 4 | Native stablecoin payments (USDC, ssUSD) across multiple chains |
| Agentic Runtime | 1 | 8 | Knowledge loading, agent delegation, policy evaluation |

### 8.3 Zod Validation

Every tool parameter is validated with Zod schemas before execution. Numeric fields use `.int()` and `.positive()` where appropriate, string IDs enforce `.min(1)`, email fields use `.email()`, and enums use `.enum()`. This prevents malformed data from reaching the core engine.

### 8.4 Permission Model

Six permission levels govern tool access:

| Level | Value | Allowed Operations |
|-------|-------|--------------------|
| `none` | 0 | No operations |
| `read` | 1 | List, get, query |
| `preview` | 2 | Read + show what would happen |
| `write` | 3 | Create, update |
| `delete` | 4 | Cancel, void, delete |
| `admin` | 5 | Bulk operations, settings |

Each tool is mapped to a required permission level. The `--apply` flag elevates the session from `preview` to `write`/`delete`. Audit logging captures every tool invocation with actor, resource, action, and decision.

---

## 9. Agent-to-Agent (A2A) Commerce Protocol

### 9.1 Motivation

When AI agents operate autonomously, they need to transact with each other: a data-processing agent pays an API provider agent, a buyer agent negotiates prices with a seller agent, a platform agent distributes revenue to vendor agents. The A2A Commerce Protocol provides these primitives natively.

### 9.2 Protocol Primitives

#### 9.2.1 Direct Payments

```javascript
await a2a.pay({
  to: 'agent-wallet-address',
  amount: 10.00,
  asset: 'USDC',
  network: 'set_chain',
  memo: 'API call fee'
});
```

Direct payments transfer stablecoins between agent wallets. The protocol supports USDC, USDT, ssUSD (yield-bearing), and DAI across multiple networks. Idempotency keys prevent duplicate payments.

#### 9.2.2 Quote Negotiation (RFQ Protocol)

The quote flow enables structured price negotiation:

```
Buyer                           Seller
  │                               │
  ├── a2a_request_quote ────────► │
  │                               ├── a2a_provide_quote
  │ ◄──────────────────────────── │
  ├── a2a_counter_quote ────────► │  (up to 5 rounds)
  │                               ├── a2a_revise_quote
  │ ◄──────────────────────────── │
  ├── a2a_accept_quote ─────────► │
  │                               ├── a2a_fulfill_quote
  │ ◄──────────────────────────── │
```

Quotes include line items, subtotals, fees, tax, terms, and validity periods (typically 24-48 hours). Counter-offers are capped at 5 rounds to prevent infinite negotiation loops.

#### 9.2.3 Conditional Payments (Escrow)

Funds can be held in escrow with programmable release conditions:

| Condition Type | Description |
|----------------|-------------|
| `seller_fulfilled` | Released when seller marks order fulfilled |
| `buyer_confirmed` | Released when buyer confirms receipt |
| `time_lock` | Auto-released after a specified duration |
| `milestone` | Released upon milestone completion |

Escrow payments link to x402 payment intents for on-chain settlement. If conditions are not met within the timeout period, funds are automatically returned.

#### 9.2.4 Split Payments

A single payment can be distributed to multiple recipients:

```javascript
await a2a.createSplitPayment({
  amount: 100.00,
  recipients: [
    { address: '0xVendor1', percent: 70 },
    { address: '0xVendor2', percent: 20 },
    { address: '0xPlatform', percent: 10 }
  ]
});
```

The split engine supports both percentage and fixed-amount splits, with configurable platform fees and rounding drift prevention (ensuring distributed amounts always sum exactly to the total).

#### 9.2.5 Recurring Subscriptions

Agents can subscribe to other agents' services with recurring payments:

```
Status Machine: pending → trial → active → paused → past_due → cancelled → expired
```

Billing intervals: weekly, biweekly, monthly, bimonthly, quarterly, semiannual, annual. Trial periods, skip billing, pause/resume, and graceful cancellation are all supported.

#### 9.2.6 Dispute Resolution

When transactions go wrong, the dispute protocol provides structured resolution:

1. Either party creates a dispute with evidence
2. Counterparty submits evidence (documents, images, transaction logs)
3. Auto-escalation after 7 days if unresolved
4. Resolution with refund or payout decision

#### 9.2.7 Reputation, Trust & Sybil Resistance

Agent reputation is tracked across transactions:

- **Trust levels**: Verified, unverified, suspended
- **Reputation scores**: 0-100, based on transaction history
- **Ratings**: Buyer and seller ratings per transaction
- **Agent Cards**: ERC-8004-compatible identity registry with wallet proofs and capability declarations

**Sybil Resistance.** In an open agent ecosystem, a malicious actor could spin up thousands of agents to spam the RFQ system or manipulate reputation scores. iCommerce defends against this through multiple layers:

1. **ERC-8004 Identity Binding**: Agent Cards require cryptographic wallet proofs — each agent must control a wallet with on-chain history, making mass agent creation expensive
2. **Reputation Bootstrapping**: New agents start with a `0` reputation score and `unverified` trust level. High-value operations (escrow, large quotes) require minimum reputation thresholds enforced by the policy engine
3. **Rate Limiting**: The `stateset-authz` crate enforces per-agent rate limits on quote requests, dispute creation, and payment frequency — configurable per-operation
4. **Staking (Roadmap)**: Future versions will require agents to stake collateral proportional to their transaction volume, creating economic cost for Sybil attacks
5. **Behavioral Analysis**: The heartbeat system monitors for anomalous patterns (burst quote requests, rapid-fire disputes) and can auto-suspend agents pending review

**Compliance & AML.** Any system that moves money between autonomous agents must address regulatory requirements. The policy engine natively supports compliance guardrails: operators can restrict x402 and A2A transactions to agents holding verified KYC/KYB (Know Your Business) credentials in their ERC-8004 Agent Cards, enforce jurisdictional restrictions based on geographic IP or wallet provenance, cap transaction sizes by verification tier, and maintain immutable audit trails that satisfy AML (Anti-Money Laundering) reporting obligations. These policies are declarative and can be updated without code changes, allowing compliance teams to adapt to evolving regulations independently of engineering releases

### 9.3 Webhook Notifications

A2A events trigger HMAC-SHA256-signed webhooks for real-time notification:

```
POST /webhook HTTP/1.1
X-StateSet-Signature: sha256=<hmac>
Content-Type: application/json

{ "event": "payment.completed", "data": { ... } }
```

SSRF protection validates webhook URLs against private IP ranges (localhost, 127.0.0.1, 10.x, 192.168.x, 172.16-31.x, .internal, .local). Delivery uses exponential backoff with a maximum of 3 retries.

### 9.4 Event Streaming (SSE)

Real-time events are delivered via Server-Sent Events with wildcard/prefix matching:

```javascript
// Subscribe to all payment events
a2a.subscribe('payment.*', (event) => { ... });

// Subscribe to specific quote events
a2a.subscribe('quote.requested', (event) => { ... });
```

Events are persisted in an append-only log for replay. A 30-second heartbeat maintains connection health.

### 9.5 Storage Schema

The A2A module extends the SQLite schema with 13 tables covering payments, quotes, escrows, disputes, feedback, services, notifications, subscriptions, splits, and event streaming. All `update*()` methods use column whitelists to prevent SQL injection.

---

## 10. x402 Payment Protocol

### 10.1 Overview

The x402 protocol enables AI agents to create, sign, and settle payment intents without requiring real-time network access. Intents are created and signed locally, then batched for on-chain settlement.

### 10.2 Intent Lifecycle

```
                              Off-Chain                    On-Chain
                     ┌─────────────────────────┐    ┌──────────────────┐
                     │                         │    │                  │
  Agent A            │  1. Create Intent       │    │                  │
    │                │  2. Compute Signing Hash │    │                  │
    ├───────────────►│     (JCS + SHA-256)      │    │                  │
    │                │  3. Sign (Ed25519)       │    │                  │
    │                │  4. Submit to Sequencer  │───►│  5. Batch Settle │
    │                │                         │    │  6. Merkle Proof  │
    │                │  7. Receive Receipt      │◄───│                  │
    │                │                         │    │                  │
                     └─────────────────────────┘    └──────────────────┘
```

### 10.3 Signing Hash Computation

The signing hash is computed deterministically:

1. **Canonicalize** the intent payload using RFC 8785 JSON Canonicalization Scheme
2. **Apply domain separation** with the `VES_EVENTSIG_V1` prefix
3. **Hash** with SHA-256: `H = SHA256(domain_prefix || canonical_json)`
4. **Sign** with Ed25519: `sig = Ed25519_Sign(private_key, H)`

This produces identical hashes regardless of JSON key ordering, whitespace, or serialization library.

### 10.4 Gas Abstraction & Settlement Economics

A key design goal of x402 is that **agents never need to hold or manage native gas tokens** (ETH, SOL, etc.). Settlement economics work as follows:

1. **Off-chain accumulation**: Payment intents accumulate in the sequencer's mempool throughout the day. Each intent is a signed promise to pay — not an on-chain transaction
2. **Batch settlement**: The sequencer periodically compresses hundreds of intents into a single on-chain transaction using a Merkle commitment. This amortizes gas costs across all participants
3. **Relayer network**: StateSet operates a relayer that pays gas on behalf of agents. Gas costs are recovered through a configurable settlement fee (basis points on the transaction amount), deducted from the payment before disbursement
4. **ERC-4337 compatibility**: On EVM chains, the relayer uses account abstraction (ERC-4337 paymasters) so agents transact via smart contract wallets without needing ETH balances
5. **Native settlement on SET Chain**: The SET Chain L2 — an OP Stack rollup purpose-built for high-frequency agent transactions — provides sub-cent gas costs with ssUSD as the native gas token, eliminating the gas abstraction problem entirely for agents operating within the StateSet ecosystem. ssUSD is a yield-bearing stablecoin backed by U.S. Treasury reserves; idle agent balances automatically accrue interest, turning treasury management from a cost center into a revenue source

This design means an agent can be initialized with only a stablecoin balance and immediately begin transacting — no faucet calls, no gas estimation, no token bridging.

**Sequencer Permissionlessness.** While StateSet provides a default relayer for convenience, the x402 protocol is permissionless: any organization can run its own relayer, operate an independent sequencer node, or batch-submit intents directly to the settlement chain. This ensures that the system has no single point of failure or centralized gatekeeper — if the StateSet relayer goes offline, agents can route through alternative relayers or settle directly.

### 10.5 Supported Networks

| Network | Asset | Settlement Model |
|---------|-------|-----------------|
| SET Chain L2 | ssUSD (yield-bearing) | Native, fast finality, sub-cent gas |
| Solana | USDC | SPL token transfer via relayer |
| Base | USDC | ERC-4337 paymaster |
| Ethereum | USDC | ERC-4337 paymaster |
| Arbitrum | USDC | ERC-4337 paymaster |
| Bitcoin | BTC | UTXO-based |
| Zcash | ZEC | Privacy-preserving |

### 10.6 Budget Governance

Each agent maintains a budget state that caps spending:

```javascript
const budget = createBudgetState({
  maxPerIntent: 100.00,      // Maximum per single intent
  maxPerDay: 1000.00,        // Daily spending cap
  maxPerMonth: 10000.00,     // Monthly spending cap
});
```

Budget exhaustion triggers a `BudgetExceededError` with a structured message the LLM can interpret — including the remaining budget, the attempted amount, and when the budget resets — rather than silently failing.

### 10.7 Replay Protection

Every intent includes a monotonically increasing nonce per payer address. The sequencer rejects intents with reused or out-of-order nonces, preventing double-spend attacks.

---

## 11. Verifiable Encrypted Signatures (VES) v1.0

### 11.1 Purpose

VES provides the cryptographic foundation for tamper-proof event synchronization between agents. Every state mutation in the commerce engine can be signed, encrypted, and verified across language boundaries.

### 11.2 Specification

The VES specification consists of five components:

#### 11.2.1 JSON Canonicalization (RFC 8785)

All JSON payloads are canonicalized before hashing or signing, ensuring byte-identical output regardless of serialization library:

- Object keys sorted lexicographically (Unicode code point order)
- No insignificant whitespace
- Numbers in shortest representation
- UTF-8 string encoding

Implementation: `serde_jcs` crate (Rust), custom `canonicalizeJson()` (JavaScript).

#### 11.2.2 Domain-Separated Hashing

Every hash operation includes a domain prefix to prevent cross-protocol signature reuse:

```
Hash(domain, data) = SHA-256(domain || data)
```

Eleven domain prefixes are defined:

| Prefix | Use |
|--------|-----|
| `VES_PAYLOAD_PLAIN_V1` | Plaintext payload hash |
| `VES_PAYLOAD_AAD_V1` | Additional authenticated data |
| `VES_PAYLOAD_CIPHER_V1` | Encrypted payload hash |
| `VES_RECIPIENTS_V1` | Recipient list hash |
| `VES_EVENTSIG_V1` | Event signing hash |
| `VES_LEAF_V1` | Merkle leaf hash |
| `VES_NODE_V1` | Merkle internal node hash |
| `VES_PAD_LEAF_V1` | Merkle padding leaf |
| `VES_STREAM_V1` | Stream identifier hash |
| `VES_RECEIPT_V1` | Receipt hash |

#### 11.2.3 Ed25519 Signing

Event signatures use Ed25519 (via `ed25519-dalek`):

```rust
pub fn sign(message: &[u8], secret_key: &SigningKey) -> Signature {
    secret_key.sign(message)
}

pub fn verify(message: &[u8], signature: &Signature, public_key: &VerifyingKey) -> bool {
    public_key.verify(message, signature).is_ok()
}
```

#### 11.2.4 AES-256-GCM Encryption

Payload encryption uses AES-256-GCM with X25519 ECDH key exchange and HKDF key derivation:

```
1. Generate ephemeral X25519 keypair
2. Perform ECDH: shared_secret = ECDH(ephemeral_private, recipient_public)
3. Derive key: DEK = HKDF-SHA256(shared_secret, salt, info)
4. Encrypt: (ciphertext, tag) = AES-256-GCM(DEK, nonce, plaintext, AAD)
5. Zeroize DEK from memory
```

Key material is scrubbed from memory after use via the `zeroize` crate. Hash comparisons use constant-time equality checks to prevent timing side-channels.

#### 11.2.5 Merkle Trees

Batch integrity is verified through Merkle trees with domain-separated leaf and node hashing:

```
Leaf:  H_leaf  = SHA-256(VES_LEAF_V1 || data)
Node:  H_node  = SHA-256(VES_NODE_V1 || left || right)
Pad:   H_pad   = SHA-256(VES_PAD_LEAF_V1 || index)
```

This enables O(log n) verification of individual events within a batch.

### 11.3 Cross-Language Verification

VES implementations exist in both Rust and JavaScript with 65 cross-language test vectors ensuring identical outputs. All vectors produce identical hex digests across both implementations, guaranteeing that events signed in Rust can be verified in JavaScript and vice versa.

### 11.4 NAPI Bindings

Seven cryptographic operations are exposed to Node.js via NAPI:

```javascript
import {
  jcsCanonicalize,   // RFC 8785 canonical JSON
  domainHash,        // Domain-separated SHA-256
  ed25519Sign,       // Ed25519 signature
  ed25519Verify,     // Ed25519 verification
  aesGcmEncrypt,     // AES-256-GCM encryption
  aesGcmDecrypt,     // AES-256-GCM decryption
  merkleRoot,        // Merkle tree root hash
} from '@stateset/embedded';
```

A JavaScript fallback provides the same operations using Web Crypto APIs when native bindings are unavailable, ensuring the system works in constrained environments (serverless, WASM, browser).

---

## 12. Policy Engine: Declarative Safety Guardrails

### 12.1 Architecture

The policy engine enables declarative business rules without hardcoding logic. Policies are defined in YAML or JSON and evaluated at runtime against a context object. This is the primary mechanism by which human operators maintain control over autonomous agents.

### 12.2 Rule Structure

```yaml
rules:
  - name: "Large Order Review"
    enabled: true
    conditions:
      operator: "and"
      conditions:
        - field: "order.total"
          operator: "gt"
          value: 10000
        - field: "customer.status"
          operator: "eq"
          value: "new"
    actions:
      - action: "deny"
        reason: "Orders over $10,000 from new customers require manual review"
        remediation: "Contact the sales team for approval"
```

### 12.3 Operators

The engine supports 20+ operators:

| Category | Operators |
|----------|-----------|
| Comparison | `eq`, `ne`, `gt`, `gte`, `lt`, `lte` |
| String | `contains`, `startsWith`, `endsWith`, `regex` |
| Collection | `in`, `notIn`, `hasAny`, `hasAll`, `hasNone` |
| Type | `type`, `exists`, `isNull`, `isNotNull` |
| Numeric | `between`, `divisibleBy` |

### 12.4 Deny-Override Semantics

When multiple rules match, **any deny action overrides all allow actions**. This ensures safety: a single security rule can block an operation even if ten other rules permit it. This is the principle of least privilege applied to policy evaluation — the system defaults to caution.

### 12.5 Explainable Denials

Every denial includes a per-condition breakdown:

```json
{
  "decision": "deny",
  "explanation": {
    "conditions": [
      {
        "field": "order.total",
        "operator": "gt",
        "expected": 10000,
        "actual": 15000,
        "matched": true
      },
      {
        "field": "customer.status",
        "operator": "eq",
        "expected": "new",
        "actual": "new",
        "matched": true
      }
    ],
    "reason": "Orders over $10,000 from new customers require manual review",
    "remediation": "Contact the sales team for approval"
  }
}
```

**Why this matters for autonomous agents:** Traditional APIs return opaque error codes that cause LLMs to retry the same failing request in a loop. Explainable denials provide structured remediation text directly to the LLM's context window, enabling the agent to autonomously determine whether to adjust its parameters and retry, escalate to a human, or abandon the operation. The `remediation` field is specifically designed to be LLM-readable — a short, actionable instruction the model can interpret without ambiguity.

### 12.6 Transform Audit Trail

Policies can transform data (e.g., apply default values, normalize fields). Every transformation produces a before/after audit entry, enabling full traceability of how agent inputs were modified before reaching the core engine.

### 12.7 Dry Run Evaluation

The `evaluateDryRun()` method evaluates policies without executing actions, returning the full evaluation trace. This is used by the preview system: agents can check what policies would apply to a proposed operation before committing to it.

---

## 13. Autonomous Engine: Self-Governing Commerce

### 13.1 Overview

The autonomous engine combines six subsystems into a unified orchestrator for self-governing commerce operations:

```
┌─────────────────────────────────────────────────────┐
│                  Autonomous Engine                    │
│                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │Scheduler │  │ Workflow  │  │  Policy  │         │
│  │(Cron)    │  │ (FSM)    │  │ (Rules)  │         │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘         │
│       └──────────────┼──────────────┘               │
│                      │                               │
│  ┌──────────┐  ┌─────┴────┐  ┌──────────┐         │
│  │ Webhook  │  │ Approval │  │Heartbeat │         │
│  │ Server   │  │  Queue   │  │ Monitor  │         │
│  └──────────┘  └──────────┘  └──────────┘         │
│                      │                               │
│               ┌──────┴──────┐                       │
│               │ EventBridge │                       │
│               └──────┬──────┘                       │
│                      │                               │
│         ┌────────────┼────────────┐                 │
│         │            │            │                 │
│    ┌────┴───┐  ┌─────┴────┐  ┌───┴────┐           │
│    │ Slack  │  │ Discord  │  │WhatsApp│           │
│    └────────┘  └──────────┘  └────────┘           │
└─────────────────────────────────────────────────────┘
```

### 13.2 Subsystems

- **Scheduler**: Cron, interval, and one-time job execution with retry and exponential backoff
- **Workflow Engine**: Multi-step state machine workflows with conditional branching
- **Policy Engine**: Real-time rule evaluation against commerce events
- **Webhook Server**: Inbound event handling from external systems
- **Approval Queue**: Multi-level human-in-the-loop approval chains for high-stakes operations
- **Heartbeat Monitor**: Proactive health checks that detect commerce anomalies before they become problems

### 13.3 Heartbeat Checks

Built-in health checks detect commerce anomalies:

| Check | Description |
|-------|-------------|
| `low-stock` | SKUs below reorder threshold |
| `abandoned-carts` | Carts idle beyond configured window |
| `revenue-milestone` | Revenue threshold alerts |
| `pending-returns` | Unprocessed returns accumulating |
| `overdue-invoices` | Past-due invoices requiring attention |
| `subscription-churn` | Churn rate monitoring and early warning |

Alerts route through the EventBridge to all configured messaging channels (Slack, Discord, WhatsApp, SMS).

---

## 14. Multi-Agent System: Specialized Commerce Agents

### 14.1 Agent Architecture

iCommerce provides 18 specialized agent configurations, each with a domain-specific system prompt and curated tool set:

| Agent | Domain | Tool Access |
|-------|--------|-------------|
| `customer-service` | Full-service | All 700+ tools |
| `checkout` | Cart + checkout flow | Carts, shipping, payments |
| `orders` | Order lifecycle | Orders, shipments |
| `inventory` | Stock management | Inventory, forecasting |
| `returns` | RMA processing | Returns, refunds |
| `analytics` | Business intelligence | Analytics, forecasting |
| `promotions` | Discounts + coupons | Promotions, coupons |
| `subscriptions` | Recurring billing | Plans, billing cycles |
| `payments` | Payment processing | Payments, refunds |
| `manufacturing` | Production | BOMs, work orders |
| `shipments` | Fulfillment | Shipments, tracking |
| `suppliers` | Procurement | Suppliers, purchase orders |
| `invoices` | B2B AR | Invoices, payments |
| `warranties` | Claims | Warranties, claims |
| `currency` | Multi-currency | Exchange rates, conversion |
| `tax` | Tax calculation | Tax, jurisdictions |
| `sync` | Event sync | Push, pull, outbox |
| `storefront` | Site scaffolding | Template generation |

### 14.2 Semantic Routing

The agent router matches natural language requests to the most appropriate specialized agent using confidence scoring, ensuring that inventory questions go to the inventory agent, payment questions go to the payments agent, and so on.

### 14.3 Session Management

Multi-turn conversations are persisted with full context, enabling complex multi-step operations that span multiple tool calls:

```bash
stateset "create a cart for alice@example.com"
# Output: Session ID: abc-123-def

stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
stateset --apply --resume abc-123-def "complete the checkout"
```

### 14.4 Multi-Provider Support

The agent harness supports multiple AI providers with automatic fallback:

| Provider | Models | MCP Tool Support |
|----------|--------|-----------------|
| Claude (Anthropic) | Opus 4, Sonnet 4, Haiku 4.5 | Full |
| OpenAI | GPT-4, GPT-4o, o1 | Chat only |
| Google | Gemini | Chat only |
| Ollama | Local models | Chat only |

If the primary model is unavailable, the system automatically falls back to the next configured model, ensuring high availability for production agent deployments.

### 14.5 Air-Gapped Commerce

The combination of local-first SQLite execution (Section 3.1) and local LLM support via Ollama unlocks a deployment model with profound privacy implications: **air-gapped commerce**. An enterprise can run iCommerce alongside a local Llama, Mistral, or DeepSeek model, allowing autonomous agents to process highly sensitive ERP and financial data without a single byte ever leaving the corporate intranet. No data is sent to OpenAI, Anthropic, or any external API. This is critical for defense contractors, healthcare organizations, financial institutions, and any enterprise subject to data residency regulations (GDPR, HIPAA, SOC 2). The full 700+ tool surface remains available — only the LLM provider changes.

---

## 15. Sync Architecture: Eventually Consistent Multi-Agent State

### 15.1 Overview

When multiple agents operate on the same commerce data, their local states must eventually converge. The sync architecture provides this through an outbox pattern with cryptographic verification.

### 15.2 Event Flow

**Push (Local → Remote):**

```
Local Mutation → Event Created → Ed25519 Signed → Outbox Queued
→ Batch Commitment (Merkle Root) → Submit to Sequencer → ACK → Remove from Outbox
```

**Pull (Remote → Local):**

```
Fetch Events (since last sequence) → Verify Signatures → Check Conflicts
→ [No Conflict] Apply Directly
→ [Conflict] Resolve (LWW / Merge / Manual) → Apply Resolution
```

### 15.3 Conflict Resolution Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| `LAST_WRITE_WINS` | Most recent timestamp wins | General use |
| `FIRST_WRITE_WINS` | Earliest timestamp wins | Orders (don't overwrite) |
| `MERGE` | Field-level automatic merge | Inventory updates |
| `MANUAL` | Requires human intervention | High-value operations |
| `CUSTOM` | User-defined resolver function | Domain-specific logic |

Strategies are configurable per entity type, so inventory can use `MERGE` (additive updates are safe) while orders use `FIRST_WRITE_WINS` (preserving the original order is critical).

### 15.4 Key Management

- **Identity keys**: Long-term Ed25519 signing keys, stored encrypted at rest
- **Session keys**: Ephemeral X25519 for key exchange
- **Content keys**: Derived AES-256 for payload encryption
- **Rotation policy**: Automatic rotation with configurable intervals and grace periods for smooth transitions

### 15.5 Multi-Tenant Isolation

Sync groups enable scoped synchronization — an agent can sync only the entity types it has permission to access, and tenant isolation ensures that one organization's data is never visible to another's agents.

---

## 16. Observability & Telemetry

### 16.1 Metrics

Business-level counters track agentic commerce activity across all protocol dimensions: A2A quotes, x402 intents, policy evaluations, split payments, subscription renewals, webhook deliveries, and more. Counters use lock-free atomics for contention-free updates. A `LatencyHistogram` provides p50/p95/p99 latencies for critical operations.

### 16.2 Tracing

Structured tracing with request ID propagation across all layers. OpenTelemetry export is available behind a feature flag, enabling integration with Datadog, Grafana, Honeycomb, and other observability platforms without adding dependencies to the default build.

### 16.3 Audit Logging

Every tool invocation is logged with actor identity, resource type and ID, action, parameters, decision (allowed/denied with explanation), and microsecond-precision timestamp. This produces a complete, tamper-evident record of every action taken by every agent.

### 16.4 Subsystem Logging

The logger supports subsystem-scoped log channels with color-coded prefixes and JSON-structured output, making it straightforward to filter logs by domain (A2A, sync, payments, inventory) in production environments.

---

## 17. Security Architecture

### 17.1 Threat Model

iCommerce is hardened against both traditional web application threats and agent-specific attack vectors:

| Threat | Mitigation |
|--------|-----------|
| SQL Injection | Column whitelists on all update methods — only pre-approved columns can be modified |
| SSRF | URL validation with private IP blocklist (localhost, 10.x, 192.168.x, 172.16-31.x) |
| Prototype Pollution | Deep merge operations filter `__proto__`, `constructor`, `prototype` keys |
| ReDoS | All regex patterns use non-greedy quantifiers |
| Shell Injection | Host/command validation with strict character whitelists |
| XSS | CSP nonce per request in admin dashboard |
| Path Traversal | Safe ID schema (alphanumeric, hyphens, underscores, dots) |
| Timing Attacks | Constant-time equality checks for all cryptographic comparisons |
| Key Leakage | `zeroize` on all DEK/wrapping keys after use |
| Budget Exhaustion | Per-agent spending caps with structured error reporting |
| Sybil Attacks | ERC-8004 identity binding, reputation gating, rate limiting |

### 17.2 Cryptographic Hygiene

- Memory-safe Rust core with no `unsafe` code blocks
- Ed25519 keys stored encrypted at rest
- HMAC-SHA256 webhook signatures
- TLS 1.3 for all transport
- Automatic key rotation with configurable intervals
- All production error paths use typed `Result<T, E>` — no panics in the hot path

### 17.3 Permission Sandboxing

The HTTP gateway supports API key authentication with per-route permission levels. Sandbox mode blocks dangerous operations (browser evaluation, shell access) even for admin-level keys.

---

## 18. Language Bindings & Portability

### 18.1 Binding Strategy

The Rust core is exposed to 10 languages through a stable C ABI with language-specific wrappers:

| Language | Technology | Status |
|----------|-----------|--------|
| Node.js | NAPI (`napi-rs`) | Production |
| Python | PyO3 | Production |
| Ruby | Magnus | Production |
| PHP | ext-php-rs | Production |
| Go | cgo | Available |
| Java | JNI | Available |
| Kotlin | JNI | Available |
| Swift | C FFI | Available |
| .NET | P/Invoke | Available |
| WASM | wasm-bindgen | Available |

### 18.2 API Consistency

Every binding exposes the same domain operations with idiomatic naming conventions:

```python
# Python
from stateset_embedded import Commerce
db = Commerce("store.db")
order = db.create_order(customer_id="cust-123", items=[...])
```

```ruby
# Ruby
require 'stateset_embedded'
db = StateSet::Commerce.new("store.db")
order = db.create_order(customer_id: "cust-123", items: [...])
```

```javascript
// Node.js
import { Commerce } from '@stateset/embedded';
const db = new Commerce('store.db');
const order = await db.createOrder({ customerId: 'cust-123', items: [...] });
```

The same SQLite database file can be opened by any binding — an agent written in Python and an agent written in Node.js can share the same commerce state.

---

## 19. Admin Dashboard

### 19.1 Technology Stack

The admin dashboard is built with Next.js 14 (App Router), TypeScript, Tailwind CSS, Radix UI primitives, and Tremor charts. It provides a human-operator overlay for monitoring and configuring the autonomous engine.

### 19.2 Pages

| Page | Purpose |
|------|---------|
| `/` | Unified operations dashboard (KPIs, activity feed) |
| `/orders` | Order pipeline (Kanban view, status tracking) |
| `/products` | Product catalog management |
| `/inventory` | Stock levels, demand forecasting, low-stock alerts |
| `/customers` | Customer profiles, health scores, RFM segmentation |
| `/returns` | Return management and RMA workflow |
| `/subscriptions` | Plan management, active subscriptions, billing history |
| `/analytics` | Revenue trends, forecasting, funnel analysis |
| `/chat` | Agentic chat interface with generative component rendering |
| `/gateway` | 10-channel messaging gateway dashboard |
| `/settings` | Engine status, configuration, health checks |

### 19.3 Shared Libraries

The shared module provides production-grade infrastructure: Zod schemas for request validation, a structured error class hierarchy with factory methods, standard response envelopes (success/error/paginated), request-scoped context with request ID propagation, CSRF token generation, and Prometheus metrics integration.

---

## 20. Testing & Quality Assurance

### 20.1 Test Coverage

The project maintains comprehensive automated test coverage across all layers:

| Layer | Framework | Coverage |
|-------|-----------|----------|
| Rust Core | `cargo test` | 3,000+ tests, 0 failures |
| CLI | `node --test` | 6,000+ tests |
| Admin | Vitest | 260+ tests |
| Cross-language | Custom vectors | 65 crypto compatibility tests |
| **Total** | | **10,000+** |

### 20.2 Test Categories

- **Unit tests**: Individual function and module tests across all crates and CLI modules
- **Integration tests**: Cross-crate and cross-module tests verifying end-to-end behavior
- **Snapshot tests**: Serialization format stability via `insta`
- **Property-based tests**: `proptest` for pricing calculations and cryptographic operations, catching edge cases that example-based tests miss
- **Cross-language vectors**: Identical VES cryptographic outputs verified across Rust and JavaScript
- **Security tests**: Dedicated suites for SQL injection, SSRF, prototype pollution, ReDoS, and column injection
- **Tool coverage tests**: Every MCP tool has at least basic exercise coverage

### 20.3 Quality Philosophy

The project enforces several invariants that go beyond typical test coverage:

- **Zero compiler warnings**: Both `cargo clippy` (Rust) and ESLint (JavaScript) produce zero warnings
- **No silent failures**: All error paths produce typed results; no empty catch blocks remain in the codebase
- **Consistent error contracts**: Every tool response follows a uniform shape, preventing LLMs from encountering inconsistent API behavior
- **Column-level SQL safety**: All database update methods validate column names against a whitelist before constructing queries
- **`unused_must_use = "deny"`**: The Rust compiler rejects code that ignores return values marked as important — a critical safeguard for a financial system

---

## 21. Performance

### 21.1 Build Configuration

Release builds use thin LTO, symbol stripping, and abort-on-panic for minimal binary size and maximum throughput. A dedicated profiling profile preserves symbols for `perf` and flamegraph analysis without sacrificing optimization.

### 21.2 Runtime Characteristics

- **SQLite operations**: Single-digit millisecond latency for typical CRUD
- **Policy evaluation**: Microsecond-level for simple rule sets
- **Cryptographic operations**: Ed25519 sign/verify in microseconds (native), sub-millisecond (JavaScript fallback)
- **MCP server startup**: Sub-second tool loading from modular files
- **Merkle proof generation**: O(n log n) for batch, O(log n) for individual verification

### 21.3 Benchmark Infrastructure

The `stateset-benches` crate uses Criterion for statistically rigorous benchmarking with regression detection, enabling performance changes to be caught before they reach production.

---

## 22. Related Work

### 22.1 Traditional Commerce Platforms

Shopify, WooCommerce, and Medusa.js are designed for human operators with web dashboards. They lack agent-native primitives, deterministic execution guarantees, and embeddable runtimes. An AI agent interacting with these platforms must navigate human-designed UIs or poorly documented APIs, with no policy enforcement or cryptographic auditability.

### 22.2 Headless Commerce APIs

Commerce.js, Saleor, and BigCommerce APIs provide RESTful access to commerce operations. However, they require network connectivity, introduce latency, and lack agent-to-agent transaction primitives. They are architecturally unable to run in-process — every operation is a network round-trip.

### 22.3 Agent Frameworks

LangChain, CrewAI, and AutoGPT provide agent orchestration but no commerce-specific tooling. iCommerce complements these frameworks by providing the domain-specific tool surface that agents need for commerce operations. An AutoGPT agent can use iCommerce's MCP tools just as easily as a Claude agent.

### 22.4 Payment Protocols

Stripe, Square, and traditional payment processors are optimized for human-initiated transactions with card-present or card-not-present flows. x402 is designed for machine-initiated, cryptographically verifiable payment intents between autonomous agents — a fundamentally different interaction model where there is no human to enter a credit card number.

### 22.5 Blockchain Commerce

Previous attempts at "decentralized commerce" (OpenBazaar, Origin Protocol) required full blockchain nodes and sacrificed usability for decentralization purity. iCommerce takes a pragmatic hybrid approach: local-first execution with optional on-chain settlement, combining the auditability of blockchain with the performance and simplicity of local computation.

---

## 23. Roadmap

### 23.1 Near-Term (v0.8 — v0.9)

| Feature | Description |
|---------|-------------|
| **Agent Staking** | Require agents to stake collateral proportional to transaction volume, creating economic cost for Sybil attacks and ensuring accountability |
| **ZK Privacy Layer** | Zero-knowledge proofs for private agent transactions — prove payment was made without revealing amount or counterparty |
| **Cross-Chain Bridge** | Native bridge between SET Chain and EVM/Solana chains, enabling seamless multi-chain agent operations |
| **Multi-Agent Coordination Protocol** | Structured protocol for agent swarms to negotiate, vote, and reach consensus on shared commerce decisions |
| **Policy Marketplace** | Community-contributed policy templates with versioning, ratings, and one-click deployment |

### 23.2 Medium-Term (v1.0)

| Feature | Description |
|---------|-------------|
| **Formal Verification** | Machine-checked proofs of state machine correctness for critical paths (payment, escrow) |
| **Federated Agent Directory** | Decentralized agent discovery across organizational boundaries, built on ERC-8004 |
| **Streaming Payments** | Per-second payment streams for continuous agent services (e.g., real-time data feeds) |
| **On-Chain Policy Execution** | Publish policy rules as smart contracts for trustless enforcement without relying on the agent's local policy engine |
| **GPU-Accelerated Search** | Vector similarity search using GPU acceleration for sub-millisecond semantic product discovery |

### 23.3 Long-Term Vision

The long-term vision for iCommerce is a **global agent economy** where millions of specialized AI agents autonomously conduct commerce — buying, selling, negotiating, fulfilling, and settling — with cryptographic guarantees at every step. iCommerce provides the embedded runtime that makes each agent economically sovereign: able to hold funds, enforce policies, sign contracts, and participate in markets without requiring a centralized platform.

The "SQLite of Commerce" analogy extends to its ultimate conclusion: just as SQLite enabled every application to embed a database, iCommerce enables every AI agent to embed a complete commerce engine. The result is not a platform that agents connect to, but a capability that agents carry with them.

---

## 24. Conclusion

StateSet iCommerce represents a fundamental rethinking of commerce infrastructure for the age of autonomous AI agents. By providing an embeddable, deterministic, and cryptographically verifiable commerce engine with native agent-to-agent transaction primitives, it enables a new class of applications where AI agents independently manage entire commerce operations — from inventory forecasting to payment settlement, from customer service to supplier procurement.

The system's architecture — a type-safe Rust core with 50 repository traits, 700+ MCP tools, the A2A and x402 protocols, VES v1.0 cryptography, and a declarative policy engine with explainable denials — provides the safety guarantees that autonomous agents need to operate at scale. Comprehensive test coverage, memory-safe cryptographic primitives, and defense-in-depth security hardening make iCommerce production-ready for the agentic commerce era.

The transition from eCommerce to iCommerce is not merely a technological upgrade; it is a paradigm shift in how commerce systems are designed, deployed, and operated. Where eCommerce assumed a human at every decision point, iCommerce assumes an agent — and provides the runtime, protocols, and safety guarantees to make that assumption viable.

StateSet iCommerce is the engine that makes autonomous commerce possible.

---

## Appendix A: Repository

- **Source**: `github.com/stateset/stateset-icommerce`
- **License**: MIT OR Apache-2.0
- **Version**: 0.7.15
- **Rust Edition**: 2024 (rust-version 1.85)
- **Node.js**: 20.20.0+

## Appendix B: Crate Dependency Graph

```
stateset-primitives (foundation: IDs, Money, Sku, CurrencyCode)
    ↓
stateset-core (domain models, repository traits, state machines)
    ↓
stateset-db (SQLite + PostgreSQL implementations)
    ↓
stateset-embedded (unified API surface, async runtime)
    ↓
stateset-sdk (facade with feature gates)

stateset-crypto ←─── (VES v1.0, Ed25519, AES-GCM, Merkle)
stateset-policy ←──── (rule engine, conditions, actions)
stateset-a2a ←─────── (splits, escrow, subscriptions)
stateset-pricing ←──── (pure functions, deterministic)
stateset-authz ←────── (RBAC, rate limiting, audit)
stateset-sync ←─────── (outbox, conflict resolution)
stateset-jobs ←─────── (cron, intervals, retries)
stateset-http ←─────── (Axum REST + SSE)
stateset-observability (metrics, tracing, OTel)
```

## Appendix C: Tool Count Summary

| Category | Count |
|----------|-------|
| Core Commerce (18 modules) | ~140 |
| A2A Commerce | 53 |
| x402 Protocol | 13 |
| Vector Search | 16 |
| Platform Operations | 40 |
| Specialized Domains | 40 |
| Blockchain/Stablecoin | 4 |
| Agentic Runtime | 8 |
| Sync | 10 |
| Import/Export | 7 |
| Custom Objects | 12 |
| Connectors | 11 |
| Treasury | 6 |
| ERC-8004 | 5 |
| **Total** | **~365** |
