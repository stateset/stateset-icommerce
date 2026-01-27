# StateSet CLI Reference

Quick reference for all StateSet Commerce CLI commands.

> **Note:** Read operations don't need flags. Write operations require `--apply`.

## Table of Contents

- [Getting Started](#getting-started)
- [Customers](#customers)
- [Products](#products)
- [Vector Search](#vector-search)
- [Inventory](#inventory)
- [Orders](#orders)
- [Carts](#carts)
- [Returns](#returns)
- [Payments](#payments)
- [Subscriptions](#subscriptions)
- [Promotions](#promotions)
- [Analytics](#analytics)
- [Sync Commands](#sync-commands)
- [Environment Variables](#environment-variables)

---

## Getting Started

```bash
# Basic usage
stateset "your question or command"
stateset --apply "command that modifies data"

# Specify database
stateset --db ./store.db "list products"

# Help
stateset --help
stateset-sync --help
```

---

## Customers

### Read Operations

```bash
# List all customers
stateset "list customers"
stateset "show all customers"

# Find customer
stateset "find customer alice@example.com"
stateset "show customer CUST-123"

# Search
stateset "search customers named Alice"
stateset "customers who ordered in last 30 days"

# Count
stateset "how many customers do we have?"
```

### Write Operations

```bash
# Create customer
stateset --apply "create customer alice@example.com Alice Smith"
stateset --apply "create customer bob@example.com Bob Jones +1-555-0123"

# Update customer
stateset --apply "update customer CUST-123 email newemail@example.com"
stateset --apply "update customer CUST-123 phone +1-555-9999"

# Delete customer
stateset --apply "delete customer CUST-123"
```

---

## Products

### Read Operations

```bash
# List products
stateset "list products"
stateset "show all products"

# Find product
stateset "show product PROD-123"
stateset "find product by sku WBH-001"

# Search
stateset "search products named widget"
stateset "products under $50"
stateset "products in category electronics"

# Count
stateset "how many products do we have?"
```

### Write Operations

```bash
# Create product
stateset --apply "create product 'Widget Pro' SKU-001 29.99"
stateset --apply "create product 'Gadget' SKU-002 49.99 'Product description here'"

# Update product
stateset --apply "update product PROD-123 price 34.99"
stateset --apply "update product PROD-123 name 'Widget Pro Max'"

# Activate/deactivate
stateset --apply "deactivate product PROD-123"
stateset --apply "activate product PROD-123"

# Delete product
stateset --apply "delete product PROD-123"
```

---

## Vector Search

Hybrid semantic + BM25 search is available when `OPENAI_API_KEY` is set. If SQLite
FTS5 isn't available, it falls back to embedding-only search.

```bash
# Find similar products/customers/orders/inventory
stateset "find products similar to wireless earbuds"
stateset "search customers like enterprise retail buyers"
stateset "find orders mentioning backorder or late shipment"
stateset "find inventory items like outdoor gear"
```

---

## Inventory

### Read Operations

```bash
# Check stock levels
stateset "stock level for SKU-001"
stateset "what's the inventory for WBH-001?"
stateset "show inventory levels"

# Low stock
stateset "which products are low on stock?"
stateset "products below reorder point"

# Stock history
stateset "inventory history for SKU-001"
```

### Write Operations

```bash
# Create inventory item
stateset --apply "create inventory item SKU-001 'Widget' 100 units"

# Add stock
stateset --apply "add 50 units of SKU-001"
stateset --apply "add 100 units of SKU-001 to inventory"

# Adjust stock
stateset --apply "adjust inventory SKU-001 by -10 reason 'Damaged'"
stateset --apply "adjust inventory SKU-001 to 95 reason 'Physical count'"

# Reserve stock
stateset --apply "reserve 5 units of SKU-001 for order ORD-123"

# Confirm/release reservation
stateset --apply "confirm reservation RES-123"
stateset --apply "release reservation RES-123"

# Set reorder point
stateset --apply "set reorder point for SKU-001 to 25"
stateset --apply "set safety stock for SKU-001 to 10"

# Transfer (multi-location)
stateset --apply "transfer 20 units of SKU-001 from warehouse-a to warehouse-b"
```

---

## Orders

### Read Operations

```bash
# List orders
stateset "list orders"
stateset "show recent orders"
stateset "show pending orders"

# Find order
stateset "show order ORD-123"
stateset "find order ORD-2024-001234"

# Filter orders
stateset "orders for customer alice@example.com"
stateset "orders from last 7 days"
stateset "shipped orders"
stateset "orders over $100"

# Order items
stateset "show items in order ORD-123"
```

### Write Operations

```bash
# Create order
stateset --apply "create order for alice@example.com with 2x SKU-001"
stateset --apply "create order for CUST-123 with 1x SKU-001, 2x SKU-002"

# Update status
stateset --apply "confirm order ORD-123"
stateset --apply "process order ORD-123"
stateset --apply "ship order ORD-123"
stateset --apply "ship order ORD-123 with tracking FEDEX123456"
stateset --apply "deliver order ORD-123"

# Cancel order
stateset --apply "cancel order ORD-123"
stateset --apply "cancel order ORD-123 reason 'Customer request'"

# Add note
stateset --apply "add note to order ORD-123: 'Gift wrap requested'"
```

---

## Carts

### Read Operations

```bash
# List carts
stateset "list carts"
stateset "show active carts"
stateset "show abandoned carts"

# View cart
stateset "show cart CART-123"
stateset "what's in cart CART-123?"
```

### Write Operations

```bash
# Create cart
stateset --apply "create cart for alice@example.com"
stateset --apply "create cart"  # Anonymous cart

# Add items
stateset --apply "add 2x SKU-001 to cart CART-123"
stateset --apply "add SKU-002 to cart CART-123"

# Update quantity
stateset --apply "update cart CART-123 item SKU-001 quantity 3"

# Remove items
stateset --apply "remove SKU-001 from cart CART-123"

# Set shipping
stateset --apply "set shipping on cart CART-123 to '123 Main St, City, ST 12345'"

# Apply discount
stateset --apply "apply coupon SAVE20 to cart CART-123"

# Checkout
stateset --apply "checkout cart CART-123"

# Abandon/cancel
stateset --apply "abandon cart CART-123"
stateset --apply "cancel cart CART-123"
```

---

## Returns

### Read Operations

```bash
# List returns
stateset "list returns"
stateset "show pending returns"
stateset "show approved returns"

# View return
stateset "show return RET-123"

# Filter
stateset "returns for order ORD-123"
stateset "returns from last 30 days"
```

### Write Operations

```bash
# Create return
stateset --apply "create return for order ORD-123 reason 'defective'"
stateset --apply "create return for order ORD-123 reason 'wrong_item' notes 'Received blue instead of red'"

# Approve/reject
stateset --apply "approve return RET-123"
stateset --apply "reject return RET-123 reason 'Outside return window'"

# Process refund
stateset --apply "refund return RET-123"
stateset --apply "refund return RET-123 amount 25.00"

# Receive items
stateset --apply "receive return RET-123"
stateset --apply "restock return RET-123"
```

**Return Reasons:** `defective`, `wrong_item`, `not_as_described`, `changed_mind`, `damaged`, `other`

---

## Payments

### Read Operations

```bash
# List payments
stateset "list payments"
stateset "payments for order ORD-123"

# View payment
stateset "show payment PAY-123"

# Filter
stateset "pending payments"
stateset "failed payments"
stateset "refunded payments"
```

### Write Operations

```bash
# Create payment
stateset --apply "create payment for order ORD-123 amount 99.99 via credit_card"
stateset --apply "create payment for order ORD-123 amount 99.99 via paypal"

# Complete payment
stateset --apply "complete payment PAY-123"

# Refund
stateset --apply "refund payment PAY-123"
stateset --apply "refund payment PAY-123 amount 25.00"  # Partial refund
```

**Payment Methods:** `credit_card`, `debit_card`, `paypal`, `apple_pay`, `google_pay`, `bank_transfer`, `crypto`

---

## Subscriptions

### Read Operations

```bash
# List plans
stateset "list subscription plans"
stateset "show plan PLAN-123"

# List subscriptions
stateset "list subscriptions"
stateset "show subscription SUB-123"
stateset "subscriptions for customer CUST-123"

# Billing
stateset "billing history for SUB-123"
stateset "subscriptions renewing this week"
```

### Write Operations

```bash
# Create plan
stateset --apply "create subscription plan 'Pro Monthly' 29.99 per month"
stateset --apply "create subscription plan 'Pro Annual' 299.99 per year"
stateset --apply "create subscription plan 'Starter' 9.99 per month trial_days 14"

# Manage plans
stateset --apply "activate plan PLAN-123"
stateset --apply "archive plan PLAN-123"

# Subscribe customer
stateset --apply "subscribe alice@example.com to 'Pro Monthly'"
stateset --apply "subscribe CUST-123 to plan PLAN-123"

# Manage subscription
stateset --apply "pause subscription SUB-123"
stateset --apply "resume subscription SUB-123"
stateset --apply "cancel subscription SUB-123"
stateset --apply "skip next billing for SUB-123"
```

---

## Promotions

### Read Operations

```bash
# List promotions
stateset "list promotions"
stateset "show active promotions"

# View promotion
stateset "show promotion PROMO-123"

# Validate coupon
stateset "is coupon SAVE20 valid?"
stateset "check coupon SUMMER20 for cart CART-123"

# List coupons
stateset "list coupons"
stateset "coupons for promotion PROMO-123"
```

### Write Operations

```bash
# Create promotion
stateset --apply "create promotion SUMMER20 '20% Summer Sale' type percentage value 20"
stateset --apply "create promotion SAVE10 'Save \$10' type fixed value 10"
stateset --apply "create promotion FREESHIP 'Free Shipping' type free_shipping minimum 50"

# Set rules
stateset --apply "set promotion SUMMER20 active from 2024-06-01 to 2024-08-31"
stateset --apply "set promotion SUMMER20 minimum order 25"
stateset --apply "set promotion SUMMER20 max uses 1000"

# Activate/deactivate
stateset --apply "activate promotion PROMO-123"
stateset --apply "deactivate promotion PROMO-123"

# Create coupon
stateset --apply "create coupon FRIEND20 for promotion SUMMER20"
stateset --apply "create coupon VIP50 for promotion SUMMER20 single_use true"

# Apply to cart
stateset --apply "apply coupon SAVE20 to cart CART-123"
stateset --apply "remove coupon from cart CART-123"
```

---

## Analytics

### Sales

```bash
# Revenue
stateset "what's my revenue today?"
stateset "revenue this week"
stateset "revenue this month"
stateset "revenue this year"
stateset "compare revenue this month vs last month"

# Sales summary
stateset "sales summary for today"
stateset "sales summary for last 30 days"
```

### Products

```bash
# Top products
stateset "top 10 selling products"
stateset "top products by revenue"
stateset "top products this month"

# Product performance
stateset "how is product SKU-001 performing?"
stateset "products with no sales in 90 days"
```

### Customers

```bash
# Top customers
stateset "who are my top customers?"
stateset "top customers by revenue"
stateset "top customers by order count"

# Metrics
stateset "average customer lifetime value"
stateset "new customers this month"
stateset "customer retention rate"
```

### Orders

```bash
# Order metrics
stateset "order status breakdown"
stateset "average order value"
stateset "orders per day this week"
stateset "average time to ship"
```

### Inventory

```bash
# Inventory health
stateset "inventory health report"
stateset "products below reorder point"
stateset "stockout risk this week"
stateset "dead stock report"
```

### Forecasting

```bash
# Demand forecast
stateset "forecast demand for next 30 days"
stateset "demand forecast for SKU-001"

# Revenue forecast
stateset "project revenue for next quarter"
```

---

## Sync Commands

```bash
# Initialize sync
stateset-sync init \
  --sequencer-url http://localhost:8080 \
  --tenant-id YOUR_TENANT_ID \
  --store-id YOUR_STORE_ID \
  --api-key YOUR_API_KEY \
  --db ./store.db

# Push/pull events
stateset-sync push              # Push local events to sequencer
stateset-sync pull              # Pull remote events
stateset-sync status            # Check sync status

# Verify events
stateset-sync verify <event-id> # Verify event inclusion proof
stateset-sync history           # Show sync history

# Conflict resolution
stateset-sync conflicts         # List conflicts
stateset-sync rebase            # Resolve conflicts

# Key management
stateset-sync keys:generate     # Generate signing keys
stateset-sync keys:register     # Register with sequencer
stateset-sync keys:rotate       # Rotate keys
stateset-sync keys:expiry       # Check expiration

# Encryption groups
stateset-sync groups:create --name "team-name"
stateset-sync groups:add-member --group-id ID --agent-id ID
stateset-sync groups:list
```

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STATESET_DB` | Database path | `./store.db` |
| `STATESET_SEQUENCER_URL` | Sequencer URL | `http://localhost:8080` |
| `STATESET_TENANT_ID` | Tenant ID | - |
| `STATESET_STORE_ID` | Store ID | - |
| `STATESET_API_KEY` | API key | - |
| `ANTHROPIC_API_KEY` | Claude API key (for AI mode) | - |
| `OPENAI_API_KEY` | OpenAI API key (for vector search embeddings) | - |
| `STATESET_TIMEOUT` | Request timeout (ms) | `30000` |
| `DEBUG` | Enable debug logging | - |

```bash
# Set environment variables
export STATESET_DB=./store.db
export STATESET_SEQUENCER_URL=http://localhost:8080
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...

# Now commands use these defaults
stateset "list products"
stateset-sync push
```

---

## Command Patterns

### Read (no flag needed)
```bash
stateset "list X"
stateset "show X"
stateset "find X"
stateset "search X"
stateset "count X"
stateset "how many X"
stateset "what is X"
```

### Write (requires --apply)
```bash
stateset --apply "create X"
stateset --apply "update X"
stateset --apply "delete X"
stateset --apply "add X"
stateset --apply "remove X"
stateset --apply "set X"
stateset --apply "cancel X"
stateset --apply "approve X"
stateset --apply "reject X"
```

---

## Quick Examples

```bash
# Full checkout flow
stateset --apply "create customer alice@example.com Alice Smith"
stateset --apply "create product 'Widget' WDG-001 29.99"
stateset --apply "add 100 units of WDG-001"
stateset --apply "create cart for alice@example.com"
stateset --apply "add 2x WDG-001 to cart CART-123"
stateset --apply "checkout cart CART-123"
stateset --apply "ship order ORD-123 tracking FEDEX123"

# Check business health
stateset "revenue today"
stateset "pending orders"
stateset "low stock items"
stateset "top 5 products"

# Sync with sequencer
stateset-sync push
stateset-sync status
```

---

**More Help:** `stateset --help` | [Workflows](./workflows.md) | [Troubleshooting](./troubleshooting.md)
