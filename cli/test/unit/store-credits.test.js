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

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockCredit = {
  id: 'sc_001',
  customerId: 'cust_001',
  originalAmount: '50.00',
  currentBalance: '35.00',
  currency: 'USD',
  reason: 'refund',
  status: 'active',
  expiresAt: null,
  createdAt: '2026-02-21T00:00:00Z',
  updatedAt: '2026-02-21T00:00:00Z',
};

const mockTx = {
  id: 'tx_001',
  creditId: 'sc_001',
  orderId: 'ord_001',
  amount: '20.00',
  type: 'apply',
  createdAt: '2026-02-21T00:00:00Z',
};

function makeStoreCreditCommerce(overrides = {}) {
  return {
    storeCredits: {
      create: async (data) => ({ ...mockCredit, ...data }),
      get: async (_id) => mockCredit,
      list: async () => [mockCredit],
      count: async () => 1,
      adjust: async (_data) => ({ ...mockCredit, currentBalance: '45.00' }),
      apply: async (_data) => mockTx,
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Store Credit Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(storeCreditTools));
  });

  it('has at least 5 tools', () => {
    assert.ok(storeCreditTools.length >= 5, `Expected >= 5, got ${storeCreditTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of storeCreditTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// create_store_credit
// ============================================================================

describe('create_store_credit', () => {
  const tool = findTool(storeCreditTools, 'create_store_credit');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { customerId: 'cust_001', amount: 50 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates store credit with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { customerId: 'cust_001', amount: 50, currency: 'USD', reason: 'refund' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('issued'));
    assert.ok(result.credit);
    assert.equal(result.credit.customerId, 'cust_001');
  });

  it('returns error when commerce.storeCredits.create throws', async () => {
    const commerce = makeStoreCreditCommerce({
      create: async () => {
        throw new Error('Customer not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { customerId: 'bad_id', amount: 50 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Customer not found'));
    }
  });
});

// ============================================================================
// get_store_credit
// ============================================================================

describe('get_store_credit', () => {
  const tool = findTool(storeCreditTools, 'get_store_credit');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns credit for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.credit.id, 'sc_001');
    assert.equal(result.credit.customerId, 'cust_001');
    assert.equal(result.credit.originalAmount, '50.00');
    assert.equal(result.credit.currentBalance, '35.00');
    assert.equal(result.credit.currency, 'USD');
    assert.equal(result.credit.reason, 'refund');
    assert.equal(result.credit.status, 'active');
  });

  it('returns success: false when credit not found', async () => {
    const commerce = makeStoreCreditCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { creditId: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeStoreCreditCommerce({
      get: async () => {
        throw new Error('DB lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { creditId: 'sc_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB lookup failed'));
    }
  });
});

// ============================================================================
// list_store_credits
// ============================================================================

describe('list_store_credits', () => {
  const tool = findTool(storeCreditTools, 'list_store_credits');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with totalCount and returned', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.credits.length, 1);
    assert.equal(result.credits[0].id, 'sc_001');
  });

  it('maps all expected fields on each credit', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: {},
    });
    const c = result.credits[0];
    assert.ok('id' in c);
    assert.ok('customerId' in c);
    assert.ok('originalAmount' in c);
    assert.ok('currentBalance' in c);
    assert.ok('currency' in c);
    assert.ok('reason' in c);
    assert.ok('status' in c);
    assert.ok('expiresAt' in c);
    assert.ok('createdAt' in c);
  });

  it('returns error when list throws', async () => {
    const commerce = makeStoreCreditCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: {} });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// adjust_store_credit
// ============================================================================

describe('adjust_store_credit', () => {
  const tool = findTool(storeCreditTools, 'adjust_store_credit');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', amount: 10, reason: 'goodwill' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('adjusts credit with allowApply: true (positive amount)', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', amount: 10, reason: 'goodwill compensation' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('adjusted'));
    assert.ok(result.credit);
  });

  it('adjusts credit with negative amount (deduction)', async () => {
    let calledWith = null;
    const commerce = makeStoreCreditCommerce({
      adjust: async (data) => {
        calledWith = data;
        return { ...mockCredit, currentBalance: '25.00' };
      },
    });
    const result = await tool.handler({
      commerce,
      params: { creditId: 'sc_001', amount: -10, reason: 'correction' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(calledWith.amount, '-10');
  });

  it('returns error when adjust throws', async () => {
    const commerce = makeStoreCreditCommerce({
      adjust: async () => {
        throw new Error('Balance would go negative');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { creditId: 'sc_001', amount: -999, reason: 'test' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Balance would go negative'));
    }
  });
});

// ============================================================================
// apply_store_credit
// ============================================================================

describe('apply_store_credit', () => {
  const tool = findTool(storeCreditTools, 'apply_store_credit');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', orderId: 'ord_001', amount: 20 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('applies credit to order with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeStoreCreditCommerce(),
      params: { creditId: 'sc_001', orderId: 'ord_001', amount: 20 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('applied'));
    assert.ok(result.transaction);
    assert.equal(result.transaction.orderId, 'ord_001');
  });

  it('passes correct amount string to apply()', async () => {
    let calledWith = null;
    const commerce = makeStoreCreditCommerce({
      apply: async (data) => {
        calledWith = data;
        return mockTx;
      },
    });
    await tool.handler({
      commerce,
      params: { creditId: 'sc_001', orderId: 'ord_001', amount: 20.5 },
      allowApply: true,
    });
    assert.equal(calledWith.amount, '20.5');
    assert.equal(calledWith.creditId, 'sc_001');
    assert.equal(calledWith.orderId, 'ord_001');
  });

  it('returns error when apply throws', async () => {
    const commerce = makeStoreCreditCommerce({
      apply: async () => {
        throw new Error('Order not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { creditId: 'sc_001', orderId: 'bad_ord', amount: 20 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Order not found'));
    }
  });
});
