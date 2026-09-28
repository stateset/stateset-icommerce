/**
 * explain_order and explain_cart_pricing through the real MCP server.
 *
 * Builds the same journey as mcp-golden-path.test.js -- customer, first-order
 * coupon, cart, tax, checkout through the kernel, payment, shipment, return,
 * refund -- with real `executeTool` calls on a fresh store, then asks the
 * explain tools to tell the story back and checks it against what happened.
 *
 * Kernel mode is non-strict (see mcp-golden-path.test.js for why).
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';
import { addDecimals, compareDecimals, subtractDecimals } from '../../src/utils/exact-decimal.js';

const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-explain-'));
const capabilities = [...new Set(Object.values(KERNEL_CAPABILITY_BY_TOOL))];
const commandPolicy = (capability) => ({
  required_capabilities: [capability],
  requires_approval: false,
  requires_tenant: true,
  requires_store: true,
  allowed_tenant_ids: ['tenant:explain'],
  allowed_store_ids: ['store:explain'],
  requires_agent_delegation: true,
  requires_signed_authority: false,
});

let server;
let seq = 0;
const journey = {};

/** Call a tool; fail with the tool's own error when it (or its receipt) says it failed. */
async function call(name, params) {
  const r = await server.executeTool(name, params, { idempotencyKey: `explain-${name}-${++seq}` });
  const failed = !r.success || r.result?.success === false;
  if (failed) {
    const why = r.error ?? r.result?.error ?? r.result?.receipt?.error_message ?? r;
    assert.fail(`${name} failed: ${typeof why === 'string' ? why : JSON.stringify(why)}`);
  }
  return r.result;
}

/** A governed tool answers with its receipt's result; a legacy tool with its own shape. */
const entity = (response, key) => response[key] ?? response.receipt?.result ?? response.result;

const ADDRESS = {
  firstName: 'Ada',
  lastName: 'Lovelace',
  line1: '1 Main St',
  city: 'Los Angeles',
  state: 'CA',
  postalCode: '90001',
  country: 'US',
};

before(async () => {
  server = createStatesetMcpServer({
    dbPath: path.join(dir, 'store.db'),
    allowApply: true,
    kernel: {
      strict: false,
      storeId: 'store:explain',
      principal: {
        id: 'agent:explain',
        kind: 'agent',
        tenantId: 'tenant:explain',
        delegatedBy: 'user:explain',
        capabilities,
      },
      policy: {
        version: 'explain-v1',
        commands: Object.fromEntries(capabilities.map((c) => [c, commandPolicy(c)])),
        trusted_authority_keys: {},
      },
    },
  });

  journey.customer = entity(
    await call('create_customer', { email: 'ada@example.com', firstName: 'Ada', lastName: 'L' }),
    'customer',
  );
  await call('create_inventory_item', { sku: 'W-1', name: 'Widget', initialQuantity: '10' });
  const promotion = (
    await call('create_promotion', {
      name: 'Welcome 10%',
      type: 'percentage_off',
      trigger: 'coupon_code',
      percentageOff: 0.1,
      conditions: [{ conditionType: 'first_order', operator: 'equals', value: 'true' }],
    })
  ).promotion;
  await call('activate_promotion', { promotionId: promotion.id });
  await call('create_coupon', { promotionId: promotion.id, code: 'WELCOME10' });

  const cartId = entity(
    await call('create_cart', { customerId: journey.customer.id, currency: 'USD' }),
    'cart',
  ).id;
  journey.cartId = cartId;
  await call('add_cart_item', { cartId, sku: 'W-1', name: 'Widget', quantity: 2, unitPrice: 50 });
  await call('set_cart_shipping_address', { cartId, ...ADDRESS });
  // Coupon first, then tax: tax is charged on the discounted price.
  await call('apply_cart_discount', { cartId, couponCode: 'WELCOME10' });
  await call('calculate_cart_tax', { cartId });
  await call('set_cart_payment', { cartId, paymentMethod: 'credit_card', paymentToken: 'tok' });
  journey.cart = (await call('get_cart', { identifier: cartId })).cart;
  // Priced before checkout: the coupon is live, and a bogus code is refused.
  journey.cartExplained = await call('explain_cart_pricing', {
    cartId,
    couponCodes: ['NOT-A-CODE'],
  });

  await call('complete_checkout', { cartId });
  const orders = await call('list_orders', { customerId: journey.customer.id });
  journey.orderId = (orders.orders ?? orders)[0].id;
  journey.order = (await call('get_order', { identifier: journey.orderId })).order;

  const payment = await call('create_payment', {
    orderId: journey.orderId,
    amount: Number(journey.order.totalAmount),
    currency: 'USD',
    method: 'credit_card',
  });
  journey.paymentId = payment.payment?.id ?? payment.receipt?.aggregate_id;
  await call('complete_payment', { paymentId: journey.paymentId });
  await call('update_order_status', { orderId: journey.orderId, status: 'processing' });
  await call('create_shipment', {
    orderId: journey.orderId,
    recipientName: 'Ada Lovelace',
    shippingAddress: '1 Main St, Los Angeles, CA 90001, US',
    carrier: 'ups',
    service: 'ground',
  });
  await call('ship_order', { orderId: journey.orderId, trackingNumber: '1Z999' });
  const ret = await call('create_return', {
    orderId: journey.orderId,
    reason: 'defective',
    items: [{ orderItemId: journey.order.items[0].id, quantity: 1 }],
  });
  journey.returnId = entity(ret, 'return')?.id ?? ret.id ?? ret.receipt?.aggregate_id;
  await call('approve_return', { returnId: journey.returnId });
  await call('add_return_tracking', { returnId: journey.returnId, trackingNumber: 'RET-1Z' });
  await call('mark_return_received', { returnId: journey.returnId });
  await call('complete_return', { returnId: journey.returnId });
  await call('create_refund', { paymentId: journey.paymentId, amount: '10.00', reason: 'return' });

  journey.explained = await call('explain_order', { orderId: journey.orderId });
  journey.explainedByNumber = await call('explain_order', {
    orderId: journey.order.orderNumber,
  });

  // A returning customer's next cart: the first-order coupon no longer applies.
  const nextCartId = entity(
    await call('create_cart', { customerId: journey.customer.id, currency: 'USD' }),
    'cart',
  ).id;
  await call('add_cart_item', {
    cartId: nextCartId,
    sku: 'W-1',
    name: 'Widget',
    quantity: 1,
    unitPrice: 50,
  });
  await call('set_cart_shipping_address', { cartId: nextCartId, ...ADDRESS });
  journey.nextCartExplained = await call('explain_cart_pricing', {
    cartId: nextCartId,
    couponCodes: ['WELCOME10'],
  });
});

after(async () => {
  await server?.close?.();
  rmSync(dir, { recursive: true, force: true });
});

describe('explain_order through the MCP server', () => {
  it('tells the story in order: checkout, payment, shipment, return', () => {
    const { timeline } = journey.explained;
    const first = (kind) => timeline.findIndex((entry) => entry.kind === kind);
    const order = ['checkout', 'payment', 'shipment', 'return'].map(first);
    for (const [i, kind] of ['checkout', 'payment', 'shipment', 'return'].entries()) {
      assert.ok(order[i] >= 0, `no ${kind} entry in ${JSON.stringify(timeline, null, 1)}`);
    }
    assert.deepEqual(
      [...order].sort((a, b) => a - b),
      order,
      JSON.stringify(timeline.map((e) => `${e.at} ${e.kind}`)),
    );
    assert.ok(timeline.every((entry) => entry.at && entry.summary && entry.ref));
  });

  it(
    'puts the refund on the timeline after the return',
    {
      todo: 'the binding has no refund read (payments.getRefunds / amountRefunded); createRefund leaves the refund pending and the payment completed',
    },
    () => {
      const kinds = journey.explained.timeline.map((e) => e.kind);
      assert.ok(kinds.lastIndexOf('refund') > kinds.lastIndexOf('return'));
    },
  );

  it('sums the money exactly: charged is the order total, net = charged - refunded', () => {
    const { money } = journey.explained;
    assert.equal(money.currency, 'USD');
    // 2 x 50.00 - 10% coupon + the seeded Los Angeles sales tax.
    assert.equal(compareDecimals(money.orderTotal, String(journey.order.totalAmount)), 0);
    assert.equal(compareDecimals(money.orderTotal, '90.00'), 1, 'the seeded CA tax applies');
    assert.equal(money.charged, money.orderTotal);
    assert.equal(money.discount, '10.00');
    assert.equal(money.couponCode, 'WELCOME10');
    assert.equal(money.explainedTotal, money.orderTotal);
    assert.equal(typeof money.refunded, 'string');
    assert.equal(money.net, subtractDecimals(money.charged, money.refunded));
    assert.equal(
      compareDecimals(addDecimals(money.net, money.refunded), money.charged),
      0,
      'net + refunded must equal charged exactly',
    );
  });

  it(
    'counts the 10.00 refund in money.refunded',
    { todo: 'refund amounts are not readable through the Node binding' },
    () => {
      assert.equal(journey.explained.money.refunded, '10.00');
      assert.equal(
        journey.explained.money.net,
        subtractDecimals(journey.explained.money.charged, '10.00'),
      );
    },
  );

  it('finds the order statuses kept in step with payment and shipment', () => {
    const codes = journey.explained.flags.map((f) => f.code);
    assert.ok(!codes.includes('order_payment_status_stale'), codes.join(','));
    assert.ok(!codes.includes('order_fulfillment_status_stale'), codes.join(','));
    assert.ok(codes.includes('return_refund_unverifiable'), codes.join(','));
    assert.ok(!codes.includes('over_captured'));
    assert.ok(!codes.includes('order_total_differs_from_cart'));
  });

  it('names what it cannot see instead of guessing', () => {
    const topics = journey.explained.unavailable.map((u) => u.topic);
    for (const topic of ['refunds', 'promotion_usage', 'order_tax', 'kernel_receipts']) {
      assert.ok(topics.includes(topic), topics.join(','));
    }
    assert.equal(journey.explained.tax.basis, 'recomputed_now');
    assert.equal(journey.explained.fraud.assessed, false);
  });

  it('resolves an order number to the same story', () => {
    assert.equal(journey.explainedByNumber.order.id, journey.orderId);
    assert.deepEqual(journey.explainedByNumber.money, journey.explained.money);
    assert.ok(journey.explainedByNumber.unavailable.some((u) => u.topic === 'order_by_number'));
  });

  it('refuses an unknown order cleanly', async () => {
    const r = await server.executeTool('explain_order', {
      orderId: '00000000-0000-4000-8000-000000000000',
    });
    assert.equal(r.result?.success ?? r.success, false);
  });
});

describe('explain_cart_pricing through the MCP server', () => {
  it('reconciles the explained total with the stored grand total', () => {
    const { total, stored, subtotal } = journey.cartExplained;
    assert.equal(compareDecimals(stored.grandTotal, '90.00'), 1, 'the seeded CA tax applies');
    assert.equal(compareDecimals(stored.tax, '0'), 1);
    assert.equal(Number(stored.grandTotal), Number(journey.cart.grandTotal));
    assert.equal(total.explainedTotal, stored.grandTotal);
    assert.equal(total.matches, true);
    assert.equal(subtotal.fromLines, subtotal.stored);
    assert.deepEqual(
      journey.cartExplained.flags.filter((f) => f.severity !== 'info'),
      [],
      'a freshly priced cart has nothing to flag',
    );
  });

  it('shows the applied coupon and a refused code with its reason code', () => {
    const { promotions, couponCheck } = journey.cartExplained;
    assert.equal(promotions.basis, 'reevaluated_now');
    assert.equal(promotions.totalDiscount, '10.00');
    assert.ok(promotions.applied.some((p) => p.couponCode === 'WELCOME10'));
    const bogus = couponCheck.rejected.find((p) => p.couponCode === 'NOT-A-CODE');
    assert.equal(bogus?.reasonCode, 'invalid_code', JSON.stringify(couponCheck));
  });

  it('breaks the tax down as recomputed from the cart address', () => {
    const { tax, stored } = journey.cartExplained;
    assert.equal(tax.basis, 'recomputed_now');
    assert.equal(tax.totalTax, stored.tax);
    assert.ok(Array.isArray(tax.breakdown));
  });

  it("explains why a returning customer's first-order coupon is refused", () => {
    const { couponCheck, total } = journey.nextCartExplained;
    const refused = couponCheck.rejected.find((p) => p.couponCode === 'WELCOME10');
    assert.ok(refused, JSON.stringify(couponCheck));
    assert.ok(refused.reasonCode, 'refusal carries a reason code');
    assert.notEqual(refused.reasonCode, 'invalid_code');
    assert.equal(total.matches, true);
  });
});
