# StateSet iCommerce
## Product Requirements Document (PRD)

**Version:** 1.0  
**Last Updated:** December 2025  
**Author:** StateSet Product Team  
**Status:** Draft  

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Product Vision](#3-product-vision)
4. [Target Users](#4-target-users)
5. [Goals & Success Metrics](#5-goals--success-metrics)
6. [Core Architecture](#6-core-architecture)
7. [Feature Requirements](#7-feature-requirements)
8. [Data Models](#8-data-models)
9. [API Specification](#9-api-specification)
10. [Agentic Commerce Protocol (ACP)](#10-agentic-commerce-protocol-acp)
11. [Neuro-Symbolic Reasoning Engine](#11-neuro-symbolic-reasoning-engine)
12. [Sync & Replication](#12-sync--replication)
13. [Security Requirements](#13-security-requirements)
14. [Performance Requirements](#14-performance-requirements)
15. [Platform Support](#15-platform-support)
16. [Roadmap](#16-roadmap)
17. [Dependencies](#17-dependencies)
18. [Open Questions & Risks](#18-open-questions--risks)
19. [Appendix](#19-appendix)

---

## 1. Executive Summary

StateSet iCommerce is an embedded commerce engine designed for AI agents. It provides a complete, portable commerce runtime that can be embedded directly into applications, enabling autonomous agents to perform commerce operations without network dependencies, authentication flows, or rate limits.

**Key value propositions:**

- **Embedded:** Runs in-process, no network required for core operations
- **Portable:** Complete commerce state in a single SQLite file
- **Deterministic:** Predictable operations that agents can reason about
- **Agent-native:** Designed from the ground up for autonomous operation
- **Policy-enforced:** Neuro-symbolic reasoning engine provides guardrails

**Target outcome:** Become the standard commerce runtime for AI agents — the "SQLite of commerce."

---

## 2. Problem Statement

### 2.1 Current State

Existing commerce infrastructure (Shopify, BigCommerce, Salesforce Commerce Cloud) was designed for human operators interacting through dashboards and web interfaces. These platforms assume:

- Users authenticate via OAuth flows
- Operations happen at human speed (seconds/minutes between actions)
- State is managed in vendor-controlled cloud infrastructure
- UIs are the primary interface for surfacing information
- Rate limits are appropriate for human interaction patterns

### 2.2 The Agent Commerce Gap

AI agents are increasingly capable of performing complex commerce operations:

- Processing returns and refunds
- Managing customer inquiries
- Coordinating fulfillment
- Handling subscription management
- Executing B2B procurement

However, agents face critical infrastructure challenges:

| Challenge | Impact |
|-----------|--------|
| OAuth/authentication flows | Agents cannot complete human-centric auth |
| Rate limits | Agents hit limits within seconds |
| Network latency | Each operation requires round-trip |
| Vendor lock-in | State trapped in third-party clouds |
| Non-determinism | API changes break agent workflows |
| Dashboard-centric design | Structured data inaccessible |

### 2.3 Opportunity

There is no commerce infrastructure purpose-built for AI agents. StateSet iCommerce fills this gap by providing an embedded, deterministic, portable commerce engine that agents can carry with them.

---

## 3. Product Vision

### 3.1 Vision Statement

> "Every AI agent that transacts in the real world runs StateSet."

### 3.2 Product Principles

1. **Embedded-first:** Network is optional, not required
2. **Portable state:** Single file contains complete commerce state
3. **Deterministic execution:** Same input produces same output, always
4. **Policy-enforced:** Business rules are code, not suggestions
5. **Protocol-native:** ACP is the interface, not an afterthought
6. **Developer experience:** Three lines of code to a working commerce engine

### 3.3 Category Definition

StateSet defines "iCommerce" (intelligent commerce) as commerce infrastructure built for autonomous agents rather than human operators. This is the third wave of commerce platforms:

- **Wave 1 (1995-2015):** eCommerce — humans buying online
- **Wave 2 (2015-2024):** Headless — developers building custom frontends
- **Wave 3 (2024+):** iCommerce — agents transacting autonomously

---

## 4. Target Users

### 4.1 Primary Users

#### 4.1.1 AI/Agent Developers

**Profile:** Engineers building AI agents that need to perform commerce operations

**Use cases:**
- Customer service agents processing returns
- Procurement agents managing B2B purchasing
- Subscription management agents handling billing
- Fulfillment agents coordinating logistics

**Needs:**
- Simple integration (npm/pip install)
- No authentication complexity
- Deterministic behavior for testing
- Structured data access

#### 4.1.2 Platform Engineers

**Profile:** Engineers building commerce-enabled applications

**Use cases:**
- Embedding commerce into existing applications
- Building offline-capable POS systems
- Creating edge-deployed commerce workers
- Developing multi-tenant commerce platforms

**Needs:**
- Embeddable library (not a service)
- Multi-runtime support (Node, Python, WASM)
- Sync capabilities for distributed systems
- Enterprise-grade reliability

### 4.2 Secondary Users

#### 4.2.1 Enterprise Operations Teams

**Profile:** Teams deploying agent-driven commerce automation

**Needs:**
- Policy configuration (return windows, approval thresholds)
- Audit logging
- Integration with existing systems
- Compliance controls

#### 4.2.2 Startup Founders

**Profile:** Early-stage companies building commerce-adjacent products

**Needs:**
- Zero upfront cost (open source)
- Quick time-to-value
- Scalable architecture
- Cloud upgrade path

---

## 5. Goals & Success Metrics

### 5.1 Product Goals

| Goal | Description | Timeline |
|------|-------------|----------|
| G1 | Ship production-ready embedded engine | Q1 2025 |
| G2 | Publish ACP specification | Q1 2025 |
| G3 | Achieve 1,000 active deployments | Q2 2025 |
| G4 | Launch StateSet Cloud (sync) | Q2 2025 |
| G5 | Integration with 3+ agent frameworks | Q3 2025 |
| G6 | First enterprise customer | Q3 2025 |

### 5.2 Success Metrics

#### 5.2.1 Adoption Metrics

| Metric | Year 1 Target | Year 2 Target |
|--------|---------------|---------------|
| npm/PyPI downloads | 50,000 | 500,000 |
| GitHub stars | 5,000 | 25,000 |
| Active deployments | 1,000 | 10,000 |
| Cloud subscribers | 100 | 1,000 |

#### 5.2.2 Usage Metrics

| Metric | Target |
|--------|--------|
| Operations per deployment/day | 1,000+ |
| GMV processed (annual) | $100M (Y1), $1B (Y2) |
| Sync events per day | 100,000+ |

#### 5.2.3 Quality Metrics

| Metric | Target |
|--------|--------|
| Operation latency (p99) | < 10ms |
| Data durability | 99.999% |
| API stability | Zero breaking changes per major version |
| Test coverage | > 90% |

---

## 6. Core Architecture

### 6.1 System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application                              │
├─────────────────────────────────────────────────────────────────┤
│                    stateset-icommerce                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │   ACP       │  │    NSR      │  │    Sync     │             │
│  │  Protocol   │  │   Engine    │  │   Layer     │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│  ┌──────┴────────────────┴────────────────┴──────┐             │
│  │              stateset-core                     │             │
│  │     (Domain Models, Business Logic)            │             │
│  └────────────────────┬───────────────────────────┘             │
│                       │                                         │
│  ┌────────────────────┴───────────────────────────┐             │
│  │              stateset-db                        │             │
│  │         (SQLite + cr-sqlite CRDTs)              │             │
│  └─────────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────────┘
                              │
                         store.db
                    (Single file database)
```

### 6.2 Crate Structure

```
stateset-icommerce/
├── Cargo.toml                          # Workspace manifest
├── crates/
│   ├── stateset-core/                  # Domain models, business logic
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── customers/
│   │   │   ├── orders/
│   │   │   ├── products/
│   │   │   ├── inventory/
│   │   │   ├── returns/
│   │   │   ├── payments/
│   │   │   ├── purchase_orders/
│   │   │   ├── invoices/
│   │   │   ├── shipments/
│   │   │   ├── warranties/
│   │   │   ├── work_orders/
│   │   │   └── bom/
│   │   └── Cargo.toml
│   │
│   ├── stateset-db/                    # SQLite persistence layer
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── migrations/
│   │   │   ├── queries/
│   │   │   └── schema.rs
│   │   └── Cargo.toml
│   │
│   ├── stateset-nsr/                   # Neuro-symbolic reasoning
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── policies/
│   │   │   ├── rules/
│   │   │   └── engine.rs
│   │   └── Cargo.toml
│   │
│   ├── stateset-sync/                  # CRDT sync layer
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── crdt.rs
│   │   │   └── protocol.rs
│   │   └── Cargo.toml
│   │
│   ├── stateset-acp/                   # Agentic Commerce Protocol
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── capabilities.rs
│   │   │   ├── schemas.rs
│   │   │   └── tools.rs
│   │   └── Cargo.toml
│   │
│   └── stateset-embedded/              # Unified public interface
│       ├── src/
│       │   ├── lib.rs
│       │   └── commerce.rs
│       └── Cargo.toml
│
├── bindings/
│   ├── node/                           # N-API bindings
│   │   ├── src/
│   │   ├── package.json
│   │   └── index.d.ts
│   │
│   ├── python/                         # PyO3 bindings
│   │   ├── src/
│   │   ├── pyproject.toml
│   │   └── stateset_icommerce/
│   │
│   └── wasm/                           # WebAssembly
│       ├── src/
│       └── package.json
│
└── cli/                                # CLI with Claude integration
    ├── src/
    └── package.json
```

### 6.3 Layer Responsibilities

| Layer | Responsibility | Dependencies |
|-------|---------------|--------------|
| `stateset-embedded` | Public API, orchestration | All crates |
| `stateset-acp` | Protocol definitions, tool schemas | core |
| `stateset-nsr` | Policy engine, business rules | core |
| `stateset-sync` | CRDT sync, replication | db |
| `stateset-core` | Domain models, business logic | None |
| `stateset-db` | SQLite persistence, migrations | core |

---

## 7. Feature Requirements

### 7.1 Commerce Domains

#### 7.1.1 Customers (P0)

**Description:** Customer identity and profile management

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create customer | P0 | Create new customer with email, name, phone |
| Get customer | P0 | Retrieve customer by ID or email |
| Update customer | P0 | Update customer profile fields |
| List customers | P0 | List customers with pagination |
| Delete customer | P1 | Soft delete customer record |
| Search customers | P1 | Full-text search on customer fields |
| Customer tags | P2 | Add/remove tags for segmentation |
| Custom attributes | P2 | Store arbitrary key-value metadata |

**Data model:** See [Section 8.1](#81-customer)

#### 7.1.2 Orders (P0)

**Description:** Order lifecycle management

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create order | P0 | Create order with line items |
| Get order | P0 | Retrieve order with line items |
| List orders | P0 | List orders with filters |
| Update status | P0 | Transition order through states |
| Ship order | P0 | Mark shipped with tracking |
| Cancel order | P0 | Cancel order, handle inventory |
| Add line item | P1 | Add item to existing order |
| Remove line item | P1 | Remove item from order |
| Apply discount | P1 | Apply discount code/amount |
| Split order | P2 | Split order for partial fulfillment |

**State machine:**

```
                    ┌─────────────┐
                    │   pending   │
                    └──────┬──────┘
                           │ confirm()
                    ┌──────▼──────┐
         ┌──────────│  confirmed  │──────────┐
         │          └──────┬──────┘          │
         │ cancel()        │ process()       │ cancel()
         │          ┌──────▼──────┐          │
         │          │ processing  │          │
         │          └──────┬──────┘          │
         │                 │ ship()          │
         │          ┌──────▼──────┐          │
         │          │   shipped   │          │
         │          └──────┬──────┘          │
         │                 │ deliver()       │
         │          ┌──────▼──────┐          │
         │          │  delivered  │          │
         │          └─────────────┘          │
         │                                   │
    ┌────▼────┐                         ┌────▼────┐
    │cancelled│                         │cancelled│
    └─────────┘                         └─────────┘
```

#### 7.1.3 Products (P0)

**Description:** Product catalog with variants

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create product | P0 | Create product with variants |
| Get product | P0 | Retrieve product with variants |
| List products | P0 | List products with pagination |
| Create variant | P0 | Add variant to product |
| Update variant | P0 | Update variant price, SKU |
| Get variant by SKU | P0 | Look up variant by SKU |
| Delete product | P1 | Soft delete product |
| Product categories | P1 | Assign products to categories |
| Product images | P2 | Associate images with products |
| Product bundles | P2 | Create product bundles |

#### 7.1.4 Inventory (P0)

**Description:** Stock management with reservations

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create item | P0 | Create inventory item by SKU |
| Get stock | P0 | Get available/allocated quantities |
| Adjust stock | P0 | Increase/decrease with reason |
| Reserve stock | P0 | Reserve for order (temporary) |
| Confirm reservation | P0 | Convert reservation to allocation |
| Release reservation | P0 | Cancel reservation |
| Set reorder point | P1 | Alert threshold configuration |
| Stock history | P1 | Audit log of adjustments |
| Multi-location | P2 | Stock by warehouse/location |
| Transfer stock | P2 | Move between locations |

**Inventory model:**

```
┌─────────────────────────────────────────────────┐
│                 Inventory Item                  │
├─────────────────────────────────────────────────┤
│  total_on_hand = 100                            │
│  total_allocated = 20                           │
│  total_available = 80   (on_hand - allocated)   │
├─────────────────────────────────────────────────┤
│  Reservations:                                  │
│    - order-123: 10 units (expires: 1hr)         │
│    - order-456: 10 units (expires: 1hr)         │
└─────────────────────────────────────────────────┘
```

#### 7.1.5 Returns (P0)

**Description:** Return request and processing

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create return | P0 | Initiate return for order |
| Get return | P0 | Retrieve return details |
| List returns | P0 | List returns with filters |
| Approve return | P0 | Approve return request |
| Reject return | P0 | Reject with reason |
| Receive return | P1 | Mark items received |
| Process refund | P1 | Trigger refund flow |
| Exchange | P2 | Return + new order |

**Return reasons (enumerated):**

- `defective`
- `wrong_item`
- `not_as_described`
- `no_longer_needed`
- `changed_mind`
- `better_price_found`
- `damaged`
- `other`

#### 7.1.6 Payments (P1)

**Description:** Payment tracking (not processing)

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Record payment | P1 | Record payment against order |
| Get payment | P1 | Retrieve payment details |
| List payments | P1 | List payments for order |
| Record refund | P1 | Record refund transaction |
| Payment methods | P2 | Store payment method references |

**Note:** StateSet does not process payments directly. It records payment events from external processors (Stripe, etc.).

#### 7.1.7 Purchase Orders (P1)

**Description:** B2B procurement

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create PO | P1 | Create purchase order to vendor |
| Get PO | P1 | Retrieve PO details |
| List POs | P1 | List purchase orders |
| Submit PO | P1 | Send to vendor |
| Receive PO | P1 | Mark items received |
| Close PO | P1 | Complete purchase order |

#### 7.1.8 Invoices (P1)

**Description:** Invoice generation and tracking

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create invoice | P1 | Generate invoice for order |
| Get invoice | P1 | Retrieve invoice |
| List invoices | P1 | List invoices with filters |
| Mark paid | P1 | Record payment received |
| Send invoice | P2 | Trigger invoice delivery |

#### 7.1.9 Shipments (P1)

**Description:** Shipment tracking

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create shipment | P1 | Create shipment for order |
| Get shipment | P1 | Retrieve shipment details |
| Add tracking | P1 | Add tracking number |
| Update status | P1 | Update shipment status |
| List shipments | P1 | List shipments |

#### 7.1.10 Warranties (P2)

**Description:** Warranty tracking

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create warranty | P2 | Create warranty for order item |
| Get warranty | P2 | Retrieve warranty details |
| Check coverage | P2 | Verify warranty validity |
| File claim | P2 | Initiate warranty claim |

#### 7.1.11 Work Orders (P2)

**Description:** Manufacturing/service work orders

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create work order | P2 | Create work order |
| Assign resources | P2 | Assign to workers/machines |
| Track progress | P2 | Update completion status |
| Close work order | P2 | Mark complete |

#### 7.1.12 Bill of Materials (P2)

**Description:** BOM management for manufacturing

**Capabilities:**

| Capability | Priority | Description |
|------------|----------|-------------|
| Create BOM | P2 | Define component structure |
| Get BOM | P2 | Retrieve BOM tree |
| Calculate requirements | P2 | Compute component needs |
| Update BOM | P2 | Modify component list |

### 7.2 Additional Modules (Future)

| Module | Priority | Description |
|--------|----------|-------------|
| Subscriptions | P1 | Recurring billing management |
| Pricing | P1 | Price lists, tiered pricing |
| Discounts | P1 | Promotions, coupons |
| Refunds | P1 | Refund processing (separate from returns) |
| Taxes | P1 | Tax calculation interface |
| Accounts | P1 | B2B company/organization |
| Quotes | P2 | Quote-to-order flow |
| Contracts | P2 | B2B agreements |
| Vendors | P2 | Supplier management |
| Locations | P2 | Multi-warehouse |
| Transfers | P2 | Inter-location transfers |
| Fulfillments | P2 | 3PL integration |
| Receiving | P2 | Inbound inventory |
| Policies | P0 | NSR rule definitions |
| Approvals | P1 | Human-in-the-loop workflows |
| Events | P1 | Event log, audit trail |
| Conversations | P2 | Customer interaction history |

---

## 8. Data Models

### 8.1 Customer

```rust
pub struct Customer {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub status: CustomerStatus,
    pub accepts_marketing: bool,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum CustomerStatus {
    Active,
    Inactive,
    Suspended,
}
```

### 8.2 Order

```rust
pub struct Order {
    pub id: Uuid,
    pub order_number: String,
    pub customer_id: Uuid,
    pub status: OrderStatus,
    pub payment_status: PaymentStatus,
    pub fulfillment_status: FulfillmentStatus,
    pub currency: String,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub shipping_total: Decimal,
    pub discount_total: Decimal,
    pub total_amount: Decimal,
    pub notes: Option<String>,
    pub tracking_number: Option<String>,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub metadata: Option<JsonValue>,
}

pub enum OrderStatus {
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
}

pub enum PaymentStatus {
    Pending,
    Authorized,
    Paid,
    PartiallyPaid,
    Refunded,
    Failed,
}

pub enum FulfillmentStatus {
    Unfulfilled,
    PartiallyFulfilled,
    Fulfilled,
    Shipped,
    Delivered,
}
```

### 8.3 Product

```rust
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: ProductStatus,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ProductVariant {
    pub id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub name: Option<String>,
    pub price: Decimal,
    pub compare_at_price: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub barcode: Option<String>,
    pub weight: Option<Decimal>,
    pub metadata: Option<JsonValue>,
}

pub enum ProductStatus {
    Draft,
    Active,
    Archived,
}
```

### 8.4 Inventory

```rust
pub struct InventoryItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub total_on_hand: i32,
    pub total_allocated: i32,
    pub reorder_point: Option<i32>,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct InventoryReservation {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub quantity: i32,
    pub reference_type: String,
    pub reference_id: String,
    pub status: ReservationStatus,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct InventoryAdjustment {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub quantity_change: i32,
    pub reason: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub enum ReservationStatus {
    Pending,
    Confirmed,
    Released,
    Expired,
}
```

### 8.5 Return

```rust
pub struct Return {
    pub id: Uuid,
    pub return_number: String,
    pub order_id: Uuid,
    pub status: ReturnStatus,
    pub reason: ReturnReason,
    pub reason_details: Option<String>,
    pub refund_amount: Option<Decimal>,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ReturnItem {
    pub id: Uuid,
    pub return_id: Uuid,
    pub order_item_id: Uuid,
    pub quantity: i32,
    pub condition: Option<String>,
}

pub enum ReturnStatus {
    Requested,
    Approved,
    Rejected,
    Received,
    Processed,
    Closed,
}

pub enum ReturnReason {
    Defective,
    WrongItem,
    NotAsDescribed,
    NoLongerNeeded,
    ChangedMind,
    BetterPriceFound,
    Damaged,
    Other,
}
```

---

## 9. API Specification

### 9.1 JavaScript/TypeScript API

```typescript
import { Commerce } from '@stateset/icommerce';

// Initialize
const commerce = new Commerce('./store.db');
// Or in-memory: new Commerce(':memory:');

// Customers
interface CustomerInput {
  email: string;
  firstName: string;
  lastName: string;
  phone?: string;
  acceptsMarketing?: boolean;
}

const customer = await commerce.customers.create(input: CustomerInput);
const customer = await commerce.customers.get(id: string);
const customer = await commerce.customers.getByEmail(email: string);
const customers = await commerce.customers.list(options?: ListOptions);
const count = await commerce.customers.count();

// Orders
interface OrderInput {
  customerId: string;
  items: OrderItemInput[];
  currency?: string;
  notes?: string;
}

interface OrderItemInput {
  sku: string;
  name: string;
  quantity: number;
  unitPrice: number;
}

const order = await commerce.orders.create(input: OrderInput);
const order = await commerce.orders.get(id: string);
const orders = await commerce.orders.list(options?: ListOptions);
const order = await commerce.orders.updateStatus(id: string, status: string);
const order = await commerce.orders.ship(id: string, trackingNumber?: string);
const order = await commerce.orders.cancel(id: string);

// Products
interface ProductInput {
  name: string;
  description?: string;
  variants?: ProductVariantInput[];
}

interface ProductVariantInput {
  sku: string;
  name?: string;
  price: number;
  compareAtPrice?: number;
}

const product = await commerce.products.create(input: ProductInput);
const product = await commerce.products.get(id: string);
const variant = await commerce.products.getVariantBySku(sku: string);
const products = await commerce.products.list(options?: ListOptions);

// Inventory
interface InventoryItemInput {
  sku: string;
  name: string;
  description?: string;
  initialQuantity?: number;
  reorderPoint?: number;
}

const item = await commerce.inventory.createItem(input: InventoryItemInput);
const stock = await commerce.inventory.getStock(sku: string);
await commerce.inventory.adjust(sku: string, quantity: number, reason: string);
const reservation = await commerce.inventory.reserve(
  sku: string,
  quantity: number,
  referenceType: string,
  referenceId: string,
  expiresInSeconds?: number
);
await commerce.inventory.confirmReservation(reservationId: string);
await commerce.inventory.releaseReservation(reservationId: string);

// Returns
interface ReturnInput {
  orderId: string;
  reason: ReturnReason;
  reasonDetails?: string;
  items: ReturnItemInput[];
}

interface ReturnItemInput {
  orderItemId: string;
  quantity: number;
}

const ret = await commerce.returns.create(input: ReturnInput);
const ret = await commerce.returns.get(id: string);
const ret = await commerce.returns.approve(id: string);
const ret = await commerce.returns.reject(id: string, reason: string);
const returns = await commerce.returns.list(options?: ListOptions);
```

### 9.2 Python API

```python
from stateset_icommerce import Commerce, CreateOrderItemInput

# Initialize
commerce = Commerce("./store.db")
# Or in-memory: Commerce(":memory:")

# Customers
customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith",
    phone="+1234567890",
    accepts_marketing=True
)
customer = commerce.customers.get(customer_id)
customer = commerce.customers.get_by_email("alice@example.com")
customers = commerce.customers.list()
count = commerce.customers.count()

# Orders
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        CreateOrderItemInput(
            sku="WIDGET-001",
            name="Premium Widget",
            quantity=2,
            unit_price=29.99
        )
    ],
    currency="USD",
    notes="Gift wrap please"
)
order = commerce.orders.get(order_id)
order = commerce.orders.update_status(order_id, "processing")
order = commerce.orders.ship(order_id, tracking_number="1Z123...")
order = commerce.orders.cancel(order_id)

# Products
product = commerce.products.create(
    name="Premium Widget",
    description="High-quality widget",
    variants=[
        CreateProductVariantInput(
            sku="WIDGET-SM",
            price=19.99,
            name="Small"
        )
    ]
)

# Inventory
item = commerce.inventory.create_item(
    sku="WIDGET-001",
    name="Premium Widget",
    initial_quantity=100,
    reorder_point=10
)
stock = commerce.inventory.get_stock("WIDGET-001")
commerce.inventory.adjust("WIDGET-001", -5, "Sold 5 units")
reservation = commerce.inventory.reserve(
    sku="WIDGET-001",
    quantity=2,
    reference_type="order",
    reference_id=order_id,
    expires_in_seconds=3600
)
commerce.inventory.confirm_reservation(reservation.id)

# Returns
ret = commerce.returns.create(
    order_id=order.id,
    reason="defective",
    items=[
        CreateReturnItemInput(
            order_item_id=order.items[0].id,
            quantity=1
        )
    ],
    reason_details="Product arrived damaged"
)
ret = commerce.returns.approve(return_id)
```

### 9.3 Rust API

```rust
use stateset_icommerce::Commerce;

// Initialize
let commerce = Commerce::new("./store.db")?;
// Or in-memory: Commerce::new(":memory:")?;

// Customers
let customer = commerce.customers().create(CreateCustomerInput {
    email: "alice@example.com".to_string(),
    first_name: "Alice".to_string(),
    last_name: "Smith".to_string(),
    phone: Some("+1234567890".to_string()),
    accepts_marketing: true,
})?;

let customer = commerce.customers().get(&customer_id)?;
let customers = commerce.customers().list(ListOptions::default())?;

// Orders
let order = commerce.orders().create(CreateOrderInput {
    customer_id: customer.id,
    items: vec![
        CreateOrderItemInput {
            sku: "WIDGET-001".to_string(),
            name: "Premium Widget".to_string(),
            quantity: 2,
            unit_price: Decimal::new(2999, 2),
        }
    ],
    currency: Some("USD".to_string()),
    notes: None,
})?;

commerce.orders().ship(&order.id, Some("1Z123..."))?;
```

---

## 10. Agentic Commerce Protocol (ACP)

### 10.1 Overview

The Agentic Commerce Protocol (ACP) is an open standard for AI agents performing commerce operations. It defines:

- **Capabilities:** Standardized operations agents can perform
- **Schemas:** Common data models for commerce entities
- **Policies:** Declarative business rules constraining agent behavior
- **Trust boundaries:** What agents can do autonomously vs. requiring approval

### 10.2 Protocol Structure

```
ACP/
├── capabilities/
│   ├── commerce.customers.*
│   ├── commerce.orders.*
│   ├── commerce.products.*
│   ├── commerce.inventory.*
│   ├── commerce.returns.*
│   └── commerce.payments.*
├── schemas/
│   ├── Customer
│   ├── Order
│   ├── Product
│   ├── Inventory
│   └── Return
└── policies/
    ├── return_policy
    ├── refund_policy
    ├── pricing_rules
    └── agent_constraints
```

### 10.3 Capability Definitions

```typescript
// ACP Capability Definition
interface ACPCapability {
  name: string;                    // e.g., "commerce.orders.create"
  description: string;
  parameters: JSONSchema;
  returns: JSONSchema;
  requires_approval?: boolean;
  policy_checks?: string[];        // Policies to evaluate before execution
}

// Example: commerce.orders.create
{
  name: "commerce.orders.create",
  description: "Create a new order for a customer",
  parameters: {
    type: "object",
    properties: {
      customer_id: { type: "string", format: "uuid" },
      items: {
        type: "array",
        items: {
          type: "object",
          properties: {
            sku: { type: "string" },
            quantity: { type: "integer", minimum: 1 },
            unit_price: { type: "number", minimum: 0 }
          }
        }
      }
    },
    required: ["customer_id", "items"]
  },
  returns: { $ref: "#/schemas/Order" },
  policy_checks: ["inventory_available", "customer_credit_check"]
}
```

### 10.4 MCP Integration

ACP is designed as a domain-specific layer atop Anthropic's Model Context Protocol (MCP):

```typescript
// MCP Tool Definition for ACP
{
  name: "acp_execute",
  description: "Execute an ACP commerce operation",
  input_schema: {
    type: "object",
    properties: {
      capability: {
        type: "string",
        description: "ACP capability name (e.g., commerce.orders.create)"
      },
      parameters: {
        type: "object",
        description: "Capability-specific parameters"
      }
    },
    required: ["capability", "parameters"]
  }
}
```

### 10.5 Tool Packages

| Package | Description | Framework |
|---------|-------------|-----------|
| `@stateset/acp-mcp` | MCP server for Claude | Anthropic MCP |
| `@stateset/acp-openai` | Function definitions | OpenAI Functions |
| `@stateset/acp-langchain` | Toolkit integration | LangChain |
| `@stateset/acp-llamaindex` | Tool integration | LlamaIndex |

---

## 11. Neuro-Symbolic Reasoning Engine

### 11.1 Overview

The NSR (Neuro-Symbolic Reasoning) Engine provides the trust boundary between agent intent and commerce execution. It combines:

- **Neural:** LLM flexibility for understanding intent
- **Symbolic:** Deterministic rules for policy enforcement

### 11.2 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Agent Request                            │
│              "Give this customer a full refund"                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      NSR Policy Engine                          │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  1. Parse intent → refund_request(order_id, amount: full)   ││
│  │  2. Load applicable policies                                ││
│  │  3. Evaluate constraints                                    ││
│  │  4. Return decision + alternatives                          ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  Policies:                                                      │
│    - return_window: 30 days                                     │
│    - refund_max: order_total                                    │
│    - requires_approval_above: $500                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Decision Output                           │
│  {                                                              │
│    "allowed": false,                                            │
│    "reason": "order_age_45_days_exceeds_30_day_window",         │
│    "alternatives": [                                            │
│      { "action": "store_credit", "allowed": true },             │
│      { "action": "exchange", "allowed": true }                  │
│    ]                                                            │
│  }                                                              │
└─────────────────────────────────────────────────────────────────┘
```

### 11.3 Policy Definition

```rust
// Policy definition structure
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rules: Vec<Rule>,
    pub priority: i32,
    pub enabled: bool,
}

pub struct Rule {
    pub condition: Condition,
    pub action: Action,
}

pub enum Condition {
    // Comparison conditions
    Equals { field: String, value: Value },
    GreaterThan { field: String, value: Value },
    LessThan { field: String, value: Value },
    InRange { field: String, min: Value, max: Value },
    
    // Temporal conditions
    WithinDays { field: String, days: i32 },
    BeforeDate { field: String, date: DateTime<Utc> },
    
    // Logical operators
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

pub enum Action {
    Allow,
    Deny { reason: String },
    RequireApproval { approver: String },
    Suggest { alternatives: Vec<String> },
}
```

### 11.4 Example Policies

```yaml
# return_policy.yaml
id: return_policy_standard
name: Standard Return Policy
description: Default return policy for all orders
priority: 100
enabled: true
rules:
  - condition:
      type: within_days
      field: order.created_at
      days: 30
    action:
      type: allow
  
  - condition:
      type: and
      conditions:
        - type: greater_than
          field: order.days_since_created
          value: 30
        - type: less_than
          field: order.days_since_created
          value: 90
    action:
      type: suggest
      alternatives:
        - store_credit
        - exchange
  
  - condition:
      type: greater_than
      field: order.days_since_created
      value: 90
    action:
      type: deny
      reason: "Order exceeds 90-day return window"

# approval_thresholds.yaml
id: refund_approval_thresholds
name: Refund Approval Thresholds
rules:
  - condition:
      type: greater_than
      field: refund.amount
      value: 500
    action:
      type: require_approval
      approver: manager
  
  - condition:
      type: greater_than
      field: refund.amount
      value: 2000
    action:
      type: require_approval
      approver: director
```

### 11.5 NSR API

```typescript
// Evaluate a policy
const result = await commerce.policies.evaluate({
  policy: 'return_policy',
  context: {
    order_id: 'order-123',
    requested_action: 'full_refund',
    customer_id: 'customer-456'
  }
});

// Result structure
interface PolicyResult {
  allowed: boolean;
  reason?: string;
  requires_approval?: {
    approver: string;
    threshold: string;
  };
  alternatives?: Array<{
    action: string;
    allowed: boolean;
    conditions?: string[];
  }>;
  policy_trace: Array<{
    policy_id: string;
    rule_index: number;
    matched: boolean;
    action_taken: string;
  }>;
}
```

---

## 12. Sync & Replication

### 12.1 Overview

StateSet uses cr-sqlite (Conflict-free Replicated SQLite) for synchronization. This enables:

- Offline-first operation
- Multi-device sync
- Eventual consistency without conflicts

### 12.2 Sync Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Device A      │     │  Sync Server    │     │   Device B      │
│  ┌───────────┐  │     │  ┌───────────┐  │     │  ┌───────────┐  │
│  │ store.db  │◄─┼─────┼─►│  Hub DB   │◄─┼─────┼─►│ store.db  │  │
│  │ (cr-sql)  │  │     │  │           │  │     │  │ (cr-sql)  │  │
│  └───────────┘  │     │  └───────────┘  │     │  └───────────┘  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                                               │
        │              CRDT Merge                       │
        │         (automatic conflict                   │
        │           resolution)                         │
        └───────────────────────────────────────────────┘
```

### 12.3 Sync API

```typescript
// Enable sync
await commerce.sync.connect('wss://sync.stateset.io/tenant-123', {
  auth_token: 'sk_...',
  auto_sync: true,
  sync_interval_ms: 5000
});

// Manual sync
await commerce.sync.push();
await commerce.sync.pull();

// Sync status
const status = commerce.sync.status();
// { connected: true, last_sync: '2024-12-17T...', pending_changes: 5 }

// Disconnect
await commerce.sync.disconnect();
```

### 12.4 Conflict Resolution

cr-sqlite uses CRDTs (Conflict-free Replicated Data Types) for automatic merge:

| Data Type | Merge Strategy |
|-----------|----------------|
| Scalar fields | Last-writer-wins (LWW) |
| Counters (inventory) | Counter CRDT (add/subtract merge) |
| Sets (tags) | Add-wins set |
| Lists (line items) | Sequence CRDT |

### 12.5 Selective Sync

```typescript
// Sync configuration
await commerce.sync.configure({
  // Sync specific tables
  tables: ['orders', 'customers', 'inventory'],
  
  // Filter by date range
  since: new Date('2024-01-01'),
  
  // Exclude data
  exclude: {
    orders: { status: 'cancelled' }
  }
});
```

---

## 13. Security Requirements

### 13.1 Data Security

| Requirement | Implementation |
|-------------|----------------|
| Encryption at rest | SQLite encryption extension (SQLCipher) |
| Encryption in transit | TLS 1.3 for sync connections |
| Key management | Customer-managed keys for enterprise |
| Data isolation | Separate database files per tenant |

### 13.2 Access Control

```typescript
// Role-based access control
const commerce = new Commerce('./store.db', {
  access_control: {
    roles: {
      agent: {
        customers: ['read'],
        orders: ['read', 'update_status'],
        returns: ['create', 'read'],
        inventory: ['read']
      },
      admin: {
        customers: ['*'],
        orders: ['*'],
        returns: ['*'],
        inventory: ['*']
      }
    },
    current_role: 'agent'
  }
});
```

### 13.3 Audit Logging

```typescript
// All operations logged
commerce.events.on('operation', (event) => {
  // {
  //   timestamp: '2024-12-17T...',
  //   operation: 'orders.create',
  //   actor: 'agent-123',
  //   resource_id: 'order-456',
  //   changes: { ... },
  //   policy_evaluation: { ... }
  // }
});

// Query audit log
const logs = await commerce.audit.query({
  resource_type: 'orders',
  since: new Date('2024-12-01'),
  actor: 'agent-123'
});
```

### 13.4 Compliance

| Standard | Status | Notes |
|----------|--------|-------|
| SOC 2 Type II | Planned (Q3 2025) | For StateSet Cloud |
| GDPR | Supported | Data export, deletion APIs |
| PCI DSS | N/A | No payment processing |
| HIPAA | Planned (Q4 2025) | For healthcare use cases |

---

## 14. Performance Requirements

### 14.1 Latency Targets

| Operation | Target (p50) | Target (p99) |
|-----------|--------------|--------------|
| Create order | < 5ms | < 20ms |
| Get order | < 1ms | < 5ms |
| List orders (100) | < 10ms | < 50ms |
| Inventory adjustment | < 2ms | < 10ms |
| Policy evaluation | < 5ms | < 20ms |
| Sync (100 records) | < 100ms | < 500ms |

### 14.2 Throughput Targets

| Metric | Target |
|--------|--------|
| Operations per second (single node) | 10,000+ |
| Concurrent connections | 1,000+ |
| Database size supported | 100GB+ |
| Records per table | 10M+ |

### 14.3 Resource Requirements

| Runtime | Memory | CPU | Disk |
|---------|--------|-----|------|
| Node.js | 50MB base | Minimal | SQLite file |
| Python | 50MB base | Minimal | SQLite file |
| WASM | 10MB | Minimal | In-memory |

---

## 15. Platform Support

### 15.1 Runtime Support Matrix

| Platform | Package | Status |
|----------|---------|--------|
| Node.js 18+ | `@stateset/icommerce` | P0 |
| Node.js 20+ | `@stateset/icommerce` | P0 |
| Python 3.9+ | `stateset-icommerce` | P0 |
| Browser (WASM) | `@stateset/icommerce-wasm` | P0 |
| Deno | `@stateset/icommerce` | P1 |
| Bun | `@stateset/icommerce` | P1 |
| Cloudflare Workers | `@stateset/icommerce-wasm` | P1 |
| Rust native | `stateset-icommerce` | P0 |

### 15.2 OS Support

| OS | Architecture | Status |
|----|--------------|--------|
| Linux | x64 | P0 |
| Linux | arm64 | P0 |
| macOS | x64 | P0 |
| macOS | arm64 (M1/M2) | P0 |
| Windows | x64 | P1 |

### 15.3 Edge Deployment

| Platform | Status | Notes |
|----------|--------|-------|
| Cloudflare Workers | P1 | WASM build |
| Vercel Edge | P1 | WASM build |
| Deno Deploy | P1 | WASM build |
| AWS Lambda | P0 | Node.js native |

---

## 16. Roadmap

### 16.1 Phase 1: Foundation (Q1 2025)

**Goal:** Production-ready embedded engine

| Milestone | Target Date | Deliverables |
|-----------|-------------|--------------|
| M1.1 | Jan 2025 | Core commerce modules (P0) |
| M1.2 | Feb 2025 | Node.js + Python bindings |
| M1.3 | Feb 2025 | ACP specification v1.0 |
| M1.4 | Mar 2025 | WASM build |
| M1.5 | Mar 2025 | CLI with Claude integration |

### 16.2 Phase 2: Protocol (Q2 2025)

**Goal:** ACP adoption and sync infrastructure

| Milestone | Target Date | Deliverables |
|-----------|-------------|--------------|
| M2.1 | Apr 2025 | MCP integration package |
| M2.2 | Apr 2025 | OpenAI functions package |
| M2.3 | May 2025 | LangChain toolkit |
| M2.4 | May 2025 | StateSet Cloud (sync) beta |
| M2.5 | Jun 2025 | NSR Engine v1.0 |

### 16.3 Phase 3: Scale (Q3 2025)

**Goal:** Enterprise features and adoption

| Milestone | Target Date | Deliverables |
|-----------|-------------|--------------|
| M3.1 | Jul 2025 | Multi-tenant support |
| M3.2 | Aug 2025 | NSR Studio (policy builder) |
| M3.3 | Aug 2025 | Enterprise SSO |
| M3.4 | Sep 2025 | Audit logging |
| M3.5 | Sep 2025 | SOC 2 certification |

### 16.4 Phase 4: Ecosystem (Q4 2025)

**Goal:** Platform ecosystem

| Milestone | Target Date | Deliverables |
|-----------|-------------|--------------|
| M4.1 | Oct 2025 | Marketplace launch |
| M4.2 | Nov 2025 | Third-party integrations (Stripe, etc.) |
| M4.3 | Dec 2025 | ACP v2.0 specification |

---

## 17. Dependencies

### 17.1 Core Dependencies

| Dependency | Version | Purpose | License |
|------------|---------|---------|---------|
| SQLite | 3.40+ | Storage engine | Public domain |
| cr-sqlite | 0.16+ | CRDT sync | MIT |
| rusqlite | 0.30+ | Rust SQLite bindings | MIT |
| serde | 1.0+ | Serialization | MIT/Apache-2.0 |
| tokio | 1.0+ | Async runtime | MIT |
| uuid | 1.0+ | ID generation | MIT/Apache-2.0 |
| chrono | 0.4+ | Date/time | MIT/Apache-2.0 |
| rust_decimal | 1.0+ | Decimal math | MIT |

### 17.2 Binding Dependencies

| Binding | Dependencies | Purpose |
|---------|--------------|---------|
| Node.js | napi-rs | N-API bindings |
| Python | PyO3, maturin | Python bindings |
| WASM | wasm-bindgen, wasm-pack | WebAssembly |

### 17.3 External Services (Optional)

| Service | Purpose | Required |
|---------|---------|----------|
| StateSet Cloud | Sync, backup | Optional |
| Anthropic API | Claude CLI | Optional |
| Stripe | Payment webhooks | Optional |

---

## 18. Open Questions & Risks

### 18.1 Open Questions

| # | Question | Owner | Target Date |
|---|----------|-------|-------------|
| Q1 | Should subscriptions be in core or a separate module? | Product | Jan 2025 |
| Q2 | What's the right conflict resolution for inventory? | Engineering | Jan 2025 |
| Q3 | How do we handle payment provider webhooks? | Engineering | Feb 2025 |
| Q4 | What's the enterprise licensing model? | Business | Mar 2025 |
| Q5 | How do we version the ACP specification? | Product | Mar 2025 |

### 18.2 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| SQLite performance at scale | Low | High | Benchmark early, document limits |
| CRDT merge edge cases | Medium | Medium | Extensive testing, escape hatches |
| WASM size bloat | Medium | Low | Tree shaking, modular builds |
| Cross-platform compatibility | Low | Medium | CI matrix testing |

### 18.3 Market Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Slow agent adoption | Medium | High | Focus on developer experience |
| Competing protocol emerges | Low | High | Move fast, build community |
| Enterprise sales cycle | High | Medium | Land-and-expand with open source |

---

## 19. Appendix

### 19.1 Glossary

| Term | Definition |
|------|------------|
| ACP | Agentic Commerce Protocol — open standard for agent commerce |
| CRDT | Conflict-free Replicated Data Type |
| iCommerce | Intelligent commerce — commerce for autonomous agents |
| MCP | Model Context Protocol (Anthropic) |
| NSR | Neuro-Symbolic Reasoning engine |
| SKU | Stock Keeping Unit |

### 19.2 References

- [SQLite Documentation](https://sqlite.org/docs.html)
- [cr-sqlite (vlcn.io)](https://vlcn.io)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [Rust SQLite (rusqlite)](https://github.com/rusqlite/rusqlite)

### 19.3 Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | Dec 2025 | Product Team | Initial draft |

---

*Document Status: Draft*  
*Next Review: January 2025*