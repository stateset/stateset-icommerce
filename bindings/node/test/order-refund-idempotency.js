'use strict';
const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { mkdtempSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

test('refund creation and completion survive reopen without double refunding', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'set-native-refund-'));
  const path = join(directory, 'commerce.sqlite');
  let commerce = new Commerce(path);
  try {
    assert.equal(commerce.payments.supportsRefundIdempotency(), true);
    const customer = await commerce.customers.create({ email: 'refund@example.invalid', firstName: 'Set', lastName: 'Test' });
    const order = await commerce.orders.create({ customerId: customer.id, items: [{ sku: 'SET', name: 'Test', quantity: 1, unitPriceExact: '100.00' }] });
    const payment = await commerce.payments.createExact({ orderId: order.id, amount: '100.00', currency: 'USD', paymentMethod: 'crypto', idempotencyKey: 'refund-original' });
    await commerce.payments.markCompleted(payment.id);
    const input = { paymentId: payment.id, amount: '25.00', reason: 'verified reverse transfer', idempotencyKey: 'set-refund-retry-1' };
    const pending = await commerce.payments.createRefundExact(input);
    assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '0');
    await commerce.close(); commerce = new Commerce(path);
    assert.equal((await commerce.payments.createRefundExact(input)).id, pending.id);
    for (const change of [{ amount: '26.00' }, { reason: 'changed' }]) {
      await assert.rejects(commerce.payments.createRefundExact({ ...input, ...change }));
    }
    await commerce.payments.completeRefund(pending.id);
    await commerce.close(); commerce = new Commerce(path);
    const replay = await commerce.payments.createRefundExact(input);
    assert.equal(replay.id, pending.id); assert.equal(replay.status, 'completed');
    await commerce.payments.completeRefund(pending.id);
    assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '25.00');
    assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'partially_refunded');
    const rest = await commerce.payments.createRefundExact({ ...input, amount: '75.00', idempotencyKey: 'set-refund-retry-2' });
    await commerce.payments.completeRefund(rest.id);
    assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'refunded');
    assert.equal((await commerce.payments.createRefundExact(input)).id, pending.id);
    assert.equal((await commerce.payments.getRefunds(payment.id)).length, 2);
    await assert.rejects(commerce.payments.createRefundExact({ ...input, amount: '0.01', idempotencyKey: 'over-refund' }));
    assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '100.00');
  } finally {
    await commerce.close(); rmSync(directory, { recursive: true, force: true });
  }
});

test('concurrent keyed refunds share one identity; failed refunds cannot complete', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'set-native-refund-race-'));
  const path = join(directory, 'commerce.sqlite');
  const first = new Commerce(path); const second = new Commerce(path);
  try {
    const payment = await first.payments.createExact({ amount: '40.00', currency: 'USD', paymentMethod: 'crypto', idempotencyKey: 'original' });
    await first.payments.markCompleted(payment.id);
    const input = { paymentId: payment.id, amount: '10.00', reason: 'verified refund', idempotencyKey: 'refund-race' };
    const results = await Promise.all([first.payments.createRefundExact(input), second.payments.createRefundExact(input)]);
    assert.equal(results[0].id, results[1].id);
    assert.equal((await first.payments.getRefunds(payment.id)).length, 1);
    await first.payments.failRefund(results[0].id, 'needs reconciliation');
    await assert.rejects(first.payments.completeRefund(results[0].id));
    assert.equal((await first.payments.get(payment.id)).amountRefundedExact, '0');
  } finally {
    await first.close(); await second.close(); rmSync(directory, { recursive: true, force: true });
  }
});
