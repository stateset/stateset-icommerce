/**
 * Promotions Tools — Comprehensive Test Suite
 *
 * Tests every tool exported from src/tools/promotions.js:
 *   list_promotions, get_promotion, create_promotion, activate_promotion,
 *   deactivate_promotion, create_coupon, validate_coupon, list_coupons,
 *   get_active_promotions, apply_cart_promotions
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { promotionTools } from '../../src/tools/promotions.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = promotionTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in promotionTools`);
  return tool;
}

function makePromotion(overrides = {}) {
  return {
    id: 'promo_001',
    code: 'SUMMER25',
    name: 'Summer Sale',
    description: '25% off everything',
    promotionType: 'PercentageOff',
    status: 'active',
    trigger: 'Automatic',
    target: 'Order',
    percentageOff: 0.25,
    fixedAmountOff: null,
    maxDiscountAmount: null,
    startsAt: '2026-06-01T00:00:00Z',
    endsAt: '2026-08-31T23:59:59Z',
    usageCount: 42,
    totalUsageLimit: 1000,
    conditions: [],
    ...overrides,
  };
}

function makeCoupon(overrides = {}) {
  return {
    id: 'coupon_001',
    code: 'SAVE25',
    promotionId: 'promo_001',
    status: 'active',
    usageCount: 10,
    usageLimit: 100,
    startsAt: '2026-06-01T00:00:00Z',
    endsAt: '2026-08-31T23:59:59Z',
    ...overrides,
  };
}

/**
 * The promotions source calls `commerce.promotions()` as a function
 * that returns an object with methods. This is different from most
 * other tool modules that use `commerce.resource.method`.
 */
function makeCommerce(overrides = {}) {
  const promoMethods = {
    list: async () => [makePromotion()],
    get: async (id) => (id === 'nonexistent' ? null : makePromotion({ id })),
    getByCode: async (code) => (code === 'NONEXISTENT' ? null : makePromotion({ code })),
    create: async (data) => makePromotion({ id: 'promo_new', ...data }),
    activate: async (id) => makePromotion({ id, status: 'active' }),
    deactivate: async (id) => makePromotion({ id, status: 'paused' }),
    createCoupon: async (data) => makeCoupon({ id: 'coupon_new', ...data }),
    validateCoupon: async (code) => (code === 'INVALID' ? null : makeCoupon({ code })),
    listCoupons: async () => [makeCoupon()],
    getActive: async () => [makePromotion()],
    ...(overrides.promoMethods || {}),
  };

  return {
    promotions: () => promoMethods,
    applyCartPromotions: overrides.applyCartPromotions || (async () => ({
      originalSubtotal: 100,
      totalDiscount: 25,
      discountedSubtotal: 75,
      shippingDiscount: 0,
      grandTotal: 75,
      appliedPromotions: [
        {
          promotionName: 'Summer Sale',
          discountType: 'PercentageOff',
          discountAmount: 25,
          description: '25% off',
          couponCode: null,
        },
      ],
      rejectedPromotions: [],
    })),
    ...overrides.commerceTop,
  };
}

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Promotion Tools — structure', () => {
  it('exports an array of 10 tools', () => {
    assert.ok(Array.isArray(promotionTools));
    assert.strictEqual(promotionTools.length, 10);
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of promotionTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = promotionTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });
});

// ---------------------------------------------------------------------------
// list_promotions
// ---------------------------------------------------------------------------

describe('list_promotions', () => {
  const tool = findTool('list_promotions');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns promotions array with count', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.promotions));
    assert.strictEqual(result.count, 1);
  });

  it('maps promotion fields correctly', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    const promo = result.promotions[0];
    assert.strictEqual(promo.id, 'promo_001');
    assert.strictEqual(promo.code, 'SUMMER25');
    assert.strictEqual(promo.name, 'Summer Sale');
    assert.strictEqual(promo.type, 'PercentageOff');
    assert.strictEqual(promo.percentageOff, 0.25);
  });

  it('passes status filter', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      promoMethods: {
        list: async (filter) => { calledWith = filter; return []; },
      },
    });
    await tool.handler({ commerce, params: { status: 'active' } });
    assert.strictEqual(calledWith.status, 'active');
  });

  it('passes type filter as promotionType', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      promoMethods: {
        list: async (filter) => { calledWith = filter; return []; },
      },
    });
    await tool.handler({ commerce, params: { type: 'percentage_off' } });
    assert.strictEqual(calledWith.promotionType, 'percentage_off');
  });
});

// ---------------------------------------------------------------------------
// get_promotion
// ---------------------------------------------------------------------------

describe('get_promotion', () => {
  const tool = findTool('get_promotion');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns promotion by ID', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { identifier: 'promo_001' } });
    assert.strictEqual(result.success, true);
    assert.ok(result.promotion);
    assert.strictEqual(result.promotion.id, 'promo_001');
  });

  it('falls back to code lookup on get failure', async () => {
    let codeUsed = false;
    const commerce = makeCommerce({
      promoMethods: {
        get: async () => { throw new Error('Not found by ID'); },
        getByCode: async (code) => { codeUsed = true; return makePromotion({ code }); },
      },
    });
    const result = await tool.handler({ commerce, params: { identifier: 'SUMMER25' } });
    assert.strictEqual(result.success, true);
    assert.ok(codeUsed);
  });

  it('returns error when promotion not found by either method', async () => {
    const commerce = makeCommerce({
      promoMethods: {
        get: async () => { throw new Error('Not found'); },
        getByCode: async () => null,
      },
    });
    const result = await tool.handler({ commerce, params: { identifier: 'NONEXISTENT' } });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });

  it('maps full promotion fields', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { identifier: 'promo_001' } });
    const p = result.promotion;
    assert.strictEqual(p.maxDiscount, null);
    assert.strictEqual(p.usageLimit, 1000);
    assert.strictEqual(p.usageCount, 42);
  });
});

// ---------------------------------------------------------------------------
// create_promotion
// ---------------------------------------------------------------------------

describe('create_promotion', () => {
  const tool = findTool('create_promotion');
  const params = { name: 'Winter Sale', type: 'percentage_off', trigger: 'automatic', percentageOff: 0.2 };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldCreate);
  });

  it('creates promotion when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.promotion);
    assert.ok(result.message.includes('draft'));
    assert.ok(result.hint.includes('activate_promotion'));
  });

  it('maps type enum to PascalCase', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      promoMethods: {
        create: async (data) => { calledWith = data; return makePromotion(data); },
      },
    });
    await tool.handler({ commerce, params, allowApply: true });
    assert.strictEqual(calledWith.promotionType, 'PercentageOff');
    assert.strictEqual(calledWith.trigger, 'Automatic');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      promoMethods: {
        create: async () => { throw new Error('Duplicate code'); },
      },
    });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Duplicate code/);
  });
});

// ---------------------------------------------------------------------------
// activate_promotion
// ---------------------------------------------------------------------------

describe('activate_promotion', () => {
  const tool = findTool('activate_promotion');
  const params = { promotionId: 'promo_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldActivate);
  });

  it('activates promotion when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('activated'));
    assert.strictEqual(result.promotion.status, 'active');
  });
});

// ---------------------------------------------------------------------------
// deactivate_promotion
// ---------------------------------------------------------------------------

describe('deactivate_promotion', () => {
  const tool = findTool('deactivate_promotion');
  const params = { promotionId: 'promo_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldDeactivate);
  });

  it('deactivates promotion when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('deactivated'));
    assert.strictEqual(result.promotion.status, 'paused');
  });
});

// ---------------------------------------------------------------------------
// create_coupon
// ---------------------------------------------------------------------------

describe('create_coupon', () => {
  const tool = findTool('create_coupon');
  const params = { promotionId: 'promo_001', code: 'save25', usageLimit: 100 };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldCreate);
  });

  it('creates coupon when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.coupon);
    assert.strictEqual(result.coupon.code, 'SAVE25'); // uppercased
  });

  it('uppercases coupon code', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      promoMethods: {
        createCoupon: async (data) => { calledWith = data; return makeCoupon(data); },
      },
    });
    await tool.handler({ commerce, params, allowApply: true });
    assert.strictEqual(calledWith.code, 'SAVE25');
  });
});

// ---------------------------------------------------------------------------
// validate_coupon
// ---------------------------------------------------------------------------

describe('validate_coupon', () => {
  const tool = findTool('validate_coupon');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns valid result for existing coupon', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { code: 'SAVE25' } });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.valid, true);
    assert.ok(result.coupon);
    assert.strictEqual(result.coupon.code, 'SAVE25');
  });

  it('returns invalid result for unknown coupon', async () => {
    const commerce = makeCommerce({
      promoMethods: {
        validateCoupon: async () => null,
      },
    });
    const result = await tool.handler({ commerce, params: { code: 'INVALID' } });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.valid, false);
    assert.ok(result.message.includes('Invalid'));
  });

  it('uppercases the code before validating', async () => {
    let calledWith;
    const commerce = makeCommerce({
      promoMethods: {
        validateCoupon: async (code) => { calledWith = code; return makeCoupon({ code }); },
        get: async () => makePromotion(),
      },
    });
    await tool.handler({ commerce, params: { code: 'save25' } });
    assert.strictEqual(calledWith, 'SAVE25');
  });

  it('calculates usageRemaining as unlimited when no limit', async () => {
    const commerce = makeCommerce({
      promoMethods: {
        validateCoupon: async () => makeCoupon({ usageLimit: null, usageCount: 10 }),
        get: async () => makePromotion(),
      },
    });
    const result = await tool.handler({ commerce, params: { code: 'SAVE25' } });
    assert.strictEqual(result.coupon.usageRemaining, 'unlimited');
  });
});

// ---------------------------------------------------------------------------
// list_coupons
// ---------------------------------------------------------------------------

describe('list_coupons', () => {
  const tool = findTool('list_coupons');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns coupons array with count', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.coupons));
    assert.strictEqual(result.count, 1);
  });

  it('passes filters', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      promoMethods: {
        listCoupons: async (filter) => { calledWith = filter; return []; },
      },
    });
    await tool.handler({ commerce, params: { promotionId: 'promo_001', status: 'active' } });
    assert.strictEqual(calledWith.promotionId, 'promo_001');
    assert.strictEqual(calledWith.status, 'active');
  });
});

// ---------------------------------------------------------------------------
// get_active_promotions
// ---------------------------------------------------------------------------

describe('get_active_promotions', () => {
  const tool = findTool('get_active_promotions');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns active promotions', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.promotions));
    assert.strictEqual(result.count, 1);
  });
});

// ---------------------------------------------------------------------------
// apply_cart_promotions
// ---------------------------------------------------------------------------

describe('apply_cart_promotions', () => {
  const tool = findTool('apply_cart_promotions');
  const params = { cartId: 'cart_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldApplyTo);
  });

  it('applies promotions when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cartId, 'cart_001');
    assert.strictEqual(result.originalSubtotal, 100);
    assert.strictEqual(result.totalDiscount, 25);
    assert.strictEqual(result.grandTotal, 75);
    assert.ok(Array.isArray(result.appliedPromotions));
    assert.strictEqual(result.appliedPromotions.length, 1);
  });

  it('returns empty rejectedPromotions when none rejected', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.ok(Array.isArray(result.rejectedPromotions));
    assert.strictEqual(result.rejectedPromotions.length, 0);
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({
      applyCartPromotions: async () => { throw new Error('Cart empty'); },
    });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Cart empty/);
  });
});
