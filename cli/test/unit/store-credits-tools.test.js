/**
 * Store Credit Tools Test Suite
 *
 * Tests for cli/src/tools/store-credits.js
 * Covers: create_store_credit, get_store_credit, list_store_credits,
 *         adjust_store_credit, apply_store_credit
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { storeCreditTools } from '../../src/tools/store-credits.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = storeCreditTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockCredit = {
  id: 'sc_001',
  customerId: 'cust_001',
  originalAmount: '100.00',
  currentBalance: '75.00',
  currency: 'USD',
  reason: 'refund',
  status: 'active',
  expiresAt: '2027-06-30T23:59:59Z',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-15T00:00:00Z',
};

const mockTransaction = {
  id: 'txn_001',
  creditId: 'sc_001',
  orderId: 'ord_001',
  amount: '25.00',
  createdAt: '2026-01-20T00:00:00Z',
};

function makeStoreCreditCommerce(overrides = {}) {
  return {
    storeCredits: {
      create: async (data) => ({ ...mockCredit, ...data }),
      get: async (_id) => mockCredit,
      list: async (_filters) => [mockCredit],
      count: async (_filters) => 1,
      adjust: async (data) => ({ ...mockCredit, ...data }),
      apply: async (data) => ({ ...mockTransaction, ...data }),
      ...overrides,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('storeCreditTools -- module exports', () => {
  it('exports an array of 5 tools', () => {
    assert.ok(Array.isArray(storeCreditTools));
    assert.equal(storeCreditTools.length, 5);
  });

  it('exports expected tool names in order', () => {
    const names = storeCreditTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'create_store_credit',
      'get_store_credit',
      'list_store_credits',
      'adjust_store_credit',
      'apply_store_credit',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of storeCreditTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of storeCreditTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of storeCreditTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have inputSchema objects', () => {
    for (const tool of storeCreditTools) {
      assert.equal(typeof tool.inputSchema, 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('storeCreditTools -- input schemas', () => {
  it('create_store_credit has customerId, amount, currency, reason, note, expiresAt', () => {
    const schema = findTool('create_store_credit').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('amount'));
    assert.ok(keys.includes('currency'));
    assert.ok(keys.includes('reason'));
    assert.ok(keys.includes('note'));
    assert.ok(keys.includes('expiresAt'));
  });

  it('get_store_credit has creditId', () => {
    const schema = findTool('get_store_credit').inputSchema;
    assert.ok(Object.keys(schema).includes('creditId'));
  });

  it('list_store_credits has customerId, status, limit', () => {
    const schema = findTool('list_store_credits').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('status'));
    assert.ok(keys.includes('limit'));
  });

  it('adjust_store_credit has creditId, amount, reason', () => {
    const schema = findTool('adjust_store_credit').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('creditId'));
    assert.ok(keys.includes('amount'));
    assert.ok(keys.includes('reason'));
  });

  it('apply_store_credit has creditId, orderId, amount', () => {
    const schema = findTool('apply_store_credit').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('creditId'));
    assert.ok(keys.includes('orderId'));
    assert.ok(keys.includes('amount'));
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('storeCreditTools -- permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(findTool('get_store_credit').permission, 'read');
    assert.equal(findTool('list_store_credits').permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(findTool('create_store_credit').permission, 'write');
    assert.equal(findTool('adjust_store_credit').permission, 'write');
    assert.equal(findTool('apply_store_credit').permission, 'write');
  });
});

// ============================================================================
// Handler apply-guard (write tools without --apply)
// ============================================================================

describe('storeCreditTools -- apply-guard', () => {
  it('create_store_credit requires --apply', async () => {
    const tool = findTool('create_store_credit');
    const result = await tool.handler({
      params: { customerId: 'cust_001', amount: 50 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('adjust_store_credit requires --apply', async () => {
    const tool = findTool('adjust_store_credit');
    const result = await tool.handler({
      params: { creditId: 'sc_001', amount: 10, reason: 'test' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('apply_store_credit requires --apply', async () => {
    const tool = findTool('apply_store_credit');
    const result = await tool.handler({
      params: { creditId: 'sc_001', orderId: 'ord_001', amount: 25 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('apply-guard returns hint about --apply', async () => {
    const tool = findTool('create_store_credit');
    const result = await tool.handler({
      params: { customerId: 'cust_001', amount: 50 },
      allowApply: false,
      commerce: {},
    });
    assert.ok(result.hint);
    assert.ok(result.hint.includes('--apply'));
  });

  it('apply-guard returns preview (wouldDo) with params', async () => {
    const params = { creditId: 'sc_001', amount: -10, reason: 'correction' };
    const tool = findTool('adjust_store_credit');
    const result = await tool.handler({ params, allowApply: false, commerce: {} });
    assert.equal(result.success, false);
    assert.deepStrictEqual(result.wouldDo, params);
  });
});

// ============================================================================
// Handler success paths (with mocked commerce)
// ============================================================================

describe('storeCreditTools -- create_store_credit handler', () => {
  it('creates store credit when allowApply is true', async () => {
    const tool = findTool('create_store_credit');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { customerId: 'cust_001', amount: 100 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Store credit issued');
    assert.ok(result.credit);
  });
});

describe('storeCreditTools -- get_store_credit handler', () => {
  it('returns store credit with expected fields', async () => {
    const tool = findTool('get_store_credit');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.credit);
    assert.equal(result.credit.id, 'sc_001');
    assert.equal(result.credit.customerId, 'cust_001');
    assert.equal(result.credit.originalAmount, '100.00');
    assert.equal(result.credit.currentBalance, '75.00');
    assert.equal(result.credit.currency, 'USD');
    assert.equal(result.credit.reason, 'refund');
    assert.equal(result.credit.status, 'active');
  });

  it('returns not found when credit is null', async () => {
    const tool = findTool('get_store_credit');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce({ get: async () => null }),
      params: { creditId: 'sc_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Store credit not found');
  });
});

describe('storeCreditTools -- list_store_credits handler', () => {
  it('returns list with totalCount and returned', async () => {
    const tool = findTool('list_store_credits');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.credits));
    assert.equal(result.credits[0].id, 'sc_001');
  });

  it('maps expected fields on each credit', async () => {
    const tool = findTool('list_store_credits');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: {},
    });
    const c = result.credits[0];
    const expectedKeys = [
      'id',
      'customerId',
      'originalAmount',
      'currentBalance',
      'currency',
      'reason',
      'status',
      'expiresAt',
      'createdAt',
    ];
    for (const key of expectedKeys) {
      assert.ok(key in c, `missing key: ${key}`);
    }
  });
});

describe('storeCreditTools -- adjust_store_credit handler', () => {
  it('adjusts store credit when allowApply is true', async () => {
    const tool = findTool('adjust_store_credit');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', amount: 25, reason: 'goodwill bonus' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Store credit adjusted');
    assert.ok(result.credit);
  });
});

describe('storeCreditTools -- apply_store_credit handler', () => {
  it('applies store credit to order when allowApply is true', async () => {
    const tool = findTool('apply_store_credit');
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', orderId: 'ord_001', amount: 25 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Store credit applied to order');
    assert.ok(result.transaction);
  });
});

// ============================================================================
// Handler error paths (commerce object missing methods)
// ============================================================================

describe('storeCreditTools -- error paths', () => {
  it('get_store_credit throws when commerce.storeCredits is undefined', async () => {
    const tool = findTool('get_store_credit');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { creditId: 'sc_001' } }),
      (err) => err instanceof TypeError,
    );
  });

  it('list_store_credits throws when commerce.storeCredits is undefined', async () => {
    const tool = findTool('list_store_credits');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: {} }),
      (err) => err instanceof TypeError,
    );
  });

  it('create_store_credit throws when commerce.storeCredits.create is missing', async () => {
    const tool = findTool('create_store_credit');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { storeCredits: {} },
          params: { customerId: 'cust_001', amount: 50 },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('adjust_store_credit throws when commerce.storeCredits.adjust is missing', async () => {
    const tool = findTool('adjust_store_credit');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { storeCredits: {} },
          params: { creditId: 'sc_001', amount: 10, reason: 'test' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('apply_store_credit throws when commerce.storeCredits.apply is missing', async () => {
    const tool = findTool('apply_store_credit');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { storeCredits: {} },
          params: { creditId: 'sc_001', orderId: 'ord_001', amount: 25 },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });
});
