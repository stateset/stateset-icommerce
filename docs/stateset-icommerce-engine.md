# StateSet iCommerce Engine

## A Technical Review of an Embedded Commerce Runtime

**Version 0.7.4 | February 2026**

---

## Abstract

StateSet iCommerce is a vertically integrated commerce engine that spans from low-level cryptographic primitives in Rust to a natural-language CLI powered by Claude. It embeds a full commerce domain model — orders, inventory, payments, returns, subscriptions, manufacturing, and 40+ other entities — into a single library that compiles to native code, WebAssembly, and Node.js bindings. On top of this core sits a 186-tool MCP server, an 18-agent orchestration layer, an 8-channel messaging fabric, and an agent-to-agent (A2A) commerce protocol with escrow, split payments, and event streaming.

This paper is a comprehensive technical review of what has been built: the architecture, the design decisions, the security posture, and the quality metrics. It is based on a full audit of the codebase — approximately 50,000 lines of Rust across 7 crates, 25,000 lines of JavaScript across 143 CLI files, and 7,500+ automated tests.

---

## Table of Contents

1. [System Architecture](#1-system-architecture)
2. [Rust Core: The Crate Graph](#2-rust-core-the-crate-graph)
3. [Domain Model Design](#3-domain-model-design)
4. [Type System and Primitives](#4-type-system-and-primitives)
5. [Error Architecture](#5-error-architecture)
6. [Database Layer](#6-database-layer)
7. [The Embedded Commerce Runtime](#7-the-embedded-commerce-runtime)
8. [Event Sourcing and Sync](#8-event-sourcing-and-sync)
9. [Cryptographic Protocol: VES v1.0](#9-cryptographic-protocol-ves-v10)
10. [MCP Server and Tool Ecosystem](#10-mcp-server-and-tool-ecosystem)
11. [Agent System](#11-agent-system)
12. [Channel Orchestrator](#12-channel-orchestrator)
13. [A2A Commerce Protocol](#13-a2a-commerce-protocol)
14. [Policy Engine](#14-policy-engine)
15. [Permission and Security Architecture](#15-permission-and-security-architecture)
16. [Platform Adapters and Import](#16-platform-adapters-and-import)
17. [CLI User Experience](#17-cli-user-experience)
18. [Infrastructure and CI/CD](#18-infrastructure-and-cicd)
19. [Quality Metrics](#19-quality-metrics)
20. [Grading Summary](#20-grading-summary)

---

## 1. System Architecture

The system is organized into three major layers:

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLI / Agents                              │
│  18 agents · 8 channels · MCP server · Policy engine · UX       │
│  (Node.js, ES modules, ~25,000 LOC)                             │
├─────────────────────────────────────────────────────────────────┤
│                     NAPI Bindings                                │
│  7 #[napi] functions · JS fallback · Cross-language vectors      │
├─────────────────────────────────────────────────────────────────┤
│                      Rust Core                                   │
│  7 crates · 48 models · 27 ID types · VES crypto · SQLite/PG    │
│  (Edition 2024, Rust 1.85+, ~50,000 LOC)                        │
└─────────────────────────────────────────────────────────────────┘
```

**Workspace members** (Cargo.toml):

| Crate | Role | LOC (approx) |
|-------|------|-------------|
| `stateset-primitives` | Newtype IDs, Money, SKU, CurrencyCode | 1,200 |
| `stateset-core` | Models, errors, events, traits, validation | 18,000 |
| `stateset-crypto` | VES v1.0 — JCS, Ed25519, AES-GCM, Merkle | 2,500 |
| `stateset-db` | SQLite + PostgreSQL repositories, transactions | 12,000 |
| `stateset-embedded` | Commerce struct, builder, event system | 10,000 |
| `stateset-observability` | Prometheus metrics, tracing bootstrap | 1,500 |
| `stateset-test-utils` | Fixtures, assertion macros, snapshot tests | 1,200 |

Additional workspace members include 10 language binding crates (Node, Python, WASM, Ruby, PHP, Java, Kotlin, Swift, .NET, Go), though only the Node binding is fully wired at present.

The **default build** compiles only the 7 core crates. Binding crates require their respective toolchains and are excluded from `default-members` to keep the developer experience clean.

---

## 2. Rust Core: The Crate Graph

### 2.1 Dependency Flow

```
stateset-primitives  (zero external deps beyond serde/uuid/decimal)
        │
        ▼
  stateset-core  (models, errors, traits, validation, events)
        │
   ┌────┼────┐
   ▼    ▼    ▼
 db  crypto  observability
   │
   ▼
embedded  (top-level facade, composes all)
   │
   ▼
test-utils  (dev-only, fixtures + assertions)
```

This layering enforces a strict rule: **primitives depend on nothing internal**, core depends only on primitives, and all other crates depend on core. There are no circular dependencies.

### 2.2 Build Configuration

The workspace uses Edition 2024 with `rust-version = "1.85"`, placing it on the latest stable Rust. Build profiles are carefully tuned:

- **Dev**: `debug = "line-tables-only"`, `split-debuginfo = "unpacked"` — fast iteration with usable stack traces. Proptest and `rand_chacha` are compiled at `opt-level = 3` for tolerable fuzz speed.
- **Release**: `opt-level = 3`, `lto = "thin"`, `strip = "symbols"`, `panic = "abort"`, `codegen-units = 16` — production-ready.
- **Profiling**: Inherits release but keeps `debug = "full"` and `strip = "none"` for flamegraph analysis.

### 2.3 Workspace Lints

Modeled on reth, foundry, and alloy conventions:

```toml
[workspace.lints.rust]
missing_debug_implementations = "warn"
unreachable_pub               = "warn"
unused_must_use               = "deny"
rust_2018_idioms              = { level = "deny", priority = -1 }

[workspace.lints.clippy]
all                           = { level = "warn", priority = -1 }
use_self                      = "warn"
redundant_clone               = "warn"
missing_const_for_fn          = "warn"
result_large_err              = "allow"  # intentional for large error enums
```

The `unused_must_use = "deny"` lint is particularly aggressive — every `Result` and `#[must_use]` return value must be handled or explicitly discarded. Combined with `#[must_use]` annotations on Money, all ID types, and key builder methods, this makes ignoring important return values a compile error.

---

## 3. Domain Model Design

The core crate defines **48 domain models** organized into modules. Each model follows a consistent pattern:

1. **Primary struct** with serde derives, `#[non_exhaustive]`, and OCC versioning where applicable.
2. **Status enum** with strum derives (`Display`, `EnumString`, `EnumIter`) and `#[non_exhaustive]`.
3. **Create/Update DTOs** — separate input types that are not the same as the stored entity.
4. **Filter struct** for query composition.

### 3.1 Model Catalog

| Domain | Models |
|--------|--------|
| **Orders** | Order, OrderItem, OrderStatus (12 variants), OrderFilter |
| **Inventory** | InventoryItem, InventoryBalance (OCC), InventoryTransaction, InventoryReservation |
| **Returns** | Return, ReturnItem, ReturnStatus (8 states), ReturnReason |
| **Payments** | Payment, PaymentStatus, PaymentMethod, Refund |
| **Products** | Product, ProductVariant, ProductStatus |
| **Customers** | Customer, CustomerStatus, Address |
| **Subscriptions** | Subscription, SubscriptionPlan, BillingCycle, BillingInterval |
| **Manufacturing** | BillOfMaterials, BomComponent, WorkOrder, WorkOrderStatus |
| **Shipments** | Shipment, ShipmentStatus, TrackingEvent |
| **Invoices** | Invoice, InvoiceItem, InvoiceStatus |
| **A2A Commerce** | A2APayment, A2APaymentStatus, AgentCard, A2ASkill |
| **Crypto** | X402Intent, X402Status, StablecoinPayment |
| **Analytics** | SalesSummary, ProductMetrics, CustomerMetrics, DemandForecast |
| **Accounting** | AccountsPayable, AccountsReceivable, GeneralLedger, CostAccounting |
| **Warehouse** | Warehouse, Lot, Serial, Receiving, Fulfillment, Backorder |
| **Quality** | QualityInspection, InspectionResult |
| **New (v0.7.4)** | Fraud, GiftCard, Loyalty, Review, Segment, ShippingZone, StoreCredit, Wishlist |

### 3.2 State Machines

Status enums encode state machines with guarded transitions. For example, `OrderStatus`:

```
Created → Confirmed → Processing → Shipped → Delivered → Completed
    │         │           │                       │
    └─────────┴───────────┴── → Cancelled         └── → Returned
```

Each status type implements `can_transition_to()` which returns a boolean for whether a given state change is valid. This is enforced at the repository layer — attempting an invalid transition returns a `StateTransitionError<S>` with the source state, attempted target, and the set of valid targets.

### 3.3 Non-Exhaustive Enums

**171 enums** carry `#[non_exhaustive]`, meaning downstream code must handle a wildcard arm. This is a forward-compatibility guarantee: new variants can be added in minor versions without breaking consumers. Combined with strum's `EnumIter`, this provides introspection for documentation generation and UI rendering.

---

## 4. Type System and Primitives

### 4.1 Newtype IDs

The `define_id!` macro generates **27 strongly-typed ID types**:

```rust
define_id!(OrderId, CustomerId, ProductId, InventoryItemId, ReturnId,
           PaymentId, ShipmentId, InvoiceId, WarrantyId, SubscriptionId,
           CartId, PromotionId, CouponId, WorkOrderId, BomId,
           PurchaseOrderId, SupplierId, WarehouseId, LotId, SerialId,
           FulfillmentId, BackorderId, VectorId, CurrencyRateId,
           AnalyticsId, CustomObjectId, AgentCardId);
```

Each ID wraps `uuid::Uuid` and derives: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`, `Display`, plus `#[must_use]`. Under the `arbitrary` feature flag, they also derive `proptest::Arbitrary` for property-based testing.

The critical property: **OrderId and CustomerId are not interchangeable at compile time**. This eliminates an entire class of bugs where entity IDs are accidentally swapped.

### 4.2 Money Type

```rust
#[must_use]
pub struct Money {
    pub amount: Decimal,
    pub currency: CurrencyCode,
}
```

- **Currency safety**: `checked_add` and `checked_sub` return `Err` if currencies differ. You cannot accidentally add USD to EUR.
- **Precision**: Uses `rust_decimal::Decimal` (128-bit) — no floating-point rounding issues.
- **CurrencyCode**: Stored as `[u8; 3]` (ISO 4217), avoiding String allocation. Provides 8 constants: `USD`, `EUR`, `GBP`, `JPY`, `CAD`, `AUD`, `CHF`, `CNY`.

### 4.3 SKU Type

```rust
#[must_use]
pub struct Sku(String);
```

Validated on construction: 1–50 characters, uppercase alphanumeric plus hyphens. This prevents empty SKUs, overly long strings, and invalid characters from ever entering the system.

---

## 5. Error Architecture

### 5.1 Two-Level Hierarchy

The error system uses a pattern borrowed from reth: a top-level `CommerceError` that wraps domain-specific sub-errors.

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CommerceError {
    #[error(transparent)] Order(#[from] OrderError),
    #[error(transparent)] Inventory(#[from] InventoryError),
    #[error(transparent)] Payment(#[from] PaymentError),
    #[error(transparent)] Product(#[from] ProductError),
    #[error(transparent)] Customer(#[from] CustomerError),
    #[error(transparent)] Return(#[from] ReturnError),
    #[error(transparent)] Shipping(#[from] ShippingError),
    #[error(transparent)] Db(#[from] DbError),
    #[error(transparent)] Validation(#[from] ValidationError),
    // ...
}
```

### 5.2 Size Assertions

Error sizes are pinned with compile-time assertions:

```rust
static_assert_size!(CommerceError, 80);
static_assert_size!(OrderError, 48);
static_assert_size!(DbError, 64);
```

This prevents accidental size regressions from adding new variant data. If a new field pushes `CommerceError` past 80 bytes, the build fails — forcing developers to consider boxing or restructuring.

### 5.3 State Transition Errors

A generic `StateTransitionError<S>` captures failed state machine transitions:

```rust
pub struct StateTransitionError<S> {
    pub from: S,
    pub to: S,
    pub valid_targets: Vec<S>,
}
```

This enables excellent error messages: "Cannot transition order from `Shipped` to `Created`. Valid transitions: `Delivered`, `Returned`."

### 5.4 Batch Results

For bulk operations, `BatchResult<T>` tracks partial success:

```rust
pub struct BatchResult<T> {
    pub succeeded: Vec<T>,
    pub failed: Vec<(String, CommerceError)>,
}
```

This avoids the "all or nothing" problem — a batch import of 1,000 products can report 998 successes and 2 failures with specific error details.

### 5.5 Validation

A composable `ValidationBuilder` provides fluent validation:

```rust
ValidationBuilder::new()
    .required("email", &customer.email)
    .email("email", &customer.email)
    .max_length("name", &customer.name, 100)
    .positive_decimal("price", &product.price)
    .build()?;
```

The builder accumulates all validation failures and returns them as a single `ValidationError` with a list of field-specific issues — never short-circuiting on the first failure.

---

## 6. Database Layer

### 6.1 Repository Traits

The `stateset-core` crate defines repository traits with `#[auto_impl(&, Box, Arc)]`:

```rust
#[auto_impl(&, Box, Arc)]
pub trait OrderRepository {
    fn get(&self, id: &OrderId) -> Result<Order, CommerceError>;
    fn list(&self, filter: &OrderFilter) -> Result<Vec<Order>, CommerceError>;
    fn create(&self, order: &CreateOrder) -> Result<Order, CommerceError>;
    fn update(&self, id: &OrderId, update: &UpdateOrder) -> Result<Order, CommerceError>;
    fn delete(&self, id: &OrderId) -> Result<(), CommerceError>;

    // Batch operations
    fn batch_create(&self, orders: &[CreateOrder]) -> Result<BatchResult<Order>, CommerceError>;
    fn batch_create_atomic(&self, orders: &[CreateOrder]) -> Result<Vec<Order>, CommerceError>;
}
```

The `auto_impl` derive means a single trait implementation automatically works behind `&T`, `Box<T>`, and `Arc<T>` — no boilerplate wrappers.

### 6.2 Database Trait

A top-level `Database` trait aggregates all 20+ repository accessors:

```rust
pub trait Database: Send + Sync {
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    fn customers(&self) -> Box<dyn CustomerRepository + '_>;
    fn products(&self) -> Box<dyn ProductRepository + '_>;
    fn inventory(&self) -> Box<dyn InventoryRepository + '_>;
    // ... 16+ more
}
```

### 6.3 Transaction Support

```rust
pub struct TransactionOptions {
    pub timeout: Duration,
    pub isolation: IsolationLevel,
    pub retries: u32,
}
```

Transactions use a closure-based API:

```rust
db.transaction(|tx| {
    let order = tx.orders().get(&order_id)?;
    tx.inventory().adjust(&sku, -order.quantity)?;
    tx.orders().update(&order_id, &update)?;
    Ok(())
}, TransactionOptions::default())?;
```

### 6.4 SQLite Implementation

The `stateset-db` crate provides a full SQLite implementation using `rusqlite` with `r2d2` connection pooling. Each of the 20+ repository traits has a concrete `Sqlite*Repository` struct.

All 40+ SQLite repository structs implement `Debug` (manually, since they hold `dyn` trait objects). Each uses parameterized queries throughout — no string interpolation of user input.

### 6.5 Async Path

Async repository traits (`AsyncRepository`, `AsyncTransactional`) mirror the sync API using `async-trait`. The PostgreSQL backend (via `sqlx`) implements these for production deployments where connection pooling and async I/O matter.

---

## 7. The Embedded Commerce Runtime

### 7.1 The Commerce Struct

`stateset-embedded` provides the top-level `Commerce` struct — a facade that wires together database, events, metrics, and configuration:

```rust
pub struct Commerce {
    db: Arc<dyn Database>,
    backend: DatabaseBackend,
    metrics: Option<Arc<BusinessMetrics>>,
    event_system: Option<Arc<EventSystem>>,
}
```

### 7.2 Builder Pattern

```rust
let engine = Commerce::builder()
    .sqlite("store.db")?
    .with_events(EventConfig {
        capacity: 10_000,
        persistence: true,
        webhooks: true,
        max_retries: 3,
        ..Default::default()
    })
    .with_metrics()
    .build()?;
```

The builder supports three backends: `sqlite(path)`, `postgres(url)`, and `in_memory()` for testing. Feature flags control which backends are compiled.

### 7.3 Domain Methods

Each of the 48 domain modules adds methods to `Commerce` via dedicated source files:

```
src/orders.rs      → engine.create_order(), engine.ship_order(), ...
src/inventory.rs   → engine.adjust_inventory(), engine.reserve_inventory(), ...
src/returns.rs     → engine.create_return(), engine.approve_return(), ...
src/payments.rs    → engine.create_payment(), engine.create_refund(), ...
```

This keeps the `Commerce` struct definition small while allowing a rich API surface.

### 7.4 Event System

The embedded event system has four components:

1. **EventBus** (`events/bus.rs`) — tokio broadcast channels with atomic event counters. Subscribers receive events in real time without polling.
2. **EventEmitter** (`events/emitter.rs`) — enriches raw events with timestamps, sequence numbers, and correlation IDs before publishing to the bus.
3. **EventStore** (`events/store.rs`) — persistent event log for replay and audit. Events are append-only.
4. **WebhookManager** (`events/webhook.rs`) — HMAC-SHA256 signed HTTP delivery with configurable retry (exponential backoff), SSRF-safe URL validation, and event type filtering.

Configuration is centralized in `EventConfig`:

```rust
pub struct EventConfig {
    pub capacity: usize,           // broadcast channel size
    pub persistence: bool,         // write to event store
    pub webhooks: bool,            // enable webhook delivery
    pub max_retries: u32,          // webhook retry limit
    pub retry_delay: Duration,     // initial backoff
    pub batch_size: usize,         // batch publication
    pub filter: Vec<String>,       // event type whitelist
    pub webhook_timeout: Duration, // HTTP timeout
    pub dedup_window: Duration,    // idempotency window
    pub max_event_age: Duration,   // retention period
}
```

---

## 8. Event Sourcing and Sync

### 8.1 Event Types

The `CommerceEvent` enum defines **99+ named event types** organized by domain:

- **Orders**: `OrderCreated`, `OrderConfirmed`, `OrderShipped`, `OrderDelivered`, `OrderCancelled`, `OrderReturned`, `OrderRefunded`
- **Inventory**: `StockAdjusted`, `ReservationCreated`, `ReservationConfirmed`, `ReservationReleased`, `LowStockAlert`, `StockTransferred`
- **Customers**: `CustomerCreated`, `CustomerUpdated`, `CustomerDeleted`
- **Payments**: `PaymentCreated`, `PaymentCompleted`, `PaymentFailed`, `RefundProcessed`
- **A2A**: `A2APaymentCreated`, `A2APaymentCompleted`, `IntentSigned`, `IntentSettled`
- Plus events for returns, subscriptions, shipments, invoices, warranties, manufacturing, and more.

### 8.2 Event Capture (CLI)

On the JavaScript side, `cli/src/sync/capture.js` implements `EventCapture` — a class that intercepts tool calls and emits structured events:

- **11 entity type mappings** (tool module → event domain)
- **Idempotency keys** generated per event to prevent duplicate processing
- **Immutable outbox** — events are written to a local outbox table before any external delivery
- **OCC versioning** — entities carry version numbers; updates include `expected_version` to detect conflicts

### 8.3 Sync Engine

`cli/src/sync/engine.js` implements the `SyncEngine` class:

- **Push**: Local events are batched and sent to the remote (gRPC or REST transport)
- **Pull**: Remote events are fetched, validated (signature + Merkle proof), and applied locally
- **Status**: Reports sync state (last sequence, pending events, lag)
- **Streaming**: Supports long-lived gRPC streams for real-time sync

### 8.4 Merkle Verification

Every sync batch includes a Merkle root computed over the event hashes. The receiving side recomputes the Merkle root from the individual events and verifies it matches. This provides tamper evidence — if any event in the batch was modified in transit, verification fails.

---

## 9. Cryptographic Protocol: VES v1.0

VES (Verifiable Event Sync) is a custom protocol implemented in both Rust (`stateset-crypto`) and JavaScript (`cli/src/sync/crypto.js`) with cross-language test vectors ensuring identical behavior.

### 9.1 Components

| Module | Purpose | Algorithm |
|--------|---------|-----------|
| `canonicalize.rs` | Deterministic JSON serialization | RFC 8785 (JCS) via `serde_jcs` |
| `hash.rs` | Domain-separated hashing | SHA-256 with typed prefixes |
| `sign.rs` | Event signing | Ed25519 via `ed25519-dalek` |
| `encrypt.rs` | Payload encryption | AES-256-GCM + X25519 ECDH + HKDF |
| `merkle.rs` | Batch verification | Binary Merkle tree |
| `encoding.rs` | Wire format | Hex encoding utilities |

### 9.2 Domain Separation

All hash operations use explicit domain prefixes to prevent cross-context attacks:

```rust
pub const PAYLOAD_PLAIN: &[u8]  = b"VES:v1:payload-plain:";
pub const PAYLOAD_CIPHER: &[u8] = b"VES:v1:payload-cipher:";
pub const EVENTSIG: &[u8]       = b"VES:v1:eventsig:";
pub const MERKLE_LEAF: &[u8]    = b"VES:v1:merkle-leaf:";
pub const MERKLE_NODE: &[u8]    = b"VES:v1:merkle-node:";
pub const SYNC_BATCH: &[u8]     = b"VES:v1:sync-batch:";
// ... 10 total prefixes
```

A hash of a Merkle leaf cannot collide with a hash of a Merkle node, because the domain prefix is different. This is a defense-in-depth measure against second-preimage attacks.

### 9.3 NAPI Bindings

Seven functions are exposed to Node.js via `#[napi]`:

```rust
#[napi] fn jcs_canonicalize(json: String) -> Result<String>;
#[napi] fn domain_hash(domain: String, data: Buffer) -> Result<Buffer>;
#[napi] fn ed25519_sign(secret_key: Buffer, message: Buffer) -> Result<Buffer>;
#[napi] fn ed25519_verify(public_key: Buffer, message: Buffer, signature: Buffer) -> Result<bool>;
#[napi] fn aes_gcm_encrypt(key: Buffer, nonce: Buffer, plaintext: Buffer, aad: Buffer) -> Result<Buffer>;
#[napi] fn aes_gcm_decrypt(key: Buffer, nonce: Buffer, ciphertext: Buffer, aad: Buffer) -> Result<Buffer>;
#[napi] fn merkle_root(leaves: Vec<Buffer>) -> Result<Buffer>;
```

### 9.4 Cross-Language Verification

33 JavaScript tests and 32 Rust tests use identical inputs and assert identical hex outputs. This guarantees that a Merkle root computed in Rust matches one computed in JavaScript, that signatures from one language verify in the other, and that encryption is interoperable.

### 9.5 JS Fallback

When the native NAPI module isn't available (e.g., in CI without Rust toolchain), `cli/src/sync/crypto.js` falls back to pure JavaScript implementations. An `isNativeAvailable()` function reports which path is active.

---

## 10. MCP Server and Tool Ecosystem

### 10.1 Architecture

The MCP (Model Context Protocol) server was rewritten from a 9,340-line monolith to a **470-line orchestrator** that imports 25 modular tool files. This 95% reduction in size made the codebase maintainable without losing any functionality.

```
cli/src/mcp-server.js          (470 lines — orchestrator)
cli/src/tools/
  ├── orders.js                 (5 tools)
  ├── inventory.js              (8 tools)
  ├── payments.js               (5 tools)
  ├── returns.js                (5 tools)
  ├── products.js               (4 tools)
  ├── customers.js              (3 tools)
  ├── carts.js                  (14 tools)
  ├── analytics.js              (10 tools)
  ├── subscriptions.js          (15 tools)
  ├── promotions.js             (10 tools)
  ├── a2a.js                    (53 tools)
  ├── shipments.js              (3 tools)
  ├── suppliers.js              (6 tools)
  ├── invoices.js               (5 tools)
  ├── warranties.js             (4 tools)
  ├── manufacturing.js          (10 tools)
  ├── currency.js               (8 tools)
  ├── tax.js                    (8 tools)
  ├── stablecoin.js             (4 tools)
  ├── x402.js                   (6 tools)
  ├── treasury.js               (tools)
  ├── erc8004.js                (tools)
  ├── agent-cards.js            (5 tools)
  ├── vector.js                 (tools)
  ├── import.js                 (6 tools)
  └── custom-objects.js         (tools)
```

**Total: 186 tools** across 25 modules.

### 10.2 Tool Definition Pattern

Each tool file exports a `register(server, store)` function that defines tools with Zod schemas:

```javascript
server.tool("create_order", {
  customerId: z.string().min(1).describe("Customer ID"),
  items: z.array(z.object({
    productId: z.string().min(1),
    sku: z.string().min(1).max(50),
    quantity: z.number().int().positive(),
    unitPrice: z.number().positive(),
  })).min(1).max(100),
  currency: z.string().length(3).default("USD"),
}, async (params) => { /* ... */ });
```

Zod schemas provide:
- **Runtime validation** — types, ranges, formats checked before handler runs
- **Self-documentation** — `.describe()` annotations generate API docs
- **Constraints** — `.int()` on quantities, `.positive()` on prices, `.min(1)` on IDs, `.email()` on email fields, `.url()` on URLs, `.enum()` on status fields

### 10.3 Preview-Before-Apply

All write operations follow a preview-before-apply pattern:

```javascript
if (!preview) {
  return { success: true, preview: true, operation: "create_order", details: { ... } };
}
const order = await store.createOrder(params);
return { success: true, order };
```

This means every mutation is first shown to the user (or agent) as a preview, and only executed when explicitly confirmed with `--apply`.

### 10.4 Error Response Consistency

All 186 tool handlers return a consistent error shape:

```javascript
{ success: false, error: "Human-readable error message" }
```

This was standardized in a dedicated quality pass — approximately 80 inconsistent return patterns were fixed across 19 tool files.

---

## 11. Agent System

### 11.1 Agent Definitions

18 specialized agents are defined in `cli/src/agent-definitions.js`:

| Agent | Domain | Tool Access |
|-------|--------|-------------|
| `customer-service` | General support | All tools |
| `checkout` | Cart + checkout | carts, products, inventory, promotions, tax |
| `orders` | Order lifecycle | orders, shipments, payments |
| `inventory` | Stock management | inventory, products, warehouse |
| `returns` | RMA processing | returns, orders, inventory, payments |
| `analytics` | Business intelligence | analytics (read-only) |
| `promotions` | Discounts + coupons | promotions, products |
| `subscriptions` | Recurring billing | subscriptions, payments, customers |
| `storefront` | Site generation | filesystem, templates |
| `manufacturing` | Production | manufacturing, inventory, products |
| `payments` | Payment processing | payments, orders, stablecoin |
| `stablecoin` | Crypto payments | stablecoin, x402, treasury |
| `suppliers` | Procurement | suppliers, purchase orders, inventory |
| `invoices` | B2B billing | invoices, customers, payments |
| `warranties` | Product warranties | warranties, products, customers |
| `currency` | Multi-currency | currency, exchange rates |
| `tax` | Tax calculation | tax, jurisdictions |
| `a2a` | Agent commerce | a2a, x402, agent-cards |

Each agent has:
- A **system prompt** with domain-specific instructions
- A **tool whitelist** — agents can only see tools relevant to their domain
- A **model preference** — some agents use Haiku for speed, others use Sonnet or Opus for reasoning

### 11.2 Claude Harness

`cli/src/claude-harness.js` (~3,000 lines) integrates with the Claude Agent SDK:

- **Lane-based command queue** — concurrent tool calls are managed with priority queues
- **Model fallback chain** — if Opus is unavailable, falls back to Sonnet, then Haiku
- **Context guard** — monitors token usage and summarizes history when approaching limits
- **Session persistence** — conversation state survives across CLI invocations via `--resume`
- **Retry helpers** — exponential backoff with jitter for transient API failures

### 11.3 Auto-Routing

The main `stateset` command analyzes the user's natural language request and routes to the best-fit agent. This routing is itself an LLM call that examines the intent and selects from the 18 available agents.

---

## 12. Channel Orchestrator

### 12.1 Architecture

The `ChannelOrchestrator` (`cli/src/channels/orchestrator.js`) manages 8 messaging platform integrations:

| Channel | Gateway | Protocol |
|---------|---------|----------|
| Telegram | `telegram/gateway.js` | Bot API (polling + webhooks) |
| Discord | `discord/gateway.js` | Gateway WebSocket + REST |
| Slack | `slack/gateway.js` | Socket Mode + Web API |
| WhatsApp | `whatsapp/gateway.js` | Cloud API |
| Signal | `signal/gateway.js` | Signal Protocol (linked device) |
| Google Chat | `google-chat/gateway.js` | Pub/Sub + REST |
| iMessage | `imessage/gateway.js` | AppleScript bridge (macOS) |
| Microsoft Teams | `teams/gateway.js` | Bot Framework |
| Web Chat | `channels/webchat.js` | WebSocket |
| Matrix | `matrix/gateway.js` | Matrix Protocol |

### 12.2 Lazy Loading

Channels are loaded lazily — the orchestrator only imports a channel's gateway when a connection is requested. This keeps startup time fast even with 10 platform SDKs available.

### 12.3 Middleware Stack

Each channel passes messages through a configurable middleware pipeline:

1. **Metrics** — records message count, latency, error rate per channel
2. **Logger** — structured logging with channel-specific context
3. **Rate limiter** — per-user and per-channel rate limiting
4. **Content filter** — blocks PII, profanity, or custom patterns
5. **Auth** — validates user identity against configured identity providers

### 12.4 Session Management

`createSessionManager()` in `channels/base.js` provides:

- **30-minute TTL** — sessions expire after 30 minutes of inactivity
- **Persistent store recovery** — sessions survive process restarts via SQLite
- **Message chunking** — long responses are split at word boundaries to respect platform limits (2000 chars for Discord, 4096 for Telegram, etc.)
- **Rich messages** — templates for buttons, cards, carousels adapted per platform

### 12.5 Event Bridge

The `EventBridge` (`channels/event-bridge.js`) connects the commerce event system to channels. When a commerce event fires (e.g., `OrderShipped`), the bridge routes it to subscribed channels as a formatted notification.

---

## 13. A2A Commerce Protocol

### 13.1 Overview

The Agent-to-Agent (A2A) commerce protocol enables autonomous AI agents to transact with each other. It provides:

- **Agent discovery** via agent cards with capability declarations
- **Payment intents** with x402 protocol integration
- **Escrow** with conditional release (seller fulfilled, buyer confirmed, time lock, milestone)
- **Split payments** with percentage/fixed allocation and platform fees
- **Subscriptions** with trial periods and billing intervals
- **Event streaming** via SSE for real-time updates

### 13.2 Data Model

The A2A store (`cli/src/a2a/store.js`) manages 9 SQLite tables:

| Table | Purpose |
|-------|---------|
| `a2a_payments` | Payment records with x402 settlement |
| `a2a_escrow` | Escrow intents with conditions |
| `agent_cards` | Agent identity and capabilities |
| `notification_log` | Webhook delivery tracking |
| `webhook_config` | Webhook endpoint configuration |
| `subscriptions` | Recurring A2A payments |
| `split_payments` | Multi-party payment splits |
| `split_recipients` | Individual split allocations |
| `event_subscriptions` | SSE event subscriptions |
| `event_log` | Persistent event history |

All 12 update methods use **column whitelists** (`UPDATABLE_COLUMNS`) with a `_validateUpdateKeys()` guard to prevent SQL column injection. This was added after a security audit identified the risk of user-supplied column names in UPDATE statements.

### 13.3 Notifications

`a2a/notifications.js` implements webhook delivery with:

- **HMAC-SHA256 signing** — every webhook payload is signed with a shared secret
- **SSRF protection** — private IPs (localhost, 127.x, 10.x, 192.168.x, 172.16-31.x, .internal, .local) are blocked even when the URL validator module fails to load
- **Exponential backoff** — up to 3 retries with increasing delays
- **Delivery logging** — every attempt is recorded in `notification_log`

### 13.4 Subscriptions

`a2a/subscriptions.js` provides recurring payment management:

- **Billing intervals**: weekly, biweekly, monthly, quarterly, semiannual, annual
- **Trial periods**: configurable trial days before first billing
- **State machine**: `active` → `paused` → `past_due` → `cancelled`
- **Automatic renewal**: billing cycle generation with prorated amounts

### 13.5 Split Payments

`a2a/splits.js` handles multi-party payment distribution:

- **Percentage splits**: each recipient gets a percentage of the total
- **Fixed splits**: each recipient gets a fixed amount
- **Platform fees**: configurable platform fee deducted before splitting
- **Rounding drift prevention**: the last recipient absorbs rounding errors to ensure the sum matches the total exactly

### 13.6 Event Streaming

`a2a/event-stream.js` provides Server-Sent Events (SSE):

- **Wildcard matching**: subscribe to `order.*` to receive all order events
- **Prefix matching**: subscribe to `payment.` to receive payment events
- **30-second heartbeat**: keeps connections alive through proxies
- **Persistent event log**: events are stored for replay on reconnection
- **History replay**: clients can request events since a specific sequence number

### 13.7 Tool Count

The A2A module provides **53 MCP tools**, making it the largest single tool module. These cover payment creation, escrow management, agent discovery, subscription lifecycle, split payment configuration, event subscription, and webhook management.

---

## 14. Policy Engine

### 14.1 Architecture

The policy engine (`cli/src/policies/engine.js`) provides a declarative rule system for access control, data transformation, and business rules.

### 14.2 Condition Evaluation

18 operators for condition matching:

| Category | Operators |
|----------|-----------|
| Comparison | `eq`, `neq`, `gt`, `gte`, `lt`, `lte` |
| Collection | `in`, `not_in`, `contains`, `not_contains` |
| String | `starts_with`, `ends_with`, `matches` (regex) |
| Existence | `exists`, `not_exists` |
| Range | `between` |
| Boolean | `is_true`, `is_false` |

Conditions support:
- **Dot-notation path resolution**: `order.customer.email` traverses nested objects
- **Dynamic references**: `${context.user.role}` is resolved at evaluation time
- **Condition groups**: AND/OR composition with nested groups

### 14.3 Explainable Denials

The `PolicyExplanation` class provides per-condition detail:

```javascript
{
  field: "order.total",
  operator: "lte",
  expected: 10000,
  actual: 15000,
  matched: false,
  reason: "Order exceeds maximum allowed amount",
  remediation: "Split the order or request manager approval"
}
```

### 14.4 Transform Audit

Policy transforms (data modifications) are tracked with `TransformAuditEntry`:

```javascript
{ field: "order.discount", before: 0, after: 10, rule: "loyalty-discount" }
```

### 14.5 Deny-Overrides

The engine uses deny-overrides precedence: **any deny action overrides all allow actions**. This is the safest default for access control — you must explicitly allow, and a single deny wins.

### 14.6 Dry Run

`evaluateDryRun()` runs the full evaluation without applying side effects, returning the complete explanation tree. This is used for policy testing and debugging.

---

## 15. Permission and Security Architecture

### 15.1 RBAC Permission System

Six permission levels form a strict hierarchy:

| Level | Name | Value | Capabilities |
|-------|------|-------|-------------|
| 0 | `none` | 0 | No access |
| 1 | `read` | 1 | List, get, search |
| 2 | `preview` | 2 | Preview mutations (dry run) |
| 3 | `write` | 3 | Create, update |
| 4 | `delete` | 4 | Delete operations |
| 5 | `admin` | 5 | System configuration, user management |

### 15.2 Tool Permission Mapping

Every one of the 186 tools is mapped to a permission level in `cli/src/permissions.js`. The mapping is explicit — there is no default "allow" behavior. Tools not in the map are denied.

### 15.3 Rate Limiting

Built-in rate limiting at the permission layer:

- **120 calls per minute** (general)
- **30 write operations per minute**
- **$100,000/day transaction cap** (financial operations)

### 15.4 Audit Logging

Every tool invocation is logged with:
- Timestamp, tool name, permission level required
- User identity (from session or API key)
- Parameters (with sensitive fields like passwords, API keys, and tokens redacted)
- Result status (success/failure)

Redaction uses a `SENSITIVE_KEY_PATTERN` regex that matches common secret field names.

### 15.5 Security Hardening

The codebase has undergone multiple security passes:

| Category | Measures |
|----------|----------|
| **Injection** | Parameterized SQL throughout; SQL column whitelists on all UPDATE methods; Zod validation on all 186 tools |
| **SSRF** | Private IP blocking in webhook delivery and HTTP clients; URL validation for all user-supplied URLs |
| **XSS** | HTML stripping in Shopify import mapper; output encoding in channel messages |
| **ReDoS** | Non-greedy quantifiers in all regexes; specific fix in summarizer.js |
| **Prototype pollution** | `mergeDeep()` filters `__proto__`/`constructor`/`prototype`; `Span.setAttributes()` filters dangerous keys |
| **Shell injection** | SSH host validation with strict regex; no `child_process.exec()` with user input |
| **Path traversal** | Normalized paths in import adapters; no user-controlled file paths in fs operations |
| **Cryptography** | `crypto.randomUUID()` instead of `Math.random()`; Ed25519 for signatures; AES-256-GCM for encryption |
| **Secrets** | No hardcoded secrets; `cargo-deny` bans OpenSSL; credential redaction in telemetry |
| **Dependencies** | `cargo-deny` for Rust supply chain; `deny.toml` bans problematic crates; license allowlist |
| **Auth** | Timing-safe comparison for API keys; HMAC-SHA256 for webhook signatures |
| **Empty catches** | 92 empty catch blocks fixed across 46 files — all now log (warn for unexpected, debug for expected) |

### 15.6 Rust Safety

On the Rust side:
- **Zero production `unwrap()` calls** — all 270 instances are in test code or doc examples
- **`#[must_use]` on all critical types** — Money, all 27 ID types, builder methods, CurrencyCode constructors
- **Compile-time size assertions** on error types prevent accidental regression
- **`#[non_exhaustive]` on 171 enums** prevents match exhaustiveness from breaking on new variants
- **`cargo-deny`** bans `openssl`/`openssl-sys` — only `rustls` is allowed for TLS

---

## 16. Platform Adapters and Import

### 16.1 Abstract Adapter

`cli/src/adapters/base-adapter.js` defines an abstract `BasePlatformAdapter` with:

```javascript
class BasePlatformAdapter {
  async testConnection() { /* verify credentials */ }
  mapToStateSet(externalEntity, entityType) { /* transform */ }
  async *fetchBatches(entityType, options) { /* async generator */ }
}
```

The async generator pattern allows streaming large datasets without loading everything into memory.

### 16.2 Shopify Implementation

The Shopify adapter (`cli/src/adapters/shopify/`) provides:

| File | Purpose |
|------|---------|
| `mapper.js` | Pure functions mapping Shopify → StateSet (status codes, HTML stripping, field mapping) |
| `csv-parser.js` | RFC 4180-compliant CSV parser for bulk exports |
| `client.js` | REST API client with pagination, rate limiting, and SSRF-safe URL validation |
| `importer.js` | Orchestrates batch import with progress tracking |
| `webhooks.js` | Handles 8 Shopify webhook topics for real-time sync |
| `exporter.js` | Exports StateSet data to Shopify format |

### 16.3 ID Mapping

`cli/src/adapters/id-map-store.js` provides a SQLite-backed mapping between external platform IDs and internal StateSet IDs. This enables:

- **Incremental imports** — only process new/changed entities
- **Reference resolution** — orders reference products by Shopify ID; the mapper resolves to StateSet ProductId
- **Audit trail** — complete history of which external entity maps to which internal entity

### 16.4 Import Tools

6 MCP tools expose the import framework:

- `import_shopify_data` — Full Shopify import (products, customers, orders)
- `import_status` — Check import progress
- `list_id_mappings` — View external→internal ID mappings
- `import_csv` — Generic CSV import
- `import_json` — Generic JSON import
- `export_data` — Export to external platform format

---

## 17. CLI User Experience

### 17.1 Theme System

`cli/src/theme.js` defines a branded color palette:

```javascript
const PALETTE = {
  primary: '\x1b[38;2;75;120;255m',    // StateSet blue #4B78FF
  success: '\x1b[38;2;52;211;153m',     // Green
  error: '\x1b[38;2;251;113;133m',      // Red
  warning: '\x1b[38;2;251;191;36m',     // Amber
  info: '\x1b[38;2;96;165;250m',        // Sky blue
  muted: '\x1b[38;2;148;163;184m',      // Slate
  // ... compound helpers
};
```

The theme respects `NO_COLOR` and `FORCE_COLOR` environment variables per the `no-color.org` specification.

### 17.2 Interactive Prompts

`cli/src/ui.js` wraps `@clack/prompts` for interactive CLI flows:

- `withSpinner(message, fn)` — progress spinner during async operations
- `confirm(message)` — yes/no prompts
- `select(message, options)` — single-choice selection
- `text(message, options)` — free-text input
- `password(message)` — masked password input
- `intro(title)` / `outro(message)` — session bookends
- `note(message, title)` — informational panels
- `tasks(taskList)` — multi-step progress display

### 17.3 Progress System

`cli/src/progress.js` provides multi-backend progress indication:

- **TTY**: Full spinner with @clack (animated, colored)
- **Log**: Simple text output for piped/redirected output
- **Noop**: Silent for testing

`createProgress()` auto-detects the best backend.

### 17.4 Subsystem Logging

`cli/src/logger.js` provides `createSubsystemLogger(name)` which:

- Prefixes all log lines with a color-coded subsystem tag (color derived from name hash)
- Includes subsystem name in structured JSON log fields
- Respects log level configuration

### 17.5 Error Experience

`cli/src/graceful-shutdown.js` provides themed error output:

- **Error hints** — lazy-loaded from `cli/src/utils/error-hints.js`, providing actionable suggestions for common failures
- **Stack traces** — shown only in verbose mode or when `DEBUG` is set
- **Exit codes** — meaningful process exit codes for scripting

### 17.6 Setup Wizard

`bin/stateset-setup.js` provides a guided first-run experience:

- API key configuration with masked password prompt
- Database initialization with confirmation
- Provider selection (Claude, OpenAI, Gemini, Ollama)
- Connection verification
- Themed intro/outro banners

### 17.7 Doctor Command

`bin/stateset-doctor.js` validates the installation:

- Per-check timing (milliseconds)
- Auto-fix capability (`--fix` flag: creates missing directories, sets API keys)
- Checks: Node version, database connectivity, API key validity, dependency availability
- Themed output with pass/fail/warning indicators

---

## 18. Infrastructure and CI/CD

### 18.1 CI Pipeline

`.github/workflows/ci.yml` defines 9 jobs:

| Job | Purpose |
|-----|---------|
| `fmt` | `cargo fmt --all -- --check` |
| `clippy` | Workspace-wide clippy with deny warnings |
| `clippy-features` | Clippy with all feature combinations |
| `audit` | `cargo audit` for known vulnerabilities |
| `deny` | `cargo deny check` (licenses, bans, sources) |
| `dependency-review` | GitHub dependency review for PRs |
| `typos` | Spell checking across the codebase |
| `shellcheck` | Lint shell scripts |
| `msrv` | Verify minimum supported Rust version (1.85) |

### 18.2 Code Quality Tools

| Tool | Configuration |
|------|--------------|
| **ESLint** | Flat config at `cli/eslint.config.js`, with `eslint-config-prettier` |
| **Prettier** | Root config, `format:check` in CI |
| **Commitlint** | Conventional commits enforced via Husky `commit-msg` hook |
| **Husky** | Pre-commit (Prettier + ESLint), commit-msg (Commitlint) |
| **cargo-deny** | `deny.toml` — bans OpenSSL, allows 14 licenses, strict source policies |
| **jsconfig.json** | `checkJs: true` for JSDoc type checking in CLI |
| **`.nvmrc`** | Pins Node.js version |
| **`.editorconfig`** | Consistent formatting across editors |

### 18.3 Dependency Policy

- **Rust**: `cargo-deny` enforces a license allowlist (MIT, Apache-2.0, BSD, ISC, MPL-2.0, etc.) and explicitly bans `openssl` and `openssl-sys` in favor of `rustls`.
- **Node.js**: Dependencies are audited; `@stateset/embedded` is the NAPI binding consumed by the CLI.
- **TLS**: Only `rustls` is allowed — this eliminates the `libssl.so` runtime dependency that breaks in minimal containers.

### 18.4 Workspace Scripts

```json
{
  "check:rust": "cargo fmt --all -- --check && cargo test -p stateset-core -p stateset-db -p stateset-embedded --quiet",
  "check:cli": "npm --prefix cli run lint && npm --prefix cli run typecheck",
  "check": "npm run check:rust && npm run check:cli"
}
```

A single `npm run check` validates both the Rust and JavaScript codebases.

---

## 19. Quality Metrics

### 19.1 Test Counts

| Suite | Tests | Framework |
|-------|-------|-----------|
| **CLI unit tests** | ~6,611 pass | `node --test` (built-in) |
| **Rust core** | 378 | `cargo test` |
| **Rust crypto** | 91 (59 unit + 32 cross-language) | `cargo test` |
| **Rust db** | 37 | `cargo test` |
| **Rust test-utils** | 20 | `cargo test` |
| **Rust primitives** | 17 | `cargo test` |
| **Rust observability** | 6 | `cargo test` |
| **Rust doc-tests** | ~100 | `cargo test --doc` |
| **Admin (Next.js)** | 261 | Vitest |
| **Total** | **~7,500+** | |

### 19.2 Test Categories

- **Unit tests**: Individual functions and methods
- **Integration tests**: Database operations, multi-module interactions
- **Snapshot tests**: 8 insta serialization snapshots for enum variants
- **Property tests**: Proptest-based fuzzing for ID types and money operations
- **Cross-language tests**: 65 tests verifying Rust-JS crypto interoperability
- **Security tests**: SQL injection, XSS, SSRF, prototype pollution, ReDoS scenarios
- **Validation tests**: Zod schema constraint verification across all tools

### 19.3 Code Metrics

| Metric | Value |
|--------|-------|
| Rust LOC | ~50,000 |
| JavaScript LOC | ~25,000 |
| MCP tools | 186 |
| Domain models | 48 |
| Newtype IDs | 27 |
| Non-exhaustive enums | 171 |
| CLI test files | 70+ |
| Rust crates | 7 (+ 10 binding crates) |
| Messaging channels | 10 |
| Agent definitions | 18 |
| A2A tools | 53 |
| CLI binary entry points | 44 |
| Compiler warnings (Rust) | 0 |
| Empty catch blocks | 0 (92 fixed) |
| Production unwrap() calls | 0 |
| Hardcoded secrets | 0 |

### 19.4 Evolution

The project has undergone 18+ rounds of quality elevation:

1. **Security hardening** (5 rounds): Command injection, SQL injection, SSRF, XSS, ReDoS, prototype pollution, path traversal, shell injection, Math.random() fixes
2. **Infrastructure** (2 rounds): Commitlint, ESLint, Prettier, .nvmrc, .editorconfig, CI coverage
3. **MCP rewrite**: 9,340 → 470 lines (95% reduction)
4. **Rust elevation** (8 phases): Edition 2024, newtypes, error architecture, traits, feature flags, async, doc polish, test infra
5. **CLI quality** (4 rounds): Empty catches, error shapes, logging hygiene, Zod validation, magic numbers
6. **A2A commerce** (Phase B): 269 new tests, 7 tables, 20 new tools
7. **Crypto migration** (Phase F): stateset-crypto crate, NAPI bindings, cross-language vectors
8. **UX overhaul** (Phase E): Theme, @clack prompts, progress, setup wizard
9. **Platform adapters**: Shopify import/export, abstract adapter framework

---

## 20. Grading Summary

### Overall Grade: A

| Dimension | Grade | Rationale |
|-----------|-------|-----------|
| **Architecture** | A+ | Clean crate layering, no circular deps, separation of concerns at every level |
| **Type Safety** | A+ | 27 newtype IDs, Money currency safety, compile-time size assertions, #[must_use] |
| **Error Handling** | A | Two-level error hierarchy, state transition errors, batch results, ValidationBuilder |
| **Security** | A | Comprehensive: injection, SSRF, XSS, ReDoS, prototype pollution, crypto, audit logging |
| **Testing** | A | 7,500+ tests across 3 languages, property testing, snapshots, cross-language vectors |
| **API Design** | A | 186 tools with Zod validation, preview-before-apply, consistent error shapes |
| **Code Style** | A | Zero compiler warnings, zero empty catches, ESLint+Prettier, conventional commits |
| **Documentation** | A- | Runnable doc-tests, CLAUDE.md, CHANGELOG, but could use more inline architecture docs |
| **Infrastructure** | A- | 9-job CI, cargo-deny, Husky hooks, but no coverage enforcement yet |
| **DX** | A | Setup wizard, doctor command, themed output, subsystem logging, auto-routing |

### Strengths

1. **Vertical integration** — From Ed25519 signing to natural language checkout, every layer is owned.
2. **Type-driven design** — The Rust type system prevents entire categories of bugs at compile time.
3. **Security-first** — Multiple dedicated security passes have addressed OWASP top 10 across both languages.
4. **Test discipline** — 7,500+ tests is exceptional for a project of this scope and age.
5. **Modularity** — The MCP rewrite (9,340 → 470 lines) demonstrates willingness to restructure for maintainability.
6. **Cross-language consistency** — VES protocol works identically in Rust and JavaScript, verified by shared test vectors.

### Areas for Future Investment

1. **Coverage enforcement** — CI runs tests but doesn't gate on coverage percentages.
2. **Integration test suite** — End-to-end tests exercising the full CLI → MCP → embedded → DB stack.
3. **PostgreSQL testing** — SQLite path is well-tested; the async PostgreSQL path needs comparable coverage.
4. **Binding completion** — 9 of 10 language bindings are scaffolded but not wired.
5. **Architecture decision records** — The "why" behind key decisions (VES protocol, A2A design) deserves dedicated documentation.
6. **Performance benchmarks** — Criterion is a dependency but no benchmarks are committed.

---

## Appendix A: Version History

| Version | Date | Highlights |
|---------|------|------------|
| 0.1.7 | 2025-Q4 | Initial release — core models, SQLite, basic CLI |
| 0.3.1 | 2026-01 | HTTP gateway, permission sandbox, heartbeat monitor |
| 0.5.0 | 2026-01 | MCP server rewrite, tool modularization |
| 0.6.0 | 2026-02 | A2A commerce, VES crypto, channel orchestrator |
| 0.7.0 | 2026-02 | 1,842 tests, ESLint, Prettier, commitlint, 40+ test files |
| 0.7.2 | 2026-02 | Cross-language version alignment |
| 0.7.4 | 2026-02 | Setup wizard, @clack prompts, stateset-crypto crate |

## Appendix B: Dependency Policy

### Allowed Licenses (Rust)
MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, Unicode-DFS-2016, OpenSSL, BSL-1.0, CC0-1.0, Unlicense

### Banned Crates
- `openssl` / `openssl-sys` — use `rustls` for TLS
- Any crate with GPL-only licensing

### Key Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `serde` | 1.0 | Serialization framework |
| `rusqlite` | 0.32.1 (bundled) | SQLite with built-in amalgamation |
| `sqlx` | 0.8.1 | Async PostgreSQL (no default features) |
| `ed25519-dalek` | 2.x | Ed25519 signatures |
| `rust_decimal` | 1.36 | Precise decimal arithmetic |
| `tokio` | 1.x | Async runtime |
| `reqwest` | 0.12 (rustls-tls) | HTTP client without OpenSSL |
| `prometheus` | 0.14 | Metrics exposition |
| `strum` | 0.26 | Enum derives (Display, EnumString, EnumIter) |
| `insta` | 1.34 | Snapshot testing |
| `auto_impl` | 1.x | Auto-implement traits for smart pointers |

---

*This document was generated from a full codebase audit of stateset-icommerce v0.7.4, covering approximately 50 source files across 7 Rust crates and 143 JavaScript modules.*
