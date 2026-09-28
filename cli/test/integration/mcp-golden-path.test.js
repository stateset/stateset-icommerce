/**
 * The whole commerce journey, through the real MCP server, on a fresh store.
 *
 * Customer, governed stock, a first-order welcome coupon, cart, tax,
 * checkout committed through the trusted kernel, payment, shipment, return
 * and refund -- every step a real `executeTool` call, and the money checked
 * where it changes hands. Each piece has unit tests; this checks that they
 * compose, as an agent using the product composes them.
 *
 * Kernel mode is non-strict: strict mode exposes only governed commands, and
 * carts, customers and promotions are not governed, so a strict endpoint
 * cannot run a checkout end to end today.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';

const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-golden-'));
const capabilities = [...new Set(Object.values(KERNEL_CAPABILITY_BY_TOOL))];
const commandPolicy = (capability) => ({
  required_capabilities: [capability],
  requires_approval: false,
  requires_tenant: true,
  requires_store: true,
  allowed_tenant_ids: ['tenant:golden'],
  allowed_store_ids: ['store:golden'],
  requires_agent_delegation: true,
  requires_signed_authority: false,
});

let server;
let seq = 0;
const journey = {};

/**
 * Call a tool; fail with the tool's own error when it (or its receipt) says it
 * failed. The result contract (`ok`, `failure`) already reads every failure
 * shape, a rejected kernel receipt included.
 */
async function call(name, params) {
  const r = await server.executeTool(name, params, { idempotencyKey: `golden-${name}-${++seq}` });
  if (!r.ok || r.preview) {
    const why = r.failure ? `${r.failure.code}: ${r.failure.message}` : 'preview only';
    assert.fail(`${name} failed: ${why}`);
  }
  return r.result;
}

const money = (value) => Number(value);

before(async () => {
  server = createStatesetMcpServer({
    dbPath: path.join(dir, 'store.db'),
    allowApply: true,
    kernel: {
      strict: false,
      storeId: 'store:golden',
      principal: {
        id: 'agent:golden',
        kind: 'agent',
        tenantId: 'tenant:golden',
        delegatedBy: 'user:golden',
        capabilities,
      },
      policy: {
        version: 'golden-v1',
        commands: Object.fromEntries(capabilities.map((c) => [c, commandPolicy(c)])),
        trusted_authority_keys: {},
      },
    },
  });

  journey.customer = (
    await call('create_customer', {
      email: 'ada@example.com',
      firstName: 'Ada',
      lastName: 'Lovelace',
    })
  ).customer;
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

  const cartId = (await call('create_cart', { customerId: journey.customer.id, currency: 'USD' }))
    .cart.id;
  await call('add_cart_item', { cartId, sku: 'W-1', name: 'Widget', quantity: 2, unitPrice: 50 });
  await call('set_cart_shipping_address', {
    cartId,
    firstName: 'Ada',
    lastName: 'Lovelace',
    line1: '1 Main St',
    city: 'Los Angeles',
    state: 'CA',
    postalCode: '90001',
    country: 'US',
  });
  journey.tax = await call('calculate_cart_tax', { cartId });
  await call('apply_cart_discount', { cartId, couponCode: 'WELCOME10' });
  await call('set_cart_payment', {
    cartId,
    paymentMethod: 'credit_card',
    paymentToken: 'tok_test',
  });
  journey.cart = (await call('get_cart', { identifier: cartId })).cart;

  journey.checkout = await call('complete_checkout', { cartId });
  const orders = await call('list_orders', { customerId: journey.customer.id });
  journey.orderId = (orders.orders ?? orders)[0].id;
  journey.order = (await call('get_order', { identifier: journey.orderId })).order;

  const payment = await call('create_payment', {
    orderId: journey.orderId,
    amount: money(journey.order.totalAmount),
    currency: 'USD',
    method: 'credit_card',
  });
  journey.paymentId = payment.payment?.id ?? payment.receipt?.aggregate_id;
  journey.payment = (await call('complete_payment', { paymentId: journey.paymentId })).payment;

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
  journey.returnId = ret.return?.id ?? ret.id;
  await call('approve_return', { returnId: journey.returnId });
  await call('add_return_tracking', { returnId: journey.returnId, trackingNumber: 'RET-1Z' });
  await call('mark_return_received', { returnId: journey.returnId });
  await call('complete_return', { returnId: journey.returnId });
  journey.refund = await call('create_refund', {
    paymentId: journey.paymentId,
    amount: '10.00',
    reason: 'return',
  });
  journey.finalOrder = (await call('get_order', { identifier: journey.orderId })).order;
});

after(async () => {
  await server?.close?.();
  rmSync(dir, { recursive: true, force: true });
});

describe('the commerce journey through the MCP server', () => {
  it('prices the cart: first-order coupon applied to a new customer', () => {
    assert.equal(money(journey.cart.subtotal), 100);
    assert.equal(money(journey.cart.discountAmount), 10);
    assert.equal(journey.cart.couponCode, 'WELCOME10');
  });

  it('commits checkout through the kernel with an allowed, audited receipt', () => {
    assert.equal(journey.checkout.kernel, true);
    assert.equal(journey.checkout.receipt.policy.allowed, true);
    assert.ok(journey.checkout.receipt.audit_hash);
  });

  it('carries the cart grand total onto the order', () => {
    assert.equal(money(journey.order.totalAmount), money(journey.cart.grandTotal));
    assert.equal(journey.order.items.length, 1);
  });

  it('takes a payment for exactly the order total', () => {
    assert.equal(journey.payment.status, 'completed');
    assert.equal(
      money(journey.payment.amountExact ?? journey.payment.amount),
      money(journey.order.totalAmount),
    );
  });

  it('ships, accepts the return and refunds within the payment', () => {
    assert.equal(journey.finalOrder.status, 'shipped', JSON.stringify(journey.finalOrder));
    const refunded = money(
      journey.refund.refund?.amountExact ??
        journey.refund.refund?.amount ??
        journey.refund.amount ??
        journey.refund.receipt?.result?.amount,
    );
    assert.equal(refunded, 10, JSON.stringify(journey.refund).slice(0, 600));
    assert.ok(refunded <= money(journey.payment.amount));
  });

  it(
    'taxes a Los Angeles sale at the seeded California rate',
    {
      todo: 'fresh SQLite stores charge zero tax: seeded jurisdiction ids never match (fix/tax-seed-ids)',
    },
    () => {
      assert.equal(money(journey.tax.totalTax ?? journey.tax.tax?.totalTax), 7.25);
    },
  );

  it(
    'keeps the order payment and fulfillment status in step with payments and shipments',
    { todo: 'nothing maintains orders.payment_status / fulfillment_status after checkout' },
    () => {
      assert.notEqual(journey.finalOrder.paymentStatus, 'pending');
      assert.notEqual(journey.finalOrder.fulfillmentStatus, 'unfulfilled');
    },
  );
});
