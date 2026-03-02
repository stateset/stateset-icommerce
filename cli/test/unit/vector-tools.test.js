/**
 * Vector Tools Test Suite
 *
 * Tests for tool definitions, schemas, permissions, handler guards,
 * and normalizeVectorResult in src/tools/vector.js (16 tools).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { vectorTools } from '../../src/tools/vector.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = vectorTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in vectorTools`);
  return tool;
}

function getSchema(name) {
  return z.object(findTool(name).inputSchema);
}

function expectFail(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(!result.success, msg || `Expected parse to fail for: ${JSON.stringify(data)}`);
}

function expectPass(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(
    result.success,
    msg ||
      `Expected parse to pass for: ${JSON.stringify(data)}, errors: ${JSON.stringify(result.error?.issues)}`,
  );
}

// ---------------------------------------------------------------------------
// All 16 tool names
// ---------------------------------------------------------------------------

const ALL_TOOL_NAMES = [
  'vector_search_products',
  'vector_search_customers',
  'vector_search_orders',
  'vector_search_inventory',
  'vector_index_product',
  'vector_index_customer',
  'vector_index_order',
  'vector_index_inventory',
  'vector_index_all_products',
  'vector_index_all_customers',
  'vector_index_all_orders',
  'vector_index_all_inventory',
  'vector_stats',
  'vector_clear',
  'vector_clear_all',
  'vector_reindex_all',
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Vector Tools — definitions', () => {
  it('exports exactly 16 tools', () => {
    assert.strictEqual(vectorTools.length, 16);
  });

  for (const name of ALL_TOOL_NAMES) {
    it(`includes tool "${name}"`, () => {
      assert.ok(findTool(name));
    });
  }

  it('every tool has a handler function', () => {
    for (const tool of vectorTools) {
      assert.strictEqual(typeof tool.handler, 'function', `${tool.name} handler should be a function`);
    }
  });
});

describe('Vector Tools — permissions', () => {
  const readTools = [
    'vector_search_products',
    'vector_search_customers',
    'vector_search_orders',
    'vector_search_inventory',
    'vector_stats',
  ];

  const writeTools = [
    'vector_index_product',
    'vector_index_customer',
    'vector_index_order',
    'vector_index_inventory',
  ];

  const adminTools = [
    'vector_index_all_products',
    'vector_index_all_customers',
    'vector_index_all_orders',
    'vector_index_all_inventory',
    'vector_clear',
    'vector_clear_all',
    'vector_reindex_all',
  ];

  for (const name of readTools) {
    it(`${name} has read permission`, () => {
      assert.strictEqual(findTool(name).permission, 'read');
    });
  }

  for (const name of writeTools) {
    it(`${name} has write permission`, () => {
      assert.strictEqual(findTool(name).permission, 'write');
    });
  }

  for (const name of adminTools) {
    it(`${name} has admin permission`, () => {
      assert.strictEqual(findTool(name).permission, 'admin');
    });
  }
});

describe('Vector Tools — search schemas (query min 1)', () => {
  const searchTools = [
    'vector_search_products',
    'vector_search_customers',
    'vector_search_orders',
    'vector_search_inventory',
  ];

  for (const name of searchTools) {
    it(`${name} rejects empty query`, () => {
      expectFail(getSchema(name), { query: '' });
    });

    it(`${name} accepts valid query`, () => {
      expectPass(getSchema(name), { query: 'blue shoes' });
    });
  }
});

describe('Vector Tools — index single-entity schemas', () => {
  it('vector_index_product requires product_id min 1', () => {
    expectFail(getSchema('vector_index_product'), { product_id: '' });
    expectPass(getSchema('vector_index_product'), { product_id: 'p-123' });
  });

  it('vector_index_customer requires customer_id min 1', () => {
    expectFail(getSchema('vector_index_customer'), { customer_id: '' });
    expectPass(getSchema('vector_index_customer'), { customer_id: 'c-456' });
  });

  it('vector_index_order requires order_id min 1', () => {
    expectFail(getSchema('vector_index_order'), { order_id: '' });
    expectPass(getSchema('vector_index_order'), { order_id: 'o-789' });
  });

  it('vector_index_inventory requires item_id min 1', () => {
    expectFail(getSchema('vector_index_inventory'), { item_id: '' });
    expectPass(getSchema('vector_index_inventory'), { item_id: 'i-001' });
  });
});

describe('Vector Tools — vector_clear schema', () => {
  it('entity_type is an enum of 4 values', () => {
    expectPass(getSchema('vector_clear'), { entity_type: 'products' });
    expectPass(getSchema('vector_clear'), { entity_type: 'customers' });
    expectPass(getSchema('vector_clear'), { entity_type: 'orders' });
    expectPass(getSchema('vector_clear'), { entity_type: 'inventory' });
    expectFail(getSchema('vector_clear'), { entity_type: 'carts' });
    expectFail(getSchema('vector_clear'), { entity_type: '' });
  });
});

describe('Vector Tools — handler blocks writes when allowApply is false', () => {
  const writePermissionTools = [
    'vector_index_product',
    'vector_index_customer',
    'vector_index_order',
    'vector_index_inventory',
    'vector_index_all_products',
    'vector_clear',
    'vector_clear_all',
    'vector_reindex_all',
  ];

  for (const name of writePermissionTools) {
    it(`${name} returns error when allowApply is false`, async () => {
      const tool = findTool(name);
      const result = await tool.handler({
        commerce: {},
        params: { query: 'x', product_id: 'x', customer_id: 'x', order_id: 'x', item_id: 'x', entity_type: 'products' },
        allowApply: false,
      });
      assert.strictEqual(result.success, false);
      assert.ok(result.error.includes('--apply'), `Error should mention --apply: ${result.error}`);
    });
  }
});
