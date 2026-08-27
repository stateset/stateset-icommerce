import test from 'node:test';
import assert from 'node:assert/strict';
import {
  CheckoutError,
  executeCheckout,
  normalizeCheckoutEmail,
} from '../templates/storefront/lib/checkout-service.js';
import { getTaxProvider } from '../templates/storefront/lib/tax.js';
import { getShippingProvider } from '../templates/storefront/lib/shipping.js';

const txHash = `0x${'ab'.repeat(32)}`;
const payerAddress = '0x1111111111111111111111111111111111111111';

test('checkout email validation is bounded and rejects hostile input', () => {
  assert.equal(normalizeCheckoutEmail(' Ada@Example.com '), 'ada@example.com');
  assert.throws(() => normalizeCheckoutEmail(`${'a.'.repeat(100_000)}@example.com`), /valid email/);
  assert.throws(() => normalizeCheckoutEmail('ada@@example.com'), /valid email/);
  assert.throws(() => normalizeCheckoutEmail('ada@exam ple.com'), /valid email/);
});

function createStatefulCommerce({ failPaymentCompletionOnce = false } = {}) {
  const state = {
    carts: new Map([['cart-1', { id: 'cart-1', status: 'active' }]]),
    cartItems: new Map([
      [
        'cart-1',
        [
          {
            id: 'line-1',
            productId: 'product-1',
            sku: 'SKU-1',
            name: 'Integration Widget',
            quantity: 2,
            unitPriceExact: '10',
          },
        ],
      ],
    ]),
    customers: [],
    orders: [],
    payments: [],
    failPaymentCompletionOnce,
  };

  const commerce = {
    carts: {
      get: async (id) => state.carts.get(id),
      getItems: async (id) => state.cartItems.get(id) || [],
      cancel: async (id) => {
        const cart = state.carts.get(id);
        if (cart) cart.status = 'cancelled';
      },
    },
    customers: {
      list: async () => [...state.customers],
      create: async (input) => {
        if (
          state.customers.some(
            (customer) =>
              customer.email === input.email ||
              customer.metadata.walletAddress === input.metadata.walletAddress,
          )
        ) {
          throw new Error('customer unique constraint');
        }
        const customer = { id: `customer-${state.customers.length + 1}`, ...input };
        state.customers.push(customer);
        return customer;
      },
    },
    orders: {
      createExact: async (input) => {
        const existing = state.orders.find((order) => order.cartId === input.cartId);
        if (existing) return existing;
        const order = {
          id: `order-${state.orders.length + 1}`,
          orderNumber: `ORD-${state.orders.length + 1}`,
          status: 'pending',
          ...input,
        };
        state.orders.push(order);
        return order;
      },
      get: async (id) => state.orders.find((order) => order.id === id),
      updateStatus: async (id, status) => {
        const order = state.orders.find((candidate) => candidate.id === id);
        order.status = status;
        return order;
      },
      cancel: async (id) => {
        const order = state.orders.find((candidate) => candidate.id === id);
        if (order) order.status = 'cancelled';
      },
    },
    payments: {
      list: async () => [...state.payments],
      createExact: async (input) => {
        if (state.payments.some((payment) => payment.idempotencyKey === input.idempotencyKey)) {
          throw new Error('payment unique constraint');
        }
        const payment = {
          id: `payment-${state.payments.length + 1}`,
          status: 'pending',
          amountExact: input.amount,
          ...input,
        };
        state.payments.push(payment);
        return payment;
      },
      markCompleted: async (id) => {
        if (state.failPaymentCompletionOnce) {
          state.failPaymentCompletionOnce = false;
          throw new Error('simulated process interruption');
        }
        const payment = state.payments.find((candidate) => candidate.id === id);
        payment.status = 'completed';
        return payment;
      },
    },
  };
  return { commerce, state };
}

function checkoutInput(commerce) {
  return {
    commerce,
    cartId: 'cart-1',
    email: 'Ada@Example.com',
    txHash,
    payerAddress,
    shippingAddress: {
      firstName: 'Ada',
      lastName: 'Lovelace',
      line1: '123 Computing Way',
      line2: 'Suite 2',
      city: 'San Francisco',
      state: 'ca',
      postalCode: '94105',
      country: 'us',
    },
    verifySettlement: async ({ totals }) => {
      assert.equal(totals.subtotal, '20');
      assert.equal(totals.tax, '1.45');
      assert.equal(totals.total, '21.45');
      return { confirmations: 3, logIndex: 7 };
    },
  };
}

test('complete checkout records one customer, order, payment, and closed cart', async () => {
  const { commerce, state } = createStatefulCommerce();
  const result = await executeCheckout(checkoutInput(commerce));

  assert.deepEqual(result, {
    orderId: 'order-1',
    orderNumber: 'ORD-1',
    status: 'confirmed',
    confirmations: 3,
  });
  assert.equal(state.customers.length, 1);
  assert.equal(state.customers[0].email, 'ada@example.com');
  assert.equal(state.orders.length, 1);
  assert.equal(state.orders[0].stockPolicy, 'reject_if_insufficient');
  assert.deepEqual(state.orders[0].shippingAddress, {
    line1: '123 Computing Way',
    line2: 'Suite 2',
    city: 'San Francisco',
    state: 'CA',
    postalCode: '94105',
    country: 'US',
  });
  assert.equal(state.payments.length, 1);
  assert.equal(state.payments[0].amountExact, '21.45');
  assert.equal(state.payments[0].status, 'completed');
  assert.equal(state.carts.get('cart-1').status, 'cancelled');
});

test('concurrent delivery of one settlement converges on one checkout', async () => {
  const { commerce, state } = createStatefulCommerce();
  const [first, second] = await Promise.all([
    executeCheckout(checkoutInput(commerce)),
    executeCheckout(checkoutInput(commerce)),
  ]);

  assert.equal(first.orderId, 'order-1');
  assert.equal(second.orderId, 'order-1');
  assert.equal(state.customers.length, 1);
  assert.equal(state.orders.length, 1);
  assert.equal(state.payments.length, 1);
  assert.equal(state.orders[0].status, 'confirmed');
  assert.equal(state.payments[0].status, 'completed');
  assert.equal(state.carts.get('cart-1').status, 'cancelled');
});

test('retry recovers persisted checkout after interruption during finalization', async () => {
  const { commerce, state } = createStatefulCommerce({ failPaymentCompletionOnce: true });
  await assert.rejects(executeCheckout(checkoutInput(commerce)), /simulated process interruption/);
  assert.equal(state.orders.length, 1);
  assert.equal(state.payments.length, 1);
  assert.equal(state.orders[0].status, 'pending');
  assert.equal(state.payments[0].status, 'pending');
  assert.equal(state.carts.get('cart-1').status, 'active');

  const recovered = await executeCheckout(checkoutInput(commerce));
  assert.equal(recovered.orderId, 'order-1');
  assert.equal(recovered.status, 'confirmed');
  assert.equal(recovered.replayed, true);
  assert.equal(state.orders.length, 1);
  assert.equal(state.payments.length, 1);
  assert.equal(state.payments[0].status, 'completed');
  assert.equal(state.carts.get('cart-1').status, 'cancelled');
});

test('unsupported tax jurisdiction returns an actionable status', async () => {
  const { commerce } = createStatefulCommerce();
  const input = checkoutInput(commerce);
  await assert.rejects(
    executeCheckout({
      ...input,
      shippingAddress: { ...input.shippingAddress, state: 'OR' },
    }),
    (error) => error instanceof CheckoutError && error.status === 422,
  );
});

test('operator tax configuration replaces starter jurisdictions and supports zero rates', () => {
  const provider = getTaxProvider({ STATESET_TAX_RATES_JSON: '{"OR":"0","CA":"0.08"}' });
  assert.equal(provider.source, 'environment');
  assert.equal(provider.hasJurisdiction('OR'), true);
  assert.equal(provider.hasJurisdiction('NY'), false);
  assert.deepEqual(provider.calculateCart([{ unitPrice: '10', quantity: 1 }], 'CA'), {
    rate: '0.08',
    subtotal: '10',
    tax: '0.8',
    total: '10.8',
    lines: [{ unitPrice: '10', quantity: 1, tax: '0.8' }],
  });
  assert.throws(
    () => getTaxProvider({ STATESET_TAX_RATES_JSON: '{not-json}' }),
    /must be valid JSON/,
  );
});

test('paid shipping is included in the verified settlement and persisted order', async () => {
  const { commerce, state } = createStatefulCommerce();
  const input = checkoutInput(commerce);
  input.shippingMethodId = 'ground';
  input.shippingProvider = getShippingProvider({
    STATESET_SHIPPING_METHODS_JSON:
      '[{"id":"ground","label":"Ground","amount":"5","carrier":"UPS","countries":["US"]}]',
  });
  input.verifySettlement = async ({ totals }) => {
    assert.equal(totals.shipping, '5');
    assert.equal(totals.total, '26.45');
    return { confirmations: 3, logIndex: 7 };
  };

  await executeCheckout(input);
  assert.equal(state.payments[0].amountExact, '26.45');
  assert.equal(state.orders[0].shippingMethod, 'ground');
  assert.deepEqual(state.orders[0].items.at(-1), {
    sku: 'SHIPPING-GROUND',
    name: 'Ground',
    quantity: 1,
    unitPrice: '5',
    taxAmount: '0',
  });
});

test('shipping validation rejects malformed ZIP codes before settlement', async () => {
  const { commerce } = createStatefulCommerce();
  const input = checkoutInput(commerce);
  await assert.rejects(
    executeCheckout({
      ...input,
      shippingAddress: { ...input.shippingAddress, postalCode: 'not-a-zip' },
    }),
    (error) => error instanceof CheckoutError && error.status === 422,
  );
});
