/**
 * Enhanced MCP Tool Discovery System
 * Provides rich tool descriptions, examples, and categorization for better agent understanding
 */

export class ToolDiscoveryEngine {
  constructor(commerce) {
    this.commerce = commerce;
    this.toolRegistry = new Map();
    this.toolCategories = new Map();
    this.toolExamples = new Map();
    this.toolRelationships = new Map();
  }

  /**
   * Register a tool with enhanced metadata
   */
  registerTool(toolName, metadata) {
    this.toolRegistry.set(toolName, {
      name: toolName,
      category: metadata.category,
      description: metadata.description,
      purpose: metadata.purpose,
      whenToUse: metadata.whenToUse,
      prerequisite: metadata.prerequisite,
      sideEffects: metadata.sideEffects,
      performanceHints: metadata.performanceHints,
      complexity: metadata.complexity || 'medium',
      relatedTools: metadata.relatedTools || [],
      alternatives: metadata.alternatives || [],
      examples: metadata.examples || [],
      errorPatterns: metadata.errorPatterns || {},
      failureRecovery: metadata.failureRecovery || [],
    });

    // Register by category
    if (!this.toolCategories.has(metadata.category)) {
      this.toolCategories.set(metadata.category, []);
    }
    this.toolCategories.get(metadata.category).push(toolName);

    // Register examples
    if (metadata.examples) {
      this.toolExamples.set(toolName, metadata.examples);
    }

    // Register relationships
    if (metadata.relatedTools) {
      this.toolRelationships.set(toolName, metadata.relatedTools);
    }
  }

  /**
   * Get tool discovery info by intent
   * Helps agents find the right tool based on what they want to accomplish
   */
  discoverToolsByIntent(intent) {
    const intentMapping = {
      create_customer: ['create_customer'],
      view_customer: ['list_customers', 'get_customer'],
      place_order: ['get_customer', 'get_product', 'create_order'],
      check_inventory: ['get_stock', 'list_products'],
      update_order: ['update_order_status', 'ship_order', 'cancel_order'],
      refund_customer: ['create_return', 'approve_return'],
      manage_returns: [
        'list_returns',
        'get_return',
        'create_return',
        'approve_return',
        'reject_return',
      ],
      checkout_process: [
        'create_cart',
        'add_cart_item',
        'set_cart_shipping_address',
        'complete_checkout',
      ],
      get_insights: [
        'get_sales_summary',
        'get_top_products',
        'get_demand_forecast',
        'get_revenue_forecast',
      ],
      manage_inventory: [
        'get_stock',
        'adjust_inventory',
        'reserve_inventory',
        'create_inventory_item',
      ],
      handle_payments: [
        'list_payments',
        'get_payment',
        'create_payment',
        'complete_payment',
        'create_refund',
      ],
      stablecoin_payments: [
        'get_agent_wallet',
        'get_wallet_balance',
        'create_stablecoin_payment',
        'list_supported_chains',
      ],
      agentic_payments: [
        'x402_execute_agent_payment',
        'x402_create_payment_intent',
        'x402_sign_intent',
        'x402_settle_intent_onchain',
        'x402_record_incoming_settlement',
        'x402_get_intent',
      ],
      manage_subscriptions: ['list_subscriptions', 'create_subscription', 'cancel_subscription'],
      apply_promotions: ['list_promotions', 'validate_coupon', 'apply_cart_promotions'],
      calculate_taxes: ['get_tax_rate', 'list_tax_rates', 'calculate_tax', 'calculate_cart_tax'],
      semantic_search_products: ['vector_search_products'],
      semantic_search_customers: ['vector_search_customers'],
      semantic_search_orders: ['vector_search_orders'],
      semantic_search_inventory: ['vector_search_inventory'],
      vector_index_products: ['vector_index_product', 'vector_index_all_products'],
      vector_index_customers: ['vector_index_customer', 'vector_index_all_customers'],
      vector_index_orders: ['vector_index_order', 'vector_index_all_orders'],
      vector_index_inventory: ['vector_index_inventory', 'vector_index_all_inventory'],
    };

    return intentMapping[intent] || [];
  }

  /**
   * Get tool orchestration plan for complex operations
   * Returns a sequence of tools to execute together
   */
  getOrchestrationPlan(operationType) {
    const plans = {
      full_checkout: [
        'create_customer', // Step 1: Create customer if needed
        'create_cart', // Step 2: Create shopping cart
        'add_cart_item', // Step 3: Add items (repeatable)
        'get_stock', // Step 4: Verify inventory
        'set_cart_shipping_address', // Step 5: Set shipping address
        'get_shipping_rates', // Step 6: Get shipping options
        'calculate_cart_tax', // Step 7: Calculate taxes
        'complete_checkout', // Step 8: Convert cart to order
      ],
      order_fulfillment: [
        'get_order', // Step 1: Get order details
        'get_stock', // Step 2: Verify inventory
        'reserve_inventory', // Step 3: Reserve items
        'confirm_reservation', // Step 4: Deduct stock
        'update_order_status', // Step 5: Mark as processing
        'ship_order', // Step 6: Ship with tracking
        'send_order_confirmation', // Step 7: Notify customer
      ],
      return_process: [
        'get_order', // Step 1: Get order details
        'get_return', // Step 2: Check if return exists
        'create_return', // Step 3: Create return if needed
        'approve_return', // Step 4: Approve return
        'returns_receive_items', // Step 5: Mark as received
        'create_refund', // Step 6: Refund customer
        'restock_inventory', // Step 7: Return to stock
      ],
      agent_to_agent_payment: [
        'x402_execute_agent_payment', // Step 1: Create, sign, settle, and (optionally) credit incoming settlement
        'x402_get_intent', // Step 2: Verify settlement details
      ],
      inventory_replenishment: [
        'get_stock', // Step 1: Check current levels
        'get_low_stock_items', // Step 2: Items below threshold
        'create_purchase_order', // Step 3: Order from supplier
        'receive_shipment', // Step 4: Receive items
        'adjust_inventory', // Step 5: Update inventory
      ],
    };

    return plans[operationType] || [];
  }

  /**
   * Get enhanced tool description with examples and context
   */
  getToolInfo(toolName) {
    const tool = this.toolRegistry.get(toolName);
    if (!tool) {
      return null;
    }

    return {
      ...tool,
      executionOrder: this.getExecutionOrder(toolName),
      commonErrors: this.getCommonErrors(toolName),
      bestPractices: this.getBestPractices(toolName),
      performance: this.getPerformanceMetrics(toolName),
    };
  }

  /**
   * Get execution order recommendations
   * Helps agents sequence tools correctly
   */
  getExecutionOrder(toolName) {
    const orderRules = {
      create_order: { mustPrecede: ['update_order_status', 'ship_order', 'cancel_order'] },
      create_cart: { mustPrecede: ['add_cart_item', 'complete_checkout'] },
      add_cart_item: { mustFollow: ['create_cart'], mustPrecede: ['complete_checkout'] },
      complete_checkout: {
        mustFollow: ['create_cart', 'add_cart_item'],
        mustPrecede: ['update_order_status'],
      },
      reserve_inventory: { mustPrecede: ['confirm_reservation'] },
      approve_return: { mustPrecede: ['create_refund', 'restock_inventory'] },
      create_purchase_order: { mustPrecede: ['receive_shipment'] },
      x402_create_payment_intent: { mustPrecede: ['x402_sign_intent'] },
      x402_sign_intent: {
        mustFollow: ['x402_create_payment_intent'],
        mustPrecede: ['x402_settle_intent_onchain'],
      },
      x402_settle_intent_onchain: { mustFollow: ['x402_sign_intent'] },
      x402_execute_agent_payment: { mustPrecede: ['x402_get_intent'] },
    };

    return orderRules[toolName] || {};
  }

  /**
   * Get common error patterns for a tool
   */
  getCommonErrors(toolName) {
    const tool = this.toolRegistry.get(toolName);
    return tool?.errorPatterns || {};
  }

  /**
   * Get best practices for using a tool
   */
  getBestPractices(toolName) {
    const bestPractices = {
      create_order: [
        'Always verify customer exists with get_customer first',
        'Check inventory levels with get_stock before ordering',
        'Validate all product SKUs are in stock',
        'Consider using carts for complex checkouts',
      ],
      reserve_inventory: [
        'Set appropriate expiration times (e.g., 30 minutes for checkout)',
        'Always confirm reservations after order completion',
        'Release reservations if order is cancelled',
        'Handle insufficient stock gracefully',
      ],
      create_return: [
        'Verify order status is shipped or delivered',
        'Check return eligibility period',
        'Document return reasons for analytics',
        'Take photos of damaged items if applicable',
      ],
    };

    return bestPractices[toolName] || [];
  }

  /**
   * Get performance metrics for a tool
   */
  getPerformanceMetrics(toolName) {
    const metrics = {
      list_orders: { avgLatency: '50ms', p99: '200ms', recommended: true },
      create_order: { avgLatency: '100ms', p99: '500ms', recommended: true },
      get_stock: { avgLatency: '20ms', p99: '50ms', recommended: true },
      reserve_inventory: { avgLatency: '30ms', p99: '100ms', recommended: true },
      complete_checkout: { avgLatency: '200ms', p99: '1s', recommended: true },
      adjust_inventory: { avgLatency: '40ms', p99: '150ms', recommended: true },
    };

    return metrics[toolName];
  }

  /**
   * Get all tools in a category
   */
  getToolsByCategory(category) {
    return this.toolCategories.get(category) || [];
  }

  /**
   * Get tool relationships and dependencies
   */
  getToolRelationships(toolName) {
    return this.toolRelationships.get(toolName) || [];
  }

  /**
   * Get examples for a tool
   */
  getToolExamples(toolName) {
    return this.toolExamples.get(toolName) || [];
  }

  /**
   * Search tools by keyword
   */
  searchTools(keyword) {
    const results = [];
    const lowerKeyword = keyword.toLowerCase();

    for (const [name, tool] of this.toolRegistry) {
      if (
        name.toLowerCase().includes(lowerKeyword) ||
        tool.description.toLowerCase().includes(lowerKeyword) ||
        tool.purpose.toLowerCase().includes(lowerKeyword) ||
        tool.whenToUse.toLowerCase().includes(lowerKeyword)
      ) {
        results.push({
          name,
          category: tool.category,
          description: tool.description,
          purpose: tool.purpose,
        });
      }
    }

    return results;
  }

  /**
   * Get tool recommendations for a given context
   * Intelligently suggests tools based on conversation history and intent
   */
  recommendTools(conversationHistory, currentIntent) {
    const recommendations = [];
    const recentTools = conversationHistory
      .filter((entry) => entry.toolUsed)
      .map((entry) => entry.toolUsed)
      .slice(-5); // Last 5 tools used

    // Recommend based on current intent
    if (currentIntent) {
      const intentTools = this.discoverToolsByIntent(currentIntent);
      recommendations.push(...intentTools);
    }

    // Recommend related tools based on recent tool usage
    for (const toolName of recentTools) {
      const related = this.getToolRelationships(toolName);
      recommendations.push(...related);
    }

    // Remove duplicates and sort by relevance
    return [...new Set(recommendations)].slice(0, 10);
  }

  /**
   * Register tools from ALL_TOOL_DEFS-style array (name + description + optional metadata)
   */
  registerFromToolDefs(toolDefs) {
    for (const tool of toolDefs) {
      if (tool?.name && tool?.description) {
        this.registerTool(tool.name, {
          name: tool.name,
          description: tool.description,
          category: tool.policyDomain || 'general',
          purpose: tool.description,
          whenToUse: tool.description,
          inputSchema: tool.inputSchema,
          permission: tool.permission,
        });
      }
    }
  }

  /**
   * Discover tools matching a natural language intent query.
   * Searches both the intent mapping and the full tool registry by keyword.
   */
  discover(intent, limit = 5) {
    const results = [];
    const seen = new Set();

    // First, try exact intent key match
    const mapped = this.discoverToolsByIntent(intent);
    for (const name of mapped) {
      if (!seen.has(name)) {
        seen.add(name);
        const tool = this.toolRegistry.get(name);
        results.push(
          tool ? { name, description: tool.description, category: tool.category } : { name },
        );
      }
    }

    // Then keyword search across registry
    if (results.length < limit) {
      const keywordMatches = this.searchTools(intent);
      for (const match of keywordMatches) {
        if (!seen.has(match.name)) {
          seen.add(match.name);
          results.push(match);
        }
        if (results.length >= limit) break;
      }
    }

    return results.slice(0, limit);
  }

  /**
   * Export tool registry as JSON for external tools
   */
  exportRegistry() {
    const exported = {};
    for (const [name, tool] of this.toolRegistry) {
      exported[name] = {
        category: tool.category,
        description: tool.description,
        purpose: tool.purpose,
        whenToUse: tool.whenToUse,
        complexity: tool.complexity,
        relatedTools: tool.relatedTools,
        examples: tool.examples,
      };
    }
    return exported;
  }
}

/**
 * Tool categories for organization
 */
export const TOOL_CATEGORIES = {
  CUSTOMERS: 'Customers',
  ORDERS: 'Orders',
  PRODUCTS: 'Products',
  INVENTORY: 'Inventory',
  RETURNS: 'Returns',
  CARTS: 'Cart/Checkout',
  PAYMENTS: 'Payments',
  ANALYTICS: 'Analytics',
  FORECASTING: 'Forecasting',
  PROMOTIONS: 'Promotions',
  SUBSCRIPTIONS: 'Subscriptions',
  TAX: 'Tax',
  SHIPPING: 'Shipping',
  MANUFACTURING: 'Manufacturing',
  PURCHASING: 'Purchasing',
  FINANCE: 'Finance',
  SYNC: 'Sync',
};
