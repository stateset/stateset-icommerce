/**
 * Returns Tools — Comprehensive Test Suite
 *
 * Tests every tool exported from src/tools/returns.js:
 *   list_returns, get_return, create_return, approve_return, reject_return
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { returnTools } from '../../src/tools/returns.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = returnTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in returnTools`);
  return tool;
}

function makeReturn(overrides = {}) {
  return {
    id: 'ret_001',
    orderId: 'ord_001',
    status: 'pending',
    reason: 'defective',
    reasonDetails: 'Screen cracked on arrival',
    items: [{ orderItemId: 'item_001', quantity: 1 }],
    createdAt: '2026-02-20T00:00:00Z',
    ...overrides,
  };
}

function makeCommerce(overrides = {}) {
  return {
    returns: {
      list: async () => [makeReturn()],
      count: async () => 1,
      get: async (id) => (id === 'nonexistent' ? null : makeReturn({ id })),
      create: async (data) => makeReturn({ id: 'ret_new', ...data }),
      approve: async (id) => makeReturn({ id, status: 'approved' }),
      reject: async (id, reason) => makeReturn({ id, status: 'rejected', reason }),
      ...overrides,
    },
  };
}

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Return Tools — structure', () => {
  it('exports an array of 12 tools', () => {
    assert.ok(Array.isArray(returnTools));
    assert.strictEqual(returnTools.length, 12);
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of returnTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = returnTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });
});

// ---------------------------------------------------------------------------
// list_returns
// ---------------------------------------------------------------------------

describe('list_returns', () => {
  const tool = findTool('list_returns');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns success with returns array and totalCount', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { limit: 50 } });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.returns));
    assert.strictEqual(result.totalCount, 1);
    assert.strictEqual(result.returned, 1);
  });

  it('maps return fields correctly', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { limit: 50 } });
    const ret = result.returns[0];
    assert.strictEqual(ret.id, 'ret_001');
    assert.strictEqual(ret.orderId, 'ord_001');
    assert.strictEqual(ret.status, 'pending');
    assert.strictEqual(ret.reason, 'defective');
  });

  it('respects limit parameter', async () => {
    const commerce = makeCommerce({
      list: async () => Array.from({ length: 30 }, (_, i) => makeReturn({ id: `ret_${i}` })),
      count: async () => 30,
    });
    const result = await tool.handler({ commerce, params: { limit: 10 } });
    assert.strictEqual(result.returned, 10);
    assert.strictEqual(result.totalCount, 30);
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      list: async () => {
        throw new Error('DB fail');
      },
    });
    await assert.rejects(() => tool.handler({ commerce, params: { limit: 50 } }), /DB fail/);
  });
});

// ---------------------------------------------------------------------------
// get_return
// ---------------------------------------------------------------------------

describe('get_return', () => {
  const tool = findTool('get_return');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns return by ID', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { returnId: 'ret_001' },
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.return);
    assert.strictEqual(result.return.id, 'ret_001');
  });

  it('returns error when return not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { returnId: 'nonexistent' },
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      get: async () => {
        throw new Error('timeout');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { returnId: 'ret_001' } }),
      /timeout/,
    );
  });
});

// ---------------------------------------------------------------------------
// create_return
// ---------------------------------------------------------------------------

describe('create_return', () => {
  const tool = findTool('create_return');
  const params = {
    orderId: 'ord_001',
    reason: 'defective',
    items: [{ orderItemId: 'item_001', quantity: 1 }],
  };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldCreate);
    assert.strictEqual(result.wouldCreate.orderId, 'ord_001');
  });

  it('creates return when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.return);
    assert.strictEqual(result.return.orderId, 'ord_001');
    assert.strictEqual(result.return.reason, 'defective');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      create: async () => {
        throw new Error('Duplicate');
      },
    });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Duplicate/);
  });
});

// ---------------------------------------------------------------------------
// approve_return
// ---------------------------------------------------------------------------

describe('approve_return', () => {
  const tool = findTool('approve_return');
  const params = { returnId: 'ret_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldApprove);
    assert.strictEqual(result.wouldApprove.returnId, 'ret_001');
  });

  it('approves return when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.return.status, 'approved');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      approve: async () => {
        throw new Error('Already approved');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params, allowApply: true }),
      /Already approved/,
    );
  });
});

// ---------------------------------------------------------------------------
// reject_return
// ---------------------------------------------------------------------------

describe('reject_return', () => {
  const tool = findTool('reject_return');
  const params = { returnId: 'ret_001', reason: 'Outside return window' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldReject);
    assert.strictEqual(result.wouldReject.reason, 'Outside return window');
  });

  it('rejects return when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.return.status, 'rejected');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      reject: async () => {
        throw new Error('Already processed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params, allowApply: true }),
      /Already processed/,
    );
  });
});
