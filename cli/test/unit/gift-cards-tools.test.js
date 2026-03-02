/**
 * Gift Card Tools Test Suite
 *
 * Tests for cli/src/tools/gift-cards.js
 * Covers: create_gift_card, get_gift_card, list_gift_cards,
 *         charge_gift_card, refund_to_gift_card, disable_gift_card,
 *         check_gift_card_balance
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { giftCardTools } from '../../src/tools/gift-cards.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = giftCardTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockGiftCard = {
  id: 'gc_001',
  code: 'GIFT-ABCD-1234',
  initialBalance: '50.00',
  currentBalance: '50.00',
  currency: 'USD',
  status: 'active',
  customerId: 'cust_001',
  expiresAt: '2027-12-31T23:59:59Z',
  createdAt: '2026-01-01T00:00:00Z',
};

const mockTransaction = {
  id: 'txn_001',
  giftCardId: 'gc_001',
  amount: '25.00',
  type: 'charge',
  createdAt: '2026-01-15T00:00:00Z',
};

function makeGiftCardCommerce(overrides = {}) {
  return {
    giftCards: {
      create: async (data) => ({ ...mockGiftCard, ...data }),
      get: async (_id) => mockGiftCard,
      list: async (_filters) => [mockGiftCard],
      count: async (_filters) => 1,
      charge: async (data) => ({ ...mockTransaction, ...data }),
      refund: async (data) => ({ ...mockTransaction, type: 'refund', ...data }),
      disable: async (_id, _reason) => ({ ...mockGiftCard, status: 'disabled' }),
      ...overrides,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('giftCardTools -- module exports', () => {
  it('exports an array of 7 tools', () => {
    assert.ok(Array.isArray(giftCardTools));
    assert.equal(giftCardTools.length, 7);
  });

  it('exports expected tool names in order', () => {
    const names = giftCardTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'create_gift_card',
      'get_gift_card',
      'list_gift_cards',
      'charge_gift_card',
      'refund_to_gift_card',
      'disable_gift_card',
      'check_gift_card_balance',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of giftCardTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of giftCardTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of giftCardTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have inputSchema objects', () => {
    for (const tool of giftCardTools) {
      assert.equal(typeof tool.inputSchema, 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('giftCardTools -- input schemas', () => {
  it('create_gift_card has initialBalance, currency, customerId, recipientEmail, recipientName, message, expiresAt', () => {
    const schema = findTool('create_gift_card').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('initialBalance'));
    assert.ok(keys.includes('currency'));
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('recipientEmail'));
    assert.ok(keys.includes('recipientName'));
    assert.ok(keys.includes('message'));
    assert.ok(keys.includes('expiresAt'));
  });

  it('get_gift_card has identifier field', () => {
    const schema = findTool('get_gift_card').inputSchema;
    assert.ok(Object.keys(schema).includes('identifier'));
  });

  it('list_gift_cards has status, customerId, limit', () => {
    const schema = findTool('list_gift_cards').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('status'));
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('limit'));
  });

  it('charge_gift_card has giftCardId, amount, orderId, note', () => {
    const schema = findTool('charge_gift_card').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('giftCardId'));
    assert.ok(keys.includes('amount'));
    assert.ok(keys.includes('orderId'));
    assert.ok(keys.includes('note'));
  });

  it('refund_to_gift_card has giftCardId, amount, orderId, reason', () => {
    const schema = findTool('refund_to_gift_card').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('giftCardId'));
    assert.ok(keys.includes('amount'));
    assert.ok(keys.includes('orderId'));
    assert.ok(keys.includes('reason'));
  });

  it('disable_gift_card has giftCardId, reason', () => {
    const schema = findTool('disable_gift_card').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('giftCardId'));
    assert.ok(keys.includes('reason'));
  });

  it('check_gift_card_balance has identifier field', () => {
    const schema = findTool('check_gift_card_balance').inputSchema;
    assert.ok(Object.keys(schema).includes('identifier'));
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('giftCardTools -- permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(findTool('get_gift_card').permission, 'read');
    assert.equal(findTool('list_gift_cards').permission, 'read');
    assert.equal(findTool('check_gift_card_balance').permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(findTool('create_gift_card').permission, 'write');
    assert.equal(findTool('charge_gift_card').permission, 'write');
    assert.equal(findTool('refund_to_gift_card').permission, 'write');
    assert.equal(findTool('disable_gift_card').permission, 'write');
  });
});

// ============================================================================
// Handler apply-guard (write tools without --apply)
// ============================================================================

describe('giftCardTools -- apply-guard', () => {
  it('create_gift_card requires --apply', async () => {
    const tool = findTool('create_gift_card');
    const result = await tool.handler({
      params: { initialBalance: 50 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('charge_gift_card requires --apply', async () => {
    const tool = findTool('charge_gift_card');
    const result = await tool.handler({
      params: { giftCardId: 'gc_001', amount: 25 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('refund_to_gift_card requires --apply', async () => {
    const tool = findTool('refund_to_gift_card');
    const result = await tool.handler({
      params: { giftCardId: 'gc_001', amount: 10 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('disable_gift_card requires --apply', async () => {
    const tool = findTool('disable_gift_card');
    const result = await tool.handler({
      params: { giftCardId: 'gc_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('apply-guard returns preview (wouldDo) with params', async () => {
    const tool = findTool('create_gift_card');
    const params = { initialBalance: 100, currency: 'EUR' };
    const result = await tool.handler({ params, allowApply: false, commerce: {} });
    assert.equal(result.success, false);
    assert.deepStrictEqual(result.wouldDo, params);
  });
});

// ============================================================================
// Handler success paths (with mocked commerce)
// ============================================================================

describe('giftCardTools -- create_gift_card handler', () => {
  it('creates a gift card when allowApply is true', async () => {
    const tool = findTool('create_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { initialBalance: 50, currency: 'USD' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Gift card created');
    assert.ok(result.giftCard);
  });
});

describe('giftCardTools -- get_gift_card handler', () => {
  it('returns gift card with expected fields', async () => {
    const tool = findTool('get_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { identifier: 'gc_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.giftCard);
    assert.equal(result.giftCard.id, 'gc_001');
    assert.equal(result.giftCard.code, 'GIFT-ABCD-1234');
    assert.equal(result.giftCard.currency, 'USD');
    assert.equal(result.giftCard.status, 'active');
  });

  it('returns not found when gift card is null', async () => {
    const tool = findTool('get_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce({ get: async () => null }),
      params: { identifier: 'gc_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Gift card not found');
  });
});

describe('giftCardTools -- list_gift_cards handler', () => {
  it('returns list with totalCount and returned', async () => {
    const tool = findTool('list_gift_cards');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.giftCards));
    assert.equal(result.giftCards[0].id, 'gc_001');
  });

  it('maps expected fields on each gift card', async () => {
    const tool = findTool('list_gift_cards');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: {},
    });
    const gc = result.giftCards[0];
    const expectedKeys = [
      'id', 'code', 'initialBalance', 'currentBalance',
      'currency', 'status', 'customerId', 'expiresAt', 'createdAt',
    ];
    for (const key of expectedKeys) {
      assert.ok(key in gc, `missing key: ${key}`);
    }
  });
});

describe('giftCardTools -- charge_gift_card handler', () => {
  it('charges gift card when allowApply is true', async () => {
    const tool = findTool('charge_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 25 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Gift card charged');
    assert.ok(result.transaction);
  });
});

describe('giftCardTools -- refund_to_gift_card handler', () => {
  it('refunds to gift card when allowApply is true', async () => {
    const tool = findTool('refund_to_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 10 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Refund applied to gift card');
    assert.ok(result.transaction);
  });
});

describe('giftCardTools -- disable_gift_card handler', () => {
  it('disables gift card when allowApply is true', async () => {
    const tool = findTool('disable_gift_card');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', reason: 'fraud' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Gift card disabled');
    assert.ok(result.giftCard);
  });
});

describe('giftCardTools -- check_gift_card_balance handler', () => {
  it('returns balance info for valid gift card', async () => {
    const tool = findTool('check_gift_card_balance');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { identifier: 'gc_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.giftCardId, 'gc_001');
    assert.equal(result.code, 'GIFT-ABCD-1234');
    assert.equal(result.currentBalance, '50.00');
    assert.equal(result.currency, 'USD');
    assert.equal(result.status, 'active');
  });

  it('returns not found when gift card is null', async () => {
    const tool = findTool('check_gift_card_balance');
    const result = await tool.handler({
      commerce: makeGiftCardCommerce({ get: async () => null }),
      params: { identifier: 'gc_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Gift card not found');
  });
});

// ============================================================================
// Handler error paths (commerce object missing methods)
// ============================================================================

describe('giftCardTools -- error paths', () => {
  it('get_gift_card throws when commerce.giftCards is undefined', async () => {
    const tool = findTool('get_gift_card');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { identifier: 'gc_001' } }),
      (err) => err instanceof TypeError,
    );
  });

  it('list_gift_cards throws when commerce.giftCards is undefined', async () => {
    const tool = findTool('list_gift_cards');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: {} }),
      (err) => err instanceof TypeError,
    );
  });

  it('create_gift_card throws when commerce.giftCards.create is missing', async () => {
    const tool = findTool('create_gift_card');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { giftCards: {} },
          params: { initialBalance: 50 },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('charge_gift_card throws when commerce.giftCards.charge is missing', async () => {
    const tool = findTool('charge_gift_card');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { giftCards: {} },
          params: { giftCardId: 'gc_001', amount: 10 },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('check_gift_card_balance throws when commerce.giftCards is undefined', async () => {
    const tool = findTool('check_gift_card_balance');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { identifier: 'gc_001' } }),
      (err) => err instanceof TypeError,
    );
  });
});
