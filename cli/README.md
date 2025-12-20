# @stateset/cli

AI-powered command-line interface for autonomous commerce operations.

**Version:** 0.1.7

## Philosophy

The Stateset CLI is built on the premise that commerce infrastructure should be designed for AI agents, not just humans. Think of it as **"The SQLite of Commerce"** — an embedded, zero-dependency commerce engine that:

- **Runs locally** without cloud dependencies
- **Deterministic operations** for agent reliability
- **Agentic Commerce Protocol (ACP)** for standardized agent interactions
- **Safety-first architecture** — read-only by default, explicit `--apply` for writes

## Features

- **Natural Language Interface** - Ask Claude to perform commerce operations
- **Multi-Agent System** - 8 specialized agents auto-route to the best handler
- **87 MCP Tools** - Full commerce API exposed to Claude
- **Multi-turn Sessions** - Resume conversations for complex workflows
- **Preview Mode** - See what would happen before making changes
- **Direct Commands** - Fast, non-AI mode for scripting
- **Interactive Chat** - REPL for exploratory work
- **SQLite/PostgreSQL** - Flexible storage backends
- **Rich Output** - ASCII tables, progress bars, formatted displays
- **Telemetry** - Distributed tracing with `--verbose` and `--stats`

## Installation

```bash
npm install -g @stateset/cli
```

Or run locally:

```bash
cd cli
npm install
npm link
```

## Quick Start

### AI-Powered Mode

```bash
# List customers (read-only by default)
stateset "show me all customers"

# Check inventory
stateset "how much stock do we have of WIDGET-001?"

# Create a customer (requires --apply)
stateset --apply "create a customer named Alice with email alice@example.com"

# Multi-turn workflow
stateset --apply "create an order for that customer with 2 widgets at $29.99"
stateset --apply --resume <session-id> "ship that order with tracking ABC123"
```

### Interactive Chat

```bash
stateset-chat

# In chat:
> show me all orders
> /apply on
> create a product called Premium Widget with SKU WIDGET-001 at $29.99
> /status
> /exit
```

### Direct Commands (No AI)

```bash
# Customer operations
stateset-direct customers list
stateset-direct customers get alice@example.com

# Order operations
stateset-direct orders list
stateset-direct orders ship <order-id> TRACK123

# Inventory operations
stateset-direct inventory stock WIDGET-001
stateset-direct inventory adjust WIDGET-001 -5 "Sold 5 units"
```

## Commands

### Primary Commands

| Command | Description |
|---------|-------------|
| `stateset "<request>"` | AI-powered interface (auto-routes to best agent) |
| `stateset-chat` | Multi-turn interactive REPL |
| `stateset-direct <resource> <action>` | Direct CLI (no AI required) |

### Specialized Agent Commands

| Command | Agent | Description |
|---------|-------|-------------|
| `stateset-checkout` | checkout | Shopping cart & checkout flow (ACP) |
| `stateset-orders` | orders | Order lifecycle management |
| `stateset-inventory` | inventory | Stock & reservation management |
| `stateset-returns` | returns | RMA & refund processing |
| `stateset-analytics` | analytics | Sales metrics & forecasting |
| `stateset-promotions` | promotions | Promotions, discounts & coupons |
| `stateset-subscriptions` | subscriptions | Subscription plans & recurring billing |
| `stateset-create` | storefront | Scaffold e-commerce storefronts |

### Utility Commands

| Command | Description |
|---------|-------------|
| `stateset-config` | Profile and configuration management |
| `stateset-doctor` | Health check and diagnostics |
| `stateset-events` | Event management and webhooks |

## Architecture

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/       # Pure domain models (254 types, 18 modules)
│   ├── stateset-db/         # SQLite + PostgreSQL (53 tables)
│   └── stateset-embedded/   # High-level unified API (671+ methods)
├── bindings/
│   ├── node/                # @stateset/embedded (NAPI)
│   ├── python/              # stateset-embedded (PyO3)
│   └── wasm/                # WebAssembly for browsers
└── cli/
    ├── bin/                 # 14 CLI programs
    ├── src/
    │   ├── claude-harness.js    # Multi-agent SDK integration
    │   ├── mcp-server.js        # 87 MCP tools for Claude
    │   ├── permissions.js       # Fine-grained access control
    │   └── telemetry.js         # Observability & tracing
    └── .claude/
        ├── agents/          # 8 specialized agent definitions
        └── skills/          # Domain knowledge documents
```

### Technology Stack

- **Rust Core** - Pure domain logic with deterministic execution
- **@stateset/embedded** - Native Node.js bindings via NAPI
- **Claude Agent SDK** - AI agent framework with MCP tools
- **SQLite/PostgreSQL** - Flexible database backends

## Capabilities

### Commerce Operations

| Domain | Operations |
|--------|------------|
| **Orders** | Create, confirm, process, ship, deliver, cancel |
| **Customers** | Profiles, addresses, preferences, history |
| **Products** | Catalog with variants and attributes |
| **Inventory** | Multi-location stock, reservations, adjustments |
| **Carts** | Shopping cart with ACP checkout flow |
| **Returns** | RMA processing with refund management |
| **Shipments** | Fulfillment tracking with carrier integration |

### Financial Operations

| Domain | Operations |
|--------|------------|
| **Multi-Currency** | 35+ currencies (USD, EUR, GBP, JPY, BTC, ETH, USDC) |
| **Payments** | Credit card, PayPal, cryptocurrency |
| **Refunds** | Multiple payout methods |
| **Invoices** | Generation with line items |
| **Purchase Orders** | Supplier management and procurement |

### Promotions & Discounts

```bash
# Create promotions
stateset --apply "create a 20% off promotion called Summer Sale"
stateset --apply "create a buy 2 get 1 free promotion"
stateset --apply "create a free shipping promotion for orders over $50"

# Manage coupons
stateset --apply "create coupon SAVE20 with 100 use limit"
stateset "is coupon SAVE20 valid?"

# Apply to cart
stateset --apply "apply promotions to cart <cart-id>"
```

### Subscriptions & Recurring Billing

```bash
# Create plans
stateset --apply "create a monthly plan called Coffee Club at $29.99 with 14 day trial"
stateset --apply "create an annual plan called Pro at $99.99"

# Manage subscriptions
stateset --apply "subscribe customer <id> to the Coffee Club plan"
stateset --apply "pause subscription <id>"
stateset --apply "skip next billing for subscription <id>"
stateset --apply "cancel subscription <id>"

# View history
stateset "show billing history for subscription <id>"
```

### Tax Management

```bash
# Calculate tax
stateset "calculate tax for an order shipping to California"
stateset "what's the tax rate for New York?"
stateset "calculate tax for cart CART-123456"

# Tax jurisdictions
stateset "show me all tax jurisdictions"
stateset "what are the EU VAT rates?"
stateset "get tax info for Texas"

# Exemptions
stateset --apply "create tax exemption for customer abc123 - resale certificate"
```

### Analytics & Forecasting

```bash
# Sales analytics
stateset "what's my total revenue this month?"
stateset "show me my best sellers"
stateset "who are my top customers?"

# Inventory health
stateset "what inventory needs attention?"
stateset "show me low stock items"

# Forecasting
stateset "predict inventory needs for next month"
stateset "forecast revenue for next quarter"
```

## Safety Architecture

### Read-Only by Default

All write operations are **blocked by default**. The CLI shows what would happen without making changes.

```bash
# Preview what would happen
stateset "create a customer named Bob"
# Output: "Would create customer: {email: ..., name: Bob}"

# Actually create the customer
stateset --apply "create a customer named Bob"
# Output: "Created customer: abc-123-def"
```

### Permission Controls

| Feature | Description |
|---------|-------------|
| **Preview Mode** | Default read-only operation |
| **Confirmation Thresholds** | High-value operations (>$1000) prompt for confirmation |
| **Permission Gating** | Five levels: none, read, preview, write, admin |
| **Spending Limits** | Max order value, daily totals |
| **Rate Limiting** | Tool calls/minute, write ops/minute |
| **Audit Logging** | Complete operation history |

## Observability

```bash
# Verbose output with tracing
stateset --verbose "show me pending orders"

# Execution statistics
stateset --stats "process all returns"

# JSON output for integration
stateset --json "list customers"
```

## Workflows

### E-commerce Workflow

```bash
# Set up a product
stateset --apply "create a product called 'Premium Widget' with SKU WIDGET-001 at $29.99"

# Add inventory
stateset --apply "create inventory for WIDGET-001 with 100 units"

# Create a customer
stateset --apply "create customer alice@example.com named Alice Smith"

# Create an order
stateset --apply "create an order for alice@example.com: 2x WIDGET-001"

# Ship it
stateset --apply --resume <session> "ship that order with tracking FEDEX123"
```

### Shopping Cart Checkout (ACP)

```bash
# Create a cart
stateset --apply "create a cart for alice@example.com"

# Add items (multi-turn)
stateset --apply --resume <session> "add 2 Premium Widgets at $29.99"
stateset --apply --resume <session> "add 1 Deluxe Widget at $49.99"

# Set shipping address
stateset --apply --resume <session> "set shipping to Alice Smith, 123 Main St, Anytown, CA 90210"

# Apply discount
stateset --apply --resume <session> "apply discount code SAVE10"

# Check shipping options
stateset --resume <session> "what shipping options are available?"

# Complete checkout
stateset --apply --resume <session> "pay with credit card and complete checkout"
```

### Cart Recovery

```bash
stateset "show me abandoned carts"
stateset "what items are in cart CART-123456?"
```

### Inventory Management

```bash
# Check stock
stateset "how much WIDGET-001 do we have?"

# Restock
stateset --apply "add 50 units to WIDGET-001 - received shipment"

# Adjust for damage
stateset --apply "remove 3 units from WIDGET-001 - damaged in warehouse"
```

### Processing Returns

```bash
# Create return
stateset --apply "create a return for order #12345 - item defective"

# Review and approve
stateset "show me pending returns"
stateset --apply "approve return <return-id>"
```

### Multi-Currency Support

```bash
stateset "what's the exchange rate from USD to EUR?"
stateset "convert $100 USD to EUR"
stateset "list all exchange rates"
stateset --apply "set exchange rate USD to EUR at 0.92"
stateset --apply "enable currencies USD, EUR, GBP, JPY"
```

### Storefront Creation

```bash
# Preview what would be created
stateset-create "create a store called Urban Thread"

# Create the project
stateset-create --apply "create a nextjs storefront for my coffee shop"

# Create in specific directory
stateset-create --apply --dir ~/projects "build an online bookstore"
```

Available templates: `nextjs`, `nextjs-minimal`, `vite-react`, `astro`

## Agent System

Eight specialized agents handle different commerce domains:

| Agent | Tools | Purpose |
|-------|-------|---------|
| **checkout** | 14 ACP tools | Shopping cart & payment flows |
| **orders** | 6 tools | Order lifecycle & fulfillment |
| **inventory** | 6 tools | Stock management & reservations |
| **returns** | 5 tools | RMA & refund processing |
| **analytics** | 10 tools | Business intelligence & forecasting |
| **promotions** | 10 tools | Campaigns, discounts, coupons |
| **subscriptions** | 15 tools | Subscription plans & recurring billing |
| **customer-service** | All 87 tools | Full-service fallback agent |

### Auto-Routing

The main `stateset` command automatically routes requests to the best agent based on:
- Request content analysis
- Confidence scoring
- Domain keyword matching
- Ambiguity detection

## MCP Tools (87 Total)

| Domain | Count | Examples |
|--------|-------|----------|
| **Customers** | 3 | list, get, create |
| **Orders** | 6 | list, get, create, update_status, ship, cancel |
| **Products** | 4 | list, get, get_variant, create |
| **Inventory** | 6 | get_stock, create_item, adjust, reserve, confirm, release |
| **Returns** | 5 | list, get, create, approve, reject |
| **Carts/Checkout** | 14 | create, add_item, set_address, set_payment, complete_checkout |
| **Analytics** | 10 | sales_summary, top_products, demand_forecast, revenue_forecast |
| **Currency** | 8 | get_rate, convert, set_rate, format |
| **Tax** | 9 | calculate_tax, calculate_cart_tax, get_rate, list_jurisdictions |
| **Promotions** | 10 | list, create, activate, create_coupon, validate_coupon, apply |
| **Subscriptions** | 15 | list_plans, create_plan, create_subscription, pause, resume, cancel |
| **Manufacturing** | 11 | list_boms, create_bom, create_work_order, complete_work_order |
| **Payments** | 5 | list, get, create, complete, create_refund |
| **Shipments** | 3 | list, create, deliver |
| **Suppliers/POs** | 6 | list_suppliers, create_supplier, create_purchase_order |
| **Invoices** | 5 | list, create, send, record_payment, get_overdue |
| **Warranties** | 4 | list, create, create_claim, approve_claim |

## Configuration

### Environment Variables

```bash
# Claude API key (required for AI mode)
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Database

```bash
# Default: ./store.db (SQLite)
stateset --db /path/to/mystore.db "list customers"

# In-memory database (testing)
stateset --db :memory: "list customers"

# PostgreSQL (enterprise)
# Configure via environment or config file
```

### Flags Reference

| Flag | Description |
|------|-------------|
| `--db <path>` | Database path |
| `--apply` | Enable write operations |
| `--model <name>` | Claude model to use |
| `--resume <id>` | Resume previous session |
| `--json` | JSON output |
| `--verbose` | Real-time telemetry |
| `--stats` | Execution statistics |
| `--help` | Show help |

## Session Management

Sessions enable multi-turn conversations with context preservation:

```bash
# First request returns session ID
stateset --apply "create a cart for alice@example.com"
# Output includes: Session ID: abc-123-def

# Resume to continue context
stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
stateset --apply --resume abc-123-def "complete checkout"
```

## Command Reference

### `stateset` - AI Agent

```
stateset [options] "<request>"

Options:
  --db <path>     Database path (default: ./store.db)
  --apply         Enable write operations
  --model <name>  Claude model
  --resume <id>   Resume previous session
  --json          JSON output
  --verbose       Show telemetry
  --stats         Show execution stats
  --help          Show help
```

### `stateset-chat` - Interactive Mode

```
stateset-chat [options]

Options:
  --db <path>     Database path
  --apply         Start with write enabled
  --model <name>  Claude model

In-chat commands:
  /help           Show commands
  /status         Current settings
  /apply on|off   Toggle write mode
  /db <path>      Switch database
  /new            Start new session
  /exit           Exit
```

### `stateset-direct` - Direct Commands

```
stateset-direct [options] <resource> <action> [args]

Options:
  --db <path>     Database path
  --json          JSON output
  --help          Show help

Resources:
  customers       Customer management
  orders          Order management
  products        Product catalog
  inventory       Stock management
  returns         Return processing
```

## Development

```bash
cd cli
npm install
npm link

# Test
stateset --help
stateset "list customers"

# Run diagnostics
stateset-doctor
```

## License

MIT OR Apache-2.0
