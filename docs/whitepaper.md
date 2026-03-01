# StateSet iCommerce: An Embedded Commerce Engine for Autonomous AI Agents

**Technical Whitepaper v0.7.15**
**March 2026**

---

## Abstract

StateSet iCommerce is an embedded, zero-dependency commerce engine designed for autonomous AI agents. Built on a Rust core with language bindings for 10 platforms, it provides a complete commerce and ERP surface area — orders, inventory, payments, returns, subscriptions, manufacturing, and more — as deterministic, locally executable operations that AI agents can safely invoke. The system introduces three novel protocols: the **Agent-to-Agent (A2A) Commerce Protocol** for autonomous economic transactions between agents, the **x402 Payment Protocol** for cryptographically verifiable payment intents, and the **Verifiable Encrypted Signatures (VES) v1.0** specification for tamper-proof event synchronization. iCommerce exposes 365+ tools via the Model Context Protocol (MCP), governed by a declarative policy engine with explainable denials, and is backed by 10,000+ automated tests across all layers. The result is a portable, embeddable commerce runtime — the "SQLite of Commerce" — that enables AI agents to reason about, decide on, and execute commerce operations independently.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Design Principles](#2-design-principles)
3. [System Architecture](#3-system-architecture)
4. [Rust Core: Domain Model & Type System](#4-rust-core-domain-model--type-system)
5. [Database Layer: Local-First Persistence](#5-database-layer-local-first-persistence)
6. [MCP Tool Surface: 365 Deterministic Operations](#6-mcp-tool-surface-365-deterministic-operations)
7. [Agent-to-Agent (A2A) Commerce Protocol](#7-agent-to-agent-a2a-commerce-protocol)
8. [x402 Payment Protocol](#8-x402-payment-protocol)
9. [Verifiable Encrypted Signatures (VES) v1.0](#9-verifiable-encrypted-signatures-ves-v10)
10. [Policy Engine: Declarative Safety Guardrails](#10-policy-engine-declarative-safety-guardrails)
11. [Autonomous Engine: Self-Governing Commerce](#11-autonomous-engine-self-governing-commerce)
12. [Multi-Agent System: Specialized Commerce Agents](#12-multi-agent-system-specialized-commerce-agents)
13. [Sync Architecture: Eventually Consistent Multi-Agent State](#13-sync-architecture-eventually-consistent-multi-agent-state)
14. [Observability & Telemetry](#14-observability--telemetry)
15. [Security Architecture](#15-security-architecture)
16. [Language Bindings & Portability](#16-language-bindings--portability)
17. [Admin Dashboard](#17-admin-dashboard)
18. [Testing & Quality Assurance](#18-testing--quality-assurance)
19. [Performance](#19-performance)
20. [Related Work](#20-related-work)
21. [Conclusion](#21-conclusion)

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
- The **x402 Payment Protocol** for off-chain payment intents with Ed25519 signatures and on-chain settlement across 10 blockchain networks
- **VES v1.0**, a cryptographic specification combining RFC 8785 JSON Canonicalization, domain-separated SHA-256 hashing, Ed25519 signatures, AES-256-GCM encryption, and Merkle tree proofs
- A **declarative policy engine** with deny-override semantics, per-condition explainability, and transform audit trails
- An **MCP tool surface** of 365+ commerce operations, the largest known domain-specific MCP server

---

## 2. Design Principles

### 2.1 Local-First Execution

iCommerce runs entirely in-process using SQLite as its default storage backend. No network calls, no external services, no containers. An agent can `npm install @stateset/embedded` and have a full commerce engine running in the same process. This eliminates latency, reduces failure modes, and enables offline-first operation.

### 2.2 Deterministic Operations

Every operation in the commerce engine is a pure function of its inputs and the current database state. There are no hidden side effects, no background timers affecting computation, and no non-deterministic behavior. This property is critical for AI agents: it means operations can be safely replayed, simulated, and reasoned about.

### 2.3 Type Safety Through Newtypes

The Rust core uses strongly-typed newtypes for all entity identifiers. An `OrderId` cannot be accidentally passed where a `CustomerId` is expected — the compiler rejects it. This prevents an entire class of bugs that are common in stringly-typed commerce systems.

### 2.4 Explicit State Machines

Every domain aggregate (Order, Payment, Return, Subscription, WorkOrder) has an explicit state machine with validated transitions. The `can_transition_to()` method returns whether a transition is valid, and `is_terminal()` indicates whether further transitions are possible. Invalid transitions produce typed errors rather than silently corrupting state.

### 2.5 Preview Before Execute

All write operations are blocked by default. The `--apply` flag must be explicitly provided to enable mutations. Without it, every operation returns a preview of what would happen — how many records would be affected, what state changes would occur — without actually executing. This safety model is essential for autonomous agents operating at scale.

---

## 3. System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Admin Dashboard                               │
│                    (Next.js 14 + TypeScript)                         │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────────┐
│                     CLI + MCP Server                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ 18 Agent │  │ 365 MCP  │  │  Policy  │  │   Sync   │           │
│  │ Configs  │  │  Tools   │  │  Engine  │  │  Engine  │           │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘           │
│       └──────────────┼──────────────┼──────────────┘                │
│                      │              │                                │
│  ┌───────────────────┼──────────────┼───────────────────────────┐   │
│  │               MCP Server (470 lines)                          │   │
│  │  adaptTool() → permission → telemetry → handler → response   │   │
│  └───────────────────┼──────────────┼───────────────────────────┘   │
│                      │              │                                │
│  ┌───────────────────┴──────────────┴───────────────────────────┐   │
│  │                  A2A + x402 Protocols                         │   │
│  │  Payments · Quotes · Escrow · Splits · Subscriptions          │   │
│  │  Payment Intents · Ed25519 · Budget · Settlement              │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────────┐
│                     Language Bindings                                 │
│  Node (NAPI) · Python (PyO3) · Ruby (Magnus) · PHP (ext-php-rs)    │
│  Go (cgo) · Java (JNI) · Kotlin · Swift · .NET (P/Invoke) · WASM   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────────┐
│                        Rust Core (21 Crates)                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│  │ Primitives  │  │    Core     │  │   Crypto    │                │
│  │ (IDs, Money │  │  (50 repos, │  │  (VES v1.0) │                │
│  │  Sku, Curr) │  │  25 domains)│  │             │                │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                │
│         └────────────────┼────────────────┘                         │
│                    ┌─────┴─────┐                                    │
│                    │    DB     │                                    │
│                    │ SQLite +  │                                    │
│                    │ PostgreSQL│                                    │
│                    └───────────┘                                    │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐              │
│  │ Policy  │  │   A2A   │  │ Pricing │  │  Authz  │              │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐              │
│  │  Sync   │  │  Jobs   │  │  HTTP   │  │Protocol │              │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘              │
└─────────────────────────────────────────────────────────────────────┘
```

The system is organized into three layers:

1. **Rust Core** (21 crates): Pure domain models, database abstraction, cryptographic primitives, policy evaluation, and pricing calculations — all with zero I/O side effects in the core logic
2. **Language Bindings**: FFI layer exposing the Rust core to 10 programming languages, each with idiomatic APIs
3. **CLI + MCP Server**: The agent-facing interface, providing 365+ tools via the Model Context Protocol, 18 specialized agents, and the A2A/x402 protocol implementations

---

## 4. Rust Core: Domain Model & Type System

### 4.1 Crate Organization

| Crate | Purpose | Key Characteristic |
|-------|---------|-------------------|
| `stateset-primitives` | Strongly-typed IDs and value objects | Zero dependencies, `Copy + Eq + Hash` |
| `stateset-core` | Domain models, repository traits, errors | Pure logic, no I/O |
| `stateset-crypto` | VES v1.0 cryptographic operations | `deny(unsafe_code)`, `zeroize` keys |
| `stateset-db` | SQLite + PostgreSQL implementations | Trait-based backend switching |
| `stateset-embedded` | Unified high-level API surface | Primary binding target |
| `stateset-policy` | Declarative rule engine | YAML/JSON rule definitions |
| `stateset-a2a` | Agent-to-Agent commerce | Split payments, escrow, subscriptions |
| `stateset-pricing` | Deterministic pricing engine | Pure functions, WASM-compatible |
| `stateset-authz` | Authorization, RBAC, rate limiting | IO-free, framework-agnostic |
| `stateset-observability` | Metrics, tracing, OpenTelemetry | Lock-free atomic counters |
| `stateset-protocol` | Wire-format types for sync | IO-free, WASM-compatible |
| `stateset-sync` | Event-sourcing sync engine | Outbox pattern, conflict resolution |
| `stateset-http` | Axum REST + SSE server | Auth, CORS, tracing middleware |
| `stateset-jobs` | Background job scheduler | Cron, intervals, retries |
| `stateset-ffi` | Stable C ABI | `#[repr(C)]`, ABI versioning |
| `stateset-macros` | Procedural macros | Code generation for domain models |
| `stateset-migrations` | Database schema migrations | SHA-256 checksums, rollback |
| `stateset-sdk` | Facade with feature gates | Single entry point |
| `stateset-test-utils` | Shared test fixtures | Builder pattern, assertion macros |
| `stateset-benches` | Criterion benchmarks | Performance regression detection |
| `stateset-integration-tests` | Cross-crate tests | End-to-end validation |

### 4.2 Strongly-Typed Entity Identifiers

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

### 4.3 Value Types

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

### 4.4 Domain Models

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

### 4.5 State Machine Enforcement

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

Invalid transitions produce a `StateTransitionError<S>` carrying the attempted and expected states, enabling clear error messages.

### 4.6 Error Architecture

Errors form a two-level hierarchy with compile-time size assertions:

```
CommerceError (80 bytes)
├── OrderError (48 bytes)
├── InventoryError (72 bytes)
├── PaymentError (56 bytes)
├── ReturnError (48 bytes)
├── ShippingError (48 bytes)
├── CustomerError (24 bytes)
├── ProductError (24 bytes)
├── DbError (64 bytes)
├── StateTransitionError<S>
├── ValidationError
└── BatchResult<T>  (partial success tracking)
```

Every `CommerceError` exposes categorization methods: `is_not_found()`, `is_validation()`, `is_conflict()`, `is_database()`, `is_retryable()`. The `is_retryable()` method is critical for agent retry logic — only transient failures (deadlocks, connection timeouts) return `true`.

### 4.7 Repository Trait System

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

## 5. Database Layer: Local-First Persistence

### 5.1 Dual-Backend Strategy

iCommerce supports two storage backends:

- **SQLite** (default): In-process, zero-configuration, ideal for embedded agents and development. Connection pooling via `r2d2`.
- **PostgreSQL**: Server-grade, async via `sqlx`, ideal for production deployments with concurrent access.

The `Database` trait provides a unified interface:

```rust
pub trait Database: Send + Sync {
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    fn inventory(&self) -> Box<dyn InventoryRepository + '_>;
    fn customers(&self) -> Box<dyn CustomerRepository + '_>;
    fn payments(&self) -> Box<dyn PaymentRepository + '_>;
    // ... 32 repository accessors total
}
```

Backend switching is achieved at configuration time — no code changes required:

```rust
let db = match config.backend {
    Backend::Sqlite => SqliteDatabase::new(path)?,
    Backend::Postgres => PostgresDatabase::new(url).await?,
};
```

### 5.2 Transaction Support

ACID transactions are supported at multiple isolation levels:

```rust
pub enum TransactionIsolation {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}
```

Critical payment operations (complete refund, create payment method, set default payment method) use `with_immediate_transaction()` for atomicity.

### 5.3 Migration System

Schema migrations are managed by the `stateset-migrations` crate with SHA-256 checksums for integrity verification and rollback support. The `a2a/store.js` module extends the schema with 13 additional tables for agent-to-agent commerce operations.

---

## 6. MCP Tool Surface: 365 Deterministic Operations

### 6.1 Architecture

The MCP server (`mcp-server.js`, 470 lines) is a thin orchestrator that loads tools from 39 domain-specific modules. Every tool invocation passes through a standard pipeline:

```
Tool Call → Permission Gate → Telemetry Span → Handler → Response Envelope
```

The `adaptTool()` function wraps each raw handler with permission checking, treasury charging (for metered operations), span-based telemetry, and consistent error formatting. All error responses follow the shape `{ success: false, error: '...' }`.

### 6.2 Tool Categories

| Category | Module Count | Tool Count | Description |
|----------|-------------|------------|-------------|
| Core Commerce | 18 | ~140 | Orders, inventory, customers, products, carts, returns, payments, shipments, manufacturing, invoices, suppliers, warranties, subscriptions, promotions, tax, currency, analytics, reviews |
| A2A Commerce | 1 | 53 | Direct payments, quotes, escrow, splits, subscriptions, disputes, reputation, webhooks, events |
| x402 Protocol | 1 | 13 | Payment intents, signing, settlement, nonces, credit ledger |
| Search & Discovery | 2 | 21 | Vector semantic search, agent card registry |
| Platform Operations | 6 | 40 | Sync, import/export, custom objects, connectors, treasury, ERC-8004 |
| Specialized | 6 | ~40 | Fraud detection, gift cards, store credits, loyalty, segments, shipping zones, wishlists |
| Blockchain | 1 | 4 | Native stablecoin payments (USDC, ssUSD) on 10 chains |
| Agentic Runtime | 1 | 8 | Knowledge loading, agent delegation, policy evaluation |

### 6.3 Zod Validation

Every tool parameter is validated with Zod schemas before execution. Numeric fields use `.int()` and `.positive()` where appropriate, string IDs enforce `.min(1)`, email fields use `.email()`, and enums use `.enum()`. This prevents malformed data from reaching the core engine.

### 6.4 Permission Model

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

## 7. Agent-to-Agent (A2A) Commerce Protocol

### 7.1 Motivation

When AI agents operate autonomously, they need to transact with each other: a data-processing agent pays an API provider agent, a buyer agent negotiates prices with a seller agent, a platform agent distributes revenue to vendor agents. The A2A Commerce Protocol provides these primitives natively.

### 7.2 Protocol Primitives

#### 7.2.1 Direct Payments

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

#### 7.2.2 Quote Negotiation (RFQ Protocol)

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

#### 7.2.3 Conditional Payments (Escrow)

Funds can be held in escrow with programmable release conditions:

| Condition Type | Description |
|----------------|-------------|
| `seller_fulfilled` | Released when seller marks order fulfilled |
| `buyer_confirmed` | Released when buyer confirms receipt |
| `time_lock` | Auto-released after a specified duration |
| `milestone` | Released upon milestone completion |

Escrow payments link to x402 payment intents for on-chain settlement. If conditions are not met within the timeout period, funds are automatically returned.

#### 7.2.4 Split Payments

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

#### 7.2.5 Recurring Subscriptions

Agents can subscribe to other agents' services with recurring payments:

```
Status Machine: pending → trial → active → paused → past_due → cancelled → expired
```

Billing intervals: weekly, biweekly, monthly, bimonthly, quarterly, semiannual, annual. Trial periods, skip billing, pause/resume, and graceful cancellation are all supported.

#### 7.2.6 Dispute Resolution

When transactions go wrong, the dispute protocol provides structured resolution:

1. Either party creates a dispute with evidence
2. Counterparty submits evidence (documents, images, transaction logs)
3. Auto-escalation after 7 days if unresolved
4. Resolution with refund or payout decision

#### 7.2.7 Reputation & Trust

Agent reputation is tracked across transactions:

- **Trust levels**: Verified, unverified, suspended
- **Reputation scores**: 0-100, based on transaction history
- **Ratings**: Buyer and seller ratings per transaction
- **Agent Cards**: ERC-8004-compatible identity registry with wallet proofs and capability declarations

### 7.3 Webhook Notifications

A2A events trigger HMAC-SHA256-signed webhooks for real-time notification:

```
POST /webhook HTTP/1.1
X-StateSet-Signature: sha256=<hmac>
Content-Type: application/json

{ "event": "payment.completed", "data": { ... } }
```

SSRF protection validates webhook URLs against private IP ranges (localhost, 127.0.0.1, 10.x, 192.168.x, 172.16-31.x, .internal, .local). Delivery uses exponential backoff with a maximum of 3 retries.

### 7.4 Event Streaming (SSE)

Real-time events are delivered via Server-Sent Events with wildcard/prefix matching:

```javascript
// Subscribe to all payment events
a2a.subscribe('payment.*', (event) => { ... });

// Subscribe to specific quote events
a2a.subscribe('quote.requested', (event) => { ... });
```

Events are persisted in an append-only log for replay. A 30-second heartbeat maintains connection health.

### 7.5 Storage Schema

The A2A module extends the SQLite schema with 13 tables:

```
a2a_payments            a2a_payment_requests     a2a_quotes
a2a_escrows             a2a_disputes             a2a_feedback
a2a_services            a2a_notification_log     a2a_subscriptions
a2a_split_payments      a2a_split_recipients     a2a_event_subscriptions
a2a_event_log
```

All `update*()` methods use an `UPDATABLE_COLUMNS` whitelist with `_validateUpdateKeys()` to prevent SQL column injection.

---

## 8. x402 Payment Protocol

### 8.1 Overview

The x402 protocol enables AI agents to create, sign, and settle payment intents without requiring real-time network access. Intents are created and signed locally, then batched for on-chain settlement.

### 8.2 Intent Lifecycle

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

### 8.3 Signing Hash Computation

The signing hash is computed deterministically:

1. **Canonicalize** the intent payload using RFC 8785 JSON Canonicalization Scheme
2. **Apply domain separation** with the `VES_EVENTSIG_V1` prefix
3. **Hash** with SHA-256: `H = SHA256(domain_prefix || canonical_json)`
4. **Sign** with Ed25519: `sig = Ed25519_Sign(private_key, H)`

This produces identical hashes regardless of JSON key ordering, whitespace, or serialization library.

### 8.4 Supported Networks

| Network | Asset | Settlement |
|---------|-------|-----------|
| SET Chain L2 | ssUSD (yield-bearing) | Native, fast finality |
| Solana | USDC | SPL token transfer |
| Base | USDC | ERC-20 transfer |
| Ethereum | USDC | ERC-20 transfer |
| Arbitrum | USDC | ERC-20 transfer |
| Bitcoin | BTC | UTXO-based |
| Zcash | ZEC | Privacy-preserving |

### 8.5 Budget Governance

Each agent maintains a budget state that caps spending:

```javascript
const budget = createBudgetState({
  maxPerIntent: 100.00,      // Maximum per single intent
  maxPerDay: 1000.00,        // Daily spending cap
  maxPerMonth: 10000.00,     // Monthly spending cap
});
```

Budget exhaustion triggers a `BudgetExceededError` rather than silently failing.

### 8.6 Replay Protection

Every intent includes a monotonically increasing nonce per payer address. The `x402_get_next_nonce` tool retrieves the next valid nonce, and the sequencer rejects intents with reused or out-of-order nonces.

---

## 9. Verifiable Encrypted Signatures (VES) v1.0

### 9.1 Purpose

VES provides the cryptographic foundation for tamper-proof event synchronization between agents. Every state mutation in the commerce engine can be signed, encrypted, and verified across language boundaries.

### 9.2 Specification

The VES specification consists of five components:

#### 9.2.1 JSON Canonicalization (RFC 8785)

All JSON payloads are canonicalized before hashing or signing, ensuring byte-identical output regardless of serialization library:

- Object keys sorted lexicographically (Unicode code point order)
- No insignificant whitespace
- Numbers in shortest representation
- UTF-8 string encoding

Implementation: `serde_jcs` crate (Rust), custom `canonicalizeJson()` (JavaScript).

#### 9.2.2 Domain-Separated Hashing

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

#### 9.2.3 Ed25519 Signing

Event signatures use Ed25519 (via `ed25519-dalek`):

```rust
pub fn sign(message: &[u8], secret_key: &SigningKey) -> Signature {
    secret_key.sign(message)
}

pub fn verify(message: &[u8], signature: &Signature, public_key: &VerifyingKey) -> bool {
    public_key.verify(message, signature).is_ok()
}
```

#### 9.2.4 AES-256-GCM Encryption

Payload encryption uses AES-256-GCM with X25519 ECDH key exchange and HKDF key derivation:

```
1. Generate ephemeral X25519 keypair
2. Perform ECDH: shared_secret = ECDH(ephemeral_private, recipient_public)
3. Derive key: DEK = HKDF-SHA256(shared_secret, salt, info)
4. Encrypt: (ciphertext, tag) = AES-256-GCM(DEK, nonce, plaintext, AAD)
5. Zeroize DEK from memory
```

The `zeroize` crate ensures key material is scrubbed from memory after use. Hash comparisons use `subtle::ConstantTimeEq` to prevent timing attacks.

#### 9.2.5 Merkle Trees

Batch integrity is verified through Merkle trees with domain-separated leaf and node hashing:

```
Leaf:  H_leaf  = SHA-256(VES_LEAF_V1 || data)
Node:  H_node  = SHA-256(VES_NODE_V1 || left || right)
Pad:   H_pad   = SHA-256(VES_PAD_LEAF_V1 || index)
```

This enables O(log n) verification of individual events within a batch.

### 9.3 Cross-Language Verification

VES implementations exist in both Rust and JavaScript with 65 cross-language test vectors ensuring identical outputs:

- 32 Rust cross-language tests (`crates/stateset-crypto/tests/test_vectors.rs`)
- 33 JavaScript cross-language tests (`cli/test/unit/crypto-vectors.test.js`)

All vectors produce identical hex digests across both implementations.

### 9.4 NAPI Bindings

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

A JavaScript fallback (`cli/src/sync/crypto.js`) provides the same operations using Web Crypto APIs when native bindings are unavailable.

---

## 10. Policy Engine: Declarative Safety Guardrails

### 10.1 Architecture

The policy engine enables declarative business rules without hardcoding logic. Policies are defined in YAML or JSON and evaluated at runtime against a context object.

### 10.2 Rule Structure

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

### 10.3 Operators

The engine supports 20+ operators:

| Category | Operators |
|----------|-----------|
| Comparison | `eq`, `ne`, `gt`, `gte`, `lt`, `lte` |
| String | `contains`, `startsWith`, `endsWith`, `regex` |
| Collection | `in`, `notIn`, `hasAny`, `hasAll`, `hasNone` |
| Type | `type`, `exists`, `isNull`, `isNotNull` |
| Numeric | `between`, `divisibleBy` |

### 10.4 Deny-Override Semantics

When multiple rules match, **any deny action overrides all allow actions**. This ensures safety: a single security rule can block an operation even if ten other rules permit it.

### 10.5 Explainable Denials

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

This transparency is critical for autonomous agents that need to understand *why* an operation was denied and what steps to take next.

### 10.6 Transform Audit Trail

Policies can transform data (e.g., apply default values, normalize fields). Every transformation is tracked:

```json
{
  "field": "order.shipping_method",
  "before": null,
  "after": "standard",
  "rule": "default-shipping-method"
}
```

### 10.7 Pre-Built Templates

Five policy templates ship with the engine: returns eligibility, inventory thresholds, fraud detection, promotion rules, and subscription governance.

---

## 11. Autonomous Engine: Self-Governing Commerce

### 11.1 Overview

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

### 11.2 Subsystems

- **Scheduler**: Cron, interval, and one-time job execution with retry and exponential backoff
- **Workflow Engine**: Multi-step state machine workflows with conditional branching
- **Policy Engine**: Real-time rule evaluation against commerce events
- **Webhook Server**: Inbound event handling from external systems
- **Approval Queue**: Multi-level human-in-the-loop approval chains for high-stakes operations
- **Heartbeat Monitor**: Proactive health checks (low stock, abandoned carts, revenue milestones, overdue invoices, subscription churn)

### 11.3 Heartbeat Checks

Built-in health checks detect commerce anomalies:

| Check | Description | Default Interval |
|-------|-------------|-----------------|
| `low-stock` | SKUs below threshold | 1 hour |
| `abandoned-carts` | Carts idle > N hours | 24 hours |
| `revenue-milestone` | Revenue threshold alerts | 1 hour |
| `pending-returns` | Unprocessed returns | 4 hours |
| `overdue-invoices` | Past-due invoices | 24 hours |
| `subscription-churn` | Churn rate monitoring | 24 hours |

Alerts route through the EventBridge to all configured messaging channels.

---

## 12. Multi-Agent System: Specialized Commerce Agents

### 12.1 Agent Architecture

iCommerce provides 18 specialized agent configurations, each with a domain-specific system prompt and curated tool set:

| Agent | Domain | Tool Access |
|-------|--------|-------------|
| `customer-service` | Full-service | All 365 tools |
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

### 12.2 Semantic Routing

The `agent-router.js` module routes user requests to the most appropriate agent using confidence scoring:

```javascript
const result = routeToAgent("what's the status of order #12345?");
// → { agent: 'orders', confidence: 0.95 }
```

### 12.3 Session Management

Multi-turn conversations are persisted with full context:

```bash
stateset "create a cart for alice@example.com"
# Output: Session ID: abc-123-def

stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
stateset --apply --resume abc-123-def "complete the checkout"
```

### 12.4 Multi-Provider Support

The agent harness supports multiple AI providers:

| Provider | Models | MCP Tool Support |
|----------|--------|-----------------|
| Claude (Anthropic) | Opus 4, Sonnet 4, Haiku 4.5 | Full |
| OpenAI | GPT-4, GPT-4o, o1 | Chat only |
| Google | Gemini | Chat only |
| Ollama | Local models | Chat only |

Model fallback chains provide resilience: if the primary model is unavailable, the system automatically falls back to the next configured model.

---

## 13. Sync Architecture: Eventually Consistent Multi-Agent State

### 13.1 Overview

When multiple agents operate on the same commerce data, their local states must eventually converge. The sync architecture provides this through an outbox pattern with cryptographic verification.

### 13.2 Event Flow

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

### 13.3 Conflict Resolution Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| `LAST_WRITE_WINS` | Most recent timestamp wins | General use |
| `FIRST_WRITE_WINS` | Earliest timestamp wins | Orders (don't overwrite) |
| `MERGE` | Field-level automatic merge | Inventory updates |
| `MANUAL` | Requires human intervention | High-value operations |
| `CUSTOM` | User-defined resolver function | Domain-specific logic |

### 13.4 Transport Abstraction

Sync supports multiple transports:

- **REST**: Standard HTTP push/pull for low-frequency sync
- **gRPC**: Real-time streaming for high-throughput scenarios
- **SSE**: Server-sent events for browser-based clients

The unified client auto-selects the optimal transport based on availability.

### 13.5 Key Management

- **Identity keys**: Long-term Ed25519 signing keys
- **Session keys**: Ephemeral X25519 for key exchange
- **Content keys**: Derived AES-256 for payload encryption
- **Rotation policy**: Automatic rotation (default: 7 days) with configurable grace period

---

## 14. Observability & Telemetry

### 14.1 Metrics

Nine business-level counters track agentic commerce activity:

```
a2a_quotes              x402_intents              policy_evaluations
split_payments          subscription_renewals     webhook_deliveries
event_stream_processed  inventory_adjustments     payment_transactions
```

Counters use lock-free `AtomicU64` for contention-free updates. A `LatencyHistogram` provides p50/p95/p99 latencies for critical operations.

### 14.2 Tracing

Structured tracing via the `tracing` crate with request ID propagation across all layers. OpenTelemetry export is available behind the `otel` feature flag.

### 14.3 Audit Logging

Every tool invocation is logged with:

- **Actor**: Agent identity or API key
- **Resource**: Entity type and ID
- **Action**: Tool name and parameters
- **Decision**: Allowed, denied (with explanation), or transformed
- **Timestamp**: Microsecond precision

### 14.4 Subsystem Logging

The logger supports subsystem-scoped log channels with color-by-hash prefixes:

```javascript
const log = createSubsystemLogger('a2a');
log.info('Payment completed', { paymentId, amount });
// → [a2a] Payment completed { paymentId: '...', amount: 100 }
```

---

## 15. Security Architecture

### 15.1 Threat Model

iCommerce has been hardened against the OWASP Top 10 and agent-specific threat vectors:

| Threat | Mitigation |
|--------|-----------|
| SQL Injection | Column whitelist (`UPDATABLE_COLUMNS`) on all 12 `update*()` methods |
| SSRF | URL validation + private IP blocklist (localhost, 10.x, 192.168.x, 172.16-31.x) |
| Prototype Pollution | `mergeDeep()` filters `__proto__`, `constructor`, `prototype` |
| ReDoS | Non-greedy regex patterns (`.*` → `.*?`) |
| Shell Injection | Host validation with whitelisted character set |
| XSS | CSP nonce per request in admin dashboard |
| Path Traversal | Safe ID schema (alphanumeric, hyphens, underscores, dots) |
| Timing Attacks | `subtle::ConstantTimeEq` for cryptographic comparisons |
| Key Leakage | `zeroize` on all DEK/wrapping keys after use |
| Budget Exhaustion | Per-agent spending caps with `BudgetExceededError` |

### 15.2 Cryptographic Hygiene

- Zero `unsafe` code blocks in all Rust crates
- Ed25519 keys stored encrypted at rest
- HMAC-SHA256 webhook signatures
- TLS 1.3 for all transport
- Automatic key rotation (7-day default)
- No production `unwrap()` calls — all 270 instances are in test code

### 15.3 Permission Sandboxing

The HTTP gateway supports API key authentication with per-route permission levels. Sandbox mode blocks dangerous operations (browser evaluation, shell access) even for admin-level keys.

---

## 16. Language Bindings & Portability

### 16.1 Binding Strategy

The Rust core is exposed to 10 languages through a stable C ABI (`stateset-ffi`) with language-specific wrappers:

| Language | Technology | Tier |
|----------|-----------|------|
| Node.js | NAPI (`napi-rs`) | 1 |
| Python | PyO3 | 1 |
| Ruby | Magnus | 1 |
| PHP | ext-php-rs | 1 |
| Go | cgo | 2 |
| Java | JNI | 2 |
| Kotlin | JNI | 2 |
| Swift | C FFI | 2 |
| .NET | P/Invoke | 2 |
| WASM | wasm-bindgen | 2 |

Tier 1 bindings have full API coverage and production test suites. Tier 2 bindings cover core operations with ongoing expansion.

### 16.2 API Consistency

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

---

## 17. Admin Dashboard

### 17.1 Technology Stack

The admin dashboard is built with Next.js 14 (App Router), TypeScript, Tailwind CSS, Radix UI primitives, and Tremor charts.

### 17.2 Pages

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

### 17.3 Shared Libraries

The `admin/src/lib/shared/` module provides production-grade infrastructure:

- **Zod schemas** for request validation (auth, sessions, subscriptions)
- **AppError** class hierarchy with factory methods
- **Standard response envelope** (success/error/paginated)
- **Request-scoped context** with request ID propagation
- **CSRF token** generation and validation
- **Prometheus metrics** (counters, histograms)

---

## 18. Testing & Quality Assurance

### 18.1 Test Counts

| Layer | Framework | Tests | Pass Rate |
|-------|-----------|-------|-----------|
| Rust Core | `cargo test` | 3,196 | 100% (0 failures, 175 feature-gated) |
| CLI | `node --test` | ~6,611 | 98% (~130 pre-existing SQLite binary mismatch) |
| Admin | Vitest | 261 | 100% |
| **Total** | | **~10,068** | |

### 18.2 Test Categories

- **Unit tests**: Individual function and module tests
- **Integration tests**: Cross-crate and cross-module tests
- **Snapshot tests**: `insta` serialization snapshots (8 tests)
- **Property-based tests**: `proptest` for pricing and crypto (21 tests)
- **Cross-language vectors**: VES cryptographic compatibility (65 tests)
- **Security tests**: SQL injection, SSRF, prototype pollution, ReDoS (74+ tests)
- **Tool coverage tests**: Every MCP tool has at least basic exercise coverage

### 18.3 Quality Practices

- **Zero clippy warnings**: `cargo clippy --all-targets` produces 0 warnings
- **No production unwrap()**: All 270 `unwrap()` calls are in test code or doc examples
- **Zero empty catch blocks**: 92 catch blocks fixed across 46 files
- **Consistent error shapes**: All tool responses use `{ success: boolean, error?: string }`
- **Column whitelist enforcement**: SQL injection prevention on all update methods
- **Workspace lints**: `unused_must_use = "deny"`, `rust_2018_idioms = "deny"`

---

## 19. Performance

### 19.1 Build Configuration

```toml
[profile.release]
opt-level = 3
lto = "thin"
strip = "symbols"
panic = "abort"
codegen-units = 16
```

### 19.2 Development Optimizations

- `proptest` and `rand_chacha` compiled at `opt-level = 3` even in dev builds
- Debug info reduced to line tables only (`debug = "line-tables-only"`)
- Split debuginfo for faster linking

### 19.3 Runtime Characteristics

- **SQLite operations**: Single-digit millisecond latency for typical CRUD
- **Policy evaluation**: Microsecond-level for simple rules
- **Cryptographic operations**: Ed25519 sign/verify in microseconds (via `ed25519-dalek`)
- **MCP server startup**: Sub-second tool loading (470-line orchestrator)
- **Profiling**: `--profile profiling` preserves symbols for `perf` and flamegraph analysis

---

## 20. Related Work

### 20.1 Traditional Commerce Platforms

Shopify, WooCommerce, and Medusa.js are designed for human operators with web dashboards. They lack agent-native primitives, deterministic execution guarantees, and embeddable runtimes.

### 20.2 Headless Commerce APIs

Commerce.js, Saleor, and BigCommerce APIs provide RESTful access to commerce operations. However, they require network connectivity, introduce latency, and lack agent-to-agent transaction primitives.

### 20.3 Agent Frameworks

LangChain, CrewAI, and AutoGPT provide agent orchestration but no commerce-specific tooling. iCommerce complements these frameworks by providing the domain-specific tool surface that agents need for commerce operations.

### 20.4 Payment Protocols

Stripe, Square, and traditional payment processors are optimized for human-initiated transactions. x402 is designed for machine-initiated, cryptographically verifiable payment intents between autonomous agents.

### 20.5 Blockchain Commerce

Previous attempts at "decentralized commerce" (OpenBazaar, Origin Protocol) required full blockchain nodes and sacrificed usability. iCommerce takes a hybrid approach: local-first execution with optional on-chain settlement, combining the determinism of blockchain with the performance of local computation.

---

## 21. Conclusion

StateSet iCommerce represents a fundamental rethinking of commerce infrastructure for the age of autonomous AI agents. By providing an embeddable, deterministic, and cryptographically verifiable commerce engine with native agent-to-agent transaction primitives, it enables a new class of applications where AI agents independently manage entire commerce operations — from inventory forecasting to payment settlement, from customer service to supplier procurement.

The system's architecture — a type-safe Rust core with 50 repository traits, 365+ MCP tools, the A2A and x402 protocols, VES v1.0 cryptography, and a declarative policy engine — provides the safety guarantees that autonomous agents need to operate at scale. With 10,000+ tests, zero production `unwrap()` calls, and comprehensive security hardening, iCommerce is production-ready for the agentic commerce era.

The transition from eCommerce to iCommerce is not merely a technological upgrade; it is a paradigm shift in how commerce systems are designed, deployed, and operated. StateSet iCommerce is the runtime that makes this shift possible.

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
stateset-protocol ←── (wire types, EventEnvelope, SyncBatch)
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
