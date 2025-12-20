# StateSet iCommerce Engine

**The SQLite of Commerce** - An embedded, zero-dependency commerce engine for autonomous AI agents.

AI agents that reason, decide, and execute—replacing tickets, scripts, and manual operations across your entire commerce stack.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

---

## Stats at a Glance

| Metric | Count |
|--------|-------|
| **Lines of Code** | ~150,000 |
| **Domain Models** | 254 |
| **Database Tables** | 53 |
| **API Methods** | 670+ |
| **MCP Tools** | 90 |
| **AI Agents** | 8 |
| **Language Bindings** | 7 (Rust, Node.js, Python, Ruby, PHP, Java, WASM) |

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
│   ├── ruby/                # Ruby (Magnus)
│   ├── php/                 # PHP (ext-php-rs)
│   ├── java/                # Java (JNI)
│   └── wasm/                # WebAssembly (browser + Node)
└── cli/
    ├── bin/                 # 14 CLI programs
    ├── src/                 # MCP server (90 tools)
    └── .claude/             # 8 AI agents, 7 skills
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
│  │ 254 types   │  │SQLite/Postgres│ │Deterministic│             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  90 MCP     │  │  8 Agents   │  │  7 Skills   │             │
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

### Ruby

```ruby
require 'stateset_embedded'

commerce = StateSet::Commerce.new('./store.db')

# Create a customer
customer = commerce.customers.create(
  email: 'alice@example.com',
  first_name: 'Alice',
  last_name: 'Smith'
)

# Create an order
order = commerce.orders.create(
  customer_id: customer.id,
  items: [
    { sku: 'SKU-001', name: 'Widget', quantity: 2, unit_price: 29.99 }
  ]
)

# Manage subscriptions
plan = commerce.subscriptions.create_plan(
  name: 'Pro Monthly',
  price: 29.99,
  interval: 'month'
)

subscription = commerce.subscriptions.subscribe(
  plan_id: plan.id,
  customer_id: customer.id
)
```

### PHP

```php
<?php
use StateSet\Commerce;

$commerce = new Commerce('./store.db');

// Create a customer
$customer = $commerce->customers()->create(
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith'
);

// Create an order
$order = $commerce->orders()->create(
    customerId: $customer->getId(),
    items: [
        ['sku' => 'SKU-001', 'name' => 'Widget', 'quantity' => 2, 'unit_price' => 29.99]
    ]
);

// Calculate tax
$tax = $commerce->tax()->calculate(
    amount: 100.00,
    jurisdictionId: $jurisdiction->getId()
);

// Apply promotions
$promo = $commerce->promotions()->create(
    code: 'SUMMER20',
    name: 'Summer Sale',
    discountType: 'percentage',
    discountValue: 20.0
);
```

### Java

```java
import com.stateset.embedded.*;

try (Commerce commerce = new Commerce("./store.db")) {
    // Create a customer
    Customer customer = commerce.customers().create(
        "alice@example.com",
        "Alice",
        "Smith"
    );

    // Create an order
    Order order = commerce.orders().create(
        customer.getId(),
        "USD"
    );

    // Process payments
    commerce.payments().recordPayment(
        order.getId(),
        order.getTotalAmount(),
        "card",
        "txn_123456"
    );

    // Get analytics
    SalesSummary summary = commerce.analytics().salesSummary(30);
    System.out.println("Revenue: $" + summary.getTotalRevenue());
}
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

# Tax calculation
stateset "calculate tax for an order shipping to California"
stateset "what's the VAT rate for Germany?"

# Promotions
stateset --apply "create a 20% off promotion called Summer Sale"
stateset "is coupon SAVE20 valid?"
stateset --apply "apply promotions to cart CART-123"

# Subscriptions
stateset "show me all subscription plans"
stateset --apply "create a monthly plan called Pro at $29.99"
stateset --apply "subscribe customer alice@example.com to the Pro plan"
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
| **Tax** | TaxJurisdiction, TaxRate, TaxExemption | US/EU/CA tax calculation |
| **Promotions** | Promotion, Coupon, DiscountRule | Campaigns, coupons, discounts |
| **Subscriptions** | SubscriptionPlan, Subscription, BillingCycle | Recurring billing management |

---

## Database Schema (53 Tables)

**Core:** customers, customer_addresses, products, product_variants, orders, order_items

**Inventory:** inventory_items, inventory_locations, inventory_balances, inventory_transactions, inventory_reservations

**Financial:** payments, refunds, payment_methods, invoices, invoice_items, purchase_orders, purchase_order_items

**Fulfillment:** shipments, shipment_items, shipment_events, returns, return_items

**Manufacturing:** manufacturing_boms, manufacturing_bom_components, manufacturing_work_orders, manufacturing_work_order_tasks, manufacturing_work_order_materials

**Tax:** tax_jurisdictions, tax_rates, tax_exemptions, tax_settings

**Promotions:** promotions, promotion_rules, promotion_actions, coupons, coupon_usages, promotion_applications

**Subscriptions:** subscription_plans, subscriptions, billing_cycles, subscription_events

**Other:** warranties, warranty_claims, carts, cart_items, exchange_rates, store_currency_settings, product_currency_prices, exchange_rate_history, events

---

## MCP Tools (90 Total)

The MCP server exposes 90 tools for AI agent integration:

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
| **Tax** | calculate_tax, calculate_cart_tax, get_tax_rate, list_tax_jurisdictions, list_tax_rates, get_tax_settings, get_us_state_tax_info, get_customer_tax_exemptions, create_tax_exemption |
| **Promotions** | list_promotions, get_promotion, create_promotion, activate_promotion, deactivate_promotion, create_coupon, validate_coupon, list_coupons, get_active_promotions, apply_cart_promotions |
| **Subscriptions** | list_subscription_plans, get_subscription_plan, create_subscription_plan, activate_subscription_plan, archive_subscription_plan, list_subscriptions, get_subscription, create_subscription, pause_subscription, resume_subscription, cancel_subscription, skip_billing_cycle, list_billing_cycles, get_billing_cycle, get_subscription_events |

---

## AI Agents

Eight specialized agents for different commerce domains:

| Agent | Description | Use Case |
|-------|-------------|----------|
| **checkout** | Shopping cart & ACP specialist | Cart creation, checkout flow |
| **orders** | Order lifecycle management | Fulfillment, status updates |
| **inventory** | Stock management specialist | Reservations, adjustments |
| **returns** | RMA processing specialist | Approvals, refunds |
| **analytics** | Business intelligence | Metrics, forecasting |
| **promotions** | Promotions & discounts specialist | Campaigns, coupons, discount rules |
| **subscriptions** | Subscription management specialist | Plans, billing cycles, recurring payments |
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

### Tax Management
- Multi-jurisdiction tax calculation (US, EU, Canada)
- State/province tax rates with automatic lookup
- Tax exemptions and certificates
- VAT support for EU countries
- GST/HST/PST for Canadian provinces

### Promotions & Discounts
- Percentage and fixed-amount discounts
- Coupon codes with usage limits
- Buy-X-Get-Y promotions
- Free shipping promotions
- Time-limited campaigns

### Subscriptions & Recurring Billing
- Flexible subscription plans (daily, weekly, monthly, annual)
- Free trial periods
- Billing cycle management
- Pause/resume/cancel lifecycle
- Skip billing cycles

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
- MCP protocol integration (90 tools)
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

### Ruby

```bash
gem install stateset_embedded
```

Or add to your Gemfile:

```ruby
gem 'stateset_embedded'
```

### PHP

```bash
composer require stateset/embedded

# Install the native extension for best performance
composer install-extension
```

Then add to your `php.ini`:

```ini
extension=stateset_embedded
```

### Java (Maven)

```xml
<dependency>
    <groupId>com.stateset</groupId>
    <artifactId>embedded</artifactId>
    <version>0.1.7</version>
</dependency>
```

### Java (Gradle)

```groovy
implementation 'com.stateset:embedded:0.1.7'
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

## Language Bindings

StateSet provides native bindings for 7 languages, all built from the same Rust core:

| Language | Package | Install | Docs |
|----------|---------|---------|------|
| **Rust** | `stateset-embedded` | `cargo add stateset-embedded` | [crates.io](https://crates.io/crates/stateset-embedded) |
| **Node.js** | `@stateset/embedded` | `npm install @stateset/embedded` | [npm](https://www.npmjs.com/package/@stateset/embedded) |
| **Python** | `stateset-embedded` | `pip install stateset-embedded` | [PyPI](https://pypi.org/project/stateset-embedded/) |
| **Ruby** | `stateset_embedded` | `gem install stateset_embedded` | [RubyGems](https://rubygems.org/gems/stateset_embedded) |
| **PHP** | `stateset/embedded` | `composer require stateset/embedded` | [Packagist](https://packagist.org/packages/stateset/embedded) |
| **Java** | `com.stateset:embedded` | Maven/Gradle | [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded) |
| **WASM** | `@stateset/embedded-wasm` | `npm install @stateset/embedded-wasm` | [npm](https://www.npmjs.com/package/@stateset/embedded-wasm) |

### Platform Support

| Platform | Node.js | Python | Ruby | PHP | Java |
|----------|---------|--------|------|-----|------|
| Linux x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Linux arm64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS arm64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Windows x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Browser (WASM) | ✅ | - | - | - | - |

### Framework Integration

**Laravel (PHP)**
```php
// AppServiceProvider.php
$this->app->singleton(Commerce::class, fn() =>
    new Commerce(storage_path('stateset/commerce.db'))
);
```

**Rails (Ruby)**
```ruby
# config/initializers/stateset.rb
Rails.application.config.stateset = StateSet::Commerce.new(
  Rails.root.join('db', 'commerce.db').to_s
)
```

**Spring Boot (Java)**
```java
@Configuration
public class CommerceConfig {
    @Bean(destroyMethod = "close")
    public Commerce commerce() {
        return new Commerce("commerce.db");
    }
}
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
│   ├── stateset-core/         # Domain models (18 domain modules)
│   │   └── src/models/        # 254 types
│   ├── stateset-db/           # Database layer
│   │   ├── src/sqlite/        # 18 SQLite modules
│   │   ├── src/postgres/      # 8 PostgreSQL modules
│   │   └── migrations/        # 11 SQL migrations
│   └── stateset-embedded/     # High-level API
│       └── src/               # 20+ API modules
├── bindings/
│   ├── node/                  # NAPI bindings (@stateset/embedded)
│   ├── python/                # PyO3 bindings (stateset-embedded)
│   ├── ruby/                  # Magnus bindings (stateset_embedded gem)
│   ├── php/                   # ext-php-rs bindings (stateset/embedded)
│   ├── java/                  # JNI bindings (com.stateset:embedded)
│   └── wasm/                  # WebAssembly bindings (@stateset/embedded-wasm)
├── cli/
│   ├── bin/                   # 14 CLI programs
│   ├── src/mcp-server.js      # 90 MCP tools
│   └── .claude/               # 8 agents, 7 skills
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
