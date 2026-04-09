import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';

import {
  ToolRegistry,
  AGENT_TOOL_CATEGORIES,
  createToolRegistry,
} from '../../src/tools/index.js';
import { getStaticMcpToolDefinitions } from '../../src/mcp-server.js';

// ============================================================================
// ToolRegistry — constructor
// ============================================================================

describe('ToolRegistry', () => {
  let registry;

  beforeEach(() => {
    registry = new ToolRegistry();
  });

  describe('constructor', () => {
    it('starts with empty tools map', () => {
      assert.equal(registry.size, 0);
    });

    it('starts with empty loadedCategories set', () => {
      assert.equal(registry.loadedCategories.size, 0);
    });
  });

  // --------------------------------------------------------------------------
  // loadCategory
  // --------------------------------------------------------------------------

  describe('loadCategory', () => {
    it('loads customers category (sync)', async () => {
      await registry.loadCategory('customers');
      assert.ok(registry.loadedCategories.has('customers'));
      assert.ok(registry.size > 0);
    });

    it('loads orders category', async () => {
      await registry.loadCategory('orders');
      assert.ok(registry.loadedCategories.has('orders'));
    });

    it('loads vector category', async () => {
      await registry.loadCategory('vector');
      assert.ok(registry.loadedCategories.has('vector'));
    });

    it('does not reload an already-loaded category', async () => {
      await registry.loadCategory('customers');
      const size1 = registry.size;
      await registry.loadCategory('customers');
      assert.equal(registry.size, size1);
    });

    it('throws for unknown category', async () => {
      await assert.rejects(() => registry.loadCategory('nonexistent'), /Unknown tool category/);
    });

    it('assigns category to each loaded tool', async () => {
      await registry.loadCategory('customers');
      const tools = registry.getByCategory('customers');
      assert.ok(tools.length > 0);
      for (const tool of tools) {
        assert.equal(tool.category, 'customers');
      }
    });
  });

  // --------------------------------------------------------------------------
  // loadForAgent
  // --------------------------------------------------------------------------

  describe('loadForAgent', () => {
    it('loads categories for known agent', async () => {
      await registry.loadForAgent('analytics');
      assert.ok(registry.loadedCategories.has('analytics'));
    });

    it('loads categories for the agents agent', async () => {
      await registry.loadForAgent('agents');
      assert.ok(registry.loadedCategories.has('agent-runtime'));
      assert.ok(registry.loadedCategories.has('agent-cards'));
      assert.ok(registry.loadedCategories.has('a2a'));
    });

    it('treats storefront as a valid no-op scope in the commerce registry', async () => {
      await registry.loadForAgent('storefront');
      assert.equal(registry.size, 0);
    });

    it('does nothing for unknown agent', async () => {
      // Should not throw
      await registry.loadForAgent('nonexistent-agent');
      assert.equal(registry.size, 0);
    });
  });

  // --------------------------------------------------------------------------
  // get / has
  // --------------------------------------------------------------------------

  describe('get', () => {
    it('returns tool by name after loading', async () => {
      await registry.loadCategory('customers');
      const tools = registry.getAll();
      const firstName = tools[0].name;
      const tool = registry.get(firstName);
      assert.ok(tool);
      assert.equal(tool.name, firstName);
    });

    it('returns undefined for non-loaded tool', () => {
      assert.equal(registry.get('nonexistent'), undefined);
    });
  });

  describe('has', () => {
    it('returns true for loaded tool', async () => {
      await registry.loadCategory('customers');
      const tools = registry.getAll();
      assert.ok(registry.has(tools[0].name));
    });

    it('returns false for non-existent tool', () => {
      assert.equal(registry.has('nope'), false);
    });
  });

  // --------------------------------------------------------------------------
  // getAll / getByCategory / getByPermission
  // --------------------------------------------------------------------------

  describe('getAll', () => {
    it('returns empty array when nothing loaded', () => {
      assert.deepEqual(registry.getAll(), []);
    });

    it('returns all loaded tools', async () => {
      await registry.loadCategory('customers');
      await registry.loadCategory('orders');
      const all = registry.getAll();
      assert.ok(all.length > 0);
    });
  });

  describe('getByCategory', () => {
    it('filters tools by category', async () => {
      await registry.loadCategory('customers');
      await registry.loadCategory('orders');
      const customerTools = registry.getByCategory('customers');
      const orderTools = registry.getByCategory('orders');
      assert.ok(customerTools.length > 0);
      assert.ok(orderTools.length > 0);
      // They should be disjoint
      const customerNames = new Set(customerTools.map((t) => t.name));
      for (const tool of orderTools) {
        assert.ok(!customerNames.has(tool.name), `${tool.name} should not be in customers`);
      }
    });
  });

  describe('getByPermission', () => {
    it('filters tools by permission level', async () => {
      await registry.loadCategory('customers');
      const readTools = registry.getByPermission('read');
      for (const tool of readTools) {
        assert.equal(tool.permission, 'read');
      }
    });
  });

  describe('getReadOnly', () => {
    it('returns only read-permission tools', async () => {
      await registry.loadCategory('customers');
      const readOnly = registry.getReadOnly();
      for (const tool of readOnly) {
        assert.equal(tool.permission, 'read');
      }
    });
  });

  describe('getWriteTools', () => {
    it('returns write/delete/admin tools', async () => {
      await registry.loadCategory('customers');
      const writeTools = registry.getWriteTools();
      for (const tool of writeTools) {
        assert.ok(
          ['write', 'delete', 'admin'].includes(tool.permission),
          `Expected write/delete/admin, got ${tool.permission}`,
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // toMcpFormat
  // --------------------------------------------------------------------------

  describe('toMcpFormat', () => {
    it('returns MCP-formatted tool definitions', async () => {
      await registry.loadCategory('customers');
      const mcpTools = registry.toMcpFormat({});
      assert.ok(mcpTools.length > 0);
      for (const tool of mcpTools) {
        assert.ok(tool.name);
        assert.ok(tool.description);
        assert.ok(tool.inputSchema);
        assert.equal(typeof tool.handler, 'function');
      }
    });

    it('handler wraps results in MCP content format', async () => {
      await registry.loadCategory('customers');
      const mcpTools = registry.toMcpFormat({
        commerce: { listCustomers: async () => [{ id: 1 }] },
      });
      // Just check the handler exists and is callable
      const listTool = mcpTools.find((t) => t.name === 'list_customers');
      if (listTool) {
        // The handler will likely fail without a real commerce client, but it wraps errors
        const result = await listTool.handler({});
        assert.ok(result.content);
        assert.ok(Array.isArray(result.content));
        assert.equal(result.content[0].type, 'text');
      }
    });
  });

  // --------------------------------------------------------------------------
  // size
  // --------------------------------------------------------------------------

  describe('size', () => {
    it('returns 0 initially', () => {
      assert.equal(registry.size, 0);
    });

    it('increases after loading', async () => {
      await registry.loadCategory('customers');
      assert.ok(registry.size > 0);
    });
  });
});

// ============================================================================
// AGENT_TOOL_CATEGORIES
// ============================================================================

describe('AGENT_TOOL_CATEGORIES', () => {
  it('is an object with string keys and array values', () => {
    assert.equal(typeof AGENT_TOOL_CATEGORIES, 'object');
    for (const [key, cats] of Object.entries(AGENT_TOOL_CATEGORIES)) {
      assert.ok(typeof key === 'string');
      assert.ok(Array.isArray(cats), `${key} should map to an array`);
    }
  });

  it('includes the current live agents plus compatibility scopes', () => {
    assert.ok(Object.keys(AGENT_TOOL_CATEGORIES).length >= 20);
  });

  it('customer-service agent has broad tool access', () => {
    const cats = AGENT_TOOL_CATEGORIES['customer-service'];
    assert.ok(cats.includes('customers'));
    assert.ok(cats.includes('orders'));
    assert.ok(cats.includes('products'));
    assert.ok(cats.includes('inventory'));
  });

  it('checkout agent includes carts', () => {
    assert.ok(AGENT_TOOL_CATEGORIES.checkout.includes('carts'));
  });

  it('analytics agent includes analytics', () => {
    assert.ok(AGENT_TOOL_CATEGORIES.analytics.includes('analytics'));
  });

  it('agents mapping includes multi-agent runtime categories', () => {
    assert.ok(AGENT_TOOL_CATEGORIES.agents.includes('agent-runtime'));
    assert.ok(AGENT_TOOL_CATEGORIES.agents.includes('agent-cards'));
    assert.ok(AGENT_TOOL_CATEGORIES.agents.includes('a2a'));
  });

  it('storefront mapping exists even though scaffold tools are served elsewhere', () => {
    assert.ok('storefront' in AGENT_TOOL_CATEGORIES);
    assert.deepStrictEqual(AGENT_TOOL_CATEGORIES.storefront, []);
  });
});

// ============================================================================
// createToolRegistry
// ============================================================================

describe('createToolRegistry', () => {
  it('returns a new ToolRegistry instance', () => {
    const r = createToolRegistry();
    assert.ok(r instanceof ToolRegistry);
    assert.equal(r.size, 0);
  });

  it('returns distinct instances', () => {
    const a = createToolRegistry();
    const b = createToolRegistry();
    assert.notEqual(a, b);
  });
});

describe('ToolRegistry parity with live commerce MCP export', () => {
  it('loadAll matches the static commerce MCP tool set', async () => {
    const registry = createToolRegistry();
    await registry.loadAll();

    const registryNames = new Set(registry.getAll().map((tool) => tool.name));
    const staticNames = new Set(getStaticMcpToolDefinitions().map((tool) => tool.name));

    assert.equal(registryNames.size, staticNames.size);
    assert.deepStrictEqual([...registryNames].sort(), [...staticNames].sort());
  });
});
