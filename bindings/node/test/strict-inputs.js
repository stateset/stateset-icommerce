/**
 * Input strictness at the binding boundary.
 *
 * A malformed input must be refused with `code: 'VALIDATION'`, never silently
 * coerced: a currency the engine cannot parse used to fall back to the store
 * default, and a product id that was not a UUID used to become the nil UUID.
 * Both wrote a record the caller did not ask for.
 *
 * Float money is optional: a caller who sends the exact string need not also
 * send a float, and a caller who sends neither is told which field is missing.
 */

'use strict';

const assert = require('assert');
const { test } = require('node:test');
const { Commerce } = require('../index.js');

async function seed(commerce) {
  return commerce.customers.create({ email: 'strict@example.com', firstName: 'S', lastName: 'T' });
}

const item = (extra) => ({ sku: 'SKU-1', name: 'Widget', quantity: 1, ...extra });

test('orders.create refuses an unparseable currency instead of defaulting it', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  await assert.rejects(
    commerce.orders.create({ customerId: customer.id, items: [item({ unitPriceExact: '1.00' })], currency: 'EURO' }),
    (err) => err.code === 'VALIDATION' && /currency/i.test(err.message),
  );
});

test('orders.create refuses a non-UUID productId instead of dropping it', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  await assert.rejects(
    commerce.orders.create({
      customerId: customer.id,
      items: [item({ unitPriceExact: '1.00', productId: 'not-a-uuid' })],
    }),
    (err) => err.code === 'VALIDATION' && /product/i.test(err.message),
  );
});

test('orders.create refuses a non-UUID variantId', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  await assert.rejects(
    commerce.orders.create({
      customerId: customer.id,
      items: [item({ unitPriceExact: '1.00', variantId: 'nope' })],
    }),
    (err) => err.code === 'VALIDATION' && /variant/i.test(err.message),
  );
});

test('orders.create accepts an exact unit price with no float twin', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [item({ unitPriceExact: '19.99' })],
  });
  assert.strictEqual(order.totalAmountExact, '19.99');
});

test('orders.create names the missing price when neither form is sent', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  await assert.rejects(
    commerce.orders.create({ customerId: customer.id, items: [item({})] }),
    (err) => err.code === 'VALIDATION' && /unit price/i.test(err.message) && /Exact/.test(err.message),
  );
});

test('payments.create and createRefund accept exact amounts with no float twin', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [item({ unitPriceExact: '10.00' })],
  });
  const payment = await commerce.payments.create({ orderId: order.id, amountExact: '10.00' });
  assert.strictEqual(payment.amountExact, '10.00');
  await commerce.payments.markCompleted(payment.id);
  const refund = await commerce.payments.createRefund({ paymentId: payment.id, amountExact: '2.50' });
  assert.strictEqual(refund.amountExact, '2.50');
});

test('carts.addItem and products.create accept exact prices with no float twin', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  const product = await commerce.products.create({
    name: 'P',
    variants: [{ sku: 'V-1', priceExact: '4.25' }],
  });
  const variants = await commerce.products.getVariants(product.id);
  assert.strictEqual(variants[0].priceExact, '4.25');
  const cart = await commerce.carts.create({ customerId: customer.id });
  const line = await commerce.carts.addItem(cart.id, item({ sku: 'LOOSE-1', unitPriceExact: '4.25' }));
  assert.strictEqual(line.unitPriceExact, '4.25');
});

test('payments.create refuses an unparseable currency', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [item({ unitPriceExact: '10.00' })],
  });
  await assert.rejects(
    commerce.payments.create({ orderId: order.id, amountExact: '10.00', currency: 'dollars' }),
    (err) => err.code === 'VALIDATION' && /currency/i.test(err.message),
  );
});

test('promotions.apply refuses a non-UUID customerId', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.promotions.apply({
      customerId: 'garbage',
      lineItems: [{ id: 'l1', sku: 'SKU-1', quantity: 1, unitPrice: 1, lineTotal: 1 }],
      subtotal: 1,
    }),
    (err) => err.code === 'VALIDATION' && /customer/i.test(err.message),
  );
});

test('promotions.create refuses a malformed startsAt instead of dropping it', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.promotions.create({ name: 'Spring', percentageOff: 10, startsAt: 'yesterday' }),
    (err) => err.code === 'VALIDATION' && /starts/i.test(err.message),
  );
});

test('promotions.create refuses one bad id in a product list instead of skipping it', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.promotions.create({
      name: 'Bundle',
      percentageOff: 10,
      applicableProductIds: ['00000000-0000-0000-0000-000000000001', 'oops'],
    }),
    (err) => err.code === 'VALIDATION' && /oops/.test(err.message),
  );
});

test('subscriptions.list refuses a non-UUID customerId filter instead of listing everything', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.subscriptions.list({ customerId: 'garbage' }),
    (err) => err.code === 'VALIDATION' && /customer/i.test(err.message),
  );
});

test('tax.calculate refuses an unparseable currency instead of assuming USD', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.tax.calculate({
      lineItems: [{ id: 'l1', quantity: 1, unitPrice: 10 }],
      shippingAddress: { country: 'US', state: 'CA', postalCode: '94105' },
      currency: 'dollars',
    }),
    (err) => err.code === 'VALIDATION' && /currency/i.test(err.message),
  );
});

test('generalLedger.getAccountBalance refuses a malformed asOfDate', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.generalLedger.getAccountBalance('00000000-0000-0000-0000-000000000000', 'last tuesday'),
    (err) => err.code === 'VALIDATION' && /as of date/i.test(err.message),
  );
});

test('payments.create refuses an unknown payment method', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await seed(commerce);
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [item({ unitPriceExact: '10.00' })],
  });
  await assert.rejects(
    commerce.payments.create({ orderId: order.id, amountExact: '10.00', paymentMethod: 'iou' }),
    (err) => err.code === 'VALIDATION' && /payment method/i.test(err.message),
  );
});
