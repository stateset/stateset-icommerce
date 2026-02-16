# StateSet iCommerce Engine

**The SQLite of Commerce** - An embedded, zero-dependency commerce engine for autonomous AI agents.

AI agents that reason, decide, and execute—replacing tickets, scripts, and manual operations across your entire commerce stack.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml/badge.svg)](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/stateset/stateset-icommerce/graph/badge.svg)](https://codecov.io/gh/stateset/stateset-icommerce)

---

**Install:**
```bash
pip install stateset-embedded==0.7.0    # Python
gem install stateset_embedded -v 0.7.0  # Ruby
npm install @stateset/embedded@0.7.0    # Node.js
npm install -g @stateset/cli@0.7.0      # CLI
cargo add stateset-embedded             # Rust
```

**Development toolchain (repo root):**
```bash
nvm use                      # uses .nvmrc / .node-version (20.20.0)
rustup show active-toolchain # uses rust-toolchain.toml (1.90.0)
npm run check                # root quality checks
```

---

## What's New in v0.7.0

**Agent-to-Agent (A2A) Commerce** — AI agents can now autonomously pay, quote, subscribe, split payments, and negotiate with each other.

| Feature | Description |
|---------|-------------|
| **Policy Runtime** | Policy-driven tool guardrails (allow/deny/transform) with optional domain inference and configurable policy store |
| **A2A Payments** | Direct agent-to-agent transfers (USDC, USDT, ssUSD, DAI) across SET Chain, Base, Ethereum, Arbitrum |
| **A2A Quotes** | Request → provide → accept/decline → fulfill negotiation flow |
| **A2A Subscriptions** | Recurring inter-agent billing with trial periods, pause/resume, weekly-to-annual intervals |
| **A2A Split Payments** | Multi-party splits (percentage or fixed) with platform fees and drift prevention |
| **A2A Escrow** | Conditional payments (seller_fulfilled, buyer_confirmed, time_lock, milestone) with dispute resolution |
| **A2A Webhooks** | HMAC-SHA256 signed notifications with SSRF validation and exponential backoff retry |
| **A2A Event Streaming** | Real-time SSE push with wildcard filtering and persistent event log |
| **Agent Discovery** | Find agents by capability, reputation scoring, trust-level gating |
| **232 MCP Tools** | Up from 175 — 53 new A2A tools across 26 modular domain modules |
| **Modular MCP Server** | Rewritten from 9,340 to 470 lines (95% reduction) with `adaptTool()` composition |
| **4,300+ Tests** | 500+ new A2A tests, 60+ test files across the CLI |

---

## Documentation

- mdBook docs live in `docs/` (see `docs/README.md`).
- API reference pointers per binding: `docs/src/api/`.
- End-to-end examples and workflows: `examples/`.
- Release history: `CHANGELOG.md`.
- Security policy: `SECURITY.md`.

---

## Support Matrix

| Tier | What’s Covered | CI Coverage |
|------|----------------|------------|
| Tier 1 | Core Rust crates, CLI, Node, Python, Ruby, PHP | Full test suite + lint/format + security checks |
| Tier 2 | Go, .NET, Java, Kotlin, WASM, Swift (macOS) | Build or smoke tests |
| Tier 3 | Experimental integrations | Manual/roadmap |

Production note: use `config/stateset.production.properties` as the baseline for secure defaults.

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
│   ├── stateset-embedded/   # Unified high-level API
│   └── stateset-observability/ # Metrics + tracing helpers
├── bindings/
│   ├── node/                # JavaScript/TypeScript (NAPI)
│   ├── python/              # Python (PyO3)
│   ├── ruby/                # Ruby (Magnus)
│   ├── php/                 # PHP (ext-php-rs)
│   ├── java/                # Java (JNI)
│   ├── kotlin/              # Kotlin/JVM (JNI)
│   ├── swift/               # Swift/iOS/macOS (C FFI)
│   ├── dotnet/              # C#/.NET (P/Invoke)
│   ├── go/                  # Go (cgo)
│   └── wasm/                # WebAssembly (browser + Node)
└── cli/
    ├── bin/                 # CLI programs (40 entry points)
    ├── src/                 # MCP server (232 tools)
    ├── src/a2a/             # Agent-to-Agent commerce (payments, subscriptions, splits, events)
    ├── src/channels/        # 9-channel messaging gateway
    │   ├── base.js          # Shared sessions, commands, pipeline
    │   ├── webchat.js       # HTTP-based web chat gateway
    │   ├── middleware.js     # Rate limiter, content filter, logger
    │   ├── rich-messages.js  # Platform-agnostic rich messages
    │   ├── session-store.js  # SQLite-backed persistent sessions
    │   ├── notifier.js       # Proactive notification routing
    │   ├── orchestrator.js   # Config-driven multi-gateway launcher
    │   ├── identity.js       # Customer identity resolution
    │   ├── event-bridge.js   # Engine event → channel notifications
    │   ├── metrics.js        # Per-channel conversation metrics
    │   ├── handoff.js        # AI-to-human escalation queue
    │   └── templates.js      # Pre-built rich message templates
    ├── src/providers/       # Multi-provider AI (Claude, OpenAI, Gemini, Ollama)
    ├── src/voice/           # Voice mode (STT + TTS)
    ├── src/memory/          # Persistent conversation memory
    ├── src/browser/         # Chrome DevTools Protocol automation
    ├── src/skills/          # Skills loader, registry, marketplace
    ├── src/imessage/        # iMessage gateway (BlueBubbles)
    ├── src/matrix/          # Matrix protocol gateway
    ├── src/teams/           # Microsoft Teams gateway
    ├── src/sync/            # VES sync engine, keys, groups
    ├── skills/              # 38 commerce domain skills
    ├── deploy/              # Systemd services, Tailscale, SSH tunnels
    └── .claude/             # 8 AI agents
```

```
┌─────────────────────────────────────────────────────────────────┐
│                       AI Agent Layer                            │
│         (Claude, OpenAI, Gemini, Ollama — with fallback)        │
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
│  │  232 MCP    │  │  8 Agents   │  │  38 Skills  │             │
│  │   Tools     │  │ Specialized │  │  Knowledge  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│  ┌─────────────────────────────────────────────────┐            │
│  │         Agent-to-Agent (A2A) Commerce            │            │
│  │  Payments · Quotes · Subscriptions · Splits      │            │
│  │  Webhooks · Event Streaming · Escrow · Trust     │            │
│  └─────────────────────────────────────────────────┘            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  Voice Mode │  │  Memory     │  │  Browser    │             │
│  │  STT + TTS  │  │ Persistent  │  │  CDP Tools  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│  ┌─────────────────────────────────────────────────┐            │
│  │           9-Channel Messaging Gateway            │            │
│  │  WhatsApp · Telegram · Discord · Slack · Signal  │            │
│  │  Google Chat · WebChat · iMessage · Teams        │            │
│  │  Sessions · Middleware · Identity · Handoff       │            │
│  └─────────────────────────────────────────────────┘            │
│  ┌─────────────────────────────────────────────────┐            │
│  │           Heartbeat Monitor & Permissions        │            │
│  │  6 Health Checkers · API Key Auth · Sandboxing   │            │
│  └─────────────────────────────────────────────────┘            │
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

async function main() {
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
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

### Python

```python
from stateset_embedded import Commerce

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

### Kotlin

```kotlin
import com.stateset.embedded.*

val commerce = StateSetCommerce("./store.db")

// Create a customer
val customer = commerce.customers.create(
    email = "alice@example.com",
    firstName = "Alice",
    lastName = "Smith"
)

// Create a product
val product = commerce.products.create(
    name = "Widget",
    sku = "SKU-001",
    price = 29.99,
    description = "A premium widget"
)

// Create an order
val order = commerce.orders.create(
    customerId = customer.id,
    items = listOf(
        OrderItem(productId = product.id, sku = "SKU-001", name = "Widget", quantity = 2, unitPrice = "29.99")
    ),
    currency = "USD"
)

// Get analytics
val summary = commerce.analytics.getSalesSummary(TimePeriod.MONTH)
println("Revenue: $${summary.totalRevenue}")

commerce.close()
```

### Swift

```swift
import StateSet

let commerce = try StateSetCommerce(path: "./store.db")
defer { commerce.close() }

// Create a customer
let customer = try commerce.customers.create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith",
    phone: "+1-555-0123"
)

// Create a product
let product = try commerce.products.create(
    name: "Widget",
    sku: "SKU-001",
    price: 29.99,
    description: "A premium widget"
)

// Create an order
let order = try commerce.orders.create(
    customerId: customer.id,
    items: [
        OrderItem(productId: product.id, sku: "SKU-001", name: "Widget", quantity: 2, unitPrice: "29.99")
    ],
    currency: "USD"
)

// Update order status
try commerce.orders.updateStatus(id: order.id, status: .shipped)
```

### C# / .NET

```csharp
using StateSet;

using var commerce = new StateSetCommerce("./store.db");

// Create a customer
var customer = commerce.Customers.Create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith"
);

// Create a product
var product = commerce.Products.Create(
    name: "Widget",
    sku: "SKU-001",
    price: 29.99m,
    description: "A premium widget"
);

// Create an order
var order = commerce.Orders.Create(
    customerId: customer.Id,
    items: new[] {
        new OrderItem { ProductId = product.Id, Sku = "SKU-001", Name = "Widget", Quantity = 2, UnitPrice = "29.99" }
    },
    currency: "USD"
);

// Get analytics
var summary = commerce.Analytics.GetSalesSummary(TimePeriod.Month);
Console.WriteLine($"Revenue: ${summary.TotalRevenue}");
```

### Go

```go
package main

import (
    "fmt"
    "github.com/stateset/stateset-icommerce/bindings/go/stateset"
)

func main() {
    commerce, _ := stateset.New("./store.db")
    defer commerce.Close()

    // Create a customer
    customer, _ := commerce.Customers().Create(
        "alice@example.com", "Alice", "Smith", "+1-555-0123",
    )

    // Create a product
    product, _ := commerce.Products().Create(
        "Widget", "SKU-001", 29.99, "A premium widget",
    )

    // Create an order
    items := []stateset.OrderItem{
        {ProductID: product.ID, SKU: "SKU-001", Name: "Widget", Quantity: 2, UnitPrice: "29.99"},
    }
    order, _ := commerce.Orders().Create(customer.ID, items, "USD")
    fmt.Printf("Order created: %s\n", order.OrderNumber)

    // Get analytics
    summary, _ := commerce.Analytics().GetSalesSummary(stateset.TimePeriodMonth)
    fmt.Printf("Revenue: $%s\n", summary.TotalRevenue)
}
```

### CLI (AI-Powered)

Tip: `ss` is a shorthand alias for `stateset`.

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

# Payments & Refunds
stateset "list payments for order #12345"
stateset --apply "create payment for order #12345 amount $99.99 via card"
stateset --apply "refund payment PAY-001 amount $25.00"

# Shipments
stateset --apply "create shipment for order #12345 via FedEx tracking FEDEX123"
stateset --apply "mark shipment SHIP-001 as delivered"

# Supply Chain
stateset "list purchase orders"
stateset --apply "create purchase order for supplier SUP-001"
stateset --apply "approve purchase order PO-001"

# Invoices (B2B)
stateset "show overdue invoices"
stateset --apply "create invoice for customer CUST-001"
stateset --apply "record payment on invoice INV-001"

# Warranties
stateset "list warranties for customer CUST-001"
stateset --apply "create warranty for product SKU-001"
stateset --apply "file warranty claim for warranty WRN-001"

# Manufacturing
stateset "list bills of materials"
stateset --apply "create BOM for product WIDGET-ASSEMBLED"
stateset --apply "add component SKU-PART-A quantity 2 to BOM-001"
stateset --apply "create work order from BOM-001 quantity 50"
stateset --apply "complete work order WO-001 with 48 units produced"
```

### Agent-to-Agent (A2A) Commerce

AI agents can pay, quote, subscribe, and negotiate with each other autonomously.

```bash
# Direct agent-to-agent payment
stateset --apply "pay 10 USDC to 0x1234...5678 on Set Chain"

# Request payment from another agent
stateset --apply "request 50 USDC from 0xBuyer for API credits"

# Quotes — request, provide, accept
stateset --apply "request quote from data provider for 100 API credits"
stateset --apply "provide quote of $150 for premium data access"
stateset --apply "accept quote QT-001 and pay"

# Recurring subscriptions between agents
stateset --apply "create monthly subscription for 0xSubscriber at $49.99 with 14-day trial"
stateset "list active a2a subscriptions"
stateset --apply "pause a2a subscription SUB-001"
stateset --apply "cancel a2a subscription SUB-001"

# Split payments (multi-party)
stateset --apply "create split payment of $100: 70% to 0xSeller, 20% to 0xAffiliate, 10% platform fee"
stateset --apply "execute split payment SPLIT-001"

# Escrow with conditions
stateset --apply "create escrow payment of 100 USDC with condition seller_fulfilled"
stateset --apply "fulfill escrow condition on ESCROW-001"
stateset --apply "release escrow ESCROW-001"

# Agent discovery and trust
stateset "discover agents that can process payments on Set Chain"
stateset "find verified sellers with USDC support"
stateset "get reputation for agent 0xSeller"

# Webhook notifications
stateset --apply "configure webhook for agent 0xMyAgent to https://hooks.example.com/a2a"
stateset "list a2a notifications for agent 0xMyAgent"

# Event streaming
stateset --apply "subscribe to a2a_payment.* events"
stateset "get a2a event history for last 24 hours"
```

### Sync CLI (VES v1.0)

Verifiable Event Sync (VES) enables local SQLite databases to synchronize with the `stateset-sequencer` service, providing deterministic event ordering, conflict resolution, and cryptographic audit trails.

```bash
# Initialize sync for a store
stateset-sync init --sequencer-url grpc://sequencer.stateset.network:443 \
  --tenant-id <uuid> --store-id <uuid> --api-key <key>

# Sync operations
stateset-sync push              # Push pending events to sequencer
stateset-sync pull              # Pull remote events locally
stateset-sync status            # Show sync status
stateset-sync verify <event-id> # Verify event inclusion proof
stateset-sync rebase            # Rebase after conflict
stateset-sync conflicts         # List unresolved conflicts
stateset-sync history           # Show sync history

# Key Management (Ed25519/X25519)
stateset-sync keys:generate     # Generate signing and encryption keys
stateset-sync keys:list         # List agent keys
stateset-sync keys:register     # Register signing key with sequencer
stateset-sync keys:rotate --all --register  # Rotate keys and re-register
stateset-sync keys:export       # Export public keys for sharing

# Key Rotation Policies
stateset-sync keys:policy --key-type signing --max-age 720 --grace-period 72
stateset-sync keys:expiry       # Check key expiration warnings
stateset-sync keys:batch-rotate --key-type all --register

# Encryption Groups (multi-agent)
stateset-sync groups:create --name "warehouse-agents"
stateset-sync groups:add-member --group-id <id> --agent-id <id>
stateset-sync groups:remove-member --group-id <id> --agent-id <id>
stateset-sync groups:list       # List all groups
stateset-sync groups:show <id>  # Show group details
stateset-sync groups:my-groups  # List your group memberships
```

### Messaging Channels CLI

Deploy AI commerce agents to 9 messaging platforms. Each channel runs as a thin adapter over the shared base module.

```bash
# Single-channel gateways
stateset-telegram --db ./store.db --agent customer-service
stateset-discord --db ./store.db --agent customer-service
stateset-slack --db ./store.db --agent customer-service
stateset-whatsapp --db ./store.db --agent customer-service
stateset-signal --db ./store.db --agent customer-service
stateset-google-chat --db ./store.db --agent customer-service

# Experimental channels
stateset-webchat --db ./store.db --port 3000
stateset-imessage --db ./store.db --agent customer-service
stateset-teams --db ./store.db --agent customer-service
stateset-matrix --db ./store.db --agent customer-service --homeserver matrix.example.com

# Multi-channel orchestrator (all channels in one process)
stateset-channels --config channels.yaml

# With autonomous engine notifications
stateset-autonomous --db ./store.db --agent customer-service \
  --notify-config notify-config.json
```

**In-chat bot commands** (available on all channels):

```
/help                   — List available commands
/orders [n]             — Show recent orders (default 5)
/order <id>             — Order detail with rich card
/inventory <sku>        — Stock levels with rich card
/cart [id]              — Cart summary
/track <id>             — Shipment tracking
/customers              — Customer count
/analytics              — Sales summary with rich card
/stats                  — Channel metrics (messages, response time, errors)
/whoami                 — Show linked customer identity
/link <email>           — Link chat to a customer by email
/unlink                 — Remove customer link
/escalate [reason]      — Hand off to a human agent
/release                — Return conversation to AI bot
/reset                  — Reset session state
```

**Example `channels.yaml`:**

```yaml
shared:
  db: ./store.db
  agent: customer-service
  model: claude-sonnet-4-20250514
  middleware:
    rateLimiter: { maxPerMinute: 20 }
    contentFilter: { action: warn }
    logger: true

channels:
  telegram:
    token: ${TELEGRAM_BOT_TOKEN}
    allowList: [-1001234567890]
  discord:
    token: ${DISCORD_BOT_TOKEN}
    channelIds: ["1234567890123456"]
  slack:
    token: ${SLACK_BOT_TOKEN}
    appToken: ${SLACK_APP_TOKEN}
    channels: ["C01ABCDEF"]

notifications:
  routes:
    "order.shipped": [{ channel: slack, target: "#orders" }]
    "inventory.low": [{ channel: slack, target: "#ops" }]
    "*": [{ channel: slack, target: "#all-alerts" }]
```

### Skills CLI

Browse, install, and manage 38 commerce domain skills that enhance agent capabilities.

```bash
# List all loaded skills
stateset-skills list

# Search for skills
stateset-skills search "inventory"

# Install from marketplace
stateset-skills install commerce-warehouse

# Uninstall a skill
stateset-skills uninstall commerce-warehouse

# Show skill details
stateset-skills info commerce-fulfillment

# List skill categories
stateset-skills categories

# Browse the marketplace
stateset-skills marketplace

# Health check all skills
stateset-skills doctor
```

**Available skills:** accounts-payable, accounts-receivable, analytics, autonomous-engine, autonomous-runbook, backorders, checkout, cost-accounting, credit, currency, customer-service, customers, embedded-sdk, engine-setup, events, fulfillment, general-ledger, inventory, invoices, lots-and-serials, manufacturing, mcp-tools, orders, payments, products, promotions, quality, receiving, returns, shipments, storefront, subscriptions, suppliers, sync, tax, vector-search, warehouse, warranties.

### Daemon CLI

Manage the StateSet gateway as a background service with systemd integration.

```bash
# Start the gateway daemon
stateset-daemon start --config gateway.config.json

# Stop the daemon
stateset-daemon stop

# View logs
stateset-daemon logs --follow

# Health check
stateset-daemon health

# Tailscale VPN management
stateset-daemon tailscale up
stateset-daemon tailscale status

# SSH tunnel management
stateset-daemon tunnel add --host remote.example.com --port 8080
stateset-daemon tunnel list
stateset-daemon tunnel remove <name>
```

### Voice Mode

Enable voice interactions with speech-to-text input and text-to-speech output.

```bash
# Enable voice mode in chat
stateset-chat --voice

# Configure voice provider
stateset-chat --voice --tts-provider elevenlabs --stt-provider whisper
```

Voice mode features per-session settings, 30-minute inactivity TTL, pluggable STT/TTS providers (ElevenLabs, OpenAI Whisper), and automatic stale session cleanup.

### Multi-Provider AI

Swap between AI providers or configure fallback chains.

```bash
# Use a specific provider
stateset --provider openai "list orders"
stateset --provider gemini "show revenue"
stateset --provider ollama --model llama3 "check inventory"

# Claude (default) — full MCP tool integration
stateset "ship order #12345"
```

Non-Claude providers operate in chat-only mode (no MCP tool calls). Claude uses the Agent SDK with full access to all 232 MCP tools.

### Validation & Errors

Inputs are validated at the repository layer (SKU format, required fields, currency codes, etc.)
and errors include field-level context when possible.

```rust
use stateset_core::CommerceError;

match commerce.orders().create(order_input) {
    Ok(order) => println!("created order {}", order.order_number),
    Err(err) if err.is_validation() => {
        eprintln!("validation error: {}", err);
    }
    Err(err) => {
        eprintln!("unexpected error: {}", err);
    }
}
```

Order status updates enforce the core state machine (cancel before shipment, refund after delivery).

---

## Production Notes

- Payments are recorded as ledger events; integrate a PCI-compliant PSP for capture and store tokens/last4 only.
- Sync is event-ordered and can surface conflicts; for order/payment state use a single writer or a sequenced event log.
- Treat external processor IDs and webhook IDs as idempotency keys and de-dupe on ingest to avoid double charges/refunds.

---

## Domain Models

| Domain | Models | Description |
|--------|--------|-------------|
| **Orders** | Order, OrderItem, OrderStatus | Full order lifecycle management |
| **Customers** | Customer, CustomerAddress | Profiles, preferences, history |
| **Products** | Product, ProductVariant | Catalog with variants and attributes |
| **Inventory** | InventoryItem, Balance, Reservation | Multi-location stock tracking |
| **Carts** | Cart, CartItem, CheckoutResult | Shopping cart with ACP checkout |
| **Payments** | Payment, Refund, PaymentMethod | Payment records and refunds |
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
| **Stablecoin** | AgentWallet, StablecoinPayment | Native crypto payments (USDC, ssUSD) |
| **x402** | PaymentIntent, IntentSignature | AI agent payment protocol |
| **A2A Commerce** | AgentCard, A2APayment, Quote, Escrow, Split | Agent-to-Agent autonomous commerce |

---

## Database Schema (60 Tables)

**Core:** customers, customer_addresses, products, product_variants, orders, order_items

**Inventory:** inventory_items, inventory_locations, inventory_balances, inventory_transactions, inventory_reservations

**Financial:** payments, refunds, payment_methods, invoices, invoice_items, purchase_orders, purchase_order_items

**Fulfillment:** shipments, shipment_items, shipment_events, returns, return_items

**Manufacturing:** manufacturing_boms, manufacturing_bom_components, manufacturing_work_orders, manufacturing_work_order_tasks, manufacturing_work_order_materials

**Tax:** tax_jurisdictions, tax_rates, tax_exemptions, tax_settings

**Promotions:** promotions, promotion_rules, promotion_actions, coupons, coupon_usages, promotion_applications

**Subscriptions:** subscription_plans, subscriptions, billing_cycles, subscription_events

**A2A Commerce:** notification_log, webhook_config, subscriptions (a2a), split_payments, split_recipients, event_subscriptions, event_log

**Other:** warranties, warranty_claims, carts, cart_items, exchange_rates, store_currency_settings, product_currency_prices, exchange_rate_history, events

---

## MCP Tools (232 Total)

The MCP server exposes 232 tools across 26 domain modules for AI agent integration:

| Domain | Tools | Count |
|--------|-------|-------|
| **Customers** | list_customers, get_customer, create_customer | 3 |
| **Orders** | list_orders, get_order, create_order, update_order_status, ship_order, cancel_order | 6 |
| **Products** | list_products, get_product, get_product_variant, create_product | 4 |
| **Inventory** | get_stock, create_inventory_item, adjust_inventory, reserve_inventory, confirm_reservation, release_reservation | 6 |
| **Returns** | list_returns, get_return, create_return, approve_return, reject_return | 5 |
| **Carts** | list_carts, get_cart, create_cart, add_cart_item, update_cart_item, remove_cart_item, set_cart_shipping_address, set_cart_payment, apply_cart_discount, get_shipping_rates, complete_checkout, cancel_cart, abandon_cart, get_abandoned_carts | 14 |
| **Analytics** | get_sales_summary, get_top_products, get_customer_metrics, get_top_customers, get_inventory_health, get_low_stock_items, get_demand_forecast, get_revenue_forecast, get_order_status_breakdown, get_return_metrics | 10 |
| **Currency** | get_exchange_rate, list_exchange_rates, convert_currency, set_exchange_rate, get_currency_settings, set_base_currency, enable_currencies, format_currency | 8 |
| **Tax** | calculate_tax, calculate_cart_tax, get_tax_rate, list_tax_jurisdictions, list_tax_rates, get_tax_settings, get_us_state_tax_info, get_customer_tax_exemptions, create_tax_exemption | 9 |
| **Promotions** | list_promotions, get_promotion, create_promotion, activate_promotion, deactivate_promotion, create_coupon, validate_coupon, list_coupons, get_active_promotions, apply_cart_promotions | 10 |
| **Subscriptions** | list_subscription_plans, get_subscription_plan, create_subscription_plan, activate_subscription_plan, archive_subscription_plan, list_subscriptions, get_subscription, create_subscription, pause_subscription, resume_subscription, cancel_subscription, skip_billing_cycle, list_billing_cycles, get_billing_cycle, get_subscription_events | 15 |
| **Payments** | list_payments, get_payment, create_payment, complete_payment, create_refund | 5 |
| **Shipments** | list_shipments, create_shipment, deliver_shipment | 3 |
| **Suppliers/POs** | list_suppliers, create_supplier, list_purchase_orders, create_purchase_order, approve_purchase_order, send_purchase_order | 6 |
| **Invoices** | list_invoices, create_invoice, send_invoice, record_invoice_payment, get_overdue_invoices | 5 |
| **Warranties** | list_warranties, create_warranty, create_warranty_claim, approve_warranty_claim | 4 |
| **Manufacturing** | list_boms, get_bom, create_bom, add_bom_component, activate_bom, list_work_orders, get_work_order, create_work_order, start_work_order, complete_work_order, cancel_work_order | 11 |
| **Stablecoin** | get_agent_wallet, get_wallet_balance, create_stablecoin_payment, list_supported_chains | 4 |
| **x402** | x402_create_payment_intent, x402_sign_intent, x402_get_intent, x402_list_intents, x402_mark_settled, x402_get_next_nonce | 6 |
| **Agent Cards** | register_agent_card, discover_agents, get_agent_card, verify_agent, list_agent_cards | 5 |
| **A2A Payments** | a2a_pay, a2a_request_payment, a2a_pay_request | 3 |
| **A2A Quotes** | a2a_request_quote, a2a_provide_quote, a2a_accept_quote, a2a_decline_quote, a2a_fulfill_quote | 5 |
| **A2A Subscriptions** | a2a_create_subscription, a2a_list_subscriptions, a2a_get_subscription, a2a_pause_subscription, a2a_resume_subscription, a2a_cancel_subscription, a2a_skip_billing, a2a_list_billing_cycles | 8 |
| **A2A Splits** | a2a_create_split_payment, a2a_execute_split_payment, a2a_list_split_payments, a2a_get_split_payment, a2a_add_split_recipient, a2a_get_split_recipients | 6 |
| **A2A Notifications** | a2a_configure_webhooks, a2a_send_notification, a2a_list_notifications, a2a_retry_notifications | 4 |
| **A2A Events** | a2a_subscribe_events, a2a_push_event, a2a_get_event_history, a2a_list_event_subscriptions, a2a_handle_sse_connection | 5 |
| **A2A Escrow** | a2a_create_escrow_payment, a2a_fulfill_condition, a2a_release_escrow, a2a_dispute_escrow, a2a_get_dispute_status | 5 |
| **A2A Discovery** | a2a_discover_agents, a2a_register_service, a2a_list_services, a2a_list_payments, a2a_list_payment_requests, a2a_list_quotes, a2a_get_balance | 7 |
| **A2A Reputation** | a2a_get_reputation, a2a_list_reputation_records, a2a_create_reputation_record, a2a_update_trust_level, a2a_get_trust_profile | 5 |
| **Treasury** | treasury tools (budget, allocation, reporting) | 9 |
| **Custom Objects** | custom_objects CRUD and schema management | 8 |
| **Vector Search** | vector_upsert, vector_search, vector_delete and more | 6 |
| **Sync** | sync push, pull, status, verify, rebase and more | 9 |
| **ERC-8004** | tokenized commerce operations | 8 |

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

### Verifiable Event Sync (VES v1.0)
- Local-first with sequencer synchronization
- Ed25519 agent signatures for event authenticity
- X25519 encryption for payload confidentiality
- Key rotation policies (time-based, usage-based)
- Encryption groups for multi-agent collaboration
- Merkle proofs for on-chain commitment verification
- Conflict detection and resolution (remote-wins, local-wins, merge)

### Multi-Channel Messaging Gateway
- 9 platforms: WhatsApp, Telegram, Discord, Slack, Signal, Google Chat, WebChat, iMessage, Teams
- Persistent sessions across gateway restarts (SQLite-backed)
- Koa-style middleware pipeline (rate limiter, content filter, logger, language detect)
- Platform-aware rich messages (Embeds, Block Kit, HTML, plain text)
- 15+ instant bot commands for orders, inventory, analytics, identity
- Customer identity resolution (auto by phone, manual by email)
- Proactive notifications routed from engine events to channels
- AI-to-human handoff with conversation history
- Config-driven multi-gateway orchestrator (YAML/JSON)
- Per-channel conversation metrics and command tracking
- Interactive action handlers (Telegram buttons, Discord buttons, Slack Block Kit)
- Pre-built message templates for order lifecycle, alerts, and engagement

### Skills System
- 38 domain-specific commerce skills with SKILL.md references, helper scripts, and runbooks
- Skills marketplace for browsing, installing, and managing skill packs
- Covers accounts payable/receivable, fulfillment, quality, warehouse, cost accounting, credit, lots/serials, and more
- Each skill provides contextual knowledge, troubleshooting, and usage examples

### Voice Mode
- Speech-to-text input and text-to-speech output for hands-free commerce operations
- Pluggable providers (ElevenLabs, OpenAI Whisper)
- Per-session voice settings with 30-minute inactivity TTL
- Integrated with agent pipeline for natural voice-driven workflows

### Multi-Provider AI
- Support for Claude (full MCP tools), OpenAI, Google Gemini, and local Ollama models
- Auto-detection of provider availability and model resolution
- Configurable fallback chains (try providers in order)
- Non-Claude providers operate in chat-only mode

### Conversation Memory
- SQLite-backed persistent memory across sessions
- Stores summaries, facts, and token counts per conversation
- Keyword search across memory store
- Auto-cleanup of aged memories

### Browser Automation
- Chrome DevTools Protocol (CDP) integration — no Puppeteer dependency
- Headless Chrome spawning and lifecycle management
- Navigation, DOM queries, JavaScript evaluation, screenshots
- Useful for web scraping, storefront testing, and catalog sync

### Heartbeat Monitor
- 6 proactive health checkers: low-stock, abandoned-carts, revenue-milestone, pending-returns, overdue-invoices, subscription-churn
- EventBridge routing to all messaging channels
- HTTP API for status, manual runs, and enable/disable at runtime

### Permission Sandboxing
- API key authentication (Bearer token and query param)
- Per-route permission levels: none < read < preview < write < delete < admin
- Sandbox mode to block dangerous routes (browser automation, shell execution)

### Agent-to-Agent (A2A) Commerce
- Direct agent-to-agent payments (USDC, USDT, ssUSD, DAI across SET Chain, Base, Ethereum, Arbitrum)
- Quote/negotiate workflow (request → provide → accept/decline → fulfill)
- Recurring subscriptions between agents (weekly → annual intervals, trial periods, pause/resume)
- Multi-party split payments with percentage or fixed-amount splits and platform fees
- Conditional payments with escrow (seller_fulfilled, buyer_confirmed, time_lock, milestone)
- HMAC-SHA256 signed webhook notifications with SSRF validation and exponential backoff retry
- Real-time SSE event streaming with wildcard/prefix filtering and persistent event log
- Agent discovery by capability, reputation scoring, and trust-level gating
- Idempotency keys to prevent duplicate payments
- 53 dedicated MCP tools for the full A2A lifecycle

### AI-Ready Architecture
- Deterministic operations for agent reliability
- MCP protocol integration (232 tools across 26 modules)
- Safety architecture (--apply flag for writes)
- Event-driven for full auditability
- Portable state in single database file

---

## Installation

### Rust

```toml
[dependencies]
stateset-embedded = "0.3"
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
    <version>0.7.0</version>
</dependency>
```

### Java (Gradle)

```groovy
implementation 'com.stateset:embedded:0.7.0'
```

### Kotlin (Gradle)

```kotlin
// build.gradle.kts
dependencies {
    implementation("com.stateset:embedded-kotlin:0.7.0")
}
```

### Swift (Swift Package Manager)

```swift
// Package.swift
dependencies: [
    .package(url: "https://github.com/stateset/stateset-swift.git", from: "0.7.0")
]
```

Or with CocoaPods:

```ruby
pod 'StateSet', '~> 0.7.0'
```

### C# / .NET (NuGet)

```bash
dotnet add package StateSet.Embedded --version 0.7.0
```

Or in your `.csproj`:

```xml
<PackageReference Include="StateSet.Embedded" Version="0.7.0" />
```

### Go

```bash
go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v0.7.0
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

StateSet provides native bindings for 11 languages, all built from the same Rust core:

| Language | Package | Install | Docs |
|----------|---------|---------|------|
| **Rust** | `stateset-embedded` | `cargo add stateset-embedded` | [crates.io](https://crates.io/crates/stateset-embedded) |
| **Node.js** | `@stateset/embedded` | `npm install @stateset/embedded` | [npm](https://www.npmjs.com/package/@stateset/embedded) |
| **Python** | `stateset-embedded` | `pip install stateset-embedded` | [PyPI](https://pypi.org/project/stateset-embedded/) |
| **Ruby** | `stateset_embedded` | `gem install stateset_embedded` | [RubyGems](https://rubygems.org/gems/stateset_embedded) |
| **PHP** | `stateset/embedded` | `composer require stateset/embedded` | [Packagist](https://packagist.org/packages/stateset/embedded) |
| **Java** | `com.stateset:embedded` | Maven/Gradle | [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded) |
| **Kotlin** | `com.stateset:embedded-kotlin` | Gradle | [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded-kotlin) |
| **Swift** | `StateSet` | Swift PM / CocoaPods | [GitHub](https://github.com/stateset/stateset-swift) |
| **C# / .NET** | `StateSet.Embedded` | `dotnet add package StateSet.Embedded` | [NuGet](https://www.nuget.org/packages/StateSet.Embedded) |
| **Go** | `stateset` | `go get github.com/stateset/.../stateset` | [pkg.go.dev](https://pkg.go.dev/github.com/stateset/stateset-icommerce/bindings/go/stateset) |
| **WASM** | `@stateset/embedded-wasm` | `npm install @stateset/embedded-wasm` | [npm](https://www.npmjs.com/package/@stateset/embedded-wasm) |

### Platform Support

| Platform | Node.js | Python | Ruby | PHP | Java | Kotlin | Swift | C# | Go |
|----------|---------|--------|------|-----|------|--------|-------|-----|-----|
| Linux x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Linux arm64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS arm64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Windows x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | - | ✅ | ✅ |
| iOS | - | - | - | - | - | - | ✅ | - | - |
| Android | - | - | - | - | ✅ | ✅ | - | - | - |
| Browser (WASM) | ✅ | - | - | - | - | - | - | ✅* | - |

*Via Blazor WebAssembly

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

**Android (Kotlin)**
```kotlin
// Application class
class CommerceApp : Application() {
    val commerce by lazy {
        StateSetCommerce(getDatabasePath("commerce.db").absolutePath)
    }
}
```

**iOS/macOS (Swift)**
```swift
// AppDelegate or @main struct
let commerce = try! StateSetCommerce(
    path: FileManager.default
        .urls(for: .documentDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("commerce.db").path
)
```

**ASP.NET Core (C#)**
```csharp
// Program.cs or Startup.cs
builder.Services.AddSingleton<StateSetCommerce>(sp =>
    new StateSetCommerce("commerce.db"));
```

**Go (net/http or Gin)**
```go
// main.go
var commerce *stateset.Commerce

func init() {
    var err error
    commerce, err = stateset.New("commerce.db")
    if err != nil {
        log.Fatal(err)
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
export STATESET_POLICY_DIR=./.stateset  # Optional policy set directory (defaults to <db-path>/.stateset)
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

## Development

Core checks (matches CI):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p stateset-core -p stateset-db -p stateset-embedded
cargo bench -p stateset-core -p stateset-db -p stateset-embedded
```

Bindings:

```bash
cd bindings/node && npm ci && npm test
cd bindings/python && python -m pip install maturin pytest && maturin develop --release && pytest -q
```

CLI (4,300+ tests):

```bash
cd cli
npm test                    # All tests
npm run test:unit           # Unit tests only
npm run test:integration    # Integration tests
npm run test:e2e            # End-to-end tests
npm run lint                # ESLint
npm run typecheck           # JSDoc type checking
```

---

## Project Structure

```
stateset-icommerce/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── stateset-core/         # Domain models (32 domain modules)
│   │   └── src/models/        # 254 types
│   ├── stateset-db/           # Database layer
│   │   ├── src/sqlite/        # 18 SQLite modules
│   │   ├── src/postgres/      # 8 PostgreSQL modules
│   │   └── migrations/        # 26 SQL migrations
│   └── stateset-embedded/     # High-level API
│       └── src/               # 33 API modules
├── bindings/
│   ├── node/                  # NAPI bindings (@stateset/embedded)
│   ├── python/                # PyO3 bindings (stateset-embedded)
│   ├── ruby/                  # Magnus bindings (stateset_embedded gem)
│   ├── php/                   # ext-php-rs bindings (stateset/embedded)
│   ├── java/                  # JNI bindings (com.stateset:embedded)
│   ├── kotlin/                # JNI bindings (com.stateset:embedded-kotlin)
│   ├── swift/                 # C FFI bindings (StateSet Swift package)
│   ├── dotnet/                # P/Invoke bindings (StateSet.Embedded NuGet)
│   ├── go/                    # cgo bindings (stateset Go module)
│   └── wasm/                  # WebAssembly bindings (@stateset/embedded-wasm)
├── cli/
│   ├── bin/                   # 40 CLI entry points
│   ├── src/mcp-server.js      # MCP orchestrator (232 tools, 470 lines)
│   ├── src/tools/             # 26 modular tool modules
│   ├── src/a2a/               # Agent-to-Agent commerce
│   │   ├── index.js           # Direct payments, quotes, escrow, conditions
│   │   ├── store.js           # SQLite persistence (7 tables)
│   │   ├── notifications.js   # HMAC-SHA256 signed webhooks
│   │   ├── subscriptions.js   # Recurring agent subscriptions
│   │   ├── splits.js          # Multi-party split payments
│   │   └── event-stream.js    # SSE push, event log, subscriptions
│   ├── src/channels/          # 9-channel messaging gateway
│   ├── src/providers/         # Multi-provider AI (Claude, OpenAI, Gemini, Ollama)
│   ├── src/voice/             # Voice mode (STT + TTS)
│   ├── src/memory/            # Persistent conversation memory
│   ├── src/browser/           # Chrome DevTools Protocol automation
│   ├── src/skills/            # Skills loader, registry, marketplace
│   ├── src/chains/            # Blockchain integration (Solana, Base, SET Chain, etc.)
│   ├── src/x402/              # x402 AI agent payment protocol
│   ├── src/imessage/          # iMessage gateway (BlueBubbles)
│   ├── src/matrix/            # Matrix protocol gateway
│   ├── src/teams/             # Microsoft Teams gateway (Bot Framework)
│   ├── src/sync/              # VES sync engine
│   │   ├── engine.js          # Sync orchestration
│   │   ├── outbox.js          # Event outbox management
│   │   ├── client.js          # gRPC sequencer client
│   │   ├── keys.js            # Ed25519/X25519 key management
│   │   ├── groups.js          # Encryption group management
│   │   ├── rotation-policy.js # Key rotation policies
│   │   ├── crypto.js          # VES cryptographic operations
│   │   └── conflict.js        # Conflict resolution
│   ├── skills/                # 38 commerce domain skills
│   ├── deploy/                # Systemd services, Tailscale, SSH tunnels
│   └── .claude/               # 8 AI agents
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
