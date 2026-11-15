'use strict';
const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('invoice paymentId is honored across retries and rejects conflicting amounts', async () => {
  const commerce = new Commerce(':memory:');
  try {
    assert.equal(commerce.invoices.supportsPaymentIdempotency(), true);
    const customer = await commerce.customers.create({ email: 'idempotency@example.invalid', firstName: 'Set', lastName: 'Test' });
    const invoice = await commerce.invoices.create({ customerId: customer.id, items: [{ description: 'Settlement', quantity: '1', unitPriceExact: '100.00' }] });
    await commerce.invoices.send(invoice.id);
    const input = { amountExact: '40.00', paymentId: randomUUID(), paymentMethod: 'crypto' };
    assert.equal((await commerce.invoices.recordPayment(invoice.id, input)).amountPaidExact, '40.00');
    assert.equal((await commerce.invoices.recordPayment(invoice.id, input)).amountPaidExact, '40.00');
    await assert.rejects(commerce.invoices.recordPayment(invoice.id, { ...input, amountExact: '41.00' }), /different amount/);
    await assert.rejects(commerce.invoices.recordPayment(invoice.id, { ...input, paymentId: 'not-a-uuid' }), /UUID/);
    await assert.rejects(commerce.invoices.recordPayment(invoice.id, { ...input, paymentId: '00000000-0000-0000-0000-000000000000' }), /nil/);
    await commerce.invoices.recordPayment(invoice.id, { amountExact: '60.00', paymentId: randomUUID() });
    assert.equal((await commerce.invoices.recordPayment(invoice.id, input)).amountPaidExact, '100.00');
  } finally {
    if (typeof commerce.close === 'function') await commerce.close();
  }
});
