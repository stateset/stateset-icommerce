# StateSet iCommerce Engine v1.0.0

**The SQLite of Commerce** — an embedded, zero-dependency commerce engine for autonomous AI agents.

`cargo add stateset-sdk --features full` gives any Rust application a complete commerce backend in a single dependency. No database setup, no config files, no migrations to run. It just works.

---

## Why v1.0.0

This release marks API stability. The public types, traits, and error variants are frozen. Applications built against v1.0.0 will compile against any future v1.x release without breaking changes.

The engine is production-ready for real money: atomic inventory, idempotent checkout, financial rounding, audit logging, and cryptographic settlement via the VES protocol.

---

## By the Numbers

| Metric | Value |
|--------|-------|
| **Rust crates** | 21 |
| **Source lines** | 264,758 (Rust) + 142,035 (CLI JS) |
| **Rust tests** | 3,093 across 17 crates, zero failures |
| **Clippy warnings** | 0 |
| **REST endpoints** | 61+ |
| **Database tables** | 97 |
| **Database indexes** | 132 |
| **Migrations** | 9 (V1–V9), all with rollback |
| **Language bindings** | 11 (Rust, Python, Node.js, Go, Java, Kotlin, Swift, .NET, Ruby, PHP, WASM) |
| **A2A modules** | 16 |
| **Embedded modules** | 45 commerce operations |
| **Benchmark improvement** | ~3x vs v0.8.1 baseline |

---

## What's in the Box

### Commerce Core (45 modules)

Every operation a commerce business needs, accessible through a single `Commerce` struct:

- **Orders**: create, update, cancel, ship, fulfill, batch operations
- **Customers**: CRUD, email uniqueness, address management
- **Products**: catalog management, variants, attributes, SEO
- **Inventory**: stock tracking, reservations, lot management, backorders
- **Payments**: multi-method (credit card, stablecoin, x402), refunds, reconciliation
- **Returns**: RMA workflow, reason tracking, approval/rejection
- **Invoices**: B2B invoicing, send, record payments, dunning
- **Shipments**: multi-carrier, tracking, delivery confirmation
- **Carts**: shopping cart lifecycle, checkout, abandoned cart recovery
- **Subscriptions**: recurring billing, trial periods, pause/resume
- **Promotions**: discount rules, coupon codes, bulk pricing
- **Tax**: multi-jurisdiction, compound rates, exemptions
- **Reviews**: ratings, moderation, helpful voting, aggregation
- **Wishlists**: customer wishlists with item management
- **Gift Cards**: auto-generated codes, charge/refund, balance tracking
- **Loyalty**: programs, enrollment, points earn/redeem, tiers
- **Fraud**: risk assessment, configurable rules
- **Segments**: customer segmentation with dynamic membership
- **Store Credits**: balance management with transaction history
- **Analytics**: sales metrics, forecasting, customer insights
- **Manufacturing**: BOM, work orders, quality control
- **Accounting**: GL, AP, AR, cost accounting

### Agent-to-Agent Commerce (16 modules)

Everything AI agents need to transact autonomously:

- **Agent Cards**: identity, discovery by skill/network/trust tier
- **Negotiation Engine**: multi-round price negotiation with auto-accept/reject thresholds
- **A2A Messaging**: reliable delivery with exponential backoff retry
- **Credit Terms**: net 15/30/60/90 payment between trusted agents
- **Escrow**: conditional fund holding with 4 release condition types
- **Reputation**: 4-tier scoring (Sandbox → Standard → Verified → Enterprise)
- **Splits**: multi-party payment distribution with rounding drift prevention
- **Circuit Breaker**: spending limits, failure rate tracking, auto-recovery
- **Marketplace/RFQ**: request-for-quote with 3 scoring strategies
- **Dispute Resolution**: configurable auto-resolution rules by priority
- **Inventory Commitments**: stock locks on quote acceptance with auto-expiry
- **SLA Compliance**: service level tracking and penalty calculation
- **Notifications**: HMAC-SHA256 signed webhooks with SSRF protection
- **Event Streaming**: SSE with wildcard/prefix filtering
- **Subscriptions**: recurring billing with trial periods
- **Tax Obligations**: cross-border tax tracking for A2A transactions

### 61+ REST Endpoints

| Category | Endpoints |
|----------|-----------|
| Orders | POST, GET, GET/:id, PATCH/cancel, PATCH/ship |
| Customers | POST, GET, GET/:id, PATCH/:id, DELETE/:id |
| Products | POST, GET, GET/:id, PATCH/:id, DELETE/:id |
| Inventory | GET, GET/:sku, POST/adjust |
| Returns | POST, GET, GET/:id, PATCH/approve |
| Payments | POST, GET, GET/:id, POST/complete, POST/refund |
| Invoices | POST, GET, GET/:id, POST/send, POST/payments |
| Shipments | POST, GET, GET/:id, POST/deliver |
| Reviews | POST, GET, GET/:id, DELETE/:id |
| Wishlists | POST, GET, GET/:id, DELETE/:id, POST/items, DELETE/items |
| Gift Cards | POST, GET, GET/:id, POST/disable |
| Loyalty | POST/programs, GET/programs, POST/enroll, GET/accounts/:id |
| Negotiations | POST, GET/:id, POST/counter-offer, POST/accept, POST/reject |
| A2A Messaging | POST, GET, POST/acknowledge |
| A2A Credit | POST, GET, GET/:id, POST/charge, POST/payment |
| Health | GET/health, GET/health/ready, GET/health/deep |
| Metrics | GET/metrics (Prometheus) |
| OpenAPI | GET/openapi.json, GET/docs |

### 9 Database Migrations

| Version | Name | Tables |
|---------|------|--------|
| V1 | Core tables | customers, products, orders, inventory, returns, payments, shipments, warranties, invoices |
| V2 | Commerce extensions | carts, subscriptions, promotions, tax, currency |
| V3 | A2A commerce | x402 intents, agent cards, A2A quotes/purchases, custom objects |
| V4 | New entities | fraud, gift cards, loyalty, reviews, segments, shipping zones, store credits, wishlists |
| V5 | Composite indexes | 12 multi-column indexes for query performance |
| V6 | Production hardening | 3 idempotency constraints (order items, reservations, cart checkout) |
| V7 | Webhook dead letters | Persistent storage for failed webhook deliveries |
| V8 | Audit log | Compliance-ready mutation tracking |
| V9 | Agentic commerce | A2A messaging, negotiations, inventory commitments, credit terms, tax obligations, dispute rules |

### Performance

Benchmarked with Criterion across 20 functions:

| Benchmark | v0.8.1 | v1.0.0 | Speedup |
|-----------|--------|--------|---------|
| SQLite customers/100 | 110 ms | 17.5 ms | **6.3x** |
| Currency parse | 22 ns | 4.3 ns | **5.1x** |
| Money round | 374 ns | 75 ns | **5.0x** |
| SQLite orders/1000 | 1,832 ms | 497 ms | **3.7x** |
| JCS canonicalize | 3.9 µs | 1.4 µs | **2.9x** |
| EventBus publish | 313 µs | 113 µs | **2.8x** |
| Merkle tree/10k | 6.0 ms | 2.7 ms | **2.2x** |

Optimizations: fat LTO, codegen-units=1, target-cpu=native, SHA256 hardware acceleration, lock-free atomics, mmap I/O, WAL tuning, prepared statement caching, gzip response compression, 30-second request timeouts.

### Production Hardening

- **Atomic inventory reservation**: quantity + version check in single UPDATE WHERE clause
- **Idempotent checkout**: UNIQUE constraints on order_items(order_id, sku), reservations(item_id, ref), orders(cart_id)
- **Financial rounding**: round_dp(2) per line item and order total
- **Pricing drift detection**: warns if DB total differs from pricing engine >$0.01
- **SQLITE_FULL handling**: maps to proper StorageFull error
- **UNIQUE violations**: return 409 Conflict instead of 500
- **LIKE wildcard escaping**: prevents unintended pattern matching in search
- **Slow query logging**: transactions >500ms emit tracing::warn
- **Health check**: GET /health/deep with DB latency, pool stats, metrics
- **Graceful shutdown**: WAL checkpoint + PRAGMA optimize before exit
- **Audit log**: record_audit() for compliance tracking of all mutations
- **Webhook dead letters**: persistent storage + retry management for failed deliveries

### VES Protocol (Cryptographic Guarantees)

- Ed25519 signing and verification
- RFC 8785 JSON Canonicalization Scheme (JCS)
- AES-256-GCM encryption (VES-ENC-1)
- Merkle tree hashing with domain separation
- x402 payment intents with nonce-based replay protection
- Batched settlement on Set Chain L2

### 11 Language Bindings

| Language | Package | Install |
|----------|---------|---------|
| **Rust** | stateset-sdk | `cargo add stateset-sdk --features full` |
| **Python** | stateset-embedded | `pip install stateset-embedded` |
| **Node.js** | @stateset/embedded | `npm install @stateset/embedded` |
| **CLI** | @stateset/cli | `npm install -g @stateset/cli` |
| **Go** | stateset-go | `go get github.com/stateset/stateset-icommerce/bindings/go` |
| **Java** | stateset-java | Maven/Gradle |
| **Kotlin** | stateset-kotlin | Maven/Gradle |
| **Swift** | stateset-swift | SwiftPM |
| **.NET** | stateset-dotnet | NuGet |
| **Ruby** | stateset_embedded | `gem install stateset_embedded` |
| **PHP** | stateset-php | Composer |
| **WASM** | stateset-wasm | `npm install @stateset/wasm` |

---

## Quick Start

```rust
use stateset_sdk::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let commerce = Commerce::new("store.db")?;

    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    })?;

    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            sku: "WIDGET-001".into(),
            name: "Widget".into(),
            quantity: 2,
            unit_price: rust_decimal_macros::dec!(29.99),
            ..Default::default()
        }],
        ..Default::default()
    })?;

    println!("Order {} — ${}", order.order_number, order.total_amount);
    Ok(())
}
```

No database setup. No config files. No migrations to run. It just works.

See [QUICKSTART.md](./QUICKSTART.md) for the full 10-minute guide.

---

## Migration from v0.9.x

No breaking changes. Update your dependency version:

```toml
[dependencies]
stateset-sdk = "1.0"
```

All public APIs, error types, and database schemas are backward compatible.

---

## What's Next

- **Postgres V4+ parity**: async implementations for reviews, wishlists, gift cards, loyalty, fraud, segments
- **Smart contract arbitration**: on-chain enforcement of dispute resolution
- **Cross-chain settlement**: atomic swaps and bridge protocols
- **KYC/AML integration**: identity verification for regulated commerce

---

## Acknowledgments

Built with Rust, SQLite, and the belief that commerce infrastructure should be embeddable, local-first, and agent-native.

31 PRs shipped across 10 releases (v0.8.2 → v1.0.0) to reach this milestone.

**License**: MIT OR Apache-2.0
