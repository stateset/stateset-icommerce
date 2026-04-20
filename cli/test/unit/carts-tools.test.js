/**
 * Cart Tools — Comprehensive Test Suite
 *
 * Tests every tool exported from src/tools/carts.js:
 *   full cart lifecycle including cart state, items, addresses, discounts,
 *   shipping, payment, inventory reservation, and checkout transitions.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { cartTools } from '../../src/tools/carts.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = cartTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in cartTools`);
  return tool;
}

function makeCart(overrides = {}) {
  return {
    id: 'cart_001',
    cartNumber: 'CART-100001',
    customerId: 'cust_001',
    customerEmail: 'alice@example.com',
    customerName: 'Alice Smith',
    status: 'active',
    paymentStatus: 'pending',
    currency: 'USD',
    subtotal: 59.98,
    taxAmount: 4.8,
    shippingAmount: 5.0,
    discountAmount: 0,
    grandTotal: 69.78,
    paymentMethod: null,
    shippingMethod: null,
    couponCode: null,
    items: [
      { id: 'item_001', sku: 'WIDGET-001', name: 'Widget', quantity: 2, unitPrice: 29.99 },
    ],
    itemCount: 2,
    shippingAddress: null,
    billingAddress: null,
    createdAt: '2026-02-20T00:00:00Z',
    updatedAt: '2026-02-20T00:00:00Z',
    expiresAt: '2026-02-21T00:00:00Z',
    ...overrides,
  };
}

function makeCommerce(overrides = {}) {
  return {
    carts: {
      list: async () => [makeCart()],
      count: async () => 1,
      get: async (id) => (id === 'nonexistent' ? null : makeCart({ id })),
      getByNumber: async (num) => makeCart({ cartNumber: num }),
      create: async (data) => makeCart({ ...data, id: 'cart_new' }),
      update: async (id, data) => makeCart({ id, ...data }),
      forCustomer: async (customerId) => [makeCart({ customerId })],
      delete: async () => {},
      addItem: async (_cartId, item) => ({
        id: 'item_new',
        sku: item.sku,
        name: item.name,
        quantity: item.quantity,
        unitPrice: item.unitPrice,
        total: item.quantity * item.unitPrice,
      }),
      updateItem: async (itemId, data) => ({
        id: itemId,
        sku: 'WIDGET-001',
        quantity: data.quantity,
        total: data.quantity * 29.99,
      }),
      removeItem: async () => {},
      getItems: async () => makeCart().items,
      clearItems: async () => {},
      setShippingAddress: async (cartId, address) => makeCart({ id: cartId, shippingAddress: address }),
      setShipping: async (cartId, input) =>
        makeCart({
          id: cartId,
          shippingAddress: input.shippingAddress,
          shippingMethod: input.shippingMethod,
          shippingCarrier: input.shippingCarrier,
          shippingAmount: input.shippingAmount ?? 0,
        }),
      setBillingAddress: async (cartId, address) => makeCart({ id: cartId, billingAddress: address }),
      setPayment: async (cartId, payment) => makeCart({ id: cartId, ...payment }),
      applyDiscount: async (cartId, code) => makeCart({ id: cartId, couponCode: code, discountAmount: 10, grandTotal: 59.78 }),
      removeDiscount: async (cartId) => makeCart({ id: cartId, couponCode: null, discountAmount: 0, grandTotal: 69.78 }),
      getShippingRates: async () => [
        { id: 'rate_001', carrier: 'USPS', service: 'Priority', price: 7.99, currency: 'USD', estimatedDays: 3 },
      ],
      markReadyForPayment: async (cartId) => makeCart({ id: cartId, status: 'ready_for_payment' }),
      beginCheckout: async (cartId) => makeCart({ id: cartId, status: 'checkout_started' }),
      complete: async (cartId) => ({
        orderId: 'ord_001',
        orderNumber: 'ORD-100001',
        cartId,
        totalCharged: 69.78,
        currency: 'USD',
        paymentId: 'pay_001',
      }),
      cancel: async (cartId) => makeCart({ id: cartId, status: 'cancelled' }),
      abandon: async (cartId) => makeCart({ id: cartId, status: 'abandoned' }),
      expire: async (cartId) => makeCart({ id: cartId, status: 'expired' }),
      reserveInventory: async (cartId) => makeCart({ id: cartId, inventoryReserved: true }),
      releaseInventory: async (cartId) => makeCart({ id: cartId, inventoryReserved: false }),
      recalculate: async (cartId) => makeCart({ id: cartId }),
      setTax: async (cartId, taxAmount) => makeCart({ id: cartId, taxAmount }),
      getAbandoned: async () => [makeCart({ status: 'abandoned' })],
      getExpired: async () => [makeCart({ status: 'expired' })],
      ...overrides,
    },
  };
}

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Cart Tools — structure', () => {
  it('exports an array of 30 tools', () => {
    assert.ok(Array.isArray(cartTools));
    assert.strictEqual(cartTools.length, 30);
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of cartTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = cartTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });
});

// ---------------------------------------------------------------------------
// list_carts
// ---------------------------------------------------------------------------

describe('list_carts', () => {
  const tool = findTool('list_carts');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns success with carts array and totalCount', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { limit: 50 } });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.carts));
    assert.strictEqual(result.totalCount, 1);
    assert.strictEqual(result.returned, 1);
  });

  it('respects limit parameter', async () => {
    const commerce = makeCommerce({
      list: async () => Array.from({ length: 20 }, (_, i) => makeCart({ id: `cart_${i}` })),
      count: async () => 20,
    });
    const result = await tool.handler({ commerce, params: { limit: 5 } });
    assert.strictEqual(result.returned, 5);
    assert.strictEqual(result.totalCount, 20);
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({ list: async () => { throw new Error('DB down'); } });
    await assert.rejects(() => tool.handler({ commerce, params: { limit: 50 } }), /DB down/);
  });
});

// ---------------------------------------------------------------------------
// get_cart
// ---------------------------------------------------------------------------

describe('get_cart', () => {
  const tool = findTool('get_cart');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns cart by UUID identifier', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { identifier: 'cart_001' } });
    assert.strictEqual(result.success, true);
    assert.ok(result.cart);
    assert.strictEqual(result.cart.id, 'cart_001');
  });

  it('looks up by cart number when identifier starts with CART-', async () => {
    let calledGetByNumber = false;
    const commerce = makeCommerce({
      getByNumber: async (num) => { calledGetByNumber = true; return makeCart({ cartNumber: num }); },
    });
    const result = await tool.handler({ commerce, params: { identifier: 'CART-100001' } });
    assert.strictEqual(result.success, true);
    assert.ok(calledGetByNumber);
  });

  it('returns error when cart not found', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { identifier: 'nonexistent' } });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

// ---------------------------------------------------------------------------
// create_cart
// ---------------------------------------------------------------------------

describe('create_cart', () => {
  const tool = findTool('create_cart');
  const params = { customerEmail: 'alice@example.com', currency: 'USD' };

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

  it('creates cart when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.cart);
    assert.ok(result.cart.id);
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({ create: async () => { throw new Error('Create failed'); } });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Create failed/);
  });
});

// ---------------------------------------------------------------------------
// add_cart_item
// ---------------------------------------------------------------------------

describe('add_cart_item', () => {
  const tool = findTool('add_cart_item');
  const params = { cartId: 'cart_001', sku: 'WIDGET-001', name: 'Widget', quantity: 2, unitPrice: 29.99 };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.hint);
    assert.ok(result.wouldAdd);
    assert.strictEqual(result.wouldAdd.lineTotal, 2 * 29.99);
  });

  it('adds item when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.item);
    assert.strictEqual(result.item.sku, 'WIDGET-001');
    assert.strictEqual(result.item.quantity, 2);
  });
});

// ---------------------------------------------------------------------------
// update_cart_item
// ---------------------------------------------------------------------------

describe('update_cart_item', () => {
  const tool = findTool('update_cart_item');
  const params = { itemId: 'item_001', quantity: 5 };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldUpdate);
    assert.strictEqual(result.wouldUpdate.newQuantity, 5);
  });

  it('updates item when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.item);
    assert.strictEqual(result.item.quantity, 5);
  });
});

// ---------------------------------------------------------------------------
// remove_cart_item
// ---------------------------------------------------------------------------

describe('remove_cart_item', () => {
  const tool = findTool('remove_cart_item');
  const params = { itemId: 'item_001' };

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldRemove);
  });

  it('removes item when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('removed'));
  });
});

// ---------------------------------------------------------------------------
// set_cart_shipping_address
// ---------------------------------------------------------------------------

describe('set_cart_shipping_address', () => {
  const tool = findTool('set_cart_shipping_address');
  const params = {
    cartId: 'cart_001',
    firstName: 'Alice',
    lastName: 'Smith',
    line1: '123 Main St',
    city: 'Springfield',
    state: 'IL',
    postalCode: '62701',
    country: 'US',
  };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldSet);
    assert.ok(result.wouldSet.address.includes('Alice'));
  });

  it('sets address when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.cart);
    assert.ok(result.cart.shippingAddress);
  });
});

// ---------------------------------------------------------------------------
// set_cart_payment
// ---------------------------------------------------------------------------

describe('set_cart_payment', () => {
  const tool = findTool('set_cart_payment');
  const params = { cartId: 'cart_001', paymentMethod: 'credit_card' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldSet);
    assert.strictEqual(result.wouldSet.paymentMethod, 'credit_card');
  });

  it('sets payment when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.cart);
  });
});

// ---------------------------------------------------------------------------
// apply_cart_discount
// ---------------------------------------------------------------------------

describe('apply_cart_discount', () => {
  const tool = findTool('apply_cart_discount');
  const params = { cartId: 'cart_001', couponCode: 'SAVE10' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldApply);
    assert.strictEqual(result.wouldApply.couponCode, 'SAVE10');
  });

  it('applies discount when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('SAVE10'));
    assert.strictEqual(result.cart.couponCode, 'SAVE10');
  });
});

// ---------------------------------------------------------------------------
// get_shipping_rates
// ---------------------------------------------------------------------------

describe('get_shipping_rates', () => {
  const tool = findTool('get_shipping_rates');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns rates array', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { cartId: 'cart_001' } });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.rates));
    assert.strictEqual(result.rates.length, 1);
    assert.strictEqual(result.rates[0].carrier, 'USPS');
  });
});

// ---------------------------------------------------------------------------
// complete_checkout
// ---------------------------------------------------------------------------

describe('complete_checkout', () => {
  const tool = findTool('complete_checkout');
  const params = { cartId: 'cart_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview with cart details when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldCheckout);
    assert.strictEqual(result.wouldCheckout.cartId, 'cart_001');
  });

  it('returns error when cart not found in preview mode', async () => {
    const result = await tool.handler({
      commerce: makeCommerce({ get: async () => null }),
      params,
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('completes checkout when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.result);
    assert.strictEqual(result.result.orderId, 'ord_001');
    assert.strictEqual(result.result.orderNumber, 'ORD-100001');
  });
});

// ---------------------------------------------------------------------------
// cancel_cart
// ---------------------------------------------------------------------------

describe('cancel_cart', () => {
  const tool = findTool('cancel_cart');
  const params = { cartId: 'cart_001' };

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldCancel);
  });

  it('cancels cart when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.status, 'cancelled');
  });
});

// ---------------------------------------------------------------------------
// abandon_cart
// ---------------------------------------------------------------------------

describe('abandon_cart', () => {
  const tool = findTool('abandon_cart');
  const params = { cartId: 'cart_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldAbandon);
  });

  it('marks cart as abandoned when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.status, 'abandoned');
  });
});

// ---------------------------------------------------------------------------
// get_abandoned_carts
// ---------------------------------------------------------------------------

describe('get_abandoned_carts', () => {
  const tool = findTool('get_abandoned_carts');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns abandoned carts array', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.carts));
    assert.strictEqual(result.count, 1);
  });

  it('returns empty list when no abandoned carts', async () => {
    const commerce = makeCommerce({ getAbandoned: async () => [] });
    const result = await tool.handler({ commerce, params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 0);
  });
});

describe('update_cart', () => {
  const tool = findTool('update_cart');
  const params = { cartId: 'cart_001', customerName: 'Alice Updated' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('updates cart when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.customerName, 'Alice Updated');
  });
});

describe('list_customer_carts', () => {
  const tool = findTool('list_customer_carts');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns carts for a customer', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { customerId: 'cust_001' },
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
  });
});

describe('delete_cart', () => {
  const tool = findTool('delete_cart');

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });
});

describe('list_cart_items', () => {
  const tool = findTool('list_cart_items');

  it('returns cart items', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
  });
});

describe('clear_cart_items', () => {
  const tool = findTool('clear_cart_items');

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });
});

describe('set_cart_shipping', () => {
  const tool = findTool('set_cart_shipping');
  const params = {
    cartId: 'cart_001',
    firstName: 'Alice',
    lastName: 'Smith',
    line1: '123 Main St',
    city: 'Springfield',
    postalCode: '62701',
    country: 'US',
    shippingMethod: 'ground',
    shippingCarrier: 'UPS',
    shippingAmount: 9.99,
  };

  it('sets shipping when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.shippingMethod, 'ground');
  });
});

describe('set_cart_billing_address', () => {
  const tool = findTool('set_cart_billing_address');
  const params = {
    cartId: 'cart_001',
    firstName: 'Alice',
    lastName: 'Smith',
    line1: '123 Main St',
    city: 'Springfield',
    postalCode: '62701',
    country: 'US',
  };

  it('sets billing address when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.cart.billingAddress);
  });
});

describe('remove_cart_discount', () => {
  const tool = findTool('remove_cart_discount');

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('removes discount when allowApply is true', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.discountAmount, 0);
  });
});

describe('mark_cart_ready_for_payment', () => {
  const tool = findTool('mark_cart_ready_for_payment');

  it('marks cart ready for payment', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.status, 'ready_for_payment');
  });
});

describe('begin_cart_checkout', () => {
  const tool = findTool('begin_cart_checkout');

  it('begins checkout', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.status, 'checkout_started');
  });
});

describe('expire_cart', () => {
  const tool = findTool('expire_cart');

  it('expires cart', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.status, 'expired');
  });
});

describe('reserve_cart_inventory', () => {
  const tool = findTool('reserve_cart_inventory');

  it('reserves inventory', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.inventoryReserved, true);
  });
});

describe('release_cart_inventory', () => {
  const tool = findTool('release_cart_inventory');

  it('releases inventory', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.inventoryReserved, false);
  });
});

describe('recalculate_cart', () => {
  const tool = findTool('recalculate_cart');

  it('recalculates cart totals', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.ok(typeof result.cart.grandTotal === 'number');
  });
});

describe('set_cart_tax', () => {
  const tool = findTool('set_cart_tax');

  it('sets tax amount', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cartId: 'cart_001', taxAmount: 12.34 },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.cart.taxAmount, 12.34);
  });
});

describe('get_expired_carts', () => {
  const tool = findTool('get_expired_carts');

  it('returns expired carts array', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
  });
});
