/**
 * Unit tests for mcp-tool-discovery.js — ToolDiscoveryEngine, TOOL_CATEGORIES
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ToolDiscoveryEngine, TOOL_CATEGORIES } from '../../src/mcp-tool-discovery.js';

/** Helper: register a standard test tool */
function registerTestTool(engine, name, overrides = {}) {
  engine.registerTool(name, {
    category: overrides.category || 'Orders',
    description: overrides.description || `${name} description`,
    purpose: overrides.purpose || `Purpose of ${name}`,
    whenToUse: overrides.whenToUse || `When to use ${name}`,
    relatedTools: overrides.relatedTools || [],
    examples: overrides.examples || [{ input: 'test', output: 'test' }],
    complexity: overrides.complexity || 'medium',
    ...(overrides.extra || {}),
  });
}

// ===========================================================================
// registerTool
// ===========================================================================

describe('ToolDiscoveryEngine.registerTool', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('adds tool to registry', () => {
    registerTestTool(engine, 'create_order');
    const info = engine.toolRegistry.get('create_order');
    assert.ok(info);
    assert.equal(info.name, 'create_order');
  });

  it('stores category mapping', () => {
    registerTestTool(engine, 'create_order', { category: 'Orders' });
    const tools = engine.getToolsByCategory('Orders');
    assert.ok(tools.includes('create_order'));
  });

  it('stores examples', () => {
    const examples = [{ input: 'a', output: 'b' }];
    registerTestTool(engine, 'create_order', { examples });
    assert.deepEqual(engine.getToolExamples('create_order'), examples);
  });

  it('stores relationships', () => {
    registerTestTool(engine, 'create_order', { relatedTools: ['list_orders', 'get_order'] });
    const rels = engine.getToolRelationships('create_order');
    assert.deepEqual(rels, ['list_orders', 'get_order']);
  });

  it('defaults complexity to medium', () => {
    registerTestTool(engine, 'create_order');
    assert.equal(engine.toolRegistry.get('create_order').complexity, 'medium');
  });
});

// ===========================================================================
// discoverToolsByIntent
// ===========================================================================

describe('ToolDiscoveryEngine.discoverToolsByIntent', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('returns tools for create_customer intent', () => {
    const tools = engine.discoverToolsByIntent('create_customer');
    assert.ok(tools.includes('create_customer'));
  });

  it('returns tools for place_order intent', () => {
    const tools = engine.discoverToolsByIntent('place_order');
    assert.ok(tools.includes('create_order'));
    assert.ok(tools.includes('get_customer'));
  });

  it('returns tools for checkout_process intent', () => {
    const tools = engine.discoverToolsByIntent('checkout_process');
    assert.ok(tools.includes('create_cart'));
    assert.ok(tools.includes('complete_checkout'));
  });

  it('returns empty array for unknown intent', () => {
    const tools = engine.discoverToolsByIntent('teleport_customer');
    assert.deepEqual(tools, []);
  });

  it('returns semantic search tools', () => {
    const tools = engine.discoverToolsByIntent('semantic_search_products');
    assert.ok(tools.includes('vector_search_products'));
  });

  it('returns updated commerce payment tools for handle_payments intent', () => {
    const tools = engine.discoverToolsByIntent('handle_payments');
    assert.ok(tools.includes('create_payment'));
    assert.ok(tools.includes('create_refund'));
  });

  it('returns stablecoin toolchain for stablecoin_payments intent', () => {
    const tools = engine.discoverToolsByIntent('stablecoin_payments');
    assert.ok(tools.includes('create_stablecoin_payment'));
    assert.ok(tools.includes('list_supported_chains'));
  });

  it('returns x402 toolchain for agentic_payments intent', () => {
    const tools = engine.discoverToolsByIntent('agentic_payments');
    assert.ok(tools.includes('x402_execute_agent_payment'));
    assert.ok(tools.includes('x402_create_payment_intent'));
    assert.ok(tools.includes('x402_settle_intent_onchain'));
    assert.ok(tools.includes('x402_record_incoming_settlement'));
  });
});

// ===========================================================================
// getOrchestrationPlan
// ===========================================================================

describe('ToolDiscoveryEngine.getOrchestrationPlan', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('returns plan for full_checkout', () => {
    const plan = engine.getOrchestrationPlan('full_checkout');
    assert.ok(plan.length > 0);
    assert.ok(plan.includes('create_cart'));
    assert.ok(plan.includes('complete_checkout'));
  });

  it('returns plan for order_fulfillment', () => {
    const plan = engine.getOrchestrationPlan('order_fulfillment');
    assert.ok(plan.length > 0);
    assert.ok(plan.includes('get_order'));
    assert.ok(plan.includes('ship_order'));
  });

  it('returns plan for return_process', () => {
    const plan = engine.getOrchestrationPlan('return_process');
    assert.ok(plan.length > 0);
    assert.ok(plan.includes('create_return'));
    assert.ok(plan.includes('approve_return'));
  });

  it('returns plan for inventory_replenishment', () => {
    const plan = engine.getOrchestrationPlan('inventory_replenishment');
    assert.ok(plan.length > 0);
    assert.ok(plan.includes('get_stock'));
    assert.ok(plan.includes('adjust_inventory'));
  });

  it('returns plan for agent_to_agent_payment', () => {
    const plan = engine.getOrchestrationPlan('agent_to_agent_payment');
    assert.ok(plan.includes('x402_execute_agent_payment'));
    assert.ok(plan.includes('x402_get_intent'));
  });

  it('returns empty array for unknown operation type', () => {
    const plan = engine.getOrchestrationPlan('time_travel');
    assert.deepEqual(plan, []);
  });
});

// ===========================================================================
// getToolInfo
// ===========================================================================

describe('ToolDiscoveryEngine.getToolInfo', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('returns null for unregistered tool', () => {
    assert.equal(engine.getToolInfo('nonexistent'), null);
  });

  it('returns enriched info for registered tool', () => {
    registerTestTool(engine, 'create_order');
    const info = engine.getToolInfo('create_order');
    assert.ok(info);
    assert.equal(info.name, 'create_order');
    assert.ok('executionOrder' in info);
    assert.ok('commonErrors' in info);
    assert.ok('bestPractices' in info);
    assert.ok('performance' in info);
  });
});

// ===========================================================================
// getExecutionOrder
// ===========================================================================

describe('ToolDiscoveryEngine.getExecutionOrder', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('create_order has mustPrecede rules', () => {
    const order = engine.getExecutionOrder('create_order');
    assert.ok(Array.isArray(order.mustPrecede));
    assert.ok(order.mustPrecede.includes('ship_order'));
  });

  it('create_cart has mustPrecede rules', () => {
    const order = engine.getExecutionOrder('create_cart');
    assert.ok(Array.isArray(order.mustPrecede));
    assert.ok(order.mustPrecede.includes('add_cart_item'));
  });

  it('x402_sign_intent has sequencing rules', () => {
    const order = engine.getExecutionOrder('x402_sign_intent');
    assert.ok(Array.isArray(order.mustFollow));
    assert.ok(order.mustFollow.includes('x402_create_payment_intent'));
    assert.ok(Array.isArray(order.mustPrecede));
    assert.ok(order.mustPrecede.includes('x402_settle_intent_onchain'));
  });

  it('x402_execute_agent_payment recommends follow-up intent lookup', () => {
    const order = engine.getExecutionOrder('x402_execute_agent_payment');
    assert.ok(Array.isArray(order.mustPrecede));
    assert.ok(order.mustPrecede.includes('x402_get_intent'));
  });

  it('returns empty object for unknown tool', () => {
    const order = engine.getExecutionOrder('unknown_tool');
    assert.deepEqual(order, {});
  });
});

// ===========================================================================
// getBestPractices
// ===========================================================================

describe('ToolDiscoveryEngine.getBestPractices', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('create_order has best practices', () => {
    const practices = engine.getBestPractices('create_order');
    assert.ok(Array.isArray(practices));
    assert.ok(practices.length > 0);
  });

  it('unknown tool returns empty array', () => {
    const practices = engine.getBestPractices('unknown');
    assert.deepEqual(practices, []);
  });
});

// ===========================================================================
// getPerformanceMetrics
// ===========================================================================

describe('ToolDiscoveryEngine.getPerformanceMetrics', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('list_orders returns metrics', () => {
    const metrics = engine.getPerformanceMetrics('list_orders');
    assert.ok(metrics);
    assert.ok('avgLatency' in metrics);
    assert.ok('p99' in metrics);
    assert.equal(metrics.recommended, true);
  });

  it('unknown tool returns undefined', () => {
    const metrics = engine.getPerformanceMetrics('unknown_tool');
    assert.equal(metrics, undefined);
  });
});

// ===========================================================================
// getToolsByCategory / getToolRelationships / getToolExamples
// ===========================================================================

describe('ToolDiscoveryEngine lookups', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('getToolsByCategory returns tools in category', () => {
    registerTestTool(engine, 'create_order', { category: 'Orders' });
    registerTestTool(engine, 'list_orders', { category: 'Orders' });
    const tools = engine.getToolsByCategory('Orders');
    assert.ok(tools.includes('create_order'));
    assert.ok(tools.includes('list_orders'));
  });

  it('getToolsByCategory returns empty for unknown category', () => {
    assert.deepEqual(engine.getToolsByCategory('NonExistent'), []);
  });

  it('getToolRelationships returns related tools', () => {
    registerTestTool(engine, 'create_order', { relatedTools: ['get_order'] });
    assert.deepEqual(engine.getToolRelationships('create_order'), ['get_order']);
  });

  it('getToolRelationships returns empty for unknown tool', () => {
    assert.deepEqual(engine.getToolRelationships('unknown'), []);
  });

  it('getToolExamples returns examples', () => {
    const examples = [{ input: 'test' }];
    registerTestTool(engine, 'create_order', { examples });
    assert.deepEqual(engine.getToolExamples('create_order'), examples);
  });

  it('getToolExamples returns empty for unknown tool', () => {
    assert.deepEqual(engine.getToolExamples('unknown'), []);
  });
});

// ===========================================================================
// searchTools
// ===========================================================================

describe('ToolDiscoveryEngine.searchTools', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
    registerTestTool(engine, 'create_order', {
      description: 'Create a new order',
      purpose: 'Order creation',
      whenToUse: 'When customer places an order',
    });
    registerTestTool(engine, 'list_customers', {
      description: 'List all customers',
      purpose: 'Customer listing',
      whenToUse: 'When viewing customer data',
    });
  });

  it('finds tool by name keyword', () => {
    const results = engine.searchTools('order');
    assert.ok(results.some((r) => r.name === 'create_order'));
  });

  it('finds tool by description keyword', () => {
    const results = engine.searchTools('customer');
    assert.ok(results.some((r) => r.name === 'list_customers'));
  });

  it('finds tool by purpose keyword', () => {
    const results = engine.searchTools('creation');
    assert.ok(results.some((r) => r.name === 'create_order'));
  });

  it('returns empty for no matches', () => {
    const results = engine.searchTools('quantum');
    assert.equal(results.length, 0);
  });
});

// ===========================================================================
// recommendTools
// ===========================================================================

describe('ToolDiscoveryEngine.recommendTools', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
    registerTestTool(engine, 'create_order', { relatedTools: ['list_orders'] });
    registerTestTool(engine, 'list_orders', { relatedTools: ['get_order'] });
  });

  it('combines intent tools and history-based tools', () => {
    const history = [{ toolUsed: 'create_order' }];
    const recommendations = engine.recommendTools(history, 'place_order');
    assert.ok(recommendations.length > 0);
  });

  it('deduplicates recommendations', () => {
    const history = [{ toolUsed: 'create_order' }];
    const recommendations = engine.recommendTools(history, 'place_order');
    const unique = new Set(recommendations);
    assert.equal(recommendations.length, unique.size);
  });

  it('works with empty history and no intent', () => {
    const recommendations = engine.recommendTools([], null);
    assert.deepEqual(recommendations, []);
  });
});

// ===========================================================================
// exportRegistry
// ===========================================================================

describe('ToolDiscoveryEngine.exportRegistry', () => {
  let engine;
  beforeEach(() => {
    engine = new ToolDiscoveryEngine(null);
  });

  it('returns a serializable object', () => {
    registerTestTool(engine, 'create_order');
    registerTestTool(engine, 'list_orders');
    const exported = engine.exportRegistry();
    assert.ok('create_order' in exported);
    assert.ok('list_orders' in exported);
    assert.ok('category' in exported.create_order);
    assert.ok('description' in exported.create_order);
    assert.ok('examples' in exported.create_order);
    // Should be JSON-serializable
    assert.doesNotThrow(() => JSON.stringify(exported));
  });

  it('returns empty object for empty registry', () => {
    const exported = engine.exportRegistry();
    assert.deepEqual(exported, {});
  });
});

// ===========================================================================
// TOOL_CATEGORIES
// ===========================================================================

describe('TOOL_CATEGORIES', () => {
  it('has CUSTOMERS key', () => {
    assert.equal(TOOL_CATEGORIES.CUSTOMERS, 'Customers');
  });

  it('has ORDERS key', () => {
    assert.equal(TOOL_CATEGORIES.ORDERS, 'Orders');
  });

  it('has INVENTORY key', () => {
    assert.equal(TOOL_CATEGORIES.INVENTORY, 'Inventory');
  });

  it('has CARTS key', () => {
    assert.equal(TOOL_CATEGORIES.CARTS, 'Cart/Checkout');
  });

  it('has ANALYTICS key', () => {
    assert.equal(TOOL_CATEGORIES.ANALYTICS, 'Analytics');
  });

  it('has PAYMENTS key', () => {
    assert.equal(TOOL_CATEGORIES.PAYMENTS, 'Payments');
  });

  it('has SHIPPING key', () => {
    assert.equal(TOOL_CATEGORIES.SHIPPING, 'Shipping');
  });

  it('has TAX key', () => {
    assert.equal(TOOL_CATEGORIES.TAX, 'Tax');
  });

  it('all values are strings', () => {
    for (const [key, val] of Object.entries(TOOL_CATEGORIES)) {
      assert.equal(typeof val, 'string', `${key} should be a string`);
    }
  });
});
