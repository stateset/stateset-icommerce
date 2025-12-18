# StateSet Commerce CLI - Project Guidance

An AI-powered command-line interface for commerce operations using the Claude Agent SDK.

## Overview

This CLI provides natural language access to commerce operations:
- **Customers** - Customer management and lookup
- **Orders** - Order lifecycle management
- **Products** - Product catalog operations
- **Inventory** - Stock tracking and allocation
- **Returns** - Return request processing
- **Carts/Checkout** - Shopping cart and checkout flow (Agentic Commerce Protocol)
- **Payments** - Payment processing and refunds
- **Currency** - Multi-currency support, exchange rates, and conversions

## Entry Points

| Command | Description |
|---------|-------------|
| `stateset "<request>"` | AI-powered interface (auto-routes to best agent) |
| `stateset-chat` | Multi-turn interactive REPL |
| `stateset-direct <resource> <action>` | Direct CLI (no AI) |

### Specialized Agent Commands

| Command | Agent | Description |
|---------|-------|-------------|
| `stateset-checkout` | checkout | Shopping cart & checkout flow (ACP) |
| `stateset-orders` | orders | Order lifecycle management |
| `stateset-inventory` | inventory | Stock & reservation management |
| `stateset-returns` | returns | RMA & refund processing |
| `stateset-analytics` | analytics | Sales metrics & forecasting |

Use specialized commands for focused workflows with domain-specific tooling and prompts.

## Safety Architecture

### Permission Model

All write operations are **blocked by default**. Use `--apply` to enable them.

```bash
# Preview only (safe)
stateset "create a cart for alice@example.com"

# Actually execute (requires explicit opt-in)
stateset --apply "create a cart for alice@example.com"
```

### Safety Rules

1. **Preview First** - Always show what would happen before executing
2. **Count Clearly** - Report how many records will be affected
3. **Explain Operations** - Tell the user what each operation does
4. **Handle Errors** - Explain failures and suggest fixes
5. **Session Awareness** - Track context across multi-turn conversations

## MCP Servers (Tools)

### commerce-customers
- `list_customers` - List all customers
- `get_customer` - Get by ID or email
- `create_customer` - Create customer (requires --apply)

### commerce-orders
- `list_orders` - List all orders
- `get_order` - Get order with items
- `create_order` - Create order (requires --apply)
- `update_order_status` - Update status (requires --apply)
- `ship_order` - Ship with tracking (requires --apply)
- `cancel_order` - Cancel order (requires --apply)

### commerce-products
- `list_products` - List catalog
- `get_product` - Get product details
- `get_product_variant` - Get by SKU
- `create_product` - Create product (requires --apply)

### commerce-inventory
- `get_stock` - Get stock levels
- `create_inventory_item` - Create inventory (requires --apply)
- `adjust_inventory` - Add/remove stock (requires --apply)
- `reserve_inventory` - Reserve for order (requires --apply)
- `confirm_reservation` - Confirm and deduct (requires --apply)
- `release_reservation` - Release reserved (requires --apply)

### commerce-returns
- `list_returns` - List returns
- `get_return` - Get return details
- `create_return` - Create return (requires --apply)
- `approve_return` - Approve return (requires --apply)
- `reject_return` - Reject with reason (requires --apply)

### commerce-carts (Agentic Commerce Protocol)
- `list_carts` - List shopping carts
- `get_cart` - Get cart with items
- `create_cart` - Create cart (requires --apply)
- `add_cart_item` - Add item (requires --apply)
- `update_cart_item` - Update quantity (requires --apply)
- `remove_cart_item` - Remove item (requires --apply)
- `set_cart_shipping_address` - Set address (requires --apply)
- `set_cart_payment` - Set payment method (requires --apply)
- `apply_cart_discount` - Apply coupon (requires --apply)
- `get_shipping_rates` - Get shipping options
- `complete_checkout` - Convert to order (requires --apply)
- `cancel_cart` - Cancel cart (requires --apply)
- `abandon_cart` - Mark abandoned (requires --apply)
- `get_abandoned_carts` - Get for recovery

### commerce-analytics
- `get_sales_summary` - Revenue, orders, AOV, items sold
- `get_top_products` - Best sellers by revenue/units
- `get_customer_metrics` - Total, new, returning customers
- `get_top_customers` - VIP customers by spend
- `get_inventory_health` - SKUs in stock, low stock, out of stock
- `get_low_stock_items` - Items needing attention
- `get_demand_forecast` - Predict future demand per SKU
- `get_revenue_forecast` - Predict future revenue
- `get_order_status_breakdown` - Orders by status
- `get_return_metrics` - Return rate and refunds

### commerce-currency
- `get_exchange_rate` - Get rate between two currencies
- `list_exchange_rates` - List all rates or filter by base
- `convert_currency` - Convert amount between currencies
- `set_exchange_rate` - Set/update rate (requires --apply)
- `get_currency_settings` - Get store currency settings
- `set_base_currency` - Set store base currency (requires --apply)
- `enable_currencies` - Enable currencies for store (requires --apply)
- `format_currency` - Format amount with currency symbol

## Agents

Specialized agents for different commerce domains:

| Agent | Description | Tools |
|-------|-------------|-------|
| `orders` | Order lifecycle specialist | commerce-orders |
| `checkout` | Cart and checkout flow specialist | commerce-carts |
| `inventory` | Stock management specialist | commerce-inventory |
| `returns` | Return processing specialist | commerce-returns |
| `analytics` | Business intelligence & forecasting | commerce-analytics |
| `customer-service` | Full customer service agent | All tools |

## Skills

Domain knowledge documents that enhance agent capabilities:

| Skill | Description |
|-------|-------------|
| `commerce-orders` | Order states, fulfillment workflows |
| `commerce-checkout` | Checkout flow, payment methods |
| `commerce-inventory` | Stock tracking, reservations |
| `commerce-returns` | Return reasons, refund workflows |
| `commerce-analytics` | Sales metrics, forecasting, business intelligence |

## Common Workflows

### Order Fulfillment
```bash
stateset "show me pending orders"
stateset --apply --resume <id> "ship order #12345 with tracking FEDEX123"
```

### Cart Checkout (ACP)
```bash
stateset --apply "create a cart for alice@example.com"
stateset --apply --resume <id> "add 2 widgets at $29.99"
stateset --apply --resume <id> "set shipping to 123 Main St, Anytown CA"
stateset --apply --resume <id> "complete the checkout"
```

### Inventory Management
```bash
stateset "how much WIDGET-001 do we have?"
stateset --apply "add 50 units to WIDGET-001 - received shipment"
```

### Return Processing
```bash
stateset "show me pending returns"
stateset --apply "approve return <id>"
```

### Cart Recovery
```bash
stateset "show me abandoned carts"
stateset "what items are in cart CART-123456?"
```

### Analytics & Forecasting
```bash
stateset "what's my total revenue this month?"
stateset "show me my best sellers"
stateset "who are my top customers?"
stateset "what inventory needs attention?"
stateset "predict inventory needs for next month"
stateset "forecast revenue for next quarter"
```

### Multi-Currency Support
```bash
stateset "what's the exchange rate from USD to EUR?"
stateset "convert $100 USD to EUR"
stateset "list all exchange rates"
stateset "what currencies are enabled?"
stateset --apply "set exchange rate USD to EUR at 0.92"
stateset --apply "enable currencies USD, EUR, GBP, JPY"
stateset --apply "set base currency to EUR"
```

## Configuration

### Environment Variables
```bash
ANTHROPIC_API_KEY=sk-ant-...   # Required for AI mode
```

### Database
Default: `./store.db` (SQLite)

```bash
stateset --db /path/to/store.db "list customers"
stateset --db :memory: "list customers"  # In-memory for testing
```

## Flags Reference

| Flag | Description |
|------|-------------|
| `--db <path>` | Database path |
| `--apply` | Enable write operations |
| `--model <name>` | Claude model to use |
| `--resume <id>` | Resume session |
| `--json` | JSON output |

## Session Management

Sessions enable multi-turn conversations:

```bash
# First request returns session ID
stateset --apply "create a cart for alice@example.com"
# Output includes: Session ID: abc-123-def

# Resume to continue context
stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
```

## Architecture

```
stateset-icommerce/cli/
├── bin/
│   ├── stateset.js           # AI agent interface (auto-routing)
│   ├── stateset-chat.js      # Interactive REPL
│   ├── stateset-direct.js    # Direct commands (no AI)
│   ├── stateset-checkout.js  # Checkout agent
│   ├── stateset-orders.js    # Orders agent
│   ├── stateset-inventory.js # Inventory agent
│   └── stateset-returns.js   # Returns agent
├── src/
│   ├── claude-harness.js     # Multi-agent SDK integration
│   ├── mcp-server.js         # MCP tools (38 total)
│   └── utils/
├── .claude/
│   ├── CLAUDE.md             # This file
│   ├── agents/               # Agent definitions
│   │   ├── checkout.md       # ACP checkout specialist
│   │   ├── orders.md         # Order lifecycle
│   │   ├── inventory.md      # Stock management
│   │   ├── returns.md        # RMA processing
│   │   └── customer-service.md # Full-service agent
│   └── skills/               # Domain knowledge
│       ├── commerce-checkout/SKILL.md
│       ├── commerce-orders/SKILL.md
│       ├── commerce-inventory/SKILL.md
│       └── commerce-returns/SKILL.md
└── package.json
```

## Development

```bash
cd cli
npm install
npm link

# Test
stateset --help
stateset "list customers"
```
