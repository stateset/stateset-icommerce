/**
 * Promotions API tests for @stateset/embedded Node.js bindings.
 *
 * Discounts are asserted through the `*Exact` twins: a computed discount is
 * an exact decimal string rounded to the currency's minor unit, never a float.
 * Every test opens its own `:memory:` store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

async function customer(commerce, email) {
  return commerce.customers.create({ email, firstName: 'Promo', lastName: 'Tester' });
}

/** Three widgets at 19.99 and one at 5.01: subtotal 64.98. */
const cart = () => [
  { id: 'line-1', sku: 'WIDGET', quantity: 3, unitPrice: 19.99, lineTotal: 59.97 },
  { id: 'line-2', sku: 'GADGET', quantity: 1, unitPrice: 5.01, lineTotal: 5.01 },
];

async function activePromotion(commerce, input) {
  const promotion = await commerce.promotions.create(input);
  return commerce.promotions.activate(promotion.id);
}

test('create returns a draft promotion with exact money fields; get, getByCode and list find it', async () => {
  const commerce = new Commerce(':memory:');
  const created = await commerce.promotions.create({
    code: 'FIVE-OFF',
    name: 'Five off',
    description: 'Take five off',
    promotionType: 'fixed_amount_off',
    trigger: 'coupon_code',
    stacking: 'exclusive',
    fixedAmountOff: 5,
    maxDiscountAmount: 1.5,
    totalUsageLimit: 10,
    perCustomerLimit: 2,
    priority: 3,
  });
  assert.ok(created.id);
  assert.equal(created.code, 'FIVE-OFF');
  assert.equal(created.status, 'draft');
  assert.equal(created.promotionType, 'fixedamountoff');
  assert.equal(created.trigger, 'couponcode');
  assert.equal(created.stacking, 'exclusive');
  assert.equal(created.fixedAmountOffExact, '5');
  assert.equal(created.maxDiscountAmountExact, '1.5');
  assert.equal(created.totalUsageLimit, 10);
  assert.equal(created.perCustomerLimit, 2);
  assert.equal(created.priority, 3);
  assert.equal(created.usageCount, 0);
  assert.equal(created.currency, 'USD');

  const byId = await commerce.promotions.get(created.id);
  assert.equal(byId.id, created.id);
  const byCode = await commerce.promotions.getByCode('FIVE-OFF');
  assert.equal(byCode.id, created.id);
  const listed = await commerce.promotions.list({ status: 'draft' });
  assert.deepEqual(listed.map((p) => p.id), [created.id]);
  assert.equal(await commerce.promotions.get('00000000-0000-0000-0000-000000000001'), null);
});

test('activate and deactivate move the status; isValid, getActive and list follow it', async () => {
  const commerce = new Commerce(':memory:');
  const promotion = await commerce.promotions.create({ name: 'Ten off', promotionType: 'percentage_off', percentageOff: 0.1 });
  assert.equal(await commerce.promotions.isValid(promotion.id), false, 'a draft is not valid');
  assert.deepEqual(await commerce.promotions.getActive(), []);

  const active = await commerce.promotions.activate(promotion.id);
  assert.equal(active.status, 'active');
  assert.equal(await commerce.promotions.isValid(promotion.id), true);
  assert.deepEqual((await commerce.promotions.getActive()).map((p) => p.id), [promotion.id]);
  assert.deepEqual((await commerce.promotions.list({ isActive: true })).map((p) => p.id), [promotion.id]);

  const paused = await commerce.promotions.deactivate(promotion.id);
  assert.equal(paused.status, 'paused');
  assert.equal(await commerce.promotions.isValid(promotion.id), false);
  assert.deepEqual(await commerce.promotions.getActive(), []);
  assert.deepEqual((await commerce.promotions.list({ status: 'paused' })).map((p) => p.id), [promotion.id]);

  const updated = await commerce.promotions.update(promotion.id, { name: 'Renamed', maxDiscountAmount: 2.25 });
  assert.equal(updated.name, 'Renamed');
  assert.equal(updated.maxDiscountAmountExact, '2.25');
});

test('a percentage promotion discounts exactly, rounded to the cent', async () => {
  const commerce = new Commerce(':memory:');
  const promotion = await activePromotion(commerce, { name: 'Ten off', promotionType: 'percentage_off', percentageOff: 0.1 });

  const result = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, shippingAmount: 7.5 });
  // 10% of 64.98 is 6.498, which rounds to 6.50.
  assert.equal(result.originalSubtotalExact, '64.98');
  assert.equal(result.totalDiscountExact, '6.50');
  assert.equal(result.discountedSubtotalExact, '58.48');
  assert.equal(result.originalShippingExact, '7.5');
  assert.equal(result.shippingDiscountExact, '0');
  assert.equal(result.finalShippingExact, '7.5');
  assert.equal(result.grandTotalExact, '65.98');
  assert.equal(result.appliedPromotions.length, 1);
  assert.equal(result.appliedPromotions[0].promotionId, promotion.id);
  assert.equal(result.appliedPromotions[0].discountAmountExact, '6.50');
  assert.equal(result.appliedPromotions[0].discountType, 'percentageoff');
  assert.equal(result.appliedPromotions[0].couponCode, undefined);
});

test('a fixed-amount coupon stacks with an automatic percentage: the percentage is taken from the remainder', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce, 'stack@example.com');
  const percent = await activePromotion(commerce, { code: 'TEN', name: 'Ten off', promotionType: 'percentage_off', percentageOff: 0.1 });
  const fixed = await activePromotion(commerce, {
    code: 'FIVE',
    name: 'Five off',
    promotionType: 'fixed_amount_off',
    fixedAmountOff: 5,
    trigger: 'coupon_code',
    stacking: 'stackable',
  });
  await commerce.promotions.createCoupon({ promotionId: fixed.id, code: 'SAVE5' });

  const result = await commerce.promotions.apply({
    lineItems: cart(),
    subtotal: 64.98,
    shippingAmount: 7.5,
    customerId: shopper.id,
    couponCodes: ['SAVE5'],
  });
  // 5.00 off first (coupon-carrying entries lead), then 10% of the remaining 59.98 = 5.998 -> 6.00.
  assert.equal(result.totalDiscountExact, '11.00');
  assert.equal(result.discountedSubtotalExact, '53.98');
  assert.equal(result.grandTotalExact, '61.48');
  assert.deepEqual(
    result.appliedPromotions.map((p) => [p.promotionId, p.couponCode ?? null, p.discountAmountExact]),
    [
      [fixed.id, 'SAVE5', '5'],
      [percent.id, null, '6.00'],
    ],
  );
});

test('maxDiscountAmount caps a percentage; free shipping only touches the shipping line', async () => {
  const commerce = new Commerce(':memory:');
  await activePromotion(commerce, { name: 'Half off, max ten', promotionType: 'percentage_off', percentageOff: 0.5, maxDiscountAmount: 10 });
  const capped = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98 });
  assert.equal(capped.totalDiscountExact, '10');
  assert.equal(capped.discountedSubtotalExact, '54.98');

  await activePromotion(commerce, { name: 'Ships free', promotionType: 'free_shipping', target: 'shipping' });
  const shipped = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, shippingAmount: 7.5 });
  assert.equal(shipped.totalDiscountExact, '10', 'item discount is unchanged');
  assert.equal(shipped.shippingDiscountExact, '7.5');
  assert.ok(['0', '0.0', '0.00'].includes(shipped.finalShippingExact), `final shipping ${shipped.finalShippingExact}`);
  assert.equal(shipped.grandTotalExact, '54.98');
  assert.deepEqual(shipped.appliedPromotions.map((p) => p.discountType).sort(), ['freeshipping', 'percentageoff']);
});

test('an exclusive promotion applies alone; lower priority runs first', async () => {
  const commerce = new Commerce(':memory:');
  const exclusive = await activePromotion(commerce, {
    code: 'EXCL',
    name: 'Quarter off',
    promotionType: 'percentage_off',
    percentageOff: 0.25,
    stacking: 'exclusive',
    priority: 1,
  });
  await activePromotion(commerce, { code: 'STK', name: 'Ten off', promotionType: 'percentage_off', percentageOff: 0.1, stacking: 'stackable', priority: 2 });

  const items = [{ id: 'line-1', sku: 'WIDGET', quantity: 2, unitPrice: 12.34, lineTotal: 24.68 }];
  const result = await commerce.promotions.apply({ lineItems: items, subtotal: 24.68 });
  assert.equal(result.totalDiscountExact, '6.17');
  assert.deepEqual(result.appliedPromotions.map((p) => p.promotionId), [exclusive.id]);
});

test('a SKU-scoped percentage discounts only the eligible lines', async () => {
  const commerce = new Commerce(':memory:');
  await activePromotion(commerce, { name: 'Half off gadgets', promotionType: 'percentage_off', percentageOff: 0.5, applicableSkus: ['GADGET'] });
  const result = await commerce.promotions.apply({
    lineItems: [
      { id: 'line-1', sku: 'WIDGET', quantity: 1, unitPrice: 10, lineTotal: 10 },
      { id: 'line-2', sku: 'GADGET', quantity: 1, unitPrice: 4.99, lineTotal: 4.99 },
    ],
    subtotal: 14.99,
  });
  // 50% of the 4.99 gadget only: 2.495 -> 2.50.
  assert.equal(result.totalDiscountExact, '2.50');
  assert.equal(result.discountedSubtotalExact, '12.49');
});

test('a zero-decimal currency rounds the discount to whole units', async () => {
  const commerce = new Commerce(':memory:');
  await activePromotion(commerce, { name: 'Ten off', promotionType: 'percentage_off', percentageOff: 0.1 });
  const result = await commerce.promotions.apply({
    lineItems: [{ id: 'line-1', sku: 'WIDGET', quantity: 1, unitPrice: 1234, lineTotal: 1234 }],
    subtotal: 1234,
    currency: 'JPY',
  });
  assert.equal(result.totalDiscountExact, '123');
  assert.equal(result.grandTotalExact, '1111');
});

test('coupon lifecycle: create, look up, validate, then usage exhausts it', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce, 'coupon@example.com');
  const promotion = await activePromotion(commerce, { name: 'Five off', promotionType: 'fixed_amount_off', fixedAmountOff: 5, trigger: 'coupon_code' });
  const coupon = await commerce.promotions.createCoupon({ promotionId: promotion.id, code: 'SAVE5', usageLimit: 1, perCustomerLimit: 1 });
  assert.equal(coupon.promotionId, promotion.id);
  assert.equal(coupon.code, 'SAVE5');
  assert.equal(coupon.status, 'active');
  assert.equal(coupon.usageLimit, 1);
  assert.equal(coupon.usageCount, 0);

  assert.equal((await commerce.promotions.getCoupon(coupon.id)).id, coupon.id);
  assert.equal((await commerce.promotions.getCouponByCode('SAVE5')).id, coupon.id);
  assert.deepEqual((await commerce.promotions.listCoupons({ promotionId: promotion.id })).map((c) => c.id), [coupon.id]);
  assert.equal((await commerce.promotions.validateCoupon('SAVE5')).id, coupon.id);
  assert.equal(await commerce.promotions.validateCoupon('NOPE'), null);

  const before = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, customerId: shopper.id, couponCodes: ['SAVE5'] });
  assert.equal(before.totalDiscountExact, '5');
  assert.equal(before.appliedPromotions[0].couponCode, 'SAVE5');

  const usage = await commerce.promotions.recordUsage(promotion.id, coupon.id, shopper.id, null, null, 5, 'USD');
  assert.equal(usage.promotionId, promotion.id);
  assert.equal(usage.couponId, coupon.id);
  assert.equal(usage.customerId, shopper.id);
  assert.equal(usage.discountAmountExact, '5');
  assert.equal(usage.currency, 'USD');

  assert.equal((await commerce.promotions.getCouponByCode('SAVE5')).usageCount, 1);
  assert.equal((await commerce.promotions.get(promotion.id)).usageCount, 1);
  assert.equal(await commerce.promotions.validateCoupon('SAVE5'), null, 'an exhausted coupon does not validate');

  await assert.rejects(
    commerce.promotions.recordUsage(promotion.id, coupon.id, shopper.id, null, null, 5, 'USD'),
    (err) => err.code === 'VALIDATION' && /usage limit/i.test(err.message),
  );
  const after = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, customerId: shopper.id, couponCodes: ['SAVE5'] });
  assert.equal(after.totalDiscountExact, '0', 'an exhausted coupon contributes nothing');
  assert.deepEqual(after.appliedPromotions, []);
});

test('a per-customer limit is enforced per customer, at pricing and at recordUsage', async () => {
  const commerce = new Commerce(':memory:');
  const first = await customer(commerce, 'first@example.com');
  const second = await customer(commerce, 'second@example.com');
  const promotion = await activePromotion(commerce, { name: 'Two off, once', promotionType: 'fixed_amount_off', fixedAmountOff: 2, perCustomerLimit: 1 });
  const items = [{ id: 'line-1', sku: 'WIDGET', quantity: 2, unitPrice: 12.34, lineTotal: 24.68 }];
  const price = (customerId) => commerce.promotions.apply({ lineItems: items, subtotal: 24.68, customerId });

  assert.equal((await price(first.id)).totalDiscountExact, '2');
  await commerce.promotions.recordUsage(promotion.id, null, first.id, null, null, 2, 'USD');
  assert.equal((await price(first.id)).totalDiscountExact, '0', 'the first customer has used their one redemption');
  assert.equal((await price(second.id)).totalDiscountExact, '2', 'another customer still qualifies');
  await assert.rejects(
    commerce.promotions.recordUsage(promotion.id, null, first.id, null, null, 2, 'USD'),
    (err) => err.code === 'VALIDATION' && /per-customer/i.test(err.message),
  );
});

test('draft, paused and expired promotions never discount, even through a coupon', async () => {
  const commerce = new Commerce(':memory:');
  const draft = await commerce.promotions.create({ name: 'Draft', promotionType: 'fixed_amount_off', fixedAmountOff: 3, trigger: 'coupon_code' });
  await commerce.promotions.createCoupon({ promotionId: draft.id, code: 'DRAFTC' });
  const expired = await activePromotion(commerce, {
    name: 'Expired',
    promotionType: 'percentage_off',
    percentageOff: 0.5,
    startsAt: '2020-01-01T00:00:00Z',
    endsAt: '2020-02-01T00:00:00Z',
  });
  assert.equal(await commerce.promotions.isValid(expired.id), false);
  const paused = await activePromotion(commerce, { name: 'Paused', promotionType: 'percentage_off', percentageOff: 0.5 });
  await commerce.promotions.deactivate(paused.id);

  const result = await commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, couponCodes: ['DRAFTC'] });
  assert.equal(result.totalDiscountExact, '0');
  assert.equal(result.grandTotalExact, '64.98');
  assert.deepEqual(result.appliedPromotions, []);
});

test('delete removes the promotion; a later activate is NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  const promotion = await commerce.promotions.create({ name: 'Gone', promotionType: 'percentage_off', percentageOff: 0.1 });
  await commerce.promotions.delete(promotion.id);
  assert.equal(await commerce.promotions.get(promotion.id), null);
  await assert.rejects(commerce.promotions.activate(promotion.id), (err) => err.code === 'NOT_FOUND');
});

test('malformed inputs are refused with VALIDATION, never coerced', async () => {
  const commerce = new Commerce(':memory:');
  const isValidation = (pattern) => (err) => err.code === 'VALIDATION' && pattern.test(err.message);
  await assert.rejects(commerce.promotions.create({ name: 'x', currency: 'EURO' }), isValidation(/currency/i));
  await assert.rejects(commerce.promotions.create({ name: 'x', startsAt: 'yesterday' }), isValidation(/starts at/i));
  await assert.rejects(commerce.promotions.create({ name: 'x', applicableProductIds: ['not-a-uuid'] }), isValidation(/product/i));
  await assert.rejects(commerce.promotions.create({ name: 'x', promotionType: 'buy_x_get_y' }), isValidation(/buy_quantity/i));
  await assert.rejects(commerce.promotions.get('not-a-uuid'), isValidation(/uuid/i));
  await assert.rejects(commerce.promotions.createCoupon({ promotionId: 'not-a-uuid', code: 'Z' }), isValidation(/promotion/i));
  await assert.rejects(commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, cartId: 'not-a-uuid' }), isValidation(/cart/i));
  await assert.rejects(commerce.promotions.apply({ lineItems: cart(), subtotal: 64.98, currency: 'EURO' }), isValidation(/currency/i));
  assert.equal((await commerce.promotions.list()).length, 0, 'nothing was written');
});

test(
  'recordUsage refuses an unparseable currency instead of recording the usage in USD',
  async () => {
    const commerce = new Commerce(':memory:');
    const promotion = await activePromotion(commerce, { name: 'Two off', promotionType: 'fixed_amount_off', fixedAmountOff: 2 });
    await assert.rejects(
      commerce.promotions.recordUsage(promotion.id, null, null, null, null, 2, 'EURO'),
      (err) => err.code === 'VALIDATION' && /currency/i.test(err.message),
    );
    assert.equal((await commerce.promotions.get(promotion.id)).usageCount, 0);
  },
);

test(
  'a duplicate coupon code is a CONFLICT',
  async () => {
    const commerce = new Commerce(':memory:');
    const promotion = await activePromotion(commerce, { name: 'Two off', promotionType: 'fixed_amount_off', fixedAmountOff: 2 });
    await commerce.promotions.createCoupon({ promotionId: promotion.id, code: 'DUP' });
    await assert.rejects(commerce.promotions.createCoupon({ promotionId: promotion.id, code: 'DUP' }), (err) => err.code === 'CONFLICT');
  },
);

test(
  'a coupon for an unknown promotion is NOT_FOUND',
  async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      commerce.promotions.createCoupon({ promotionId: '00000000-0000-0000-0000-000000000001', code: 'ORPHAN' }),
      (err) => err.code === 'NOT_FOUND',
    );
  },
);

test(
  'validateCoupon returns null for a coupon whose promotion is not active',
  async () => {
    const commerce = new Commerce(':memory:');
    const draft = await commerce.promotions.create({ name: 'Draft', promotionType: 'fixed_amount_off', fixedAmountOff: 3, trigger: 'coupon_code' });
    await commerce.promotions.createCoupon({ promotionId: draft.id, code: 'DRAFTC' });
    assert.equal(await commerce.promotions.validateCoupon('DRAFTC'), null);
  },
);
