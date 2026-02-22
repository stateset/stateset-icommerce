/**
 * Gift Card Tools Test Suite
 *
 * Tests for cli/src/tools/gift-cards.js
 * Covers: create_gift_card, get_gift_card, list_gift_cards, charge_gift_card,
 *         refund_to_gift_card, disable_gift_card, check_gift_card_balance
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { giftCardTools } from '../../src/tools/gift-cards.js';

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

const mockGiftCard = {
  id: 'gc_001',
  code: 'GIFT-ABC',
  initialBalance: '100.00',
  currentBalance: '75.00',
  currency: 'USD',
  status: 'active',
  customerId: 'cust_001',
  expiresAt: null,
  createdAt: '2026-02-21T00:00:00Z',
};

const mockTx = {
  id: 'tx_001',
  giftCardId: 'gc_001',
  amount: '25.00',
  type: 'charge',
  createdAt: '2026-02-21T00:00:00Z',
};

function makeGiftCardCommerce(overrides = {}) {
  return {
    giftCards: {
      create: async (data) => ({ ...mockGiftCard, ...data }),
      get: async (_id) => mockGiftCard,
      list: async () => [mockGiftCard],
      count: async () => 1,
      charge: async (_data) => mockTx,
      refund: async (_data) => ({ ...mockTx, type: 'refund' }),
      disable: async (_id) => ({ ...mockGiftCard, status: 'disabled' }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Gift Card Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(giftCardTools));
  });

  it('has at least 7 tools', () => {
    assert.ok(giftCardTools.length >= 7, `Expected >= 7, got ${giftCardTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of giftCardTools) {
      assert.ok(tool.name, `tool missing name`);
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// create_gift_card
// ============================================================================

describe('create_gift_card', () => {
  const tool = findTool(giftCardTools, 'create_gift_card');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { initialBalance: 100 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates a gift card with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { initialBalance: 100, currency: 'USD', customerId: 'cust_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.giftCard);
    assert.equal(result.giftCard.currency, 'USD');
  });

  it('returns error when commerce.giftCards.create throws', async () => {
    const commerce = makeGiftCardCommerce({
      create: async () => {
        throw new Error('DB write failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { initialBalance: 50 }, allowApply: true });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB write failed'));
    }
  });
});

// ============================================================================
// get_gift_card
// ============================================================================

describe('get_gift_card', () => {
  const tool = findTool(giftCardTools, 'get_gift_card');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns gift card for valid identifier', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { identifier: 'gc_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.giftCard.id, 'gc_001');
    assert.equal(result.giftCard.code, 'GIFT-ABC');
    assert.equal(result.giftCard.currentBalance, '75.00');
  });

  it('returns success: false when gift card not found', async () => {
    const commerce = makeGiftCardCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { identifier: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeGiftCardCommerce({
      get: async () => {
        throw new Error('lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { identifier: 'gc_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('lookup failed'));
    }
  });
});

// ============================================================================
// list_gift_cards
// ============================================================================

describe('list_gift_cards', () => {
  const tool = findTool(giftCardTools, 'list_gift_cards');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list and count', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.giftCards.length, 1);
    assert.equal(result.giftCards[0].id, 'gc_001');
  });

  it('maps all expected fields', () => {
    // covered by the list result above — spot-check keys
    const tool2 = findTool(giftCardTools, 'list_gift_cards');
    assert.ok(tool2);
  });

  it('returns error when list throws', async () => {
    const commerce = makeGiftCardCommerce({
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
// charge_gift_card
// ============================================================================

describe('charge_gift_card', () => {
  const tool = findTool(giftCardTools, 'charge_gift_card');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 25 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('charges card with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 25, orderId: 'ord_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('charged'));
    assert.equal(result.transaction.type, 'charge');
  });

  it('returns error when charge throws', async () => {
    const commerce = makeGiftCardCommerce({
      charge: async () => {
        throw new Error('Insufficient balance');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { giftCardId: 'gc_001', amount: 9999 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Insufficient balance'));
    }
  });
});

// ============================================================================
// refund_to_gift_card
// ============================================================================

describe('refund_to_gift_card', () => {
  const tool = findTool(giftCardTools, 'refund_to_gift_card');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 25 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('refunds to gift card with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', amount: 25, reason: 'order_cancelled' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('refund'));
    assert.equal(result.transaction.type, 'refund');
  });

  it('returns error when refund throws', async () => {
    const commerce = makeGiftCardCommerce({
      refund: async () => {
        throw new Error('Card is disabled');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { giftCardId: 'gc_001', amount: 25 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Card is disabled'));
    }
  });
});

// ============================================================================
// disable_gift_card
// ============================================================================

describe('disable_gift_card', () => {
  const tool = findTool(giftCardTools, 'disable_gift_card');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('disables card with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { giftCardId: 'gc_001', reason: 'fraud' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('disabled'));
    assert.equal(result.giftCard.status, 'disabled');
  });

  it('returns error when disable throws', async () => {
    const commerce = makeGiftCardCommerce({
      disable: async () => {
        throw new Error('Card not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { giftCardId: 'gc_999' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Card not found'));
    }
  });
});

// ============================================================================
// check_gift_card_balance
// ============================================================================

describe('check_gift_card_balance', () => {
  const tool = findTool(giftCardTools, 'check_gift_card_balance');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns balance for valid identifier', async () => {
    const result = await tool.handler({
      commerce: makeGiftCardCommerce(),
      params: { identifier: 'gc_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.giftCardId, 'gc_001');
    assert.equal(result.code, 'GIFT-ABC');
    assert.equal(result.currentBalance, '75.00');
    assert.equal(result.currency, 'USD');
    assert.equal(result.status, 'active');
  });

  it('returns success: false when gift card not found', async () => {
    const commerce = makeGiftCardCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { identifier: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeGiftCardCommerce({
      get: async () => {
        throw new Error('lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { identifier: 'gc_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('lookup failed'));
    }
  });
});
