import test from 'node:test';
import assert from 'node:assert/strict';
import { decimalToUnits, unitsToDecimal } from '../templates/storefront/lib/money.js';
import {
  calculateCartTax,
  getTaxProvider,
  hasConfiguredTaxRate,
} from '../templates/storefront/lib/tax.js';
import { verifyUsdcTransfer } from '../templates/storefront/lib/usdc-verification.js';
import {
  claimPaymentForOrder,
  finishRecordedCheckout,
} from '../templates/storefront/lib/checkout-finalization.js';

const usdcContract = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913';
const payer = '0x1111111111111111111111111111111111111111';
const merchant = '0x2222222222222222222222222222222222222222';
const topic = (address) => `0x${address.slice(2).padStart(64, '0')}`;

test('money conversion is exact and rejects excess precision', () => {
  assert.equal(decimalToUnits('12.345678'), 12345678n);
  assert.equal(unitsToDecimal(12345678n), '12.345678');
  assert.throws(() => decimalToUnits('1.0000001'), /more than 6/);
  assert.throws(() => decimalToUnits('1e3'), /base-10/);
});

test('cart tax is calculated from authoritative exact line amounts', () => {
  const result = calculateCartTax(
    [
      { unitPrice: '10.10', quantity: 2 },
      { unitPrice: '0.30', quantity: 1 },
    ],
    'CA',
  );
  assert.deepEqual(
    { subtotal: result.subtotal, tax: result.tax, total: result.total },
    {
      subtotal: '20.5',
      tax: '1.48625',
      total: '21.98625',
    },
  );
});

test('tax configuration fails closed for unsupported jurisdictions', () => {
  assert.equal(hasConfiguredTaxRate('CA'), true);
  assert.equal(hasConfiguredTaxRate('ca'), true);
  assert.equal(hasConfiguredTaxRate('OR'), false);
  assert.equal(hasConfiguredTaxRate('not-a-state'), false);
  assert.throws(
    () => getTaxProvider({ STATESET_TAX_RATES_JSON: '{"CA":"1.1"}' }),
    /between 0 and 1/,
  );
});

test('settlement requires the exact token, payer, recipient, amount, and confirmations', () => {
  const receipt = {
    status: 'success',
    blockNumber: 100n,
    logs: [
      {
        address: usdcContract,
        topics: [
          '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef',
          topic(payer),
          topic(merchant),
        ],
        data: '0x0f4240',
        logIndex: 7,
      },
    ],
  };
  assert.deepEqual(
    verifyUsdcTransfer({
      receipt,
      currentBlock: 101n,
      tokenAddress: usdcContract,
      payerAddress: payer,
      merchantAddress: merchant,
      expectedAmountUnits: 1000000n,
    }),
    { confirmations: 2, logIndex: 7 },
  );
  assert.throws(
    () =>
      verifyUsdcTransfer({
        receipt,
        currentBlock: 101n,
        tokenAddress: usdcContract,
        payerAddress: payer,
        merchantAddress: merchant,
        expectedAmountUnits: 999999n,
      }),
    /expected USDC transfer/,
  );
});

test('a payment unique-key race recovers the winning payment', async () => {
  const winner = {
    id: 'payment-1',
    orderId: 'order-1',
    idempotencyKey: 'base:tx:1',
    amountExact: '10',
    currency: 'USDC',
  };
  const commerce = {
    payments: {
      createExact: async () => {
        throw new Error('unique constraint');
      },
      list: async () => [winner],
    },
    orders: { cancel: async () => assert.fail('winning order must not be cancelled') },
  };

  const result = await claimPaymentForOrder({
    commerce,
    order: { id: 'order-1', status: 'pending' },
    paymentInput: { idempotencyKey: 'base:tx:1', amount: '10', currency: 'USDC' },
  });
  assert.deepEqual(result, { conflict: false, payment: winner });
});

test('reuse across carts cancels the unbacked losing order', async () => {
  let cancelled;
  const commerce = {
    payments: {
      createExact: async () => ({
        id: 'payment-1',
        orderId: 'winning-order',
        idempotencyKey: 'base:tx:1',
        amountExact: '10',
        currency: 'USDC',
      }),
    },
    orders: { cancel: async (id) => (cancelled = id) },
  };

  const result = await claimPaymentForOrder({
    commerce,
    order: { id: 'losing-order', status: 'pending' },
    paymentInput: { idempotencyKey: 'base:tx:1', amount: '10', currency: 'USDC' },
  });
  assert.equal(result.conflict, true);
  assert.equal(cancelled, 'losing-order');
});

test('replay finishes a partially recorded checkout without duplicating it', async () => {
  const calls = [];
  const commerce = {
    payments: { markCompleted: async (id) => calls.push(['payment', id]) },
    orders: {
      get: async () => ({
        id: 'order-1',
        orderNumber: 'ORD-1',
        status: 'pending',
        notes: 'Verified Base settlement: 0xabc; Cart: cart-1',
      }),
      updateStatus: async (id, status) => {
        calls.push(['order', id, status]);
        return { id, orderNumber: 'ORD-1', status };
      },
    },
    carts: {
      get: async () => ({ status: 'active' }),
      cancel: async (id) => calls.push(['cart', id]),
    },
  };

  const order = await finishRecordedCheckout(
    commerce,
    { id: 'payment-1', orderId: 'order-1', status: 'pending' },
    'cart-1',
  );
  assert.equal(order.status, 'confirmed');
  assert.deepEqual(calls, [
    ['payment', 'payment-1'],
    ['order', 'order-1', 'confirmed'],
    ['cart', 'cart-1'],
  ]);
});
