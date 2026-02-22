import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ToolDiscoveryEngine } from '../../src/mcp-tool-discovery.js';

function makeEngine() {
  return new ToolDiscoveryEngine(null);
}

describe('ToolDiscoveryEngine', () => {
  it('constructor creates an instance with empty registries', () => {
    const engine = makeEngine();
    assert.ok(engine instanceof ToolDiscoveryEngine);
    assert.ok(engine.toolRegistry instanceof Map);
    assert.equal(engine.toolRegistry.size, 0);
    assert.ok(engine.toolCategories instanceof Map);
    assert.ok(engine.toolExamples instanceof Map);
    assert.ok(engine.toolRelationships instanceof Map);
  });

  it('registerFromToolDefs registers tools from an array of { name, description, permission } objects', () => {
    const engine = makeEngine();
    const toolDefs = [
      { name: 'create_order', description: 'Creates a new order', permission: 'write' },
      { name: 'list_orders', description: 'Lists all orders', permission: 'read' },
    ];
    engine.registerFromToolDefs(toolDefs);
    assert.equal(engine.toolRegistry.size, 2);
    assert.ok(engine.toolRegistry.has('create_order'));
    assert.ok(engine.toolRegistry.has('list_orders'));
  });

  it('registerFromToolDefs skips entries without a name', () => {
    const engine = makeEngine();
    const toolDefs = [
      { description: 'No name here', permission: 'read' },
      { name: 'valid_tool', description: 'Has a name', permission: 'read' },
    ];
    engine.registerFromToolDefs(toolDefs);
    assert.equal(engine.toolRegistry.size, 1);
    assert.ok(engine.toolRegistry.has('valid_tool'));
  });

  it('registerFromToolDefs skips entries without a description', () => {
    const engine = makeEngine();
    const toolDefs = [
      { name: 'no_desc_tool', permission: 'read' },
      { name: 'full_tool', description: 'Has description', permission: 'read' },
    ];
    engine.registerFromToolDefs(toolDefs);
    assert.equal(engine.toolRegistry.size, 1);
    assert.ok(engine.toolRegistry.has('full_tool'));
  });

  describe('discover', () => {
    it('returns an array for a known intent', () => {
      const engine = makeEngine();
      const results = engine.discover('place_order');
      assert.ok(Array.isArray(results));
      assert.ok(results.length > 0, 'Expected at least one result for "place_order"');
    });

    it('returns an empty array for an unknown intent with no registry matches', () => {
      const engine = makeEngine();
      const results = engine.discover('completely_unknown_intent_xyz');
      assert.ok(Array.isArray(results));
      assert.equal(results.length, 0);
    });

    it('respects the limit parameter', () => {
      const engine = makeEngine();
      // Register enough tools to exceed the limit
      const toolDefs = Array.from({ length: 10 }, (_, i) => ({
        name: `order_tool_${i}`,
        description: `Order tool number ${i}`,
      }));
      engine.registerFromToolDefs(toolDefs);

      const limit = 3;
      const results = engine.discover('order', limit);
      assert.ok(results.length <= limit, `Expected at most ${limit} results, got ${results.length}`);
    });
  });

  describe('discoverToolsByIntent', () => {
    it('returns tools for the "place_order" intent', () => {
      const engine = makeEngine();
      const tools = engine.discoverToolsByIntent('place_order');
      assert.ok(Array.isArray(tools));
      assert.ok(tools.length > 0, 'Expected tools for "place_order"');
      assert.ok(tools.includes('create_order'), 'Expected "create_order" in place_order tools');
    });

    it('returns tools for the "check_inventory" intent', () => {
      const engine = makeEngine();
      const tools = engine.discoverToolsByIntent('check_inventory');
      assert.ok(Array.isArray(tools));
      assert.ok(tools.length > 0, 'Expected tools for "check_inventory"');
      assert.ok(tools.includes('get_stock'), 'Expected "get_stock" in check_inventory tools');
    });
  });

  describe('getOrchestrationPlan', () => {
    it('returns an array for the "full_checkout" operation type', () => {
      const engine = makeEngine();
      const plan = engine.getOrchestrationPlan('full_checkout');
      assert.ok(Array.isArray(plan));
      assert.ok(plan.length > 0, 'Expected steps in full_checkout plan');
      assert.ok(plan.includes('create_cart'), 'Expected "create_cart" in full_checkout plan');
      assert.ok(plan.includes('complete_checkout'), 'Expected "complete_checkout" in full_checkout plan');
    });
  });
});
