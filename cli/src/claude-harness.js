/**
 * Claude Agent SDK integration for StateSet iCommerce CLI
 * Supports multiple specialized agents with domain-specific tools and prompts
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
import { DEFAULT_MODEL } from './config.js';
import { Commerce } from '@stateset/embedded';
import { createStatesetMcpServer, TOOL_NAMES } from './mcp-server.js';
import { AgentTelemetry, noOpTelemetry } from './telemetry.js';
import { PermissionGate, createPermissionGate, PERMISSION_LEVELS } from './permissions.js';
import { RichOutput, ICONS } from './output.js';

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
  }
};

// ============================================================================
// Agent Router
// ============================================================================

/**
 * Keywords that suggest which agent to use
 */
const AGENT_KEYWORDS = {
  'checkout': ['cart', 'checkout', 'add to cart', 'shopping', 'buy', 'purchase', 'shipping rate', 'abandoned'],
  'orders': ['order', 'ship', 'shipping', 'tracking', 'fulfill', 'deliver'],
  'inventory': ['stock', 'inventory', 'restock', 'warehouse', 'reserve', 'allocation', 'on-hand', 'available'],
  'returns': ['return', 'rma', 'refund', 'exchange', 'defective', 'damaged'],
  'analytics': ['analytics', 'sales', 'revenue', 'best seller', 'top product', 'forecast', 'predict', 'trend', 'metrics', 'performance', 'how is business', 'how are sales', 'top customer', 'vip', 'lifetime value', 'aov', 'demand', 'low stock', 'out of stock', 'report', 'insight', 'dashboard'],
  'promotions': ['promotion', 'discount', 'coupon', 'promo code', 'percent off', 'percentage off', 'bogo', 'buy one get one', 'free shipping', 'sale', 'deal', 'offer', 'campaign', 'tiered discount', 'flash sale'],
  'subscriptions': ['subscription', 'subscribe', 'recurring', 'billing cycle', 'trial', 'plan', 'monthly plan', 'annual plan', 'pause subscription', 'cancel subscription', 'renew', 'renewal', 'billing', 'subscriber', 'membership'],
  'storefront': ['create store', 'new store', 'storefront', 'website', 'scaffold', 'generate', 'build store', 'create website', 'nextjs', 'react', 'ecommerce site', 'e-commerce site', 'online store', 'shop website']
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
 * @returns {object} - { primary: { agent, score, confidence }, alternatives: [...], ambiguous: boolean }
 */
export function routeToAgentWithConfidence(request) {
  const lower = request.toLowerCase();

  // Score each agent based on keyword matches
  const scores = {};
  for (const [agent, keywords] of Object.entries(AGENT_KEYWORDS)) {
    const matchedKeywords = keywords.filter(kw => lower.includes(kw));
    const score = matchedKeywords.length;
    const confidence = keywords.length > 0 ? score / keywords.length : 0;

    scores[agent] = {
      agent,
      score,
      confidence,
      matchedKeywords
    };
  }

  // Rank agents by score
  const ranked = Object.values(scores)
    .sort((a, b) => b.score - a.score || b.confidence - a.confidence);

  // Determine if routing is ambiguous
  const topScore = ranked[0]?.score || 0;
  const secondScore = ranked[1]?.score || 0;
  const ambiguous = topScore > 0 && topScore === secondScore;

  // Default to customer-service if no matches
  const primary = topScore > 0
    ? ranked[0]
    : { agent: 'customer-service', score: 0, confidence: 0, matchedKeywords: [] };

  return {
    primary,
    alternatives: ranked.slice(1, 4),
    ambiguous,
    allScores: scores
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
  permissionGate = null
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
  const commerce = new Commerce(dbPath);

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
  const options = {
    model,
    systemPrompt: agentConfig.systemPrompt,
    mcpServers: {
      'stateset-commerce': mcpServer
    },
    allowedTools: agentConfig.tools,
    maxTurns
  };

  // Track results
  const toolResults = [];
  let sessionId = resumeSessionId;
  let response = '';

  try {
    // Create streaming input
    const input = resumeSessionId
      ? { sessionId: resumeSessionId, prompt: request }
      : { prompt: request };

    // Run the query
    for await (const message of query({ prompt: input, options })) {
      // Capture session ID
      if (message.sessionId && !sessionId) {
        sessionId = message.sessionId;
      }

      // Handle different message types
      if (message.type === 'assistant') {
        // Extract tool use from assistant messages
        if (message.content) {
          for (const block of message.content) {
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
            }
          }
        }
      } else if (message.type === 'result') {
        // Match result to tool call
        const pending = toolResults.find(tr => tr.result === null);
        if (pending) {
          pending.result = message.content;
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
    }

    // Log assistant response
    telem.logAssistantMessage(response);

    if (onMessage) {
      onMessage(response);
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
      traceId: telem.traceId
    };
  } catch (error) {
    telem.logError(error, { agent: agentName, request: request.slice(0, 100) });
    telem.endSpanRef(mainSpan, 'error', { error: error.message });
    throw new Error(`Agent error: ${error.message}`);
  }
}

/**
 * Create a streaming generator for interactive use
 */
export async function* runAgentStream({
  request,
  dbPath = './store.db',
  model = DEFAULT_MODEL,
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent
}) {
  const commerce = new Commerce(dbPath);
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
