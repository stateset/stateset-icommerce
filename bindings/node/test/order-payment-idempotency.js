'use strict';
const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { mkdtempSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

test('order payment creation/completion survive reopen with exact immutable keyed terms', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'set-native-order-'));
  const path = join(directory, 'commerce.sqlite');
  let commerce = new Commerce(path);
  try {
    assert.equal(commerce.payments.supportsPaymentIdempotency(), true);
    const customer = await commerce.customers.create({ email: 'order-idempotency@example.invalid', firstName: 'Set', lastName: 'Test' });
    const order = await commerce.orders.create({ customerId: customer.id, items: [{ sku: 'SET', name: 'Test', quantity: 1, unitPriceExact: '100.00' }] });
    const input = { orderId: order.id, amount: '40.00', currency: 'USD', paymentMethod: 'crypto', idempotencyKey: 'set-order-retry-1' };
    const pending = await commerce.payments.createExact(input);
    assert.equal(pending.idempotencyKey, input.idempotencyKey);
    await commerce.close();
    commerce = new Commerce(path);
    const retry = await commerce.payments.createExact(input);
    assert.equal(retry.id, pending.id);
    assert.equal(await commerce.payments.count(), 1);
    await assert.rejects(commerce.payments.createExact({ ...input, amount: '41.00' }));
    await assert.rejects(commerce.payments.createExact({ ...input, currency: 'EUR' }));
    await assert.rejects(commerce.payments.createExact({ ...input, paymentMethod: 'cash_on_delivery' }));
    await commerce.payments.markCompleted(retry.id);
    await commerce.close();
    commerce = new Commerce(path);
    const completed = await commerce.payments.createExact(input);
    assert.equal(completed.id, pending.id);
    assert.equal(completed.status, 'completed');
    const remaining = await commerce.payments.createExact({ ...input, amount: '60.00', idempotencyKey: 'set-order-retry-2' });
    await commerce.payments.markCompleted(remaining.id);
    assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'paid');
    assert.equal(await commerce.payments.count(), 2);
    assert.equal((await commerce.payments.createExact(input)).id, pending.id);
    await assert.rejects(commerce.payments.createExact({ ...input, amount: '1.00', idempotencyKey: 'set-order-overpayment' }), /exceed/);
    assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'paid');
  } finally {
    await commerce.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('concurrent callers with the same key return one native payment identity', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'set-native-order-race-'));
  const path = join(directory, 'commerce.sqlite');
  const first = new Commerce(path);
  const second = new Commerce(path);
  try {
    const input = { amount: '19.99', currency: 'USD', paymentMethod: 'crypto', idempotencyKey: 'set-native-race' };
    const results = await Promise.all([first.payments.createExact(input), second.payments.createExact(input)]);
    assert.equal(results[0].id, results[1].id);
    assert.equal(await first.payments.count(), 1);
  } finally {
    await first.close(); await second.close();
    rmSync(directory, { recursive: true, force: true });
  }
});
