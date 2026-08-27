# StateSet iCommerce CLI - Project Guidance

An AI-powered command-line interface for commerce operations using the Claude Agent SDK.

## Overview

This CLI provides natural language access to commerce operations across
**73 tool domains (802 MCP tools)** — see [docs/TOOLS.md](../docs/TOOLS.md) for the
full generated catalog. Major areas:
- **Core commerce** - Customers, orders, products, inventory, returns, shipments
- **Carts/Checkout** - Protocol-neutral shopping cart and checkout flow
- **Payments** - Payments, refunds, stablecoin, x402, treasury
- **Finance suite** - General ledger, AP (bills, 3-way match), AR (credit memos, aging), fixed assets, revenue recognition, cost accounting, credit, prepayments, vendor credits
- **Warehouse (WMS)** - Warehouses/locations, fulfillment waves, pick tasks, receiving, cycle counts, backorders
- **Traceability & quality** - Lots, serials, inspections, NCRs, quality holds
- **Supply chain** - Suppliers/POs, supplier SKUs, inbound shipments, transfer orders, production batches, price schedules/levels, EDI documents
- **Growth** - Analytics, promotions, subscriptions, loyalty, gift cards, segments
- **Agent commerce** - A2A, agent cards, x402, ERC-8004, policies, audit/proofs

## Entry Points

| Command | Description |
|---------|-------------|
| `stateset "<request>"` | AI-powered interface (auto-routes to best agent) |
| `stateset-chat` | Multi-turn interactive REPL |
| `stateset-direct <resource> <action>` | Direct CLI (no AI) |

### Specialized Agent Commands

| Command | Agent | Description |
|---------|-------|-------------|
| `stateset-checkout` | checkout | Engine-native shopping cart and checkout flow |
| `stateset-orders` | orders | Order lifecycle management |
| `stateset-inventory` | inventory | Stock & reservation management |
| `stateset-returns` | returns | RMA & refund processing |
| `stateset-analytics` | analytics | Sales metrics & forecasting |
| `stateset-promotions` | promotions | Promotions, discounts & coupons |
| `stateset-subscriptions` | subscriptions | Subscription plans & recurring billing |
| `stateset-create` | storefront | Create e-commerce storefronts |
| `stateset-pay` | stablecoin | Native stablecoin payments (USDC, ssUSD) |

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

The engine exposes **802 tools across 73 domains**. The sections below highlight the
major domains with representative tools only — the complete, generated catalog lives in
[docs/TOOLS.md](../docs/TOOLS.md) (regenerate with `npm run docs:tools`; do not edit by hand).

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

### commerce-carts (protocol-neutral engine API)
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

### commerce-stablecoin (Native Crypto Payments)
- `get_agent_wallet` - Get agent wallet address for a blockchain
- `get_wallet_balance` - Check stablecoin balance on a chain
- `create_stablecoin_payment` - Send stablecoin payment (requires --apply)
- `list_supported_chains` - List supported blockchains (Solana, SET Chain, Base, etc.)

### commerce-x402 (AI Agent Protocol Payments)
- `x402_create_payment_intent` - Create x402 payment intent for AI agents
- `x402_sign_intent` - Sign payment intent with Ed25519 (requires --apply)
- `x402_get_intent` - Get payment intent details
- `x402_list_intents` - List payment intents with filters
- `x402_mark_settled` - Mark intent as settled on-chain (requires --apply)
- `x402_get_next_nonce` - Get next nonce for replay protection

### commerce-agent-cards (A2A Commerce)
- `register_agent_card` - Register AI agent for A2A commerce (requires --apply)
- `discover_agents` - Find agents with specific capabilities
- `get_agent_card` - Get agent card details
- `verify_agent` - Verify agent trust level (requires --apply)
- `list_agent_cards` - List all registered agents

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

### commerce-general-ledger (Finance)
- `get_trial_balance` / `get_balance_sheet` / `get_income_statement` - Financial statements
- `post_journal_entry` - Post a balanced journal entry (requires --apply)
- `close_month` - Full month-end close: depreciation, rev rec, FX revaluation, period close; supports `dryRun` (requires --apply)
- `initialize_chart_of_accounts` - Seed a standard chart of accounts (requires --apply)

Example: `stateset "run a dry-run month-end close for period 2026-07"`

### commerce-accounts-payable (Finance)
- `list_bills` / `create_bill` / `approve_bill` - Vendor bill lifecycle
- `three_way_match_bill` - Match bill vs purchase order vs receipt (with tolerance)
- `get_accounts_payable_aging_summary` - AP aging buckets
- `list_overdue_bills` / `list_bills_due_soon` - Payment planning

Example: `stateset "run a three-way match on bill BILL-123 with 2% tolerance"`

### commerce-accounts-receivable (Finance)
- `get_accounts_receivable_aging_summary` - AR aging buckets
- `get_days_sales_outstanding` - DSO metric
- `create_credit_memo` / `void_credit_memo` - Credit memos (requires --apply)
- `list_unapplied_credits` - Credits awaiting application

Example: `stateset "show me the AR aging summary and our DSO"`

### commerce-fixed-assets (Finance)
- `create_fixed_asset` / `place_asset_in_service` - Asset lifecycle (requires --apply)
- `generate_depreciation_schedule` / `post_depreciation` - Depreciation (requires --apply)
- `dispose_fixed_asset` / `write_off_fixed_asset` - Disposals (requires --apply)

Example: `stateset --apply "post depreciation for all assets through June"`

### commerce-revenue-recognition (Finance)
- `create_revenue_contract` - ASC 606-style contract (requires --apply)
- `generate_revenue_schedule` / `get_revenue_schedule` - Recognition schedules
- `recognize_revenue` - Recognize revenue through a date (requires --apply)

Example: `stateset "show the revenue schedule for contract RC-100"`

### commerce-cost-accounting & commerce-credit (Finance)
- `set_item_cost` / `update_average_item_cost` - Item costing (requires --apply)
- `get_total_inventory_value` - Inventory valuation
- `check_customer_credit` / `adjust_credit_limit` - Customer credit management
- `list_over_limit_credit_accounts` - Credit exposure review

Example: `stateset "what's our total inventory value at average cost?"`

### commerce-prepayments & commerce-vendor-credits (Finance)
- `create_prepayment` / `apply_prepayment` / `refund_prepayment` - Customer prepayments (requires --apply)
- `create_vendor_credit` / `apply_vendor_credit` - Vendor credits against bills (requires --apply)
- `reverse_prepayment_application` / `reverse_vendor_credit_application` - Corrections (requires --apply)

Example: `stateset --apply "apply vendor credit VC-9 to bill BILL-123"`

### commerce-warehouse (WMS)
- `list_warehouses` / `create_warehouse` - Warehouse master data
- `create_location` / `list_pickable_locations` - Bin/location management
- `get_warehouse_sku_available_quantity` - Location-level availability

Example: `stateset "which pickable locations have WIDGET-001 in warehouse WH-1?"`

### commerce-fulfillment (WMS)
- `create_fulfillment_wave` / `release_fulfillment_wave` / `complete_fulfillment_wave` - Wave picking (requires --apply)
- `list_pick_tasks` / `assign_pick_task` / `start_pick_task` - Pick task management
- `check_order_ready_to_pack` / `check_order_ready_to_ship` - Fulfillment gates

Example: `stateset --apply "create a fulfillment wave for today's pending orders and release it"`

### commerce-receiving (WMS)
- `create_receipt_from_purchase_order` - Receive against a PO (requires --apply)
- `start_receiving` / `complete_receiving` - Receipt lifecycle (requires --apply)
- `list_receipts` / `cancel_receipt` - Receipt management

Example: `stateset --apply "create a receipt from purchase order PO-123 and start receiving"`

### commerce-cycle-counts (WMS)
- `create_cycle_count` / `start_cycle_count` - Plan and begin a count (requires --apply)
- `record_cycle_counts` - Record counted quantities (requires --apply)
- `complete_cycle_count` - Post variances and close (requires --apply)

Example: `stateset --apply "start a cycle count for aisle A in warehouse WH-1"`

### commerce-backorders
- `list_backorders` / `create_backorder` / `cancel_backorder` - Backorder lifecycle
- `list_overdue_backorders` / `get_backorder_summary` - Exposure review

Example: `stateset "show me overdue backorders by SKU"`

### commerce-lots & commerce-serials (Traceability)
- `create_lot` / `list_available_lots_for_sku` / `list_expiring_lots` - Lot tracking
- `quarantine_lot` / `release_lot_quarantine` - Lot holds (requires --apply)
- `create_serial` / `mark_serial_sold` / `quarantine_serial` - Serial tracking

Example: `stateset "which lots of SKU MILK-1L expire in the next 30 days?"`

### commerce-quality
- `create_inspection` / `start_inspection` / `complete_inspection` - Inspections (requires --apply)
- `create_ncr` / `close_ncr` - Non-conformance reports (requires --apply)
- `create_quality_hold` / `release_quality_hold` - Quality holds (requires --apply)

Example: `stateset "list active quality holds"`

### commerce-edi-documents
- `list_edi_documents` / `get_edi_document` - EDI document tracking (850/855/856/810, etc.)
- `create_edi_document` / `set_edi_document_status` - Document lifecycle (requires --apply)
- `get_edi_summary` - Volume/status summary by partner and type

Example: `stateset "show me EDI documents received this week and their statuses"`

### commerce-supply-chain (price schedules/levels, transfers, batches, supplier SKUs, inbound)
- `create_price_schedule` / `resolve_scheduled_price` - Date-effective pricing
- `create_price_level` / `set_price_level_entry` - Customer price levels
- `create_transfer_order` / `ship_transfer_order` / `receive_transfer_order_line` - Inter-warehouse transfers (requires --apply)
- `create_production_batch` / `add_production_batch_work_orders` - Batch production (requires --apply)
- `create_supplier_sku` / `bulk_upsert_supplier_skus` - Supplier SKU cross-references (requires --apply)
- `create_inbound_shipment` / `mark_inbound_shipment_arrived` / `receive_inbound_shipment_line` - Inbound logistics (requires --apply)

Example: `stateset --apply "create a transfer order for 50 WIDGET-001 from WH-1 to WH-2"`

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
| `manufacturing` | BOM & work order specialist | commerce-manufacturing |
| `payments` | Payment processing & refunds | commerce-payments |
| `shipments` | Shipment tracking & delivery | commerce-shipments |
| `suppliers` | Supplier & purchase order management | commerce-suppliers |
| `invoices` | B2B invoice management | commerce-invoices |
| `warranties` | Product warranty & claims | commerce-warranties |
| `currency` | Multi-currency & exchange rates | commerce-currency |
| `tax` | Tax calculation & compliance | commerce-tax |
| `sync` | Verifiable Event Sync operations | sync tools |
| `storefront` | Storefront scaffolding | storefront tools |
| `customer-service` | Full customer service agent | All tools |

Agent definitions live in `.claude/agents/` (18 on disk).

## Skills

Domain knowledge documents that enhance agent capabilities:

| Skill | Description |
|-------|-------------|
| `commerce-orders` | Order states, fulfillment workflows |
| `commerce-checkout` | Checkout flow, payment methods |
| `commerce-inventory` | Stock tracking, reservations |
| `commerce-returns` | Return reasons, refund workflows |
| `commerce-analytics` | Sales metrics, forecasting, business intelligence |
| `commerce-storefront` | Storefront scaffolding and templates |
| `commerce-finance` | GL, month-end close, AP/AR, fixed assets, revenue recognition |
| `commerce-warehouse` | Locations, inbound/outbound flows, cycle counts, lots/serials |
| `commerce-edi` | EDI document tracking (850/855/856/810) and status lifecycle |

These nine skills exist in `.claude/skills/`.

## Common Workflows

### Order Fulfillment
```bash
stateset "show me pending orders"
stateset --apply --resume <id> "ship order #12345 with tracking FEDEX123"
```

### Cart Checkout (engine API)
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

### Month-End Close (Finance)
```bash
# Preview the close — nothing is written (close_month with dryRun)
stateset "dry-run the month-end close for period 2026-07"

# Review statements first
stateset "show me the trial balance for period 2026-07"
stateset "show the income statement for this period"

# Execute the close: depreciation, revenue recognition, FX revaluation, period close
stateset --apply "close the month for period 2026-07"
```

### 3-Way Match (Accounts Payable)
```bash
stateset "list bills awaiting approval"
stateset "run a three-way match on bill BILL-123 with 2% tolerance"
stateset --apply "approve bill BILL-123"
stateset "show the AP aging summary"
```

### Cycle Count (WMS)
```bash
stateset --apply "create a cycle count for warehouse WH-1 covering aisle A"
stateset --apply "start cycle count CC-42"
stateset --apply "record counts for CC-42: BIN-A1 WIDGET-001 = 47"
stateset --apply "complete cycle count CC-42"   # posts variances
```

### Purchase Order Lifecycle (Procure to Receive)
```bash
stateset --apply "create a PO for 100 WIDGET-001 from Acme Corp"
stateset --apply "approve purchase order PO-123"
stateset --apply "send purchase order PO-123 to the supplier"
stateset --apply "create a receipt from purchase order PO-123"
stateset --apply "start receiving receipt RCPT-1, then complete it"
stateset "run a three-way match on the bill for PO-123"
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

### Stablecoin Payments (Native Crypto)
```bash
# View supported blockchains
stateset pay --chains

# Show agent wallet addresses
stateset pay --wallet                    # All chains
stateset pay --wallet --chain solana     # Specific chain

# Check stablecoin balance
stateset pay --balance --chain solana
stateset pay --balance --chain set_chain

# Send stablecoin payment (simulation - safe preview)
stateset pay --to 9WzD...WWWM --amount 50.00 --chain solana

# Execute real payment (requires --apply)
stateset pay --apply --to 9WzD...WWWM --amount 50.00 --chain solana

# Pay with ssUSD on SET Chain (yield-bearing stablecoin)
stateset pay --apply --to 0x1234...5678 --amount 100 --chain set_chain

# Include order metadata for audit trail
stateset pay --apply --to 9WzD...WWWM --amount 50.00 --chain solana --order ORD-123 --memo "Widget purchase"

# AI-powered stablecoin payments
stateset --apply "pay 50 USDC to 9WzD...WWWM on Solana"
stateset "check my wallet balance on Solana"
stateset "what chains do we support for payments?"
```

Supported chains:
- `solana` - Solana mainnet (USDC) - Fast, cheap, proven liquidity
- `solana_devnet` - Solana devnet (USDC) - Testing
- `set_chain` - SET Chain L2 (ssUSD) - StateSet native, yield-bearing
- `base` - Base L2 (USDC) - Coinbase, low fees
- `ethereum` - Ethereum mainnet (USDC) - Maximum security
- `arbitrum` - Arbitrum L2 (USDC) - Fast, cheap
- `zcash` - Zcash mainnet (ZEC) - Privacy-focused, transparent t-addresses
- `zcash_testnet` - Zcash testnet (ZEC) - Testing
- `bitcoin` - Bitcoin mainnet (BTC) - Original cryptocurrency, maximum security
- `bitcoin_testnet` - Bitcoin testnet (BTC) - Testing

### x402 Protocol (AI Agent Commerce)
```bash
# Create payment intent (off-chain)
stateset "create x402 payment intent from 0xBuyer to 0xSeller for 100 USDC on Set Chain"

# Get signing hash for agent to sign
stateset "get x402 intent <id>"

# Sign with Ed25519 key (agent signs off-chain)
stateset --apply "sign x402 intent <id> with signature <sig> and public key <pk>"

# After on-chain settlement, mark settled
stateset --apply "mark x402 intent <id> settled with tx 0xTxHash at block 12345"

# List payment intents
stateset "list x402 intents for payer 0xBuyer"
stateset "list settled x402 intents"

# Get next nonce for replay protection
stateset "get next x402 nonce for 0xBuyer"
```

### Agent-to-Agent Commerce (A2A)
```bash
# Register an AI agent card
stateset --apply "register agent card for Widget Seller Bot with wallet 0xSeller on Set Chain"

# Discover agents with capabilities
stateset "discover agents that can sell on Set Chain with USDC"
stateset "find verified buyer agents"

# Get agent details
stateset "get agent card <id>"
stateset "get agent by wallet 0xSeller"

# Verify agent (admin)
stateset --apply "verify agent <id>"

# List all agents
stateset "list all active agents"
stateset "list verified agents"
```

### Permission Sandboxing (v0.3.1)

The HTTP gateway supports API key authentication, per-route permission levels, and sandbox mode.

**Config** (`gateway.config.json`):
```json
{
  "httpGateway": {
    "enabled": true,
    "port": 8080,
    "apiKeys": [
      { "key": "sk-admin-secret", "name": "admin", "level": "admin" },
      { "key": "sk-read-only",    "name": "dashboard", "level": "read" }
    ],
    "sandbox": { "browser": true, "shell": true }
  }
}
```

**Permission levels:** `none` (0) < `read` (1) < `preview` (2) < `write` (3) < `delete` (4) < `admin` (5)

**Auth methods:**
- `Authorization: Bearer <key>` header
- `?api_key=<key>` query param (disabled by default; enable with `httpGateway.allowQueryParamAuth: true`)

```bash
# Authenticated request
curl -H "Authorization: Bearer sk-admin-secret" http://localhost:8080/metrics

# Sandbox blocks dangerous routes even for admin
curl -X POST -H "Authorization: Bearer sk-admin-secret" \
     http://localhost:8080/browser/evaluate
# 403 — blocked by browser sandbox
```

When `apiKeys` is empty or omitted, protected routes return `401 Authentication required` (health/ready remain public).

To run an insecure local “open mode” for debugging, set `httpGateway.allowAnonymous: true` (not recommended).

### Heartbeat Monitor (v0.3.1)

Proactive commerce health checks that emit alerts through EventBridge to all messaging channels.

**Config** (`gateway.config.json`):
```json
{
  "heartbeat": {
    "enabled": true,
    "checks": [
      { "id": "low-stock", "name": "Low Stock", "checker": "low-stock", "intervalMs": 3600000, "enabled": true, "config": { "threshold": 10 } },
      { "id": "abandoned-carts", "name": "Abandoned Carts", "checker": "abandoned-carts", "intervalMs": 86400000, "enabled": true, "config": { "minAgeHours": 24 } }
    ]
  }
}
```

**Built-in checkers:** `low-stock`, `abandoned-carts`, `revenue-milestone`, `pending-returns`, `overdue-invoices`, `subscription-churn`

**HTTP API:**
```bash
# Monitor status
curl -H "Authorization: Bearer $KEY" http://localhost:8080/heartbeat/status

# List all checks
curl -H "Authorization: Bearer $KEY" http://localhost:8080/heartbeat/checks

# Manually run a check
curl -X POST -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/run

# Enable/disable a check at runtime
curl -X POST -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/enable
curl -X POST -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/disable
```

Alerts route through: HeartbeatMonitor -> AutonomousEngine -> EventBridge -> ChannelNotifier -> channels.

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
│   ├── stateset-subscriptions.js # Subscriptions agent
│   └── stateset-pay.js       # Native stablecoin payments
├── src/
│   ├── claude-harness.js     # Multi-agent SDK integration
│   ├── mcp-server.js         # Commerce MCP server (802 tools across 73 domains as of Jul 2026 — see docs/TOOLS.md, generated from src/tools/domain-registry.js via `npm run docs:tools`)
│   ├── chains/               # Blockchain integration
│   │   ├── index.js          # Module exports
│   │   ├── config.js         # Chain & token configurations
│   │   ├── wallet.js         # VES key → wallet derivation
│   │   └── stablecoin.js     # Payment execution & balance
│   └── utils/
├── .claude/
│   ├── CLAUDE.md             # This file
│   ├── agents/               # 18 agent definitions (orders, checkout, inventory,
│   │                         # returns, analytics, promotions, subscriptions,
│   │                         # manufacturing, payments, shipments, suppliers,
│   │                         # invoices, warranties, currency, tax, sync,
│   │                         # storefront, customer-service)
│   └── skills/               # 9 domain knowledge skills (commerce-orders,
│                             # commerce-checkout, commerce-inventory,
│                             # commerce-returns, commerce-analytics,
│                             # commerce-storefront, commerce-finance,
│                             # commerce-warehouse, commerce-edi)
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
