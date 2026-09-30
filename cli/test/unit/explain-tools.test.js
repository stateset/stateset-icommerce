/**
 * explain_order / explain_cart_pricing -- story assembly, exact money, flags,
 * graceful degradation, and the binding surface they read.
 *
 * The mock `commerce` records every call and only offers methods the Node
 * binding declares (checked against bindings/node/index.d.ts), so a tool that
 * reached for an invented method -- or a write -- fails here.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { explainTools, timeKey } from '../../src/tools/explain.js';
import { TOOL_PERMISSIONS } from '../../src/permissions.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(explainTools.map((t) => [t.name, t]));

/** Binding class behind each `commerce.<api>` the tools use. */
const API_CLASS = {
  orders: 'Orders',
  carts: 'Carts',
  payments: 'Payments',
  shipments: 'Shipments',
  returns: 'Returns',
  fraud: 'Fraud',
  activityLogs: 'ActivityLogs',
  tax: 'Tax',
  promotions: 'Promotions',
};

/** Methods the tools may call: every one is a read. */
const READ_METHODS = {
  orders: ['get', 'list'],
  carts: ['get', 'getItems', 'forCustomer'],
  payments: ['list', 'getRefunds'],
  shipments: ['list'],
  returns: ['listForOrder'],
  fraud: ['isSupported', 'getAssessment'],
  activityLogs: ['isSupported', 'historyForSubject'],
  tax: ['calculate'],
  promotions: ['apply'],
};

const ORDER_ID = '11111111-1111-4111-8111-111111111111';
const CUSTOMER_ID = '22222222-2222-4222-8222-222222222222';
const CART_ID = '33333333-3333-4333-8333-333333333333';
const ADDRESS = { line1: '1 Main St', city: 'LA', state: 'CA', postalCode: '90001', country: 'US' };

const t = (second, fraction = '000000000') => `2026-09-01T10:00:${second}.${fraction}+00:00`;

function order(overrides = {}) {
  return {
    id: ORDER_ID,
    orderNumber: 'ORD-1',
    customerId: CUSTOMER_ID,
    status: 'shipped',
    totalAmount: 90,
    totalAmountExact: '90.0',
    currency: 'USD',
    paymentStatus: 'pending',
    fulfillmentStatus: 'unfulfilled',
    trackingNumber: '1Z',
    shippingAddress: ADDRESS,
    items: [
      {
        id: 'oi-1',
        sku: 'W-1',
        name: 'Widget',
        quantity: 2,
        unitPrice: 50,
        unitPriceExact: '50',
        total: 100,
        totalExact: '100',
      },
    ],
    version: 3,
    createdAt: t('10'),
    updatedAt: t('40'),
    ...overrides,
  };
}

function cart(overrides = {}) {
  return {
    id: CART_ID,
    cartNumber: 'CART-1',
    customerId: CUSTOMER_ID,
    status: 'completed',
    currency: 'USD',
    subtotalExact: '100',
    taxAmountExact: '0',
    shippingAmountExact: '0',
    discountAmountExact: '10.0',
    grandTotalExact: '90.0',
    shippingAddress: { firstName: 'A', lastName: 'L', ...ADDRESS },
    couponCode: 'WELCOME10',
    orderId: ORDER_ID,
    createdAt: t('01'),
    updatedAt: t('10'),
    ...overrides,
  };
}

const cartItem = (overrides = {}) => ({
  id: 'ci-1',
  cartId: CART_ID,
  sku: 'W-1',
  name: 'Widget',
  quantity: 2,
  unitPriceExact: '50',
  discountAmountExact: '0',
  taxAmountExact: '0',
  totalExact: '100',
  ...overrides,
});

const payment = (overrides = {}) => ({
  id: 'pay-1',
  paymentNumber: 'PAY-1',
  orderId: ORDER_ID,
  amount: 90,
  amountExact: '90',
  currency: 'USD',
  status: 'completed',
  version: 1,
  createdAt: t('20'),
  updatedAt: t('21'),
  ...overrides,
});

const taxResult = (totalTaxExact = '0') => ({
  id: 'calc-1',
  totalTaxExact,
  shippingTaxExact: '0',
  subtotalExact: '100',
  isEstimate: true,
  exemptionsApplied: false,
  taxBreakdown: [
    {
      jurisdictionId: 'j-ca',
      jurisdictionName: 'California',
      taxType: 'sales_tax',
      rateName: 'CA state',
      rate: 0.0725,
      taxableAmountExact: '100',
      taxAmountExact: totalTaxExact,
      isCompound: false,
    },
  ],
  jurisdictions: [],
  lineItemTaxes: [],
});

const promoResult = (overrides = {}) => ({
  totalDiscountExact: '10.0',
  shippingDiscountExact: '0',
  appliedPromotions: [
    {
      promotionId: 'promo-1',
      promotionName: 'Welcome 10%',
      couponCode: 'WELCOME10',
      discountAmountExact: '10.0',
      discountType: 'percentage_off',
    },
  ],
  rejectedPromotions: [],
  ...overrides,
});

/**
 * A recording `commerce` offering only binding read methods. `impl` maps
 * `api.method` to a function (or a value, or an Error to throw).
 */
function recordingCommerce(impl = {}) {
  const calls = [];
  const commerce = {};
  for (const [api, methods] of Object.entries(READ_METHODS)) {
    commerce[api] = {};
    for (const method of methods) {
      commerce[api][method] = async (...args) => {
        calls.push({ call: `${api}.${method}`, args });
        const behaviour = impl[`${api}.${method}`];
        if (behaviour instanceof Error) throw behaviour;
        if (typeof behaviour === 'function') return behaviour(...args);
        return behaviour;
      };
    }
  }
  return { commerce, calls };
}

function orderStore(overrides = {}) {
  return recordingCommerce({
    'orders.get': order(),
    'orders.list': [order()],
    'carts.forCustomer': [cart({ id: 'other', orderId: null }), cart()],
    'carts.getItems': [cartItem()],
    'payments.list': [payment()],
    'shipments.list': [
      {
        id: 'shp-1',
        shipmentNumber: 'SHP-1',
        orderId: ORDER_ID,
        status: 'pending',
        carrier: 'ups',
        shippingMethod: 'ground',
        createdAt: t('30'),
        updatedAt: t('30'),
      },
    ],
    'returns.listForOrder': [
      {
        id: 'ret-1',
        orderId: ORDER_ID,
        status: 'completed',
        reason: 'defective',
        createdAt: t('50'),
      },
    ],
    'fraud.isSupported': true,
    'fraud.getAssessment': null,
    'activityLogs.isSupported': true,
    'activityLogs.historyForSubject': [],
    'tax.calculate': taxResult(),
    ...overrides,
  });
}

const explainOrder = (commerce, orderId = ORDER_ID) =>
  byName.explain_order.handler({ commerce, params: { orderId } });
const explainCart = (commerce, params = {}) =>
  byName.explain_cart_pricing.handler({ commerce, params: { cartId: CART_ID, ...params } });
const codes = (result) => result.flags.map((f) => f.code);

describe('explain tools -- registration and binding surface', () => {
  it('exports two read-only tools with valid schemas', () => {
    assert.deepEqual(
      explainTools.map((tool) => [tool.name, tool.permission]),
      [
        ['explain_order', 'read'],
        ['explain_cart_pricing', 'read'],
      ],
    );
    assert.equal(TOOL_PERMISSIONS.explain_order, 'read');
    assert.equal(TOOL_PERMISSIONS.explain_cart_pricing, 'read');
    assert.equal(z.object(byName.explain_order.inputSchema).safeParse({}).success, false);
    assert.equal(
      z
        .object(byName.explain_cart_pricing.inputSchema)
        .safeParse({ cartId: 'c', couponCodes: ['X'] }).success,
      true,
    );
  });

  it('the mock offers only methods the binding declares', () => {
    for (const [api, methods] of Object.entries(READ_METHODS)) {
      const declared = bindingClassMethods(API_CLASS[api]);
      for (const method of methods) {
        assert.ok(declared.has(method), `${API_CLASS[api]}.${method} is not on the binding`);
      }
    }
  });

  it('the module calls nothing but those read methods', () => {
    for (const api of Object.keys(READ_METHODS)) {
      const called = [...toolModuleCalls('explain.js', api)];
      assert.deepEqual(
        called.filter((method) => !READ_METHODS[api].includes(method)),
        [],
        `explain.js calls commerce.${api} methods outside the read list`,
      );
    }
  });
});

describe('explain_order', () => {
  it('tells the story chronologically with exact money', async () => {
    const { commerce } = orderStore({
      'activityLogs.historyForSubject': [
        {
          id: 'act-1',
          subjectType: 'order',
          subjectId: ORDER_ID,
          action: 'note',
          summary: 'Customer called',
          actorKind: 'user',
          actor: 'sam',
          metadata: '{}',
          createdAt: t('35'),
        },
      ],
    });
    const result = await explainOrder(commerce);
    assert.equal(result.success, true);
    assert.deepEqual(
      result.timeline.map((e) => `${e.kind}:${e.event}`),
      [
        'cart:created',
        'checkout:order_created',
        'payment:created',
        'payment:completed',
        'shipment:created',
        'activity:note',
        'order_status:shipped',
        'return:created',
      ],
    );
    assert.equal(result.money.orderTotal, '90.00');
    assert.equal(result.money.itemsSubtotal, '100.00');
    assert.equal(result.money.discount, '10.00');
    assert.equal(result.money.charged, '90.00');
    assert.equal(result.money.refunded, '0.00');
    assert.equal(result.money.net, '90.00');
    assert.equal(result.money.explainedTotal, '90.00');
    assert.equal(result.tax.basis, 'recomputed_now');
    assert.equal(result.tax.breakdown[0].jurisdiction, 'California');
  });

  it('flags statuses that contradict payments and shipments', async () => {
    const result = await explainOrder(orderStore().commerce);
    for (const code of [
      'order_payment_status_stale',
      'order_fulfillment_status_stale',
      'shipment_status_behind_order',
      'return_refund_unverifiable',
    ]) {
      assert.ok(codes(result).includes(code), `${code} missing from ${codes(result)}`);
    }
  });

  it('counts a fully refunded payment exactly and dates the refund', async () => {
    const { commerce } = orderStore({
      'payments.list': [payment({ status: 'refunded', updatedAt: t('55') })],
    });
    const result = await explainOrder(commerce);
    assert.equal(result.money.charged, '90.00');
    assert.equal(result.money.refunded, '90.00');
    assert.equal(result.money.net, '0.00');
    assert.deepEqual(result.timeline.at(-1), {
      at: t('55'),
      kind: 'refund',
      event: 'refunded',
      summary: 'Payment PAY-1 fully refunded (90.00 USD).',
      ref: { paymentId: 'pay-1' },
    });
  });

  it('does not guess a partial refund amount', async () => {
    const { commerce } = orderStore({
      'payments.list': [payment({ status: 'partially_refunded' })],
    });
    const result = await explainOrder(commerce);
    assert.equal(result.money.refunded, null);
    assert.equal(result.money.net, null);
    assert.ok(result.timeline.some((e) => e.kind === 'refund' && e.event === 'partially_refunded'));
  });

  it('sums float-hostile amounts exactly and flags over-capture', async () => {
    const { commerce } = orderStore({
      'orders.get': order({ totalAmountExact: '0.3' }),
      'carts.forCustomer': [
        cart({ grandTotalExact: '0.30', subtotalExact: '0.3', discountAmountExact: '0' }),
      ],
      'payments.list': [
        payment({ id: 'a', amountExact: '0.1' }),
        payment({ id: 'b', amountExact: '0.2' }),
        payment({ id: 'c', amountExact: '0.05', status: 'pending' }),
      ],
    });
    const result = await explainOrder(commerce);
    assert.equal(result.money.charged, '0.30');
    assert.equal(result.money.pendingCharges, '0.05');
    assert.ok(!codes(result).includes('over_captured'));

    const over = await explainOrder(
      orderStore({ 'payments.list': [payment(), payment({ id: 'dup' })] }).commerce,
    );
    assert.equal(over.money.charged, '180.00');
    assert.ok(codes(over).includes('over_captured'));
  });

  it('flags an order total that differs from its checkout cart', async () => {
    const { commerce } = orderStore({ 'carts.forCustomer': [cart({ grandTotalExact: '95' })] });
    assert.ok(codes(await explainOrder(commerce)).includes('order_total_differs_from_cart'));
  });

  it('reports the fraud assessment and a hold the order ignored', async () => {
    const { commerce } = orderStore({
      'fraud.getAssessment': {
        orderId: ORDER_ID,
        riskScore: 0.9,
        decision: 'review',
        needsReview: true,
        signals: [
          {
            orderId: ORDER_ID,
            signalType: 'proxy_vpn',
            score: 0.9,
            details: 'VPN',
            detectedAt: t('11'),
          },
        ],
        createdAt: t('11'),
        updatedAt: t('11'),
      },
    });
    const result = await explainOrder(commerce);
    assert.equal(result.fraud.decision, 'review');
    assert.equal(result.fraud.signals[0].type, 'proxy_vpn');
    assert.ok(codes(result).includes('fraud_hold_not_honoured'));
    assert.ok(result.timeline.some((e) => e.kind === 'fraud'));
  });

  it('degrades section by section instead of failing', async () => {
    const { commerce } = orderStore({
      'payments.list': new Error('payments backend down'),
      'fraud.isSupported': false,
      'activityLogs.historyForSubject': new Error('no table'),
      'tax.calculate': new Error('no jurisdiction'),
    });
    const result = await explainOrder(commerce);
    assert.equal(result.success, true);
    const topics = result.unavailable.map((u) => u.topic);
    for (const topic of [
      'payments',
      'fraud',
      'activity_log',
      'tax',
      'refunds',
      'kernel_receipts',
    ]) {
      assert.ok(topics.includes(topic), `${topic} missing from ${topics}`);
    }
    assert.equal(result.money.charged, '0.00');
    assert.equal(result.tax, null);
  });

  it('explains an order without a checkout cart from its own items', async () => {
    const { commerce, calls } = orderStore({ 'carts.forCustomer': [] });
    const result = await explainOrder(commerce);
    assert.equal(result.money.discount, null);
    assert.equal(result.promotions, null);
    assert.ok(result.unavailable.some((u) => u.topic === 'checkout_cart'));
    const taxCall = calls.find((c) => c.call === 'tax.calculate');
    assert.equal(taxCall.args[0].lineItems[0].id, 'oi-1');
  });

  it('resolves an order number by scanning, and refuses unknown orders', async () => {
    const found = await explainOrder(orderStore().commerce, 'ORD-1');
    assert.equal(found.order.id, ORDER_ID);
    assert.ok(found.unavailable.some((u) => u.topic === 'order_by_number'));

    const missing = await explainOrder(
      orderStore({ 'orders.get': null, 'orders.list': [] }).commerce,
      '99999999-9999-4999-8999-999999999999',
    );
    assert.equal(missing.success, false);
    const noNumber = await explainOrder(orderStore({ 'orders.list': [] }).commerce, 'ORD-404');
    assert.equal(noNumber.success, false);
  });
});

describe('explain_cart_pricing', () => {
  const cartStore = (overrides = {}) =>
    recordingCommerce({
      'carts.get': cart({ status: 'active', orderId: null }),
      'carts.getItems': [cartItem()],
      'tax.calculate': taxResult(),
      'promotions.apply': (input) =>
        input.couponCodes.includes('BOGUS')
          ? promoResult({
              rejectedPromotions: [
                { couponCode: 'BOGUS', reason: 'Invalid coupon code', reasonCode: 'invalid_code' },
              ],
            })
          : promoResult(),
      ...overrides,
    });

  it('explains and reconciles a consistent cart', async () => {
    const { commerce, calls } = cartStore();
    const result = await explainCart(commerce);
    assert.equal(result.success, true);
    assert.equal(result.lines[0].gross, '100.00');
    assert.equal(result.subtotal.fromLines, '100.00');
    assert.equal(result.promotions.totalDiscount, '10.00');
    assert.equal(result.tax.totalTax, '0.00');
    assert.equal(result.total.explainedTotal, '90.00');
    assert.equal(result.total.matches, true);
    assert.deepEqual(result.flags, []);
    assert.equal(result.couponCheck, null);
    const apply = calls.find((c) => c.call === 'promotions.apply').args[0];
    assert.deepEqual(apply.couponCodes, ['WELCOME10']);
    assert.equal(apply.lineItems[0].lineTotal, 100);
  });

  it('shows why an extra code is refused without touching the cart evaluation', async () => {
    const { commerce, calls } = cartStore();
    const result = await explainCart(commerce, { couponCodes: ['BOGUS'] });
    assert.deepEqual(result.promotions.rejected, []);
    assert.deepEqual(result.couponCheck.rejected, [
      {
        promotionId: null,
        couponCode: 'BOGUS',
        reasonCode: 'invalid_code',
        reason: 'Invalid coupon code',
      },
    ]);
    assert.deepEqual(
      calls.filter((c) => c.call === 'promotions.apply').map((c) => c.args[0].couponCodes),
      [['WELCOME10'], ['WELCOME10', 'BOGUS']],
    );
  });

  it('flags a grand total, tax and discount that do not add up', async () => {
    const { commerce } = cartStore({
      'carts.get': cart({ status: 'active', grandTotalExact: '91', taxAmountExact: '0' }),
      'tax.calculate': taxResult('7.25'),
      'promotions.apply': promoResult({ totalDiscountExact: '0', appliedPromotions: [] }),
    });
    const result = await explainCart(commerce);
    assert.equal(result.total.matches, false);
    assert.equal(result.total.difference, '1.00');
    assert.deepEqual(codes(result).sort(), [
      'discount_not_current',
      'grand_total_unexplained',
      'tax_not_current',
    ]);
  });

  it("flags the cart's own coupon when it is refused now", async () => {
    const { commerce } = cartStore({
      'promotions.apply': promoResult({
        totalDiscountExact: '0',
        appliedPromotions: [],
        rejectedPromotions: [{ couponCode: 'WELCOME10', reason: 'expired', reasonCode: 'expired' }],
      }),
    });
    const flagged = (await explainCart(commerce)).flags.find(
      (f) => f.code === 'cart_coupon_refused_now',
    );
    assert.equal(flagged.severity, 'warning');
    assert.match(flagged.message, /expired/);
  });

  it('degrades without an address and refuses an unknown cart', async () => {
    const noAddress = await explainCart(
      cartStore({ 'carts.get': cart({ status: 'active', shippingAddress: undefined }) }).commerce,
    );
    assert.equal(noAddress.tax, null);
    assert.ok(noAddress.unavailable.some((u) => u.topic === 'tax'));
    assert.equal(noAddress.total.matches, true);

    const missing = await explainCart(cartStore({ 'carts.get': null }).commerce);
    assert.equal(missing.success, false);
  });
});

describe('timeKey', () => {
  it('orders mixed-precision, mixed-offset timestamps', () => {
    assert.ok(timeKey('2026-09-01T10:00:00.000000001+00:00') > timeKey('2026-09-01T10:00:00Z'));
    assert.ok(timeKey('2026-09-01T10:00:00.5Z') < timeKey('2026-09-01T10:00:00.500000001Z'));
    assert.ok(timeKey('2026-09-01T12:00:00+02:00') === timeKey('2026-09-01T10:00:00Z'));
    assert.equal(timeKey(undefined), null);
  });
});
