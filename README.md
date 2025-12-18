# StateSet iCommerce Engine

**The SQLite of Commerce** - An embedded, zero-dependency commerce engine for autonomous AI agents.

AI agents that reason, decide, and execute—replacing tickets, scripts, and manual operations across your entire commerce stack.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

---

## Stats at a Glance

| Metric | Count |
|--------|-------|
| **Lines of Code** | ~40,000 |
| **Domain Models** | 175 |
| **Database Tables** | 38 |
| **API Methods** | 311 |
| **MCP Tools** | 56 |
| **Language Bindings** | 3 (Rust, Node.js, Python) |

---

## The Shift: From eCommerce to iCommerce

Commerce is undergoing a fundamental shift. Where eCommerce was built for humans clicking buttons in dashboards, **iCommerce** (Intelligent Commerce) is built for AI agents making decisions.

| The Old Way | The New Way |
|-------------|-------------|
| Tickets & manual operations | Autonomous agents |
| Brittle automation scripts | Deterministic execution |
| Scaling headcount for exceptions | AI reasoning against constraints |

StateSet enables this shift by providing a portable, embeddable commerce engine that agents can carry with them.

---

## Architecture

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/       # Pure domain models & business logic (no I/O)
│   ├── stateset-db/         # SQLite + PostgreSQL implementations
│   └── stateset-embedded/   # Unified high-level API
├── bindings/
│   ├── node/                # JavaScript/TypeScript (NAPI)
│   ├── python/              # Python (PyO3)
│   └── wasm/                # WebAssembly (browser + Node)
└── cli/
    ├── bin/                 # 8 CLI programs
    ├── src/                 # MCP server (56 tools)
    └── .claude/             # 6 AI agents, 5 skills
```

```
┌─────────────────────────────────────────────────────────────────┐
│                       AI Agent Layer                            │
│              (Claude, ChatGPT, Gemini, Custom)                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                   Agentic Commerce Protocol (ACP)
                              │
┌─────────────────────────────────────────────────────────────────┐
│                     StateSet iCommerce                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │   Models    │  │   Storage   │  │  Execution  │             │
│  │ 175 types   │  │SQLite/Postgres│ │Deterministic│             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  56 MCP     │  │  6 Agents   │  │  5 Skills   │             │
│  │   Tools     │  │ Specialized │  │  Knowledge  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Rust

```rust
use stateset_embedded::Commerce;
use rust_decimal_macros::dec;

// Initialize with a local database file
let commerce = Commerce::new("./store.db")?;

// Create a customer
let customer = commerce.customers().create(CreateCustomer {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    ..Default::default()
})?;

// Create inventory
commerce.inventory().create_item(CreateInventoryItem {
    sku: "SKU-001".into(),
    name: "Widget".into(),
    initial_quantity: Some(dec!(100)),
    ..Default::default()
})?;

// Create an order
let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id,
    items: vec![CreateOrderItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        quantity: 2,
        unit_price: dec!(29.99),
        ..Default::default()
    }],
    ..Default::default()
})?;

// Ship the order
commerce.orders().ship(order.id, Some("FEDEX123456".into()))?;
```

### Node.js

```javascript
const { Commerce } = require('@stateset/embedded');

const commerce = new Commerce('./store.db');

// Create a cart and checkout
const cart = await commerce.carts.create({
  customerEmail: 'alice@example.com',
  customerName: 'Alice Smith'
});

await commerce.carts.addItem(cart.id, {
  sku: 'SKU-001',
  name: 'Widget',
  quantity: 2,
  unitPrice: 29.99
});

const result = await commerce.carts.complete(cart.id);
console.log(`Order created: ${result.orderNumber}`);

// Convert currency
const conversion = await commerce.currency.convert({
  from: 'USD',
  to: 'EUR',
  amount: 100
});
console.log(`$100 USD = €${conversion.convertedAmount} EUR`);
```

### Python

```python
from stateset import Commerce

commerce = Commerce('./store.db')

# Get analytics
summary = commerce.analytics.sales_summary(period='last30days')
print(f"Revenue: ${summary.total_revenue}")
print(f"Orders: {summary.order_count}")

# Forecast demand
forecasts = commerce.analytics.demand_forecast(days_ahead=30)
for f in forecasts:
    if f.days_until_stockout and f.days_until_stockout < 14:
        print(f"WARNING: {f.sku} will stock out in {f.days_until_stockout} days")
```

### CLI (AI-Powered)

```bash
# Natural language interface
stateset "show me pending orders"
stateset "what's my revenue this month?"
stateset "convert $100 USD to EUR"

# Execute operations (requires --apply)
stateset --apply "create a cart for alice@example.com"
stateset --apply "ship order #12345 with tracking FEDEX123"
stateset --apply "approve return RET-001"
```

---

## Domain Models

| Domain | Models | Description |
|--------|--------|-------------|
| **Orders** | Order, OrderItem, OrderStatus | Full order lifecycle management |
| **Customers** | Customer, CustomerAddress | Profiles, preferences, history |
| **Products** | Product, ProductVariant | Catalog with variants and attributes |
| **Inventory** | InventoryItem, Balance, Reservation | Multi-location stock tracking |
| **Carts** | Cart, CartItem, CheckoutResult | Shopping cart with ACP checkout |
| **Payments** | Payment, Refund, PaymentMethod | Multi-method payment processing |
| **Returns** | Return, ReturnItem | RMA processing and refunds |
| **Shipments** | Shipment, ShipmentEvent | Fulfillment and tracking |
| **Manufacturing** | BOM, WorkOrder, Task | Bill of materials and production |
| **Purchase Orders** | PurchaseOrder, Supplier | Procurement management |
| **Warranties** | Warranty, WarrantyClaim | Coverage and claims |
| **Invoices** | Invoice, InvoiceItem | Billing and accounts receivable |
| **Currency** | ExchangeRate, Money | Multi-currency (35+ currencies) |
| **Analytics** | SalesSummary, DemandForecast | Business intelligence |

---

## Database Schema (38 Tables)

**Core:** customers, customer_addresses, products, product_variants, orders, order_items

**Inventory:** inventory_items, inventory_locations, inventory_balances, inventory_transactions, inventory_reservations

**Financial:** payments, refunds, payment_methods, invoices, invoice_items, purchase_orders, purchase_order_items

**Fulfillment:** shipments, shipment_items, shipment_events, returns, return_items

**Manufacturing:** manufacturing_boms, manufacturing_bom_components, manufacturing_work_orders, manufacturing_work_order_tasks, manufacturing_work_order_materials

**Other:** warranties, warranty_claims, carts, cart_items, exchange_rates, store_currency_settings, product_currency_prices, exchange_rate_history, events

---

## MCP Tools (56 Total)

The MCP server exposes 56 tools for AI agent integration:

| Domain | Tools |
|--------|-------|
| **Customers** | list_customers, get_customer, create_customer |
| **Orders** | list_orders, get_order, create_order, update_order_status, ship_order, cancel_order |
| **Products** | list_products, get_product, get_product_variant, create_product |
| **Inventory** | get_stock, create_inventory_item, adjust_inventory, reserve_inventory, confirm_reservation, release_reservation |
| **Returns** | list_returns, get_return, create_return, approve_return, reject_return |
| **Carts** | list_carts, get_cart, create_cart, add_cart_item, update_cart_item, remove_cart_item, set_cart_shipping_address, set_cart_payment, apply_cart_discount, get_shipping_rates, complete_checkout, cancel_cart, abandon_cart, get_abandoned_carts |
| **Analytics** | get_sales_summary, get_top_products, get_customer_metrics, get_top_customers, get_inventory_health, get_low_stock_items, get_demand_forecast, get_revenue_forecast, get_order_status_breakdown, get_return_metrics |
| **Currency** | get_exchange_rate, list_exchange_rates, convert_currency, set_exchange_rate, get_currency_settings, set_base_currency, enable_currencies, format_currency |

---

## AI Agents

Six specialized agents for different commerce domains:

| Agent | Description | Use Case |
|-------|-------------|----------|
| **checkout** | Shopping cart & ACP specialist | Cart creation, checkout flow |
| **orders** | Order lifecycle management | Fulfillment, status updates |
| **inventory** | Stock management specialist | Reservations, adjustments |
| **returns** | RMA processing specialist | Approvals, refunds |
| **analytics** | Business intelligence | Metrics, forecasting |
| **customer-service** | Full-service agent | All domains |

---

## Key Features

### Commerce Operations
- Full order lifecycle (create → confirm → ship → deliver)
- Multi-location inventory with reservations
- Customer profiles with addresses
- Product catalog with variants and attributes

### Financial Operations
- Multi-currency support (35+ currencies including BTC, ETH, USDC)
- Payment processing with multiple methods
- Refund management
- Invoice generation
- Purchase orders with suppliers

### Supply Chain
- Manufacturing BOMs (Bill of Materials)
- Work orders with task tracking
- Shipment tracking with carrier integration
- Returns/RMA processing

### Analytics & Forecasting
- Sales summaries and revenue trends
- Demand forecasting per SKU
- Revenue projections with confidence intervals
- Inventory health monitoring
- Customer metrics and LTV

### AI-Ready Architecture
- Deterministic operations for agent reliability
- MCP protocol integration
- Safety architecture (--apply flag for writes)
- Event-driven for full auditability
- Portable state in single database file

---

## Installation

### Rust

```toml
[dependencies]
stateset-embedded = "0.1"
rust_decimal = "1.36"
rust_decimal_macros = "1.36"
```

### Node.js

```bash
npm install @stateset/embedded
```

### Python

```bash
pip install stateset-embedded
```

### CLI

```bash
cd cli
npm install
npm link

# Verify installation
stateset --help
```

---

## Configuration

### Database Backends

**SQLite (Default):**
```rust
let commerce = Commerce::new("./store.db")?;
// Or in-memory for testing
let commerce = Commerce::new(":memory:")?;
```

**PostgreSQL:**
```rust
let commerce = Commerce::with_postgres("postgres://user:pass@localhost/db")?;
// Or with options
let commerce = Commerce::builder()
    .postgres("postgres://localhost/stateset")
    .max_connections(20)
    .build()?;
```

### CLI Environment

```bash
export ANTHROPIC_API_KEY=sk-ant-...  # Required for AI mode
stateset --db ./store.db "list customers"
```

---

## Examples

```bash
# Run Rust examples
cargo run --example basic_usage
cargo run --example manufacturing

# CLI examples
stateset "how many orders do we have?"
stateset "what's the exchange rate from USD to EUR?"
stateset --apply "add 50 units to SKU-001"
```

---

## Project Structure

```
stateset-icommerce/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── stateset-core/         # Domain models (5,685 LOC)
│   │   └── src/models/        # 14 domain modules
│   ├── stateset-db/           # Database layer (13,724 LOC)
│   │   ├── src/sqlite/        # 15 SQLite modules
│   │   ├── src/postgres/      # 8 PostgreSQL modules
│   │   └── migrations/        # 8 SQL migrations
│   └── stateset-embedded/     # High-level API (4,641 LOC)
│       └── src/               # 17 API modules
├── bindings/
│   ├── node/                  # NAPI bindings
│   ├── python/                # PyO3 bindings
│   └── wasm/                  # WebAssembly bindings
├── cli/
│   ├── bin/                   # 8 CLI programs
│   ├── src/mcp-server.js      # 56 MCP tools
│   └── .claude/               # Agents & skills
└── examples/
```

---

## Core Concepts

### Agentic Commerce Protocol (ACP)

StateSet implements the Agentic Commerce Protocol, an open standard defining how AI agents perform commerce operations:

```javascript
// ACP operations are deterministic and auditable
commerce.orders.create(...)     // Create order
commerce.inventory.reserve(...) // Reserve stock
commerce.returns.approve(...)   // Approve return
commerce.currency.convert(...)  // Convert currency
```

### Safety Architecture

All write operations require explicit opt-in:

```bash
# Preview only (safe)
stateset "create a cart for alice@example.com"

# Actually execute (requires --apply)
stateset --apply "create a cart for alice@example.com"
```

### Event-Driven Architecture

All operations emit events for auditability:

```rust
pub enum CommerceEvent {
    OrderCreated(Order),
    OrderStatusChanged { id, from, to },
    InventoryAdjusted { sku, delta, reason },
    PaymentProcessed(Payment),
    // ...
}
```

---

## License

MIT OR Apache-2.0

---

## Contributing

Contributions welcome! Please read the contributing guidelines before submitting PRs.

---

Built with Rust for reliability, designed for AI agents.
