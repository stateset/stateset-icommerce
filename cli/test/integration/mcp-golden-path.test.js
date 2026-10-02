/**
 * The whole commerce journey, through the real MCP server, on a fresh store.
 *
 * Customer, governed stock, a first-order welcome coupon, cart, tax,
 * checkout committed through the trusted kernel, payment, shipment, return
 * and refund -- every step a real `executeTool` call, and the money checked
 * where it changes hands. Each piece has unit tests; this checks that they
 * compose, as an agent using the product composes them.
 *
 * The journey runs twice: on a legacy-migration endpoint (`kernel.strict:
 * false`) and on the recommended production posture (`kernel.strict: true`),
 * where only governed commands are exposed for writes. On the strict endpoint
 * every write step is a typed kernel command with a sealed receipt. Promotions
 * and coupons are operator configuration, not agent actions, so the strict
 * journey provisions the welcome coupon directly on the store (as an operator
 * would) and the agent only redeems it.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { getCommerce } from '../../src/database.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';

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

const money = (value) => Number(value);

/** The welcome promotion: 10% off a customer's first order, by coupon. */
const WELCOME = {
  name: 'Welcome 10%',
  type: 'percentage_off',
  trigger: 'coupon_code',
  percentageOff: 0.1,
  conditions: [{ conditionType: 'first_order', operator: 'equals', value: 'true' }],
};

function journeyFor(strict) {
  const dir = mkdtempSync(path.join(os.tmpdir(), `mcp-golden-${strict ? 'strict' : 'legacy'}-`));
  const dbPath = path.join(dir, 'store.db');
  const state = { journey: {}, server: null, seq: 0, governed: [] };

  /**
   * Call a tool; fail with the tool's own error when it (or its receipt) says
   * it failed. The result contract (`ok`, `failure`) already reads every
   * failure shape, a rejected kernel receipt included.
   */
  async function call(name, params) {
    const r = await state.server.executeTool(name, params, {
      idempotencyKey: `golden-${name}-${++state.seq}`,
    });
    if (!r.ok || r.preview) {
      const why = r.failure ? `${r.failure.code}: ${r.failure.message}` : 'preview only';
      assert.fail(`${name} failed: ${why}`);
    }
    if (r.result?.kernel) state.governed.push({ tool: name, receipt: r.result.receipt });
    return r.result;
  }

  /** A governed tool answers with its receipt's result; a legacy tool with its own shape. */
  const entity = (response, key) => response[key] ?? response.receipt?.result ?? response.result;

  /** Provision the welcome coupon: through the agent (legacy) or as the operator (strict). */
  async function provisionWelcomeCoupon() {
    if (!strict) {
      const promotion = (await call('create_promotion', WELCOME)).promotion;
      await call('activate_promotion', { promotionId: promotion.id });
      await call('create_coupon', { promotionId: promotion.id, code: 'WELCOME10' });
      return;
    }
    const commerce = getCommerce(dbPath);
    const promotion = await commerce.promotions.create({
      name: WELCOME.name,
      promotionType: WELCOME.type,
      trigger: WELCOME.trigger,
      target: 'order',
      stacking: 'stackable',
      percentageOff: WELCOME.percentageOff,
      conditions: WELCOME.conditions,
      priority: 1,
    });
    await commerce.promotions.activate(promotion.id);
    await commerce.promotions.createCoupon({ promotionId: promotion.id, code: 'WELCOME10' });
  }

  before(async () => {
    state.server = createStatesetMcpServer({
      dbPath,
      allowApply: true,
      kernel: {
        strict,
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
    const { journey } = state;

    journey.customer = entity(
      await call('create_customer', {
        email: 'ada@example.com',
        firstName: 'Ada',
        lastName: 'Lovelace',
      }),
      'customer',
    );
    await call('create_inventory_item', { sku: 'W-1', name: 'Widget', initialQuantity: '10' });
    await provisionWelcomeCoupon();

    const cartId = entity(
      await call('create_cart', { customerId: journey.customer.id, currency: 'USD' }),
      'cart',
    ).id;
    await call('add_cart_item', {
      cartId,
      sku: 'W-1',
      name: 'Widget',
      quantity: 2,
      unitPrice: '50.00',
    });
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
      amount: journey.order.totalAmountExact ?? String(journey.order.totalAmount),
      currency: 'USD',
      method: 'credit_card',
    });
    journey.paymentId = payment.payment?.id ?? payment.receipt?.aggregate_id;
    journey.payment = entity(
      await call('complete_payment', { paymentId: journey.paymentId }),
      'payment',
    );

    await call('update_order_status', { orderId: journey.orderId, status: 'processing' });
    journey.shipment = entity(
      await call('create_shipment', {
        orderId: journey.orderId,
        recipientName: 'Ada Lovelace',
        shippingAddress: '1 Main St, Los Angeles, CA 90001, US',
        carrier: 'ups',
        service: 'ground',
      }),
      'shipment',
    );
    await call('ship_order', { orderId: journey.orderId, trackingNumber: '1Z999' });

    const ret = await call('create_return', {
      orderId: journey.orderId,
      reason: 'defective',
      items: [{ orderItemId: journey.order.items[0].id, quantity: 1 }],
    });
    journey.returnId = ret.return?.id ?? ret.result?.id ?? ret.id;
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
    await state.server?.close?.();
    rmSync(dir, { recursive: true, force: true });
  });

  return state;
}

for (const strict of [false, true]) {
  describe(`the commerce journey through the MCP server (kernel.strict: ${strict})`, () => {
    const state = journeyFor(strict);
    const { journey } = state;

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
      assert.ok(journey.shipment.id, 'a shipment was created for the order');
      const refunded = money(
        journey.refund.refund?.amountExact ??
          journey.refund.refund?.amount ??
          journey.refund.amount ??
          journey.refund.receipt?.result?.amount,
      );
      assert.equal(refunded, 10, JSON.stringify(journey.refund).slice(0, 600));
      assert.ok(refunded <= money(journey.payment.amount));
    });

    it('runs every write through a governed command with a sealed, allowed receipt', () => {
      const writes = [
        'create_customer',
        'create_inventory_item',
        'create_cart',
        'add_cart_item',
        'set_cart_shipping_address',
        'calculate_cart_tax',
        'apply_cart_discount',
        'set_cart_payment',
        'complete_checkout',
        'create_payment',
        'complete_payment',
        'update_order_status',
        'create_shipment',
        'ship_order',
        'create_return',
        'approve_return',
        'add_return_tracking',
        'mark_return_received',
        'complete_return',
        'create_refund',
      ];
      const governed = new Map(state.governed.map((entry) => [entry.tool, entry.receipt]));
      for (const tool of writes) {
        const receipt = governed.get(tool);
        assert.ok(receipt, `${tool} did not run through the kernel`);
        assert.equal(receipt.status, 'succeeded', `${tool}: ${receipt.error_message}`);
        assert.equal(receipt.policy.allowed, true, tool);
        assert.ok(receipt.audit_hash, `${tool} receipt is not sealed into the audit chain`);
      }
      if (strict) {
        // Promotion provisioning is operator configuration; the agent never wrote it.
        assert.ok(!state.governed.some((entry) => entry.tool.includes('promotion')));
      }
    });

    it('never seals the payment token onto a receipt', () => {
      const receipt = state.governed.find((entry) => entry.tool === 'set_cart_payment')?.receipt;
      assert.ok(receipt);
      assert.doesNotMatch(JSON.stringify(receipt), /tok_test/);
    });

    it(
      'taxes a Los Angeles sale at the seeded California rate',
      {
        todo: 'fresh SQLite stores charge zero tax: seeded jurisdiction ids never match (fix/tax-seed-ids)',
      },
      () => {
        const tax = journey.tax.result?.calculation ?? journey.tax;
        assert.equal(money(tax.total_tax ?? tax.totalTax ?? tax.tax?.totalTax), 7.25);
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
}

describe('the strict endpoint exposes no ungoverned write', () => {
  it('refuses promotion provisioning from a strict agent', async () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-golden-exposure-'));
    const server = createStatesetMcpServer({
      dbPath: path.join(dir, 'store.db'),
      allowApply: true,
      kernel: {
        strict: true,
        storeId: 'store:golden',
        principal: {
          id: 'agent:golden',
          kind: 'agent',
          tenantId: 'tenant:golden',
          delegatedBy: 'user:golden',
          capabilities,
        },
        policy: { version: 'golden-v1', commands: {}, trusted_authority_keys: {} },
      },
    });
    try {
      const r = await server.executeTool('create_promotion', WELCOME, {
        idempotencyKey: 'golden-exposure',
      });
      assert.ok(!r.success || r.result?.success === false, JSON.stringify(r).slice(0, 300));
    } finally {
      await server?.close?.();
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
