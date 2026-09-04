# Domain Model

StateSet iCommerce provides a broad domain surface covering commerce and ERP workflows. The same core domain concepts are packaged across the supported bindings with consistent behavior.

## API Surface

| API | Description | Key Operations |
|-----|-------------|---------------|
| `customers` | Customer management | Create, get, list, update, delete, count |
| `products` | Product catalog | Create, get, list, update, delete, variants |
| `orders` | Order lifecycle | Create, ship, cancel, list by status/customer |
| `inventory` | Stock management | Create, adjust, reserve, release, get levels |
| `carts` | Shopping carts | Create, add/remove items, apply discounts, checkout |
| `payments` | Payment operations | Create, capture, refund, list by order |
| `returns` | Return processing (RMA) | Create, approve, reject, complete, list by status |
| `shipments` | Shipping management | Create, track, update, proof of delivery |
| `subscriptions` | Recurring billing | Create plans, subscribe, pause, resume, cancel |
| `promotions` | Discounts & coupons | Create, activate, create coupons, validate |
| `invoices` | B2B invoicing | Create, send, mark paid, void |
| `tax` | Tax calculations | Get rates, create exemptions, multi-jurisdiction |
| `currency` | Multi-currency | Convert, get rates, 150+ currencies |
| `analytics` | Reporting & forecasts | Sales summary, top products, top customers |
| `suppliers` | Supplier management | Registry, performance tracking, contracts |
| `purchase_orders` | Purchase orders | Create, approve, receive, close |
| `warranties` | Warranty tracking | Create terms, check coverage, process claims |
| `quality` | Quality control | Inspections, non-conformance, corrective actions |
| `lots` | Lot tracking | Create, track, recall |
| `serials` | Serial numbers | Assign, track, verify |
| `warehouse` | Warehouse operations | Locations, zones, bin management |
| `receiving` | Inbound receiving | Create receipts, inspect, put away |
| `fulfillment` | Picking & packing | Create picks, pack, ship |
| `bom` | Bills of Materials | Create, version, explode |
| `work_orders` | Manufacturing | Create, schedule, track, complete |
| `accounts_payable` | A/P management | Bills, aging, payments |
| `accounts_receivable` | A/R management | Invoices, aging, collections |
| `cost_accounting` | Cost tracking | Standard costs, variances, COGS |
| `credit` | Credit management | Limits, checks, holds |
| `backorders` | Backorder tracking | Create, prioritize, fulfill |
| `general_ledger` | GL accounting | Journal entries, trial balance, reports |
| `gift_cards` | Gift card management | Issue, check balance, redeem, expire |
| `store_credits` | Store credit management | Issue, check balance, redeem |
| `loyalty` | Loyalty programs | Award points, check balance, redeem, tiers |
| `reviews` | Product reviews | Create, list, moderate, average rating |
| `wishlists` | Customer wishlists | Create, add items, share |
| `segments` | Customer segmentation | Define criteria, assign, list members |
| `shipping_zones` | Shipping rate zones | Define zones, set rates, carrier mapping |
| `fraud` | Fraud detection | Score transactions, manage rules, chargebacks |
| `custom_objects` | Schema extensions | Define custom fields, set values, validate |

## Entity Identifiers

All entities use strongly-typed UUID identifiers. In Rust, these are newtype wrappers that prevent mixing different entity types at compile time:

```rust
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
pub struct PromotionId(Uuid);
pub struct AgentId(Uuid);
// ... 24 total ID types
```

In other languages, IDs are represented as strings but validated by the engine.

## State Machines

Every domain aggregate has an explicit state machine. Invalid transitions produce typed errors rather than silently corrupting state.

### Order States
```
Pending → Processing → Shipped → Delivered
   └────→ Cancelled
```

### Payment States
```
Pending → Authorized → Captured → Settled
   └────→ Failed        └────→ Refunded (partial or full)
```

### Return States
```
Requested → Approved → Received → Completed
    └─────→ Rejected
```

### Subscription States
```
Active → Paused → Active (resume)
   └──→ Past Due → Active (payment retry)
   └──→ Cancelled (terminal)
```

### Work Order States
```
Draft → Scheduled → In Progress → Completed
                       └────────→ On Hold → In Progress
```

## Value Objects

### Money

All monetary amounts use the `Money` type with decimal arithmetic (no floating point):

```rust
use stateset_primitives::Money;

let price = Money::new(29, 99, CurrencyCode::USD); // $29.99
let total = price * 3; // $89.97 — exact, no floating-point drift
```

### CurrencyCode

Type-safe currency representation with 150+ ISO 4217 codes:

```rust
use stateset_primitives::CurrencyCode;

let usd = CurrencyCode::USD;
let eur = CurrencyCode::EUR;
// CurrencyCode::from_str("INVALID") → Err
```

### SKU

Validated stock-keeping unit identifiers:

```rust
use stateset_primitives::Sku;

let sku = Sku::new("WIDGET-001")?; // Validated format
```

## Error Hierarchy

Domain operations return structured errors that AI agents can reason about:

```rust
pub enum CommerceError {
    NotFound(String),           // Entity does not exist
    InvalidState(String),       // Invalid state transition
    Validation(String),         // Input validation failure
    Conflict(String),           // Optimistic concurrency conflict
    Database(String),           // Storage layer error
    Retryable(String),          // Transient failure, safe to retry
}
```

Each error variant carries enough context for an LLM to understand what went wrong and how to fix it.
