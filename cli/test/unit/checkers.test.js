/**
 * Unit tests for heartbeat/checkers.js — BUILTIN_CHECKERS
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { BUILTIN_CHECKERS } from '../../src/heartbeat/checkers.js';

// ---------------------------------------------------------------------------
// Helpers — mock commerce objects
// ---------------------------------------------------------------------------

function mockCommerce(overrides = {}) {
  return {
    analytics: {
      lowStockItems: async () => [],
      salesSummary: async () => ({ totalRevenue: 0 }),
      ...overrides.analytics,
    },
    carts: {
      getAbandoned: async () => [],
      ...overrides.carts,
    },
    returns: {
      list: async () => [],
      ...overrides.returns,
    },
    invoices: {
      getOverdue: async () => [],
      ...overrides.invoices,
    },
    listSubscriptions: async () => [],
    ...overrides,
  };
}

// ===========================================================================
// Registry
// ===========================================================================

describe('BUILTIN_CHECKERS registry', () => {
  it('contains all 6 checkers', () => {
    const keys = Object.keys(BUILTIN_CHECKERS);
    assert.ok(keys.includes('low-stock'));
    assert.ok(keys.includes('abandoned-carts'));
    assert.ok(keys.includes('revenue-milestone'));
    assert.ok(keys.includes('pending-returns'));
    assert.ok(keys.includes('overdue-invoices'));
    assert.ok(keys.includes('subscription-churn'));
    assert.strictEqual(keys.length, 6);
  });
});

// ===========================================================================
// low-stock
// ===========================================================================

describe('low-stock checker', () => {
  const check = BUILTIN_CHECKERS['low-stock'];

  it('not triggered when no low-stock items', async () => {
    const commerce = mockCommerce();
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.summary.includes('OK'));
  });

  it('triggered when items below threshold', async () => {
    const commerce = mockCommerce({
      analytics: {
        lowStockItems: async () => [
          { sku: 'A', qty: 3 },
          { sku: 'B', qty: 5 },
        ],
      },
    });
    const result = await check(commerce, { threshold: 10 });
    assert.strictEqual(result.triggered, true);
    assert.strictEqual(result.data.items.length, 2);
    assert.ok(result.summary.includes('2'));
  });

  it('uses default threshold of 10', async () => {
    const commerce = mockCommerce({
      analytics: {
        lowStockItems: async (t) => {
          assert.strictEqual(t, 10);
          return [];
        },
      },
    });
    await check(commerce);
  });

  it('uses custom threshold', async () => {
    const commerce = mockCommerce({
      analytics: {
        lowStockItems: async (t) => {
          assert.strictEqual(t, 25);
          return [];
        },
      },
    });
    await check(commerce, { threshold: 25 });
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      analytics: {
        lowStockItems: async () => {
          throw new Error('DB connection failed');
        },
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.data.error.includes('DB connection'));
  });
});

// ===========================================================================
// abandoned-carts
// ===========================================================================

describe('abandoned-carts checker', () => {
  const check = BUILTIN_CHECKERS['abandoned-carts'];

  it('not triggered when no abandoned carts', async () => {
    const commerce = mockCommerce();
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
  });

  it('triggered when old abandoned carts exist', async () => {
    const oldDate = new Date(Date.now() - 48 * 3600_000).toISOString();
    const commerce = mockCommerce({
      carts: {
        getAbandoned: async () => [{ id: '1', updatedAt: oldDate }],
      },
    });
    const result = await check(commerce, { minAgeHours: 24 });
    assert.strictEqual(result.triggered, true);
    assert.strictEqual(result.data.carts.length, 1);
  });

  it('not triggered when carts are recent', async () => {
    const recentDate = new Date().toISOString();
    const commerce = mockCommerce({
      carts: {
        getAbandoned: async () => [{ id: '1', updatedAt: recentDate }],
      },
    });
    const result = await check(commerce, { minAgeHours: 24 });
    assert.strictEqual(result.triggered, false);
  });

  it('supports snake_case timestamps', async () => {
    const oldDate = new Date(Date.now() - 48 * 3600_000).toISOString();
    const commerce = mockCommerce({
      carts: {
        getAbandoned: async () => [{ id: '1', created_at: oldDate }],
      },
    });
    const result = await check(commerce, { minAgeHours: 24 });
    assert.strictEqual(result.triggered, true);
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      carts: {
        getAbandoned: async () => {
          throw new Error('Timeout');
        },
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.summary.includes('failed'));
  });
});

// ===========================================================================
// revenue-milestone
// ===========================================================================

describe('revenue-milestone checker', () => {
  const check = BUILTIN_CHECKERS['revenue-milestone'];

  it('triggered when revenue meets target', async () => {
    const commerce = mockCommerce({
      analytics: {
        salesSummary: async () => ({ totalRevenue: 15000 }),
      },
    });
    const result = await check(commerce, { target: 10000 });
    assert.strictEqual(result.triggered, true);
    assert.strictEqual(result.data.revenue, 15000);
  });

  it('not triggered when below target', async () => {
    const commerce = mockCommerce({
      analytics: {
        salesSummary: async () => ({ totalRevenue: 5000 }),
      },
    });
    const result = await check(commerce, { target: 10000 });
    assert.strictEqual(result.triggered, false);
  });

  it('supports snake_case revenue field', async () => {
    const commerce = mockCommerce({
      analytics: {
        salesSummary: async () => ({ total_revenue: 20000 }),
      },
    });
    const result = await check(commerce, { target: 10000 });
    assert.strictEqual(result.triggered, true);
  });

  it('uses default target of 10000', async () => {
    const commerce = mockCommerce({
      analytics: {
        salesSummary: async () => ({ totalRevenue: 10000 }),
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, true);
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      analytics: {
        salesSummary: async () => {
          throw new Error('Service unavailable');
        },
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
  });
});

// ===========================================================================
// pending-returns
// ===========================================================================

describe('pending-returns checker', () => {
  const check = BUILTIN_CHECKERS['pending-returns'];

  it('not triggered when no pending returns', async () => {
    const commerce = mockCommerce();
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
  });

  it('triggered when old pending returns exist', async () => {
    const oldDate = new Date(Date.now() - 10 * 86400_000).toISOString();
    const commerce = mockCommerce({
      returns: {
        list: async () => [{ id: '1', status: 'pending', createdAt: oldDate }],
      },
    });
    const result = await check(commerce, { maxAgeDays: 7 });
    assert.strictEqual(result.triggered, true);
  });

  it('ignores non-pending returns', async () => {
    const oldDate = new Date(Date.now() - 10 * 86400_000).toISOString();
    const commerce = mockCommerce({
      returns: {
        list: async () => [{ id: '1', status: 'approved', createdAt: oldDate }],
      },
    });
    const result = await check(commerce, { maxAgeDays: 7 });
    assert.strictEqual(result.triggered, false);
  });

  it('includes requested status', async () => {
    const oldDate = new Date(Date.now() - 10 * 86400_000).toISOString();
    const commerce = mockCommerce({
      returns: {
        list: async () => [{ id: '1', status: 'requested', created_at: oldDate }],
      },
    });
    const result = await check(commerce, { maxAgeDays: 7 });
    assert.strictEqual(result.triggered, true);
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      returns: {
        list: async () => {
          throw new Error('DB error');
        },
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
  });
});

// ===========================================================================
// overdue-invoices
// ===========================================================================

describe('overdue-invoices checker', () => {
  const check = BUILTIN_CHECKERS['overdue-invoices'];

  it('not triggered when no overdue invoices', async () => {
    const commerce = mockCommerce();
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.summary.includes('No overdue'));
  });

  it('triggered when overdue invoices exist', async () => {
    const commerce = mockCommerce({
      invoices: {
        getOverdue: async () => [
          { id: '1', amountDue: 500 },
          { id: '2', amountDue: 300 },
        ],
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, true);
    assert.strictEqual(result.data.totalOverdue, 800);
    assert.ok(result.summary.includes('2'));
  });

  it('supports snake_case amount field', async () => {
    const commerce = mockCommerce({
      invoices: {
        getOverdue: async () => [{ id: '1', amount_due: 1000 }],
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.data.totalOverdue, 1000);
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      invoices: {
        getOverdue: async () => {
          throw new Error('Not found');
        },
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
  });
});

// ===========================================================================
// subscription-churn
// ===========================================================================

describe('subscription-churn checker', () => {
  const check = BUILTIN_CHECKERS['subscription-churn'];

  it('not triggered when no churn', async () => {
    const commerce = mockCommerce();
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.summary.includes('No subscription churn'));
  });

  it('triggered when cancelled subscriptions exist', async () => {
    const commerce = mockCommerce({
      listSubscriptions: async ({ status }) => {
        if (status === 'cancelled') return [{ id: 's1' }, { id: 's2' }];
        return [];
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, true);
    assert.ok(result.summary.includes('2 cancelled'));
  });

  it('triggered when past-due subscriptions exist', async () => {
    const commerce = mockCommerce({
      listSubscriptions: async ({ status }) => {
        if (status === 'past_due') return [{ id: 's1' }];
        return [];
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, true);
    assert.ok(result.summary.includes('1 past-due'));
  });

  it('handles API errors gracefully', async () => {
    const commerce = mockCommerce({
      listSubscriptions: async () => {
        throw new Error('Auth failure');
      },
    });
    const result = await check(commerce);
    assert.strictEqual(result.triggered, false);
    assert.ok(result.summary.includes('failed'));
  });
});
