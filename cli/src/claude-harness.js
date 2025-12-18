/**
 * Claude Agent SDK integration for StateSet iCommerce CLI
 * Supports multiple specialized agents with domain-specific tools and prompts
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
import { Commerce } from '@stateset/embedded';
import { createStatesetMcpServer, TOOL_NAMES } from './mcp-server.js';

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
  }
};

// ============================================================================
// Agent Router
// ============================================================================

/**
 * Keywords that suggest which agent to use
 */
const AGENT_KEYWORDS = {
  'checkout': ['cart', 'checkout', 'add to cart', 'shopping', 'buy', 'purchase', 'discount', 'coupon', 'shipping rate', 'abandoned'],
  'orders': ['order', 'ship', 'shipping', 'tracking', 'fulfill', 'deliver'],
  'inventory': ['stock', 'inventory', 'restock', 'warehouse', 'reserve', 'allocation', 'on-hand', 'available'],
  'returns': ['return', 'rma', 'refund', 'exchange', 'defective', 'damaged'],
  'analytics': ['analytics', 'sales', 'revenue', 'best seller', 'top product', 'forecast', 'predict', 'trend', 'metrics', 'performance', 'how is business', 'how are sales', 'top customer', 'vip', 'lifetime value', 'aov', 'demand', 'low stock', 'out of stock', 'report', 'insight', 'dashboard']
};

/**
 * Determine which agent is best suited for a request
 * @param {string} request - User's request
 * @returns {string} - Agent name
 */
export function routeToAgent(request) {
  const lower = request.toLowerCase();

  // Score each agent based on keyword matches
  const scores = {};
  for (const [agent, keywords] of Object.entries(AGENT_KEYWORDS)) {
    scores[agent] = keywords.filter(kw => lower.includes(kw)).length;
  }

  // Find highest scoring agent
  const best = Object.entries(scores)
    .filter(([_, score]) => score > 0)
    .sort((a, b) => b[1] - a[1])[0];

  // Return best match or default to customer-service
  return best ? best[0] : 'customer-service';
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
 */
export async function runAgentLoop({
  request,
  dbPath = './store.db',
  model = 'claude-sonnet-4-20250514',
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent,
  onToolCall,
  onMessage
}) {
  // Initialize commerce instance
  const commerce = new Commerce(dbPath);

  // Create MCP server
  const mcpServer = createStatesetMcpServer({ commerce, allowApply });

  // Determine which agent to use
  const agentName = agent || routeToAgent(request);
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

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
                input: block.input
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
        }
      }
    }

    if (onMessage) {
      onMessage(response);
    }

    return {
      response,
      toolResults,
      sessionId,
      agent: agentName
    };
  } catch (error) {
    throw new Error(`Agent error: ${error.message}`);
  }
}

/**
 * Create a streaming generator for interactive use
 */
export async function* runAgentStream({
  request,
  dbPath = './store.db',
  model = 'claude-sonnet-4-20250514',
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
  model = 'claude-sonnet-4-20250514',
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
