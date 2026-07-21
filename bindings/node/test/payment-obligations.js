/**
 * Payment obligation API tests for @stateset/embedded Node.js bindings.
 *
 * Create, get, list, record payment, set status, link bill, dashboard.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('PaymentObligations: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = randomUUID();

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.paymentObligations, 'paymentObligations API should exist');
    assert.equal(await commerce.paymentObligations.isSupported(), true);
  });

  let obligation;
  await t.test('create returns exact decimal amounts', async () => {
    obligation = await commerce.paymentObligations.create({
      supplierId,
      amount: '1250.75',
      currency: 'USD',
      dueDate: '2026-09-30',
      notes: 'Q3 supplies',
    });
    assert.ok(obligation.id);
    assert.equal(obligation.amount, '1250.75');
    assert.equal(obligation.amountPaid, '0');
    assert.equal(obligation.outstanding, '1250.75');
    assert.equal(obligation.currency, 'USD');
    assert.equal(obligation.dueDate, '2026-09-30');
    assert.equal(obligation.status, 'pending');
    assert.deepEqual(obligation.linkedBillIds, []);
  });

  await t.test('create rejects a malformed due date', async () => {
    await assert.rejects(
      () =>
        commerce.paymentObligations.create({
          supplierId,
          amount: '1',
          dueDate: '30/09/2026',
        }),
      /Invalid due_date date/,
    );
  });

  await t.test('get returns the obligation, and null when missing', async () => {
    const found = await commerce.paymentObligations.get(obligation.id);
    assert.equal(found.id, obligation.id);
    assert.equal(await commerce.paymentObligations.get(randomUUID()), null);
  });

  await t.test('recordPayment moves it to partially_paid', async () => {
    const updated = await commerce.paymentObligations.recordPayment(obligation.id, '250.75');
    assert.equal(updated.amountPaid, '250.75');
    assert.equal(updated.outstanding, '1000.00');
    assert.equal(updated.status, 'partially_paid');
  });

  await t.test('setStatus accepts a snake_case status', async () => {
    const updated = await commerce.paymentObligations.setStatus(obligation.id, 'scheduled');
    assert.equal(updated.status, 'scheduled');
    await assert.rejects(
      () => commerce.paymentObligations.setStatus(obligation.id, 'nope'),
      /Invalid payment obligation status: nope/,
    );
  });

  await t.test('linkBill records the bill id', async () => {
    const billId = randomUUID();
    const updated = await commerce.paymentObligations.linkBill(obligation.id, billId);
    assert.deepEqual(updated.linkedBillIds, [billId]);
  });

  await t.test('list filters by supplier and paginates', async () => {
    const all = await commerce.paymentObligations.list();
    assert.ok(all.length >= 1);
    assert.equal((await commerce.paymentObligations.list({ supplierId })).length, all.length);
    assert.equal((await commerce.paymentObligations.list({ limit: 1, offset: 0 })).length, 1);
  });

  await t.test('dashboard aggregates outstanding amounts', async () => {
    const dashboard = await commerce.paymentObligations.dashboard('2026-10-01');
    assert.equal(dashboard.openCount, '1');
    assert.equal(dashboard.totalOutstanding, '1000.00');
    assert.equal(dashboard.overdueCount, '1');
  });
});
