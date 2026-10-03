/**
 * Checkout trio — get_checkout / update_checkout / complete_checkout.
 *
 * get_checkout is a read that unifies cart state plus a readiness checklist.
 * update_checkout is a governed composite: one validated call whose steps run
 * as typed kernel commands in strict mode (each policy-checked), or as direct
 * engine calls on the legacy path. complete_checkout refuses unready carts
 * with the missing pieces instead of a deep engine error.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { cartTools } from '../../src/tools/carts.js';
import { createKernelToolExecutor } from '../../src/kernel-tool-execution.js';

function findTool(name) {
  const tool = cartTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in cartTools`);
  return tool;
}

const ADDRESS = {
  firstName: 'Ada',
  lastName: 'Lovelace',
  line1: '1 Main St',
  city: 'Novato',
  state: 'CA',
  postalCode: '94945',
  country: 'US',
};

function makeCart(overrides = {}) {
  return {
    id: 'cart_001',
    cartNumber: 'CART-100001',
    customerId: 'cust_001',
    customerEmail: 'ada@example.com',
    status: 'active',
    paymentStatus: 'none',
    currency: 'USD',
    subtotalExact: '39.98',
    discountAmountExact: '0',
    taxAmountExact: '0',
    shippingAmountExact: '5.00',
    grandTotalExact: '44.98',
    paymentMethod: null,
    shippingMethod: null,
    shippingCarrier: null,
    couponCode: null,
    itemCount: 1,
    shippingAddress: null,
    ...overrides,
  };
}

function makeItem(overrides = {}) {
  return {
    id: 'item_001',
    sku: 'DEMO-COFFEE',
    name: 'Demo Coffee',
    quantity: 2,
    unitPriceExact: '19.99',
    totalExact: '39.98',
    discountAmountExact: '0',
    ...overrides,
  };
}

/** Mock commerce; records calls for assertions. */
function makeCommerce({ cart = makeCart(), items = [makeItem()] } = {}) {
  const calls = [];
  let current = { ...cart };
  return {
    calls,
    carts: {
      get: async (id) => {
        calls.push(['carts.get', id]);
        return id === 'missing' ? null : { ...current };
      },
      getItems: async (id) => {
        calls.push(['carts.getItems', id]);
        return items.map((item) => ({ ...item }));
      },
      setShippingAddress: async (id, address) => {
        calls.push(['carts.setShippingAddress', id, address]);
        current = { ...current, shippingAddress: address };
        return { ...current };
      },
      setPayment: async (id, input) => {
        calls.push(['carts.setPayment', id, input]);
        current = {
          ...current,
          paymentMethod: input.paymentMethod,
          paymentStatus: 'method_selected',
        };
        return { ...current };
      },
      applyDiscount: async (id, code) => {
        calls.push(['carts.applyDiscount', id, code]);
        if (code === 'NOPE') throw new Error('Invalid coupon');
        current = { ...current, couponCode: code, discountAmountExact: '4.00' };
        return { ...current };
      },
      setTax: async (id, amount, exact) => {
        calls.push(['carts.setTax', id, exact]);
        current = { ...current, taxAmountExact: exact };
        return { ...current };
      },
      complete: async (id) => {
        calls.push(['carts.complete', id]);
        return {
          orderId: 'order_001',
          orderNumber: 'ORD-1',
          cartId: id,
          totalCharged: 44.98,
          totalChargedExact: '44.98',
          currency: 'USD',
        };
      },
    },
    tax: {
      calculate: async (input) => {
        calls.push(['tax.calculate', input.shippingAddress.city]);
        return { totalTaxExact: '3.60' };
      },
    },
  };
}

describe('get_checkout', () => {
  const tool = findTool('get_checkout');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns the unified summary with a readiness checklist', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({ commerce, params: { cartId: 'cart_001' } });
    assert.equal(result.success, true);
    assert.equal(result.checkout.cartId, 'cart_001');
    assert.equal(result.checkout.grandTotalExact, '44.98');
    assert.equal(result.checkout.items.length, 1);
    assert.equal(result.checkout.items[0].sku, 'DEMO-COFFEE');
    // Bare cart: no address, no payment method.
    assert.deepEqual(result.checkout.readiness, {
      ready: false,
      missing: ['shipping_address', 'payment_method'],
    });
  });

  it('reports ready when items, address, and payment are present', async () => {
    const commerce = makeCommerce({
      cart: makeCart({ shippingAddress: ADDRESS, paymentMethod: 'credit_card' }),
    });
    const result = await tool.handler({ commerce, params: { cartId: 'cart_001' } });
    assert.deepEqual(result.checkout.readiness, { ready: true, missing: [] });
  });

  it('flags an empty cart and a non-active cart', async () => {
    const empty = makeCommerce({ items: [] });
    const bare = await findTool('get_checkout').handler({
      commerce: empty,
      params: { cartId: 'cart_001' },
    });
    assert.ok(bare.checkout.readiness.missing.includes('items'));

    const cancelled = makeCommerce({ cart: makeCart({ status: 'cancelled' }) });
    const done = await findTool('get_checkout').handler({
      commerce: cancelled,
      params: { cartId: 'cart_001' },
    });
    assert.ok(done.checkout.readiness.missing.includes('cart_not_active'));
  });

  it('returns not-found for an unknown cart', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({ commerce, params: { cartId: 'missing' } });
    assert.equal(result.success, false);
    assert.match(result.error, /not found/i);
  });
});

describe('update_checkout', () => {
  const tool = findTool('update_checkout');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('refuses a call with nothing to update', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.match(result.error, /Nothing to update/);
    assert.ok(!commerce.calls.some(([name]) => name === 'carts.setPayment'));
  });

  it('previews without applying when --apply is off', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { cartId: 'cart_001', paymentMethod: 'credit_card' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.match(result.error, /--apply/);
    assert.deepEqual(result.wouldUpdate, { cartId: 'cart_001', steps: ['payment_method'] });
    assert.ok(!commerce.calls.some(([name]) => name === 'carts.setPayment'));
  });

  it('legacy path applies address, payment, coupon, then tax', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: {
        cartId: 'cart_001',
        shippingAddress: ADDRESS,
        paymentMethod: 'credit_card',
        paymentToken: 'tok_123',
        couponCode: 'WELCOME10',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(result.appliedSteps, ['shipping_address', 'payment_method', 'coupon']);
    const names = commerce.calls.map(([name]) => name);
    assert.deepEqual(names, [
      'carts.setShippingAddress',
      'carts.setPayment',
      'carts.applyDiscount',
      'carts.get',
      'carts.getItems',
      'tax.calculate',
      'carts.setTax',
      'carts.get',
      'carts.getItems',
    ]);
    assert.equal(result.checkout.shippingAddress.line1, '1 Main St');
    assert.equal(result.checkout.paymentMethod, 'credit_card');
    assert.equal(result.checkout.couponCode, 'WELCOME10');
    assert.equal(result.checkout.taxExact, '3.60');
    assert.deepEqual(result.checkout.readiness, { ready: true, missing: [] });
  });

  it('strict path traverses the governed executor in order', async () => {
    const commerce = makeCommerce();
    const governedCalls = [];
    const executeGovernedTool = async (name, params, options) => {
      governedCalls.push({ name, params, options });
      return { success: true, kernel: true, commandType: name, result: {} };
    };
    const result = await tool.handler({
      commerce,
      params: {
        cartId: 'cart_001',
        shippingAddress: ADDRESS,
        paymentMethod: 'credit_card',
        couponCode: 'WELCOME10',
      },
      allowApply: true,
      executeGovernedTool,
      kernelStrict: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(
      governedCalls.map((call) => call.name),
      [
        'set_cart_shipping_address',
        'set_cart_payment',
        'apply_cart_discount',
        'calculate_cart_tax',
      ],
    );
    for (const call of governedCalls) {
      assert.equal(call.options.requireGoverned, true);
    }
    assert.equal(governedCalls[0].params.cartId, 'cart_001');
    assert.equal(governedCalls[0].params.line1, '1 Main St');
    assert.equal(governedCalls[1].params.paymentMethod, 'credit_card');
    assert.equal(governedCalls[2].params.couponCode, 'WELCOME10');
    // No direct engine writes on the strict path.
    assert.ok(!commerce.calls.some(([name]) => name === 'carts.setPayment'));
  });

  it('strict path stops at the first failed governed step', async () => {
    const commerce = makeCommerce();
    const governedCalls = [];
    const executeGovernedTool = async (name, params) => {
      governedCalls.push(name);
      if (name === 'apply_cart_discount') {
        return {
          success: false,
          receipt: { rejection: { message: 'commerce.validation_failed: unknown coupon' } },
        };
      }
      return { success: true, kernel: true, result: {} };
    };
    const result = await tool.handler({
      commerce,
      params: {
        cartId: 'cart_001',
        shippingAddress: ADDRESS,
        paymentMethod: 'credit_card',
        couponCode: 'NOPE',
      },
      allowApply: true,
      executeGovernedTool,
      kernelStrict: true,
    });
    assert.equal(result.success, false);
    assert.equal(result.step, 'apply_cart_discount');
    assert.match(result.error, /unknown coupon/);
    assert.deepEqual(governedCalls, [
      'set_cart_shipping_address',
      'set_cart_payment',
      'apply_cart_discount',
    ]);
  });

  it('omits steps that were not requested', async () => {
    const commerce = makeCommerce();
    const governedCalls = [];
    const executeGovernedTool = async (name) => {
      governedCalls.push(name);
      return { success: true, kernel: true, result: {} };
    };
    const result = await tool.handler({
      commerce,
      params: { cartId: 'cart_001', couponCode: 'WELCOME10' },
      allowApply: true,
      executeGovernedTool,
      kernelStrict: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(result.appliedSteps, ['coupon']);
    assert.deepEqual(governedCalls, ['apply_cart_discount', 'calculate_cart_tax']);
  });
});

describe('complete_checkout readiness', () => {
  const tool = findTool('complete_checkout');

  it('refuses an unready cart listing what is missing', async () => {
    const commerce = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.deepEqual(result.missing, ['shipping_address', 'payment_method']);
    assert.ok(!commerce.calls.some(([name]) => name === 'carts.complete'));
  });

  it('completes a ready cart', async () => {
    const commerce = makeCommerce({
      cart: makeCart({ shippingAddress: ADDRESS, paymentMethod: 'credit_card' }),
    });
    const result = await tool.handler({
      commerce,
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.result.orderNumber, 'ORD-1');
    assert.equal(result.result.totalChargedExact, '44.98');
  });
});

describe('governed composite executor behavior', () => {
  const strictExecutor = () =>
    createKernelToolExecutor({
      commerce: {},
      kernel: {
        strict: true,
        policy: { version: 'test' },
        principal: { id: 'agent:test' },
        storeId: 'store:test',
      },
      allowApply: true,
      agentConfig: null,
    });

  it('lets governed composites through instead of throwing', async () => {
    const executor = strictExecutor();
    // update_checkout has no single kernel command; the executor returns null
    // so its handler runs and each nested step traverses the kernel.
    assert.equal(
      await executor('update_checkout', { cartId: 'cart_001' }, { requireGoverned: true }),
      null,
    );
    assert.equal(
      await executor('agentic_execute_plan', { steps: [] }, { requireGoverned: true }),
      null,
    );
  });

  it('still throws for unmapped non-composite writes in strict apply mode', async () => {
    const executor = strictExecutor();
    await assert.rejects(
      executor('delete_customer', { customerId: 'c_1' }, { requireGoverned: true }),
      /outside the governed kernel catalog/,
    );
  });
});
