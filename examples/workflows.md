# StateSet Commerce - Common Workflows

Step-by-step guides for real-world commerce scenarios.

## Table of Contents

1. [Complete Checkout Flow](#1-complete-checkout-flow)
2. [Process a Return](#2-process-a-return)
3. [Inventory Management](#3-inventory-management)
4. [Multi-Agent Sync](#4-multi-agent-sync)
5. [Subscription Billing](#5-subscription-billing)
6. [Promotions & Discounts](#6-promotions--discounts)
7. [Analytics & Reporting](#7-analytics--reporting)
8. [B2B Invoice Flow](#8-b2b-invoice-flow)

---

## 1. Complete Checkout Flow

A customer browses products, adds items to cart, and completes checkout.

### Step 1: Create or Identify Customer

```bash
# Check if customer exists
stateset "find customer alice@example.com"

# Or create new customer
stateset --apply "create customer alice@example.com Alice Smith +1-555-0123"
```

### Step 2: Create Shopping Cart

```bash
# Create cart for customer
stateset --apply "create cart for alice@example.com"

# Response includes cart ID, e.g., cart_abc123
```

### Step 3: Add Items to Cart

```bash
# Add products to cart
stateset --apply "add 2x WBH-001 to cart cart_abc123"
stateset --apply "add 1x USB-C-6FT to cart cart_abc123"

# View cart contents
stateset "show cart cart_abc123"
```

### Step 4: Apply Promotions (Optional)

```bash
# Check if customer has valid coupons
stateset "check coupon WELCOME10 for cart cart_abc123"

# Apply coupon
stateset --apply "apply coupon WELCOME10 to cart cart_abc123"
```

### Step 5: Set Shipping Address

```bash
stateset --apply "set shipping address on cart cart_abc123 to '123 Main St, City, ST 12345'"
```

### Step 6: Calculate Totals

```bash
# Get final totals with tax and shipping
stateset "calculate totals for cart cart_abc123"
```

### Step 7: Complete Checkout

```bash
# Convert cart to order
stateset --apply "checkout cart cart_abc123"

# Response: Order created: ORD-2024-001234
```

### Step 8: Process Payment

```bash
# Record payment (in real scenario, integrate with Stripe/etc)
stateset --apply "create payment for order ORD-2024-001234 amount 172.97 via credit_card"
```

### Step 9: Fulfill Order

```bash
# Confirm order
stateset --apply "confirm order ORD-2024-001234"

# Ship order
stateset --apply "ship order ORD-2024-001234 with tracking FEDEX123456789"

# Mark delivered
stateset --apply "deliver order ORD-2024-001234"
```

### Sync to Sequencer

```bash
# Push all events to sequencer
stateset-sync push
```

---

## 2. Process a Return

Customer wants to return items from an order.

### Step 1: Find the Original Order

```bash
stateset "show order ORD-2024-001234"
stateset "list items in order ORD-2024-001234"
```

### Step 2: Create Return Request

```bash
# Create return for specific reason
stateset --apply "create return for order ORD-2024-001234 reason 'defective' notes 'Headphones stopped working after 2 days'"
```

### Step 3: Review Return

```bash
# View return details
stateset "show return RET-2024-000456"

# List all pending returns
stateset "show pending returns"
```

### Step 4: Approve or Reject

```bash
# Approve the return
stateset --apply "approve return RET-2024-000456"

# Or reject with reason
stateset --apply "reject return RET-2024-000456 reason 'Outside return window'"
```

### Step 5: Process Refund

```bash
# Create refund for approved return
stateset --apply "refund return RET-2024-000456 amount 79.99"
```

### Step 6: Receive Returned Items

```bash
# When items arrive, add back to inventory
stateset --apply "adjust inventory WBH-001 by +1 reason 'Return received RET-2024-000456'"
```

---

## 3. Inventory Management

Track stock levels, handle low stock alerts, and manage reservations.

### Check Current Stock

```bash
# Single product
stateset "what's the stock level for WBH-001?"

# All products
stateset "show inventory levels"

# Low stock items
stateset "which products are low on stock?"
```

### Receive New Inventory

```bash
# Add stock from supplier
stateset --apply "adjust inventory WBH-001 by +100 reason 'PO-2024-0001 received'"

# Bulk receive
stateset --apply "receive inventory: WBH-001 +100, USB-C-6FT +500, PPB-10K +200"
```

### Reserve Inventory

```bash
# Reserve stock for an order (automatic during checkout)
# Manual reservation:
stateset --apply "reserve 5 units of WBH-001 for order ORD-2024-001234"

# Confirm reservation (deducts from available)
stateset --apply "confirm reservation RES-abc123"

# Release reservation (if order cancelled)
stateset --apply "release reservation RES-abc123"
```

### Transfer Between Locations

```bash
# If using multiple warehouses
stateset --apply "transfer 50 units of WBH-001 from warehouse-east to warehouse-west"
```

### Set Reorder Points

```bash
# Get alerted when stock falls below threshold
stateset --apply "set reorder point for WBH-001 to 25 units"
stateset --apply "set safety stock for WBH-001 to 10 units"
```

### Inventory Audit

```bash
# Physical count adjustment
stateset --apply "adjust inventory WBH-001 to 147 reason 'Physical count audit'"
```

---

## 4. Multi-Agent Sync

Set up multiple agents (e.g., warehouse, storefront) syncing to the same store.

### Agent 1: Storefront (handles orders)

```bash
# Initialize on storefront server
stateset-sync init \
  --sequencer-url http://sequencer.example.com:8080 \
  --tenant-id $TENANT_ID \
  --store-id $STORE_ID \
  --api-key $API_KEY \
  --db ./storefront.db

stateset-sync keys:generate
stateset-sync keys:register

# Process orders
stateset --db ./storefront.db --apply "create order for customer@example.com..."

# Push events
stateset-sync push
```

### Agent 2: Warehouse (handles inventory)

```bash
# Initialize on warehouse server
stateset-sync init \
  --sequencer-url http://sequencer.example.com:8080 \
  --tenant-id $TENANT_ID \
  --store-id $STORE_ID \
  --api-key $API_KEY \
  --db ./warehouse.db

stateset-sync keys:generate
stateset-sync keys:register

# Pull orders from storefront
stateset-sync pull

# Process fulfillment
stateset --db ./warehouse.db --apply "ship order ORD-2024-001234 tracking FEDEX123"

# Push updates
stateset-sync push
```

### Create Encryption Group (for sensitive data)

```bash
# On primary agent
stateset-sync groups:create --name "fulfillment-team"

# Get agent IDs
AGENT_1_ID=$(cat .stateset/sync.json | jq -r '.agentId')

# Add warehouse agent to group
stateset-sync groups:add-member --group-id $GROUP_ID --agent-id $WAREHOUSE_AGENT_ID

# Now both can encrypt/decrypt shared events
```

### Handle Conflicts

```bash
# Check for conflicts after pull
stateset-sync conflicts

# Resolve conflicts
stateset-sync rebase --strategy remote-wins  # Accept remote changes
# or
stateset-sync rebase --strategy local-wins   # Keep local changes
# or
stateset-sync rebase --strategy manual       # Resolve manually
```

---

## 5. Subscription Billing

Set up recurring billing for subscription products.

### Create Subscription Plans

```bash
# Monthly plan
stateset --apply "create subscription plan 'Pro Monthly' price 29.99 interval month"

# Annual plan with discount
stateset --apply "create subscription plan 'Pro Annual' price 299.99 interval year"

# Plan with trial
stateset --apply "create subscription plan 'Starter' price 9.99 interval month trial_days 14"
```

### Subscribe a Customer

```bash
# Subscribe customer to plan
stateset --apply "subscribe alice@example.com to 'Pro Monthly' plan"

# With specific start date
stateset --apply "subscribe alice@example.com to 'Pro Annual' starting 2024-01-01"
```

### Manage Subscriptions

```bash
# View subscription details
stateset "show subscription for alice@example.com"

# Pause subscription
stateset --apply "pause subscription SUB-abc123"

# Resume subscription
stateset --apply "resume subscription SUB-abc123"

# Cancel subscription
stateset --apply "cancel subscription SUB-abc123"

# Skip next billing cycle
stateset --apply "skip next billing cycle for subscription SUB-abc123"
```

### View Billing History

```bash
# Billing cycles
stateset "show billing history for subscription SUB-abc123"

# Upcoming renewals
stateset "which subscriptions renew this week?"
```

---

## 6. Promotions & Discounts

Create and manage promotional campaigns.

### Create Promotions

```bash
# Percentage discount
stateset --apply "create promotion SUMMER20 '20% Summer Sale' type percentage value 20"

# Fixed amount discount
stateset --apply "create promotion SAVE10 'Save \$10' type fixed value 10"

# Free shipping
stateset --apply "create promotion FREESHIP 'Free Shipping' type free_shipping minimum_order 50"

# Buy X Get Y
stateset --apply "create promotion BOGO 'Buy One Get One' type bogo buy_quantity 1 get_quantity 1"
```

### Set Promotion Rules

```bash
# Date range
stateset --apply "set promotion SUMMER20 active from 2024-06-01 to 2024-08-31"

# Minimum order value
stateset --apply "set promotion SUMMER20 minimum order 25"

# Product restrictions
stateset --apply "set promotion SUMMER20 applies to category 'electronics'"

# Usage limits
stateset --apply "set promotion SUMMER20 max uses 1000"
stateset --apply "set promotion SUMMER20 max uses per customer 1"
```

### Create Coupon Codes

```bash
# Single-use coupon
stateset --apply "create coupon ALICE10 for promotion SUMMER20 single_use true"

# Multi-use coupon
stateset --apply "create coupon FRIENDS20 for promotion SUMMER20 max_uses 100"
```

### Apply to Cart

```bash
# Validate coupon
stateset "is coupon SUMMER20 valid for cart cart_abc123?"

# Apply coupon
stateset --apply "apply coupon SUMMER20 to cart cart_abc123"

# View discount
stateset "show discounts on cart cart_abc123"
```

---

## 7. Analytics & Reporting

Get insights into your commerce operations.

### Sales Metrics

```bash
# Today's summary
stateset "what's my revenue today?"

# This week/month/year
stateset "show sales summary for this month"

# Compare periods
stateset "compare revenue this month vs last month"
```

### Top Products

```bash
# Best sellers
stateset "what are my top 10 selling products?"

# By revenue
stateset "which products generate the most revenue?"

# By quantity
stateset "which products sell the most units?"
```

### Customer Analytics

```bash
# Top customers
stateset "who are my top customers by revenue?"

# Customer lifetime value
stateset "what's the average customer lifetime value?"

# New vs returning
stateset "how many new customers this month?"
```

### Inventory Health

```bash
# Low stock alerts
stateset "which products are below reorder point?"

# Stockout risk
stateset "which products might stock out this week?"

# Dead stock
stateset "which products haven't sold in 90 days?"
```

### Order Metrics

```bash
# Order status breakdown
stateset "show order status breakdown"

# Average order value
stateset "what's my average order value?"

# Fulfillment time
stateset "what's my average time to ship?"
```

### Forecasting

```bash
# Demand forecast
stateset "forecast demand for next 30 days"

# Revenue projection
stateset "project revenue for next quarter"
```

---

## 8. B2B Invoice Flow

Handle business-to-business transactions with invoices.

### Create Invoice

```bash
# Create invoice for business customer
stateset --apply "create invoice for customer CUST-business-001 due in 30 days"

# Add line items
stateset --apply "add to invoice INV-2024-0001: 100x WBH-001 at 65.00 each"
stateset --apply "add to invoice INV-2024-0001: 50x USB-C-6FT at 10.00 each"
```

### Send Invoice

```bash
# Mark as sent
stateset --apply "send invoice INV-2024-0001"

# Resend
stateset --apply "resend invoice INV-2024-0001"
```

### Track Payment

```bash
# View invoice status
stateset "show invoice INV-2024-0001"

# Record partial payment
stateset --apply "record payment of 3000.00 on invoice INV-2024-0001"

# Record full payment
stateset --apply "mark invoice INV-2024-0001 as paid"
```

### Manage Overdue Invoices

```bash
# List overdue
stateset "show overdue invoices"

# Send reminder
stateset --apply "send reminder for invoice INV-2024-0001"

# Mark as written off
stateset --apply "write off invoice INV-2024-0001 reason 'Uncollectible'"
```

---

## Quick Reference Card

| Task | Command |
|------|---------|
| Create customer | `stateset --apply "create customer email@example.com Name"` |
| Create product | `stateset --apply "create product 'Name' SKU price"` |
| Add inventory | `stateset --apply "add 100 units of SKU"` |
| Create order | `stateset --apply "create order for email with 2x SKU"` |
| Ship order | `stateset --apply "ship order ORD-123 tracking TRACK123"` |
| Create return | `stateset --apply "create return for order ORD-123 reason 'defective'"` |
| Approve return | `stateset --apply "approve return RET-123"` |
| Check stock | `stateset "stock level for SKU"` |
| Revenue today | `stateset "revenue today"` |
| Top products | `stateset "top 10 products"` |
| Sync events | `stateset-sync push` |
| Pull updates | `stateset-sync pull` |

---

## Next Steps

- [Getting Started with Sync](./getting-started-sync.md)
- [Troubleshooting Guide](./troubleshooting.md)
- [API Reference](../docs/api-reference.md)
