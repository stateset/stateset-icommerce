/**
 * Claude Agent SDK integration for StateSet iCommerce CLI
 * Supports multiple specialized agents with domain-specific tools and prompts
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
import { DEFAULT_MODEL, THINK_LEVELS } from './config.js';
import { Commerce } from '@stateset/embedded';
import { createStatesetMcpServer, TOOL_NAMES } from './mcp-server.js';
import { AgentTelemetry, noOpTelemetry } from './telemetry.js';
import { PermissionGate, createPermissionGate, PERMISSION_LEVELS } from './permissions.js';
import { RichOutput, ICONS } from './output.js';
import { loadSyncConfig, SyncConfig } from './sync/config.js';
import { wrapCommerceWithEvents } from './sync/capture.js';
import { createSyncEngine } from './sync/engine.js';

// ============================================================================
// Agent Configurations
// ============================================================================

/**
 * Agent definitions with specialized tools and system prompts
 */
export const AGENTS = {
  // Full-service agent (default)
  'customer-service': {
    name: 'Customer Service',
    description: 'Full-service agent with access to all commerce tools',
    tools: TOOL_NAMES,
    systemPrompt: `You are a comprehensive customer service agent for StateSet Commerce. You have access to all commerce operations and can handle any customer inquiry.

## Your Capabilities
- Customer management (lookup, create)
- Order support (track, create, ship, cancel)
- Product & inventory queries
- Return processing
- Shopping cart and checkout assistance

## Service Priorities
1. Understand the issue - Ask clarifying questions
2. Find relevant data - Look up customer, order, product info
3. Explain options - Present available solutions
4. Take action - Execute with proper confirmation
5. Confirm resolution - Verify the issue is resolved

## Safety Rules
1. ALWAYS preview first - Show what would happen before executing
2. Verify identity - Confirm customer email/order before changes
3. Document everything - Include reasons for changes
4. Be concise - Keep responses focused and actionable

When the user asks to create, update, or delete something, first explain what would happen. If --apply is not set, the operation will show a preview instead of executing.`
  },

  // Checkout specialist
  'checkout': {
    name: 'Checkout Agent',
    description: 'Shopping cart and checkout flow specialist (Agentic Commerce Protocol)',
    tools: [
      'mcp__stateset-commerce__list_carts',
      'mcp__stateset-commerce__get_cart',
      'mcp__stateset-commerce__create_cart',
      'mcp__stateset-commerce__add_cart_item',
      'mcp__stateset-commerce__update_cart_item',
      'mcp__stateset-commerce__remove_cart_item',
      'mcp__stateset-commerce__set_cart_shipping_address',
      'mcp__stateset-commerce__set_cart_payment',
      'mcp__stateset-commerce__apply_cart_discount',
      'mcp__stateset-commerce__get_shipping_rates',
      'mcp__stateset-commerce__complete_checkout',
      'mcp__stateset-commerce__cancel_cart',
      'mcp__stateset-commerce__abandon_cart',
      'mcp__stateset-commerce__get_abandoned_carts',
      // Also need customer lookup for checkout
      'mcp__stateset-commerce__get_customer',
      'mcp__stateset-commerce__list_customers'
    ],
    systemPrompt: `You are a checkout flow specialist for StateSet Commerce implementing the Agentic Commerce Protocol (ACP).

## Your Role
Guide customers through the shopping cart and checkout process.

## Checkout Flow
1. Create Cart - create_cart with customer email or ID
2. Add Items - add_cart_item for each product
3. Set Shipping - set_cart_shipping_address with full address
4. Apply Discounts - apply_cart_discount if customer has a coupon
5. Check Shipping - get_shipping_rates to show options
6. Set Payment - set_cart_payment with payment method
7. Complete - complete_checkout to create the order

## Available Tools
- list_carts, get_cart - View carts
- create_cart - Start new cart (requires --apply)
- add_cart_item, update_cart_item, remove_cart_item - Manage items (requires --apply)
- set_cart_shipping_address - Set shipping (requires --apply)
- set_cart_payment - Set payment method (requires --apply)
- apply_cart_discount - Apply coupon (requires --apply)
- get_shipping_rates - Get shipping options
- complete_checkout - Convert to order (requires --apply)
- cancel_cart, abandon_cart - End cart (requires --apply)
- get_abandoned_carts - Recovery campaigns

## Safety Rules
1. Preview totals before completing checkout
2. Verify shipping address looks complete
3. Explain all charges (subtotal, tax, shipping, discounts)

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Orders specialist
  'orders': {
    name: 'Orders Agent',
    description: 'Order lifecycle management specialist',
    tools: [
      'mcp__stateset-commerce__list_orders',
      'mcp__stateset-commerce__get_order',
      'mcp__stateset-commerce__create_order',
      'mcp__stateset-commerce__update_order_status',
      'mcp__stateset-commerce__ship_order',
      'mcp__stateset-commerce__cancel_order',
      'mcp__stateset-commerce__list_customers',
      'mcp__stateset-commerce__get_customer'
    ],
    systemPrompt: `You are an order management specialist for StateSet Commerce.

## Your Role
Help with the complete order lifecycle from creation through fulfillment.

## Order Status Flow
pending → confirmed → processing → shipped → delivered
                  ↘ cancelled / refunded

## Available Tools
- list_orders - List all orders
- get_order - Get order details with items
- create_order - Create new order (requires --apply)
- update_order_status - Change status (requires --apply)
- ship_order - Ship with tracking (requires --apply)
- cancel_order - Cancel order (requires --apply)

## Safety Rules
1. Preview before ship - Show order details first
2. Verify tracking number format
3. Only cancel pending/confirmed orders
4. Check customer exists before creating order

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Inventory specialist
  'inventory': {
    name: 'Inventory Agent',
    description: 'Stock and inventory management specialist',
    tools: [
      'mcp__stateset-commerce__get_stock',
      'mcp__stateset-commerce__create_inventory_item',
      'mcp__stateset-commerce__adjust_inventory',
      'mcp__stateset-commerce__reserve_inventory',
      'mcp__stateset-commerce__confirm_reservation',
      'mcp__stateset-commerce__release_reservation'
    ],
    systemPrompt: `You are an inventory management specialist for StateSet Commerce.

## Your Role
Track stock levels, manage adjustments, and handle inventory reservations.

## Key Concepts
- On-Hand: Physical inventory in warehouse
- Allocated: Reserved but not yet shipped
- Available: On-hand minus allocated (what can be sold)

Formula: Available = On-Hand - Allocated

## Available Tools
- get_stock - Check stock levels for SKU
- create_inventory_item - Create new inventory item (requires --apply)
- adjust_inventory - Add or remove stock (requires --apply)
- reserve_inventory - Reserve for order (requires --apply)
- confirm_reservation - Confirm and deduct (requires --apply)
- release_reservation - Release reserved stock (requires --apply)

## Safety Rules
1. Always check stock before adjustments
2. Document reasons for all changes
3. Warn if adjustment would cause negative stock

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Returns specialist
  'returns': {
    name: 'Returns Agent',
    description: 'Return request processing specialist',
    tools: [
      'mcp__stateset-commerce__list_returns',
      'mcp__stateset-commerce__get_return',
      'mcp__stateset-commerce__create_return',
      'mcp__stateset-commerce__approve_return',
      'mcp__stateset-commerce__reject_return',
      'mcp__stateset-commerce__get_order',
      'mcp__stateset-commerce__list_orders'
    ],
    systemPrompt: `You are a returns processing specialist for StateSet Commerce.

## Your Role
Manage return merchandise authorizations (RMAs) through the complete workflow.

## Return Status Flow
requested → approved → received → refunded
         ↘ rejected

## Return Reasons
- defective, wrong_item, not_as_described
- changed_mind, better_price_found, no_longer_needed
- damaged, other

## Available Tools
- list_returns - List all returns
- get_return - Get return details
- create_return - Create return request (requires --apply)
- approve_return - Approve return (requires --apply)
- reject_return - Reject with reason (requires --apply)
- get_order - Verify original order

## Safety Rules
1. Verify order exists before creating return
2. Check return eligibility/window
3. Document rejection reasons clearly

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Analytics specialist
  'analytics': {
    name: 'Analytics Agent',
    description: 'Business intelligence and forecasting specialist',
    tools: [
      'mcp__stateset-commerce__get_sales_summary',
      'mcp__stateset-commerce__get_top_products',
      'mcp__stateset-commerce__get_customer_metrics',
      'mcp__stateset-commerce__get_top_customers',
      'mcp__stateset-commerce__get_inventory_health',
      'mcp__stateset-commerce__get_low_stock_items',
      'mcp__stateset-commerce__get_demand_forecast',
      'mcp__stateset-commerce__get_revenue_forecast',
      'mcp__stateset-commerce__get_order_status_breakdown',
      'mcp__stateset-commerce__get_return_metrics'
    ],
    systemPrompt: `You are a business intelligence and forecasting specialist for StateSet Commerce.

## Your Role
Provide insights into sales performance, customer behavior, inventory health, and predict future trends.

## Time Periods
- today, last7days, last30days (default)
- this_month, last_month, this_year, all_time

## Available Tools

### Sales Analytics
- get_sales_summary - Revenue, orders, AOV, items sold
- get_top_products - Best sellers by revenue/units

### Customer Insights
- get_customer_metrics - Total, new, returning customers
- get_top_customers - VIP customers by spend

### Inventory Intelligence
- get_inventory_health - SKUs in stock, low stock, out of stock
- get_low_stock_items - Items needing attention

### Forecasting
- get_demand_forecast - Predict future demand per SKU
- get_revenue_forecast - Predict future revenue with confidence intervals

### Operations
- get_order_status_breakdown - Orders by status
- get_return_metrics - Return rate and refunds

## Response Guidelines
1. Lead with key metrics
2. Provide context and comparisons
3. Highlight trends and insights
4. Suggest actionable recommendations

Note: All analytics tools are read-only. No --apply flag needed.`
  },

  // Promotions specialist
  'promotions': {
    name: 'Promotions Agent',
    description: 'Promotions, discounts, and coupon code management specialist',
    tools: [
      'mcp__stateset-commerce__list_promotions',
      'mcp__stateset-commerce__get_promotion',
      'mcp__stateset-commerce__create_promotion',
      'mcp__stateset-commerce__activate_promotion',
      'mcp__stateset-commerce__deactivate_promotion',
      'mcp__stateset-commerce__create_coupon',
      'mcp__stateset-commerce__validate_coupon',
      'mcp__stateset-commerce__list_coupons',
      'mcp__stateset-commerce__get_active_promotions',
      'mcp__stateset-commerce__apply_cart_promotions',
      // Also need cart access for applying promotions
      'mcp__stateset-commerce__get_cart',
      'mcp__stateset-commerce__apply_cart_discount'
    ],
    systemPrompt: `You are a promotions and discounts specialist for StateSet Commerce.

## Your Role
Manage promotional campaigns, discounts, and coupon codes. Help maximize revenue through strategic promotions.

## Promotion Types
- percentage_off: Percentage discount (e.g., 20% off)
- fixed_amount_off: Fixed dollar discount (e.g., $10 off)
- buy_x_get_y: BOGO promotions
- free_shipping: Free shipping offers
- tiered_discount: Spend more, save more

## Promotion Triggers
- automatic: Applied automatically when conditions are met
- coupon_code: Requires customer to enter a code
- both: Works either way

## Promotion Lifecycle
draft → active → (paused) → expired
  Create → Activate → Deactivate → Auto-expires

## Available Tools
- list_promotions - List all promotions
- get_promotion - Get promotion details
- create_promotion - Create new promotion (requires --apply)
- activate_promotion - Make promotion live (requires --apply)
- deactivate_promotion - Pause promotion (requires --apply)
- create_coupon - Create coupon code (requires --apply)
- validate_coupon - Check if coupon is valid
- list_coupons - List all coupon codes
- get_active_promotions - Get currently running promotions
- apply_cart_promotions - Apply discounts to cart (requires --apply)

## Safety Rules
1. Preview promotions before activating
2. Verify discount values are reasonable
3. Check date ranges for scheduled promotions
4. Warn about overlapping promotions

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Subscriptions specialist
  'subscriptions': {
    name: 'Subscriptions Agent',
    description: 'Subscription plans, recurring billing, and customer subscription lifecycle management',
    tools: [
      'mcp__stateset-commerce__list_subscription_plans',
      'mcp__stateset-commerce__get_subscription_plan',
      'mcp__stateset-commerce__create_subscription_plan',
      'mcp__stateset-commerce__activate_subscription_plan',
      'mcp__stateset-commerce__archive_subscription_plan',
      'mcp__stateset-commerce__list_subscriptions',
      'mcp__stateset-commerce__get_subscription',
      'mcp__stateset-commerce__create_subscription',
      'mcp__stateset-commerce__pause_subscription',
      'mcp__stateset-commerce__resume_subscription',
      'mcp__stateset-commerce__cancel_subscription',
      'mcp__stateset-commerce__skip_billing_cycle',
      'mcp__stateset-commerce__list_billing_cycles',
      'mcp__stateset-commerce__get_billing_cycle',
      'mcp__stateset-commerce__get_subscription_events',
      // Also need customer access
      'mcp__stateset-commerce__get_customer',
      'mcp__stateset-commerce__list_customers'
    ],
    systemPrompt: `You are a subscription management specialist for StateSet Commerce.

## Your Role
Manage subscription plans, customer subscriptions, billing cycles, and subscription lifecycle events.

## Billing Intervals
- weekly: Billed every week
- biweekly: Billed every 2 weeks
- monthly: Billed every month
- bimonthly: Billed every 2 months
- quarterly: Billed every 3 months
- semiannual: Billed every 6 months
- annual: Billed yearly

## Subscription Lifecycle
pending → trial → active → (paused) → cancelled → expired
  Create → Trial period → Active billing → Pause/Resume → Cancel

## Plan Status
- draft: Not yet available
- active: Available for new subscriptions
- archived: No new subscriptions, existing ones continue

## Subscription Status
- pending: Awaiting initial activation
- trial: In trial period (no charge)
- active: Billing normally
- paused: Temporarily stopped (can resume)
- past_due: Payment failed, in retry
- cancelled: Will end at period end
- expired: Subscription has ended

## Available Tools
- list_subscription_plans - List all plans
- get_subscription_plan - Get plan details
- create_subscription_plan - Create new plan (requires --apply)
- activate_subscription_plan - Make plan available (requires --apply)
- archive_subscription_plan - Retire a plan (requires --apply)
- list_subscriptions - List customer subscriptions
- get_subscription - Get subscription details
- create_subscription - Subscribe a customer (requires --apply)
- pause_subscription - Temporarily stop billing (requires --apply)
- resume_subscription - Resume paused subscription (requires --apply)
- cancel_subscription - Cancel subscription (requires --apply)
- skip_billing_cycle - Skip next billing (requires --apply)
- list_billing_cycles - View billing history
- get_billing_cycle - Get cycle details
- get_subscription_events - View audit log

## Safety Rules
1. Verify customer exists before creating subscription
2. Confirm cancellation intent (immediate vs end of period)
3. Show trial end dates clearly
4. Warn about billing implications of changes

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Storefront creation specialist
  'storefront': {
    name: 'Storefront Agent',
    description: 'Creates e-commerce storefront websites using StateSet iCommerce',
    tools: [
      'mcp__stateset-scaffold__list_templates',
      'mcp__stateset-scaffold__list_page_templates',
      'mcp__stateset-scaffold__list_component_templates',
      'mcp__stateset-scaffold__create_project',
      'mcp__stateset-scaffold__add_page',
      'mcp__stateset-scaffold__add_component',
      'mcp__stateset-scaffold__add_hook',
      'mcp__stateset-scaffold__add_api_route',
      'mcp__stateset-scaffold__write_file',
      'mcp__stateset-scaffold__read_file',
      'mcp__stateset-scaffold__list_files',
      'mcp__stateset-scaffold__run_command',
      'mcp__stateset-scaffold__seed_database'
    ],
    systemPrompt: `You are a storefront creation specialist for StateSet iCommerce. You help users create complete, production-ready e-commerce websites.

## Your Role
Create e-commerce storefronts using @stateset/embedded as the commerce backend. You can scaffold entire projects, add pages, components, hooks, and API routes.

## Available Templates
- nextjs: Full-stack Next.js 14 with App Router, SSR, Tailwind (recommended)
- nextjs-minimal: Minimal Next.js setup
- vite-react: Client-side SPA with WASM
- astro: Static-first with Islands

## Workflow
1. Ask about store name and requirements
2. Create project with create_project
3. Add needed pages (products, cart, checkout)
4. Add components (ProductCard, AddToCart, etc.)
5. Set up API routes for commerce operations
6. Seed database with sample products
7. Provide instructions to run the store

## Available Tools
- list_templates - Show project templates
- create_project - Initialize new storefront (requires --apply)
- add_page - Add a page (requires --apply)
- add_component - Add a component (requires --apply)
- add_hook - Add a React hook (requires --apply)
- add_api_route - Add an API route (requires --apply)
- write_file - Write any file (requires --apply)
- read_file - Read file contents
- list_files - List project files
- run_command - Run npm commands (requires --apply)
- seed_database - Create sample data (requires --apply)

## Best Practices
1. Use TypeScript for type safety
2. Use Server Components where possible
3. Style with Tailwind CSS
4. Create API routes to proxy commerce operations
5. Store database path in environment variables

## Safety
- Preview mode shows what would be created
- --apply flag enables write operations
- Never overwrite files without confirmation`
  },

  // Sync specialist
  'sync': {
    name: 'Sync Agent',
    description: 'Verifiable Event Sync (VES) management - sync local state with production sequencer',
    tools: [
      'mcp__stateset-commerce__sync_status',
      'mcp__stateset-commerce__sync_push',
      'mcp__stateset-commerce__sync_pull',
      'mcp__stateset-commerce__sync_outbox',
      'mcp__stateset-commerce__sync_retry_failed',
      'mcp__stateset-commerce__sync_entity_history',
      'mcp__stateset-commerce__sync_full'
    ],
    systemPrompt: `You are a sync management specialist for StateSet Commerce implementing Verifiable Event Sync (VES).

## Your Role
Manage synchronization between the local AI agent kernel and the production sequencer on Kubernetes. Help users understand sync status, push local changes to production, and pull remote changes locally.

## Key Concepts
- **Outbox**: Local SQLite table storing events before they're pushed to production
- **Sequencer**: Production service that assigns canonical sequence numbers to events
- **Push**: Send pending local events to the sequencer
- **Pull**: Fetch new events from the sequencer to local
- **Lag**: Number of events behind the remote head

## Event Flow
1. Local mutation (create order, update inventory) → Event captured in outbox
2. sync_push → Events sent to sequencer, assigned sequence numbers
3. sync_pull → Fetch events from other agents/sources
4. Events form immutable, verifiable audit trail

## Available Tools
- sync_status - Check sync health, connection, and lag
- sync_push - Push pending events to sequencer (requires --apply)
- sync_pull - Pull new events from sequencer
- sync_outbox - List events in local outbox
- sync_retry_failed - Reset failed events for retry (requires --apply)
- sync_entity_history - Get full event history for an entity
- sync_full - Push then pull in one operation

## Common Workflows

### Check sync health
Use sync_status to see:
- Connection to sequencer
- Pending events in outbox
- Sync lag (how far behind remote)

### Push local changes
1. Check sync_status for pending count
2. Use sync_push with --apply to send to production
3. Verify with sync_status that pending is now 0

### Investigate entity
Use sync_entity_history to see all events for an order, customer, etc.

### Recover from failures
1. Use sync_outbox with status='failed' to see failed events
2. Use sync_retry_failed with --apply to reset them
3. Use sync_push with --apply to retry

## Safety Rules
1. sync_push requires --apply flag
2. sync_retry_failed requires --apply flag
3. sync_pull and sync_status are always safe (read-only)
4. Check sync_status before pushing to verify connection

## Troubleshooting
- "Sync not configured" → Run stateset-sync init first
- High lag → Run sync_pull to catch up
- Failed events → Check sync_outbox for errors, then sync_retry_failed`
  },

  // Manufacturing specialist
  'manufacturing': {
    name: 'Manufacturing Agent',
    description: 'Bill of Materials (BOM) and work order management specialist',
    tools: [
      'mcp__stateset-commerce__list_boms',
      'mcp__stateset-commerce__get_bom',
      'mcp__stateset-commerce__create_bom',
      'mcp__stateset-commerce__add_bom_component',
      'mcp__stateset-commerce__activate_bom',
      'mcp__stateset-commerce__list_work_orders',
      'mcp__stateset-commerce__get_work_order',
      'mcp__stateset-commerce__create_work_order',
      'mcp__stateset-commerce__start_work_order',
      'mcp__stateset-commerce__complete_work_order',
      'mcp__stateset-commerce__cancel_work_order',
      // Also need inventory for production
      'mcp__stateset-commerce__get_stock',
      'mcp__stateset-commerce__adjust_inventory'
    ],
    systemPrompt: `You are a manufacturing management specialist for StateSet Commerce.

## Your Role
Manage Bill of Materials (BOM) and production work orders for manufacturing operations.

## Key Concepts
- **BOM (Bill of Materials)**: Recipe defining components needed to build a product
- **Work Order**: Production job to manufacture a quantity of products
- **Yield**: Number of finished products produced per work order

## BOM Status Flow
draft → active → archived
  Create → Activate for production → Archive when obsolete

## Work Order Status Flow
pending → in_progress → completed
       ↘ cancelled

## Available Tools

### Bill of Materials
- list_boms - List all BOMs
- get_bom - Get BOM with components
- create_bom - Create new BOM (requires --apply)
- add_bom_component - Add component to BOM (requires --apply)
- activate_bom - Activate BOM for production (requires --apply)

### Work Orders
- list_work_orders - List all work orders
- get_work_order - Get work order details
- create_work_order - Create work order from BOM (requires --apply)
- start_work_order - Start production (requires --apply)
- complete_work_order - Complete with quantity produced (requires --apply)
- cancel_work_order - Cancel work order (requires --apply)

## Safety Rules
1. Verify component stock before starting production
2. Check BOM is active before creating work order
3. Record actual vs planned quantities
4. Document reasons for variances

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Payments specialist
  'payments': {
    name: 'Payments Agent',
    description: 'Payment processing and refund management specialist',
    tools: [
      'mcp__stateset-commerce__list_payments',
      'mcp__stateset-commerce__get_payment',
      'mcp__stateset-commerce__create_payment',
      'mcp__stateset-commerce__complete_payment',
      'mcp__stateset-commerce__create_refund',
      // Also need order context
      'mcp__stateset-commerce__get_order',
      'mcp__stateset-commerce__list_orders'
    ],
    systemPrompt: `You are a payment processing specialist for StateSet Commerce.

## Your Role
Manage payment capture, processing, and refunds for orders.

## Payment Status Flow
pending → processing → completed → refunded
       ↘ failed

## Payment Methods
- credit_card: Credit/debit card payment
- ach: Bank transfer
- wallet: Digital wallet (Apple Pay, Google Pay, etc.)
- cash: Cash on delivery
- invoice: B2B invoicing (net terms)

## Available Tools
- list_payments - List all payments
- get_payment - Get payment details
- create_payment - Create payment for order (requires --apply)
- complete_payment - Mark payment as completed (requires --apply)
- create_refund - Process refund (requires --apply)

## Safety Rules
1. Verify order exists before creating payment
2. Check payment amount matches order total
3. Document refund reasons
4. Partial refunds require clear item breakdown

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Shipments specialist
  'shipments': {
    name: 'Shipments Agent',
    description: 'Shipment tracking and delivery management specialist',
    tools: [
      'mcp__stateset-commerce__list_shipments',
      'mcp__stateset-commerce__create_shipment',
      'mcp__stateset-commerce__deliver_shipment',
      // Also need order context
      'mcp__stateset-commerce__get_order',
      'mcp__stateset-commerce__ship_order'
    ],
    systemPrompt: `You are a shipment management specialist for StateSet Commerce.

## Your Role
Manage shipment creation, tracking, and delivery confirmation.

## Shipment Status Flow
created → shipped → in_transit → delivered
                             ↘ exception

## Shipping Carriers
- FEDEX, UPS, USPS, DHL
- Regional carriers

## Available Tools
- list_shipments - List all shipments
- create_shipment - Create shipment with tracking (requires --apply)
- deliver_shipment - Mark as delivered (requires --apply)
- ship_order - Ship order with tracking (requires --apply)

## Safety Rules
1. Verify tracking number format for carrier
2. Confirm shipping address is complete
3. Check inventory before shipping
4. Update order status after shipment

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Suppliers specialist
  'suppliers': {
    name: 'Suppliers Agent',
    description: 'Supplier management and purchase order specialist',
    tools: [
      'mcp__stateset-commerce__list_suppliers',
      'mcp__stateset-commerce__create_supplier',
      'mcp__stateset-commerce__list_purchase_orders',
      'mcp__stateset-commerce__create_purchase_order',
      'mcp__stateset-commerce__approve_purchase_order',
      'mcp__stateset-commerce__send_purchase_order',
      // Also need inventory context
      'mcp__stateset-commerce__get_stock',
      'mcp__stateset-commerce__get_low_stock_items'
    ],
    systemPrompt: `You are a supplier and procurement specialist for StateSet Commerce.

## Your Role
Manage supplier relationships and purchase orders for inventory replenishment.

## Purchase Order Status Flow
draft → approved → sent → partially_received → received
     ↘ cancelled

## Available Tools

### Supplier Management
- list_suppliers - List all suppliers
- create_supplier - Create new supplier (requires --apply)

### Purchase Orders
- list_purchase_orders - List all POs
- create_purchase_order - Create PO (requires --apply)
- approve_purchase_order - Approve PO (requires --apply)
- send_purchase_order - Send to supplier (requires --apply)

### Inventory Context
- get_stock - Check current stock levels
- get_low_stock_items - Identify reorder needs

## Safety Rules
1. Verify supplier exists before creating PO
2. Check reorder points and quantities
3. Approve POs before sending
4. Track expected delivery dates

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Invoices specialist
  'invoices': {
    name: 'Invoices Agent',
    description: 'B2B invoice management and accounts receivable specialist',
    tools: [
      'mcp__stateset-commerce__list_invoices',
      'mcp__stateset-commerce__create_invoice',
      'mcp__stateset-commerce__send_invoice',
      'mcp__stateset-commerce__record_invoice_payment',
      'mcp__stateset-commerce__get_overdue_invoices',
      // Also need customer/order context
      'mcp__stateset-commerce__get_customer',
      'mcp__stateset-commerce__get_order'
    ],
    systemPrompt: `You are a B2B invoice management specialist for StateSet Commerce.

## Your Role
Create and manage invoices, track payments, and monitor accounts receivable.

## Invoice Status Flow
draft → sent → viewed → partially_paid → paid
                    ↘ overdue → bad_debt

## Payment Terms
- Net 15, Net 30, Net 45, Net 60
- Due on Receipt
- Custom terms

## Available Tools
- list_invoices - List all invoices
- create_invoice - Create B2B invoice (requires --apply)
- send_invoice - Send to customer (requires --apply)
- record_invoice_payment - Record payment (requires --apply)
- get_overdue_invoices - Get overdue invoices

## Safety Rules
1. Verify customer has B2B account
2. Check credit limit before invoicing
3. Track payment terms and due dates
4. Flag overdue invoices promptly

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Warranties specialist
  'warranties': {
    name: 'Warranties Agent',
    description: 'Product warranty and claims management specialist',
    tools: [
      'mcp__stateset-commerce__list_warranties',
      'mcp__stateset-commerce__create_warranty',
      'mcp__stateset-commerce__create_warranty_claim',
      'mcp__stateset-commerce__approve_warranty_claim',
      // Also need product/order context
      'mcp__stateset-commerce__get_product',
      'mcp__stateset-commerce__get_order'
    ],
    systemPrompt: `You are a warranty management specialist for StateSet Commerce.

## Your Role
Manage product warranties and process warranty claims.

## Warranty Status Flow
active → claimed → expired
          ↘ processed

## Claim Status Flow
pending → approved → processed
       ↘ rejected

## Available Tools
- list_warranties - List all warranties
- create_warranty - Create product warranty (requires --apply)
- create_warranty_claim - File warranty claim (requires --apply)
- approve_warranty_claim - Approve claim (requires --apply)

## Safety Rules
1. Verify product is under warranty
2. Check warranty expiration date
3. Document claim reason and evidence
4. Process approved claims promptly

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Currency specialist
  'currency': {
    name: 'Currency Agent',
    description: 'Multi-currency support and exchange rate management specialist',
    tools: [
      'mcp__stateset-commerce__get_exchange_rate',
      'mcp__stateset-commerce__list_exchange_rates',
      'mcp__stateset-commerce__convert_currency',
      'mcp__stateset-commerce__set_exchange_rate',
      'mcp__stateset-commerce__get_currency_settings',
      'mcp__stateset-commerce__set_base_currency',
      'mcp__stateset-commerce__enable_currencies',
      'mcp__stateset-commerce__format_currency'
    ],
    systemPrompt: `You are a multi-currency management specialist for StateSet Commerce.

## Your Role
Manage exchange rates, currency conversions, and multi-currency store settings.

## Common Currencies
- USD, EUR, GBP, JPY, CAD, AUD
- Many others supported

## Available Tools

### Exchange Rates
- get_exchange_rate - Get rate between two currencies
- list_exchange_rates - List all rates or filter by base
- set_exchange_rate - Set/update rate (requires --apply)

### Conversions
- convert_currency - Convert amount between currencies
- format_currency - Format with currency symbol

### Store Settings
- get_currency_settings - Get store currency settings
- set_base_currency - Set store base currency (requires --apply)
- enable_currencies - Enable currencies for store (requires --apply)

## Safety Rules
1. Verify exchange rates are current
2. Use reliable rate sources
3. Consider rounding for display
4. Handle currency precision correctly

If --apply is not set, write operations show a preview instead of executing.`
  },

  // Tax specialist
  'tax': {
    name: 'Tax Agent',
    description: 'Tax calculation and compliance specialist',
    tools: [
      'mcp__stateset-commerce__calculate_tax',
      'mcp__stateset-commerce__calculate_cart_tax',
      'mcp__stateset-commerce__get_tax_rate',
      'mcp__stateset-commerce__list_tax_jurisdictions',
      'mcp__stateset-commerce__list_tax_rates',
      'mcp__stateset-commerce__get_tax_settings',
      'mcp__stateset-commerce__get_us_state_tax_info',
      'mcp__stateset-commerce__get_customer_tax_exemptions',
      'mcp__stateset-commerce__create_tax_exemption'
    ],
    systemPrompt: `You are a tax calculation and compliance specialist for StateSet Commerce.

## Your Role
Calculate sales tax for orders, manage tax rates, and handle exemptions.

## Tax Jurisdictions
- US: State + County + City taxes (nexus-based)
- EU: VAT (Value Added Tax)
- CA: GST/HST/PST
- Other regions supported

## Available Tools

### Tax Calculation
- calculate_tax - Calculate tax for line items
- calculate_cart_tax - Calculate and apply tax to cart
- get_tax_rate - Get effective rate for jurisdiction

### Tax Configuration
- list_tax_jurisdictions - List tax jurisdictions
- list_tax_rates - List all tax rates
- get_tax_settings - Get store tax settings
- get_us_state_tax_info - Get US state tax details

### Exemptions
- get_customer_tax_exemptions - Get customer's exemptions
- create_tax_exemption - Create tax exemption (requires --apply)

## Safety Rules
1. Use correct jurisdiction for shipping address
2. Handle tax-exempt customers properly
3. Apply product-specific tax rules
4. Document exemption certificates

If --apply is not set, write operations show a preview instead of executing.`
  }
};

// ============================================================================
// Agent Router
// ============================================================================

/**
 * Confidence thresholds for routing decisions
 */
const ROUTING_THRESHOLDS = {
  HIGH_CONFIDENCE: 0.7,    // Route with high confidence
  MEDIUM_CONFIDENCE: 0.4,  // Route but note alternatives
  LOW_CONFIDENCE: 0.2,     // Ambiguous - may need clarification
  MIN_SCORE: 2             // Minimum weighted score to consider a match
};

/**
 * Weighted keywords for agent routing
 * Each keyword has a weight (1-3):
 *   3 = Strong indicator (unique to this agent)
 *   2 = Moderate indicator
 *   1 = Weak indicator (may overlap with others)
 *
 * Format: { keyword: weight }
 */
const AGENT_KEYWORDS_WEIGHTED = {
  'checkout': {
    // Strong indicators
    'checkout': 3, 'shopping cart': 3, 'add to cart': 3, 'complete checkout': 3,
    'abandoned cart': 3, 'cart recovery': 3,
    // Moderate indicators
    'cart': 2, 'shopping': 2, 'shipping rate': 2, 'shipping options': 2,
    'apply discount': 2, 'coupon code': 2,
    // Weak indicators
    'buy': 1, 'purchase': 1
  },

  'orders': {
    // Strong indicators
    'order status': 3, 'order #': 3, 'order number': 3, 'ship order': 3,
    'cancel order': 3, 'order history': 3, 'update order': 3,
    'pending orders': 3, 'order tracking': 3, 'fulfill order': 3,
    // Moderate indicators
    'order': 2, 'ship': 2, 'tracking': 2, 'fulfillment': 2, 'deliver': 2,
    'shipping': 2, 'tracking number': 2, 'shipped': 2
  },

  'inventory': {
    // Strong indicators
    'stock level': 3, 'inventory count': 3, 'adjust inventory': 3,
    'reserve stock': 3, 'inventory item': 3, 'on-hand': 3, 'allocated': 3,
    'release reservation': 3, 'confirm reservation': 3,
    // Moderate indicators
    'stock': 2, 'inventory': 2, 'restock': 2, 'warehouse': 2, 'sku': 2,
    'available quantity': 2, 'stock check': 2,
    // Weak indicators
    'reserve': 1, 'available': 1
  },

  'returns': {
    // Strong indicators
    'return request': 3, 'rma': 3, 'return merchandise': 3, 'approve return': 3,
    'reject return': 3, 'pending returns': 3, 'return status': 3,
    // Moderate indicators
    'return': 2, 'refund': 2, 'exchange': 2, 'defective': 2, 'damaged': 2,
    'return policy': 2, 'return label': 2,
    // Weak indicators
    'broken': 1, 'wrong item': 1
  },

  'analytics': {
    // Strong indicators
    'analytics': 3, 'sales report': 3, 'revenue report': 3, 'forecast': 3,
    'predict demand': 3, 'top products': 3, 'best sellers': 3,
    'customer metrics': 3, 'top customers': 3, 'inventory health': 3,
    'low stock report': 3, 'revenue forecast': 3, 'demand forecast': 3,
    // Moderate indicators
    'sales': 2, 'revenue': 2, 'metrics': 2, 'performance': 2, 'trend': 2,
    'insight': 2, 'dashboard': 2, 'report': 2, 'aov': 2, 'average order': 2,
    'lifetime value': 2, 'vip customers': 2,
    // Weak indicators
    'how is business': 1, 'how are sales': 1
  },

  'promotions': {
    // Strong indicators
    'promotion': 3, 'create promotion': 3, 'activate promotion': 3,
    'promo code': 3, 'coupon': 3, 'create coupon': 3, 'validate coupon': 3,
    'percent off': 3, 'percentage off': 3, 'bogo': 3, 'buy one get one': 3,
    'tiered discount': 3, 'flash sale': 3,
    // Moderate indicators
    'discount': 2, 'sale': 2, 'deal': 2, 'offer': 2, 'campaign': 2,
    'free shipping promotion': 2,
    // Weak indicators
    'save': 1
  },

  'subscriptions': {
    // Strong indicators
    'subscription': 3, 'subscription plan': 3, 'recurring billing': 3,
    'billing cycle': 3, 'pause subscription': 3, 'cancel subscription': 3,
    'resume subscription': 3, 'skip billing': 3, 'subscriber': 3,
    'create subscription': 3, 'subscription events': 3,
    // Moderate indicators
    'subscribe': 2, 'recurring': 2, 'trial period': 2, 'monthly plan': 2,
    'annual plan': 2, 'renewal': 2, 'membership': 2,
    // Weak indicators
    'trial': 1, 'plan': 1, 'billing': 1
  },

  'storefront': {
    // Strong indicators
    'create store': 3, 'new store': 3, 'storefront': 3, 'build store': 3,
    'create website': 3, 'scaffold': 3, 'ecommerce site': 3,
    'e-commerce site': 3, 'online store': 3, 'shop website': 3,
    'nextjs store': 3, 'react store': 3,
    // Moderate indicators
    'website': 2, 'generate project': 2,
    // Weak indicators
    'nextjs': 1, 'react': 1
  },

  'sync': {
    // Strong indicators
    'sync status': 3, 'sync events': 3, 'push events': 3, 'pull events': 3,
    'outbox': 3, 'sequencer': 3, 'event sync': 3, 'sync lag': 3,
    'ves': 3, 'verifiable event': 3, 'pending events': 3,
    // Moderate indicators
    'sync': 2, 'synchronize': 2
  },

  'manufacturing': {
    // Strong indicators
    'bom': 3, 'bill of materials': 3, 'work order': 3, 'create work order': 3,
    'start work order': 3, 'complete work order': 3, 'manufacturing': 3,
    // Moderate indicators
    'production': 2, 'manufacture': 2, 'assembly': 2, 'component': 2, 'yield': 2,
    // Weak indicators
    'build product': 1
  },

  'payments': {
    // Strong indicators
    'payment': 3, 'create payment': 3, 'complete payment': 3,
    'process payment': 3, 'payment status': 3, 'payment method': 3,
    // Moderate indicators
    'pay': 2, 'charge': 2, 'capture': 2, 'credit card': 2, 'ach': 2,
    'digital wallet': 2, 'transaction': 2,
    // Weak indicators (overlap with returns for refund)
    'refund': 1
  },

  'shipments': {
    // Strong indicators
    'shipment': 3, 'create shipment': 3, 'deliver shipment': 3,
    'shipment status': 3, 'carrier': 3, 'in transit': 3,
    // Moderate indicators
    'fedex': 2, 'ups': 2, 'usps': 2, 'dhl': 2, 'parcel': 2, 'package': 2,
    // Weak indicators (overlap with orders)
    'delivery': 1
  },

  'suppliers': {
    // Strong indicators
    'supplier': 3, 'create supplier': 3, 'purchase order': 3, 'create po': 3,
    'approve purchase order': 3, 'send purchase order': 3, 'vendor': 3,
    // Moderate indicators
    'procurement': 2, 'reorder': 2, 'replenish': 2, 'po': 2,
    // Weak indicators
    'supply': 1
  },

  'invoices': {
    // Strong indicators
    'invoice': 3, 'create invoice': 3, 'send invoice': 3, 'overdue invoice': 3,
    'record payment': 3, 'accounts receivable': 3, 'net 30': 3, 'net 60': 3,
    // Moderate indicators
    'ar': 2, 'payment terms': 2, 'b2b': 2, 'overdue': 2,
    // Weak indicators
    'billing': 1
  },

  'warranties': {
    // Strong indicators
    'warranty': 3, 'create warranty': 3, 'warranty claim': 3,
    'approve warranty': 3, 'warranty status': 3, 'guarantee': 3,
    // Moderate indicators
    'claim': 2, 'repair': 2, 'replacement': 2
  },

  'currency': {
    // Strong indicators
    'exchange rate': 3, 'currency conversion': 3, 'set exchange rate': 3,
    'convert currency': 3, 'multi-currency': 3, 'base currency': 3,
    'enable currencies': 3, 'format currency': 3,
    // Moderate indicators
    'currency': 2, 'forex': 2, 'conversion': 2,
    // Weak indicators (too generic alone)
    'usd': 1, 'eur': 1, 'gbp': 1, 'jpy': 1
  },

  'tax': {
    // Strong indicators
    'sales tax': 3, 'calculate tax': 3, 'tax rate': 3, 'tax exempt': 3,
    'tax exemption': 3, 'vat': 3, 'gst': 3, 'hst': 3, 'pst': 3,
    'tax jurisdiction': 3, 'nexus': 3, 'cart tax': 3,
    // Moderate indicators
    'tax': 2, 'exemption': 2
  }
};

/**
 * Negative keywords - reduce score when these appear with the agent's keywords
 * Helps disambiguate overlapping terms
 */
const NEGATIVE_KEYWORDS = {
  'checkout': ['return', 'refund', 'analytics', 'report'],
  'orders': ['cart', 'checkout', 'warehouse', 'supplier'],
  'inventory': ['order status', 'checkout', 'return'],
  'returns': ['checkout', 'cart', 'create order'],
  'payments': ['subscription', 'billing cycle', 'return'],
  'shipments': ['order status', 'inventory'],
  'subscriptions': ['one-time', 'single purchase']
};

/**
 * Determine which agent is best suited for a request
 * @param {string} request - User's request
 * @returns {string} - Agent name
 */
export function routeToAgent(request) {
  const result = routeToAgentWithConfidence(request);
  return result.primary.agent;
}

/**
 * Determine which agent is best suited with confidence scoring
 * @param {string} request - User's request
 * @returns {object} - { primary: { agent, score, confidence, level }, alternatives: [...], ambiguous: boolean }
 */
export function routeToAgentWithConfidence(request) {
  const lower = request.toLowerCase();

  // Score each agent based on weighted keyword matches
  const scores = {};
  let maxPossibleScore = 0;

  for (const [agent, keywords] of Object.entries(AGENT_KEYWORDS_WEIGHTED)) {
    let weightedScore = 0;
    const matchedKeywords = [];
    let agentMaxScore = 0;

    // Calculate weighted score for matches
    for (const [keyword, weight] of Object.entries(keywords)) {
      agentMaxScore += weight;
      if (lower.includes(keyword)) {
        weightedScore += weight;
        matchedKeywords.push({ keyword, weight });
      }
    }

    // Apply negative keyword penalties
    const negatives = NEGATIVE_KEYWORDS[agent] || [];
    for (const negKeyword of negatives) {
      if (lower.includes(negKeyword)) {
        weightedScore -= 1;
      }
    }

    // Ensure score doesn't go negative
    weightedScore = Math.max(0, weightedScore);

    // Calculate confidence as percentage of max possible score
    const confidence = agentMaxScore > 0 ? weightedScore / agentMaxScore : 0;

    // Determine confidence level
    let level = 'none';
    if (confidence >= ROUTING_THRESHOLDS.HIGH_CONFIDENCE) {
      level = 'high';
    } else if (confidence >= ROUTING_THRESHOLDS.MEDIUM_CONFIDENCE) {
      level = 'medium';
    } else if (confidence >= ROUTING_THRESHOLDS.LOW_CONFIDENCE) {
      level = 'low';
    }

    scores[agent] = {
      agent,
      score: weightedScore,
      confidence,
      level,
      matchedKeywords,
      maxPossibleScore: agentMaxScore
    };

    maxPossibleScore = Math.max(maxPossibleScore, agentMaxScore);
  }

  // Rank agents by weighted score, then by confidence
  const ranked = Object.values(scores)
    .filter(s => s.score >= ROUTING_THRESHOLDS.MIN_SCORE || s.confidence >= ROUTING_THRESHOLDS.LOW_CONFIDENCE)
    .sort((a, b) => {
      // Primary sort by score
      if (b.score !== a.score) return b.score - a.score;
      // Secondary sort by confidence
      return b.confidence - a.confidence;
    });

  // Determine if routing is ambiguous
  const topScore = ranked[0]?.score || 0;
  const secondScore = ranked[1]?.score || 0;
  const topConfidence = ranked[0]?.confidence || 0;

  // Ambiguous if top two have similar scores and neither is high confidence
  const ambiguous = ranked.length >= 2 &&
    Math.abs(topScore - secondScore) <= 2 &&
    topConfidence < ROUTING_THRESHOLDS.HIGH_CONFIDENCE;

  // Default to customer-service if no good matches
  const primary = ranked.length > 0 && ranked[0].score >= ROUTING_THRESHOLDS.MIN_SCORE
    ? ranked[0]
    : {
      agent: 'customer-service',
      score: 0,
      confidence: 0,
      level: 'default',
      matchedKeywords: [],
      reason: 'No specific agent matched, using general customer service'
    };

  return {
    primary,
    alternatives: ranked.slice(1, 4),
    ambiguous,
    allScores: scores,
    thresholds: ROUTING_THRESHOLDS
  };
}

// ============================================================================
// Main Agent Loop
// ============================================================================

/**
 * Run the Claude agent loop
 * @param {Object} options
 * @param {string} options.request - Natural language request
 * @param {string} options.dbPath - Path to SQLite database
 * @param {string} options.model - Claude model to use
 * @param {boolean} options.allowApply - Whether to allow write operations
 * @param {number} options.maxTurns - Maximum conversation turns
 * @param {string} options.resumeSessionId - Session ID to resume
 * @param {string} options.agent - Specific agent to use (optional, auto-routes if not specified)
 * @param {Function} options.onToolCall - Callback for tool invocations
 * @param {Function} options.onMessage - Callback for assistant messages
 * @param {boolean} options.verbose - Enable verbose telemetry output
 * @param {Object} options.guardrails - Custom guardrails configuration
 * @param {Function} options.onConfirmRequired - Callback for confirmation prompts
 * @param {AgentTelemetry} options.telemetry - Custom telemetry instance
 * @param {PermissionGate} options.permissionGate - Custom permission gate instance
 * @param {boolean} options.enableSync - Enable VES sync event capture (default: auto-detect from config)
 * @param {boolean} options.autoSyncPush - Auto-push events after mutations (default: false)
 * @param {Function} options.onSyncEvent - Callback when sync event is captured
 * @param {string} options.thinkLevel - Extended thinking level: off|low|medium|high
 * @param {boolean} options.streaming - Enable streaming/partial messages
 * @param {number|null} options.maxBudgetUsd - Maximum budget in USD per query
 * @param {string} options.provider - AI provider: claude|openai|gemini|ollama
 * @param {Function} options.onPartialMessage - Callback for streaming tokens
 * @param {Function} options.onThinkingBlock - Callback for thinking content blocks
 */
export async function runAgentLoop({
  request,
  dbPath = './store.db',
  model = DEFAULT_MODEL,
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent,
  onToolCall,
  onMessage,
  verbose = false,
  guardrails = {},
  onConfirmRequired = null,
  telemetry = null,
  permissionGate = null,
  enableSync = null,
  autoSyncPush = false,
  onSyncEvent = null,
  thinkLevel = 'off',
  streaming = false,
  maxBudgetUsd = null,
  provider = 'claude',
  onPartialMessage = null,
  onThinkingBlock = null
}) {
  // Initialize telemetry
  const telem = telemetry || (verbose ? new AgentTelemetry({ verbose }) : noOpTelemetry);
  const mainSpan = telem.startSpan('agent_run', { request: request.slice(0, 100), agent });

  // Initialize permission gate
  const gate = permissionGate || createPermissionGate({
    apply: allowApply,
    guardrails,
    onConfirmRequired
  });

  // Initialize commerce instance
  let commerce = new Commerce(dbPath);
  let syncEngine = null;
  let syncConfig = null;

  // Check if sync is configured and should be enabled
  const rawSyncConfig = loadSyncConfig();
  const shouldEnableSync = enableSync !== null ? enableSync : (rawSyncConfig !== null);

  if (shouldEnableSync && rawSyncConfig) {
    syncConfig = new SyncConfig(rawSyncConfig);

    // Wrap commerce with event capture
    commerce = wrapCommerceWithEvents(commerce, syncConfig);

    // Log sync enablement
    telem.logCustomEvent('sync_enabled', {
      tenantId: syncConfig.tenantId,
      storeId: syncConfig.storeId,
      agentId: syncConfig.agentId
    });

    // Set up sync event callback if provided
    if (onSyncEvent && commerce._capture) {
      const originalCapture = commerce._capture.capture.bind(commerce._capture);
      commerce._capture.capture = (resourceMethod, entityId, payload, options) => {
        originalCapture(resourceMethod, entityId, payload, options);
        onSyncEvent({ resourceMethod, entityId, payload, options });
      };
    }

    // Initialize sync engine if auto-push is enabled
    if (autoSyncPush) {
      try {
        syncEngine = createSyncEngine({ db: commerce.db, config: syncConfig });
        await syncEngine.initialize();
      } catch (error) {
        // Log but don't fail - sync is optional
        telem.logCustomEvent('sync_init_failed', { error: error.message });
      }
    }
  }

  // Create MCP server with telemetry and permissions
  const mcpServer = createStatesetMcpServer({
    commerce,
    allowApply,
    telemetry: telem,
    permissionGate: gate
  });

  // Determine which agent to use
  const routingResult = routeToAgentWithConfidence(request);
  const agentName = agent || routingResult.primary.agent;
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

  // Log routing decision
  telem.logAgentRouting(
    request,
    agentName,
    routingResult.primary.confidence,
    routingResult.alternatives
  );

  // Build options
  const thinkTokens = THINK_LEVELS[thinkLevel] || 0;
  const options = {
    model,
    systemPrompt: agentConfig.systemPrompt,
    mcpServers: {
      'stateset-commerce': mcpServer
    },
    allowedTools: agentConfig.tools,
    maxTurns,
    // Allow MCP tools to run without prompting for permission
    permissionMode: 'bypassPermissions',
    allowDangerouslySkipPermissions: true,
    // v0.2.8: Extended thinking
    ...(thinkTokens > 0 ? { maxThinkingTokens: thinkTokens } : {}),
    // v0.2.8: Streaming partial messages
    ...(streaming ? { includePartialMessages: true } : {}),
    // v0.2.8: Budget controls
    ...(maxBudgetUsd ? { maxBudgetUsd: parseFloat(maxBudgetUsd) } : {}),
  };

  // Track results
  const toolResults = [];
  let sessionId = resumeSessionId;
  let response = '';

  // Save process.argv to restore later (prevent our CLI args from being passed to Claude Code)
  const savedArgv = process.argv;

  try {
    // If resuming, add session ID to options
    if (resumeSessionId) {
      options.resume = resumeSessionId;
    }

    // Clean process.argv before SDK call
    process.argv = process.argv.slice(0, 2); // Keep only node and script path

    // v0.2.8: Non-Claude provider path
    if (provider !== 'claude') {
      const { getProviderRegistry } = await import('./providers/base.js');
      const providerInstance = getProviderRegistry().get(provider);
      if (!providerInstance) {
        throw new Error(`Unknown provider: ${provider}. Available: ${getProviderRegistry().list().join(', ')}`);
      }
      if (!(await providerInstance.isAvailable())) {
        const providerConfig = (await import('./config.js')).PROVIDERS[provider];
        throw new Error(`Provider "${provider}" is not available. ${providerConfig?.envKey ? `Set ${providerConfig.envKey} environment variable.` : ''}`);
      }
      const messages = [
        { role: 'system', content: agentConfig.systemPrompt },
        { role: 'user', content: request },
      ];
      const providerResult = await providerInstance.chat(messages, {
        model,
        stream: streaming,
        onPartialMessage,
      });
      process.argv = savedArgv;
      return {
        response: providerResult.text,
        toolResults: [],
        sessionId: null,
        agent: agentName,
        routing: routingResult,
        provider,
        cost: providerResult.cost || null,
        thinkLevel,
      };
    }

    // Run the query (Claude provider)
    let budgetExceeded = false;
    let totalCost = null;

    for await (const message of query({ prompt: request, options })) {
      // Capture session ID
      if (message.sessionId && !sessionId) {
        sessionId = message.sessionId;
      }

      // Handle different message types
      if (message.type === 'assistant') {
        // Extract tool use from assistant messages
        // Note: SDK wraps API message in message.message
        const content = message.message?.content || message.content;
        if (content) {
          for (const block of content) {
            if (block.type === 'tool_use') {
              const toolCall = {
                id: block.id,
                name: block.name,
                input: block.input,
                startTime: Date.now()
              };
              toolResults.push({ toolCall, result: null });
              if (onToolCall) {
                onToolCall(toolCall);
              }
            } else if (block.type === 'text') {
              response += block.text;
            } else if (block.type === 'thinking' && onThinkingBlock) {
              // v0.2.8: Extended thinking content
              onThinkingBlock(block);
            }
          }
        }
      } else if (message.type === 'result') {
        // Final result message - extract the response
        if (message.result) {
          response = message.result;
        }
        // v0.2.8: Track cost and budget status
        if (message.total_cost_usd != null) {
          totalCost = message.total_cost_usd;
        }
        if (message.subtype === 'error_max_budget_usd') {
          budgetExceeded = true;
        }
      } else if (message.type === 'user') {
        // User messages contain tool results
        // Match result to pending tool call
        const pending = toolResults.find(tr => tr.result === null);
        if (pending && message.tool_use_result) {
          pending.result = message.tool_use_result;
          pending.endTime = Date.now();
          pending.duration = pending.endTime - pending.toolCall.startTime;

          // Log to telemetry
          telem.logToolCall(
            pending.toolCall.name,
            pending.toolCall.input,
            pending.result,
            pending.duration
          );
        }
      }

      // v0.2.8: Streaming partial messages
      if (streaming && onPartialMessage && message.type !== 'assistant' && message.type !== 'result' && message.type !== 'user') {
        onPartialMessage(message);
      }
    }

    // Restore process.argv
    process.argv = savedArgv;

    // Log assistant response
    telem.logAssistantMessage(response);

    if (onMessage) {
      onMessage(response);
    }

    // Auto-push sync events if enabled
    let syncResult = null;
    if (syncEngine && autoSyncPush && allowApply) {
      try {
        const pendingCount = commerce._outbox?.getPendingCount() || 0;
        if (pendingCount > 0) {
          telem.logCustomEvent('sync_push_start', { pendingCount });
          syncResult = await syncEngine.push();
          telem.logCustomEvent('sync_push_complete', {
            pushed: syncResult.pushed,
            rejected: syncResult.rejected
          });
        }
      } catch (error) {
        telem.logCustomEvent('sync_push_failed', { error: error.message });
      }
    }

    // Shutdown sync engine
    if (syncEngine) {
      await syncEngine.shutdown();
    }

    // End main span
    telem.endSpanRef(mainSpan, 'ok', { toolCallCount: toolResults.length });

    return {
      response,
      toolResults,
      sessionId,
      agent: agentName,
      routing: routingResult,
      telemetry: telem.getSummary(),
      traceId: telem.traceId,
      provider: 'claude',
      cost: totalCost,
      thinkLevel,
      budgetExceeded,
      sync: syncResult ? {
        enabled: true,
        pushed: syncResult.pushed,
        rejected: syncResult.rejected,
        receipt: syncResult.receipt
      } : (shouldEnableSync ? { enabled: true, pushed: 0 } : null)
    };
  } catch (error) {
    // Restore process.argv on error
    process.argv = savedArgv;
    // Cleanup sync engine on error
    if (syncEngine) {
      try { await syncEngine.shutdown(); } catch (e) { /* ignore */ }
    }
    telem.logError(error, { agent: agentName, request: request.slice(0, 100) });
    telem.endSpanRef(mainSpan, 'error', { error: error.message });
    throw new Error(`Agent error: ${error.message}`);
  }
}

/**
 * Create a streaming generator for interactive use
 * @param {Object} options
 * @param {boolean} options.enableSync - Enable VES sync event capture
 */
export async function* runAgentStream({
  request,
  dbPath = './store.db',
  model = DEFAULT_MODEL,
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent,
  enableSync = null
}) {
  let commerce = new Commerce(dbPath);

  // Check if sync is configured
  const rawSyncConfig = loadSyncConfig();
  const shouldEnableSync = enableSync !== null ? enableSync : (rawSyncConfig !== null);

  if (shouldEnableSync && rawSyncConfig) {
    const syncConfig = new SyncConfig(rawSyncConfig);
    commerce = wrapCommerceWithEvents(commerce, syncConfig);
  }

  const mcpServer = createStatesetMcpServer({ commerce, allowApply });

  // Determine which agent to use
  const agentName = agent || routeToAgent(request);
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

  const options = {
    model,
    systemPrompt: agentConfig.systemPrompt,
    mcpServers: {
      'stateset-commerce': mcpServer
    },
    allowedTools: agentConfig.tools,
    maxTurns
  };

  const input = resumeSessionId
    ? { sessionId: resumeSessionId, prompt: request }
    : { prompt: request };

  for await (const message of query({ prompt: input, options })) {
    yield message;
  }
}

/**
 * Create an agent session for multi-turn conversations
 */
export function createAgentSession({
  dbPath = './store.db',
  model = DEFAULT_MODEL,
  allowApply = false,
  maxTurns = 10,
  agent,
  resumeSessionId = null
}) {
  let sessionId = resumeSessionId;
  let currentAgent = agent;

  return {
    async query(message, { onToolCall = null, onText = null } = {}) {
      const result = await runAgentLoop({
        request: message,
        dbPath,
        model,
        allowApply,
        maxTurns,
        resumeSessionId: sessionId,
        agent: currentAgent,
        onToolCall,
        onMessage: onText
      });

      // Update session ID for subsequent queries
      if (result.sessionId) {
        sessionId = result.sessionId;
      }

      // Track which agent was used
      if (result.agent) {
        currentAgent = result.agent;
      }

      return result;
    },

    getSessionId() {
      return sessionId;
    },

    getAgent() {
      return currentAgent;
    },

    setAgent(name) {
      if (AGENTS[name]) {
        currentAgent = name;
      } else {
        throw new Error(`Unknown agent: ${name}. Available: ${Object.keys(AGENTS).join(', ')}`);
      }
    }
  };
}

/**
 * List available agents
 */
export function listAgents() {
  return Object.entries(AGENTS).map(([id, config]) => ({
    id,
    name: config.name,
    description: config.description,
    toolCount: config.tools.length
  }));
}

// ============================================================================
// Re-exports for convenience
// ============================================================================

export { AgentTelemetry, noOpTelemetry } from './telemetry.js';
export { PermissionGate, createPermissionGate, PERMISSION_LEVELS, TOOL_PERMISSIONS } from './permissions.js';
export { RichOutput, ICONS, createOutput } from './output.js';

// Sync (Verifiable Event Sync)
export { loadSyncConfig, saveSyncConfig, SyncConfig, isSyncConfigured } from './sync/config.js';
export { createOutbox, Outbox } from './sync/outbox.js';
export { createSyncEngine, SyncEngine } from './sync/engine.js';
export { wrapCommerceWithEvents, EventCapture } from './sync/capture.js';
export { createSequencerClient, SequencerClient } from './sync/client.js';
