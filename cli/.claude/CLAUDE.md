# StateSet iCommerce CLI - Project Guidance

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
| `stateset-promotions` | promotions | Promotions, discounts & coupons |
| `stateset-subscriptions` | subscriptions | Subscription plans & recurring billing |
| `stateset-create` | storefront | Create e-commerce storefronts |

Use specialized commands for focused workflows with domain-specific tooling and prompts.

### Storefront Creation

The `stateset-create` command scaffolds complete e-commerce websites:

```bash
# Preview what would be created
stateset-create "create a store called Urban Thread"

# Actually create the project
stateset-create --apply "create a nextjs storefront for my coffee shop"

# Create in a specific directory
stateset-create --apply --dir ~/projects "build an online bookstore"
```

Available templates:
- `nextjs` - Full-stack Next.js 14 with App Router, SSR, Tailwind (recommended)
- `nextjs-minimal` - Minimal Next.js setup
- `vite-react` - Client-side SPA with WASM
- `astro` - Static-first with Islands

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

### commerce-tax
- `calculate_tax` - Calculate tax for line items (US/EU/CA)
- `calculate_cart_tax` - Calculate and apply tax to cart based on shipping address
- `get_tax_rate` - Get effective rate for jurisdiction
- `list_tax_jurisdictions` - List tax jurisdictions
- `list_tax_rates` - List all tax rates
- `get_tax_settings` - Get store tax settings
- `get_us_state_tax_info` - Get US state tax details
- `get_customer_tax_exemptions` - Get customer's tax exemptions
- `create_tax_exemption` - Create tax exemption (requires --apply)

### commerce-promotions
- `list_promotions` - List all promotions
- `get_promotion` - Get promotion details
- `create_promotion` - Create a promotion (requires --apply)
- `activate_promotion` - Make promotion live (requires --apply)
- `deactivate_promotion` - Pause promotion (requires --apply)
- `create_coupon` - Create a coupon code (requires --apply)
- `validate_coupon` - Check if coupon code is valid
- `list_coupons` - List all coupon codes
- `get_active_promotions` - Get currently active promotions
- `apply_cart_promotions` - Apply promotions to cart (requires --apply)

### commerce-subscriptions
- `list_subscription_plans` - List all subscription plans
- `get_subscription_plan` - Get plan details
- `create_subscription_plan` - Create a plan (requires --apply)
- `activate_subscription_plan` - Make plan available (requires --apply)
- `archive_subscription_plan` - Retire a plan (requires --apply)
- `list_subscriptions` - List customer subscriptions
- `get_subscription` - Get subscription details
- `create_subscription` - Subscribe customer to plan (requires --apply)
- `pause_subscription` - Pause billing (requires --apply)
- `resume_subscription` - Resume paused subscription (requires --apply)
- `cancel_subscription` - Cancel subscription (requires --apply)
- `skip_billing_cycle` - Skip next billing (requires --apply)
- `list_billing_cycles` - View billing history
- `get_billing_cycle` - Get billing cycle details
- `get_subscription_events` - View subscription event log

### commerce-manufacturing
- `list_boms` - List all Bills of Materials
- `get_bom` - Get BOM with components
- `create_bom` - Create new BOM (requires --apply)
- `add_bom_component` - Add component to BOM (requires --apply)
- `activate_bom` - Activate BOM for production (requires --apply)
- `list_work_orders` - List manufacturing work orders
- `get_work_order` - Get work order details
- `create_work_order` - Create work order from BOM (requires --apply)
- `start_work_order` - Start production (requires --apply)
- `complete_work_order` - Complete with quantity (requires --apply)
- `cancel_work_order` - Cancel work order (requires --apply)

### commerce-payments
- `list_payments` - List all payments
- `get_payment` - Get payment details
- `create_payment` - Create payment for order (requires --apply)
- `complete_payment` - Mark payment completed (requires --apply)
- `create_refund` - Process refund (requires --apply)

### commerce-shipments
- `list_shipments` - List all shipments
- `create_shipment` - Create shipment with tracking (requires --apply)
- `deliver_shipment` - Mark as delivered (requires --apply)

### commerce-suppliers
- `list_suppliers` - List all suppliers
- `create_supplier` - Create new supplier (requires --apply)
- `list_purchase_orders` - List all purchase orders
- `create_purchase_order` - Create PO (requires --apply)
- `approve_purchase_order` - Approve PO (requires --apply)
- `send_purchase_order` - Send to supplier (requires --apply)

### commerce-invoices
- `list_invoices` - List all invoices
- `create_invoice` - Create B2B invoice (requires --apply)
- `send_invoice` - Send to customer (requires --apply)
- `record_invoice_payment` - Record payment (requires --apply)
- `get_overdue_invoices` - Get overdue invoices

### commerce-warranties
- `list_warranties` - List all warranties
- `create_warranty` - Create product warranty (requires --apply)
- `create_warranty_claim` - File warranty claim (requires --apply)
- `approve_warranty_claim` - Approve claim (requires --apply)

## Agents

Specialized agents for different commerce domains:

| Agent | Description | Tools |
|-------|-------------|-------|
| `orders` | Order lifecycle specialist | commerce-orders |
| `checkout` | Cart and checkout flow specialist | commerce-carts |
| `inventory` | Stock management specialist | commerce-inventory |
| `returns` | Return processing specialist | commerce-returns |
| `analytics` | Business intelligence & forecasting | commerce-analytics |
| `promotions` | Promotions & discounts specialist | commerce-promotions |
| `subscriptions` | Subscription & recurring billing | commerce-subscriptions |
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
| `commerce-promotions` | Promotion types, coupon codes, discount rules |
| `commerce-subscriptions` | Subscription plans, billing cycles, lifecycle management |

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

### Tax Calculation
```bash
stateset "calculate tax for an order shipping to California"
stateset "what's the tax rate for New York?"
stateset "show me all tax jurisdictions"
stateset "get tax info for Texas"
stateset "what are the EU VAT rates?"
stateset "calculate tax for order with items to Berlin, Germany"
stateset --apply "create tax exemption for customer abc123 - resale certificate"

# Cart tax calculation (integrated checkout)
stateset "calculate tax for cart CART-123456"  # Uses cart's shipping address
```

### Promotions & Discounts
```bash
# View promotions
stateset "show me all active promotions"
stateset "list promotions by type percentage_off"
stateset "get details for promotion <id>"

# Create promotions
stateset --apply "create a 20% off promotion called Summer Sale"
stateset --apply "create a $10 off promotion for orders over $50"
stateset --apply "create a free shipping promotion"
stateset --apply "create a buy 2 get 1 free promotion"

# Manage promotions
stateset --apply "activate promotion <id>"
stateset --apply "pause the Summer Sale promotion"

# Coupon codes
stateset "list all coupon codes"
stateset "is coupon SAVE20 valid?"
stateset --apply "create coupon SUMMER25 for promotion <id> with 100 use limit"

# Apply to cart
stateset --apply "apply promotions to cart <cart-id>"
```

### Subscriptions
```bash
# View subscription plans
stateset "show me all subscription plans"
stateset "list active plans"
stateset "get details for plan <id>"

# Create subscription plans
stateset --apply "create a monthly plan called Coffee Club at $29.99 with 14 day trial"
stateset --apply "create an annual plan called Pro Membership at $99.99"
stateset --apply "activate plan <id>"

# Manage subscriptions
stateset "show me all active subscriptions"
stateset "list subscriptions for customer <id>"
stateset --apply "subscribe customer <id> to the Coffee Club plan"

# Lifecycle operations
stateset --apply "pause subscription <id>"
stateset --apply "resume subscription <id>"
stateset --apply "cancel subscription <id>"
stateset --apply "skip next billing for subscription <id>"

# View history
stateset "show billing history for subscription <id>"
stateset "get events for subscription <id>"
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
│   ├── stateset-returns.js   # Returns agent
│   ├── stateset-promotions.js # Promotions agent
│   └── stateset-subscriptions.js # Subscriptions agent
├── src/
│   ├── claude-harness.js     # Multi-agent SDK integration
│   ├── mcp-server.js         # MCP tools (63 total)
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
