/**
 * Unit tests for agent-definitions.js
 *
 * Tests the AGENTS object and its structure. Since agent-definitions.js
 * imports TOOL_NAMES from mcp-server.js (which has heavy dependencies),
 * we dynamically import the module and validate the exported structure.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert';

let AGENTS;

before(async () => {
  const mod = await import('../../src/agent-definitions.js');
  AGENTS = mod.AGENTS;
});

// ===========================================================================
// AGENTS structure
// ===========================================================================

describe('AGENTS object', () => {
  const EXPECTED_AGENTS = [
    'customer-service',
    'checkout',
    'orders',
    'inventory',
    'returns',
    'analytics',
    'promotions',
    'subscriptions',
    'storefront',
    'sync',
    'manufacturing',
    'payments',
    'stablecoin',
    'shipments',
    'suppliers',
    'invoices',
    'warranties',
    'currency',
    'tax',
    'agents',
  ];

  it('is defined and is a non-null object', () => {
    assert.ok(AGENTS != null && typeof AGENTS === 'object');
  });

  it('contains exactly 20 agent definitions', () => {
    assert.strictEqual(Object.keys(AGENTS).length, 20);
  });

  it('contains all expected agent keys', () => {
    for (const key of EXPECTED_AGENTS) {
      assert.ok(key in AGENTS, `Missing agent: ${key}`);
    }
  });
});

// ===========================================================================
// Agent property validation
// ===========================================================================

describe('agent property requirements', () => {
  it('every agent has a name string', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      assert.ok(
        typeof agent.name === 'string' && agent.name.length > 0,
        `Agent "${key}" is missing or has empty name`,
      );
    }
  });

  it('every agent has a description string', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      assert.ok(
        typeof agent.description === 'string' && agent.description.length > 0,
        `Agent "${key}" is missing or has empty description`,
      );
    }
  });

  it('every agent has a tools array', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      assert.ok(Array.isArray(agent.tools), `Agent "${key}" tools is not an array`);
    }
  });

  it('every agent has a systemPrompt string', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      assert.ok(
        typeof agent.systemPrompt === 'string' && agent.systemPrompt.length > 0,
        `Agent "${key}" is missing or has empty systemPrompt`,
      );
    }
  });

  it('every agent tools array is non-empty', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      assert.ok(agent.tools.length > 0, `Agent "${key}" has an empty tools array`);
    }
  });

  it('every agent tools array contains only strings', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      for (const tool of agent.tools) {
        assert.ok(typeof tool === 'string', `Agent "${key}" has a non-string tool: ${tool}`);
      }
    }
  });
});

// ===========================================================================
// customer-service agent (full-service)
// ===========================================================================

describe('customer-service agent', () => {
  it('has the most tools (uses TOOL_NAMES)', () => {
    const csToolCount = AGENTS['customer-service'].tools.length;
    for (const [key, agent] of Object.entries(AGENTS)) {
      if (key === 'customer-service') continue;
      assert.ok(
        csToolCount >= agent.tools.length,
        `customer-service (${csToolCount}) should have >= tools than "${key}" (${agent.tools.length})`,
      );
    }
  });

  it('has significantly more tools than specialized agents', () => {
    const csToolCount = AGENTS['customer-service'].tools.length;
    // Customer service should have many more tools than a focused agent
    assert.ok(csToolCount > 20, `customer-service should have >20 tools, got ${csToolCount}`);
  });

  it('has name "Customer Service"', () => {
    assert.strictEqual(AGENTS['customer-service'].name, 'Customer Service');
  });
});

// ===========================================================================
// Specialized agents
// ===========================================================================

describe('specialized agent tool scope', () => {
  it('checkout agent has cart-related tools', () => {
    const tools = AGENTS.checkout.tools;
    assert.ok(
      tools.some((t) => t.includes('cart') || t.includes('checkout')),
      'checkout agent should have cart/checkout tools',
    );
  });

  it('orders agent has order-related tools', () => {
    const tools = AGENTS.orders.tools;
    assert.ok(
      tools.some((t) => t.includes('order')),
      'orders agent should have order tools',
    );
  });

  it('inventory agent has stock/inventory tools', () => {
    const tools = AGENTS.inventory.tools;
    assert.ok(
      tools.some((t) => t.includes('stock') || t.includes('inventory')),
      'inventory agent should have stock/inventory tools',
    );
  });

  it('returns agent has return-related tools', () => {
    const tools = AGENTS.returns.tools;
    assert.ok(
      tools.some((t) => t.includes('return')),
      'returns agent should have return tools',
    );
  });

  it('analytics agent has analytics/forecast tools', () => {
    const tools = AGENTS.analytics.tools;
    assert.ok(
      tools.some((t) => t.includes('sales') || t.includes('forecast') || t.includes('metrics')),
      'analytics agent should have analytics tools',
    );
  });

  it('promotions agent has promotion/coupon tools', () => {
    const tools = AGENTS.promotions.tools;
    assert.ok(
      tools.some((t) => t.includes('promotion') || t.includes('coupon')),
      'promotions agent should have promotion/coupon tools',
    );
  });

  it('subscriptions agent has subscription/billing tools', () => {
    const tools = AGENTS.subscriptions.tools;
    assert.ok(
      tools.some((t) => t.includes('subscription') || t.includes('billing')),
      'subscriptions agent should have subscription/billing tools',
    );
  });

  it('sync agent has sync tools', () => {
    const tools = AGENTS.sync.tools;
    assert.ok(
      tools.some((t) => t.includes('sync')),
      'sync agent should have sync tools',
    );
  });
});

// ===========================================================================
// Tool naming and uniqueness
// ===========================================================================

describe('tool naming conventions', () => {
  it('no duplicate tools within any single agent', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      const seen = new Set();
      for (const tool of agent.tools) {
        assert.ok(!seen.has(tool), `Duplicate tool "${tool}" in agent "${key}"`);
        seen.add(tool);
      }
    }
  });

  it('all tool names in specialized agents use mcp__stateset- prefix pattern', () => {
    const prefixPattern = /^mcp__stateset-/;
    for (const [key, agent] of Object.entries(AGENTS)) {
      // Skip customer-service since it uses TOOL_NAMES which are dynamically generated
      if (key === 'customer-service') continue;
      for (const tool of agent.tools) {
        assert.ok(
          prefixPattern.test(tool),
          `Tool "${tool}" in agent "${key}" does not match mcp__stateset- prefix`,
        );
      }
    }
  });

  it('storefront agent uses mcp__stateset-scaffold__ prefix', () => {
    const tools = AGENTS.storefront.tools;
    assert.ok(
      tools.every((t) => t.startsWith('mcp__stateset-scaffold__')),
      'All storefront tools should use mcp__stateset-scaffold__ prefix',
    );
  });

  it('commerce agents use mcp__stateset-commerce__ prefix', () => {
    const commerceAgents = [
      'checkout',
      'orders',
      'inventory',
      'returns',
      'analytics',
      'promotions',
      'subscriptions',
      'sync',
      'manufacturing',
      'payments',
      'shipments',
      'suppliers',
      'invoices',
      'warranties',
      'currency',
      'tax',
    ];
    for (const key of commerceAgents) {
      const tools = AGENTS[key].tools;
      assert.ok(
        tools.every((t) => t.startsWith('mcp__stateset-commerce__')),
        `All "${key}" tools should use mcp__stateset-commerce__ prefix`,
      );
    }
  });

  it('specialized agents have fewer tools than customer-service', () => {
    const csCount = AGENTS['customer-service'].tools.length;
    const specialized = ['checkout', 'orders', 'inventory', 'returns'];
    for (const key of specialized) {
      assert.ok(
        AGENTS[key].tools.length < csCount,
        `"${key}" (${AGENTS[key].tools.length}) should have fewer tools than customer-service (${csCount})`,
      );
    }
  });
});

// ===========================================================================
// System prompt content
// ===========================================================================

describe('system prompt content', () => {
  it('every system prompt mentions safety, rules, or guidelines', () => {
    for (const [key, agent] of Object.entries(AGENTS)) {
      const prompt = agent.systemPrompt.toLowerCase();
      const hasGuidance =
        prompt.includes('safety') ||
        prompt.includes('rules') ||
        prompt.includes('preview') ||
        prompt.includes('guidelines') ||
        prompt.includes('note');
      assert.ok(
        hasGuidance,
        `Agent "${key}" systemPrompt should mention safety, rules, guidelines, or note`,
      );
    }
  });

  it('write-capable agents mention --apply flag', () => {
    const writeAgents = [
      'checkout',
      'orders',
      'inventory',
      'returns',
      'promotions',
      'subscriptions',
      'manufacturing',
      'payments',
      'shipments',
      'suppliers',
      'invoices',
      'warranties',
      'currency',
      'tax',
    ];
    for (const key of writeAgents) {
      assert.ok(
        AGENTS[key].systemPrompt.includes('--apply'),
        `Agent "${key}" systemPrompt should mention --apply`,
      );
    }
  });

  it('analytics agent prompt mentions read-only', () => {
    const prompt = AGENTS.analytics.systemPrompt.toLowerCase();
    assert.ok(
      prompt.includes('read-only') || prompt.includes('read only'),
      'analytics agent prompt should mention read-only',
    );
  });
});
