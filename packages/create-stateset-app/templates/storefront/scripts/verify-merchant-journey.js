import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Commerce } from '@stateset/embedded';
import { executeCheckout } from '../lib/checkout-service.js';
import { getTaxProvider } from '../lib/tax.js';
import { getShippingProvider } from '../lib/shipping.js';
import { decimalToUnits } from '../lib/money.js';

// Always use a private disposable store, never STATESET_DB_PATH. Settlement
// verification is simulated; this check sends no funds and calls no providers.
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-merchant-check-'));
const database = path.join(directory, 'store.db');
let commerce;
try {
  commerce = new Commerce(database);
  const sku = 'MERCHANT-CHECK';
  const product = await commerce.products.create({
    name: 'Merchant check',
    slug: 'merchant-check',
    variants: [{ sku, priceExact: '10.00', isDefault: true }],
  });
  await commerce.products.activate(product.id);
  await commerce.inventory.createItem({ sku, name: 'Merchant check', initialQuantity: 3 });
  const cart = await commerce.carts.create({ currency: 'USD' });
  await commerce.carts.addItemExact(cart.id, {
    productId: product.id,
    sku,
    name: 'Merchant check',
    quantity: 2,
    unitPrice: '10.00',
  });
  const request = {
    cartId: cart.id,
    email: 'merchant-check@example.test',
    txHash: '0x' + 'ab'.repeat(32),
    payerAddress: '0x1111111111111111111111111111111111111111',
    shippingAddress: {
      firstName: 'Merchant',
      lastName: 'Check',
      line1: '1 Main St',
      city: 'San Francisco',
      state: 'CA',
      postalCode: '94105',
      country: 'US',
    },
    shippingMethodId: 'ground',
    taxProvider: getTaxProvider({ STATESET_TAX_RATES_JSON: '{"CA":"0.10"}' }),
    shippingProvider: getShippingProvider({
      STATESET_SHIPPING_METHODS_JSON:
        '[{"id":"ground","label":"Ground","amount":"5","countries":["US"]}]',
    }),
    verifySettlement: async ({ totals }) => {
      assert.equal(totals.subtotal, '20');
      assert.equal(totals.tax, '2');
      assert.equal(totals.shipping, '5');
      assert.equal(totals.total, '27');
      return { confirmations: 3, logIndex: 1 };
    },
  };
  const checkedOut = await executeCheckout({ ...request, commerce });
  assert.equal(checkedOut.status, 'confirmed');
  const replay = await executeCheckout({ ...request, commerce });
  assert.equal(replay.orderId, checkedOut.orderId);
  assert.equal(replay.replayed, true);
  assert.equal((await commerce.customers.list()).length, 1);
  assert.equal((await commerce.orders.list()).length, 1);
  assert.equal((await commerce.payments.list()).length, 1);
  const stock = await commerce.inventory.getStock(sku);
  assert.equal(stock.totalAvailable, '1');
  const [payment] = await commerce.payments.list();
  assert.equal(decimalToUnits(payment.amountExact), 27000000n);
  await commerce.orders.updateStatus(checkedOut.orderId, 'processing');
  const refundRequest = {
    paymentId: payment.id,
    amount: '1.25',
    idempotencyKey: 'merchant-check-refund',
    reason: 'Local verification',
  };
  const refund = await commerce.payments.createRefundExact(refundRequest);
  await commerce.payments.completeRefund(refund.id);
  await commerce.payments.completeRefund(refund.id);
  assert.equal((await commerce.payments.createRefundExact(refundRequest)).id, refund.id);
  assert.equal((await commerce.payments.getRefunds(payment.id)).length, 1);
  await commerce.close();
  commerce = new Commerce(database);
  const reopenedReplay = await executeCheckout({ ...request, commerce });
  assert.equal(reopenedReplay.orderId, checkedOut.orderId);
  assert.equal(reopenedReplay.status, 'processing');
  assert.equal(reopenedReplay.replayed, true);
  const reopenedPayment = await commerce.payments.get(payment.id);
  assert.equal(reopenedPayment.status, 'partially_refunded');
  assert.equal(decimalToUnits(reopenedPayment.amountRefundedExact), 1250000n);
  assert.equal((await commerce.orders.list()).length, 1);
  assert.equal((await commerce.payments.list()).length, 1);
  console.log(
    JSON.stringify({
      schemaVersion: 1,
      result: 'passed',
      settlement: 'simulated-no-funds-transferred',
      assertions: [
        'native checkout with exact tax and paid shipping',
        'one customer/order/payment on replay',
        'inventory allocation conserved',
        'refund retry changes balance once',
        'reopen and late checkout replay preserve refund and order state',
      ],
      externalProvidersVerified: false,
    }),
  );
} catch (error) {
  console.error('Local merchant journey failed:', error.message);
  process.exitCode = 1;
} finally {
  try {
    if (commerce && !commerce.isClosed) await commerce.close();
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}
