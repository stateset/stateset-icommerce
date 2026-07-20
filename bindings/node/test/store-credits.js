/**
 * Store Credits API tests for @stateset/embedded Node.js bindings.
 *
 * Money is exchanged as exact decimal strings (no f64 precision loss).
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('StoreCredits: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('storeCredits API exists and is supported', async () => {
    assert.ok(commerce.storeCredits, 'storeCredits API should exist');
    assert.equal(await commerce.storeCredits.isSupported(), true);
  });

  const customer = await commerce.customers.create({
    email: 'credit@example.com',
    firstName: 'Cred',
    lastName: 'Holder',
  });

  let credit;
  await t.test('create returns an active credit with exact-string balances', async () => {
    credit = await commerce.storeCredits.create({
      customerId: customer.id,
      amount: '25.00',
      currency: 'USD',
      reason: 'compensation',
      note: 'goodwill',
    });
    assert.equal(credit.customerId, customer.id);
    assert.equal(credit.originalBalance, '25.00');
    assert.equal(credit.currentBalance, '25.00');
    assert.equal(credit.currency, 'USD');
    assert.equal(credit.status, 'active');
    assert.equal(credit.reason, 'compensation');
    assert.equal(credit.note, 'goodwill');
    assert.ok(credit.id);
  });

  await t.test('get fetches the credit by id', async () => {
    const found = await commerce.storeCredits.get(credit.id);
    assert.ok(found);
    assert.equal(found.id, credit.id);
  });

  await t.test('apply debits the balance and records a negative-amount transaction', async () => {
    const txn = await commerce.storeCredits.apply(credit.id, '10.00', 'order-42');
    assert.equal(txn.amount, '-10.00', 'apply is recorded as a debit');
    assert.equal(txn.balanceAfter, '15.00');
    assert.equal(txn.referenceId, 'order-42');

    const after = await commerce.storeCredits.get(credit.id);
    assert.equal(after.currentBalance, '15.00');
  });

  await t.test('adjust adds to the balance', async () => {
    const adjusted = await commerce.storeCredits.adjust(credit.id, {
      amount: '5.00',
      note: 'top-up',
    });
    assert.equal(adjusted.currentBalance, '20.00');
  });

  await t.test('adjust cannot drive the balance negative', async () => {
    await assert.rejects(
      commerce.storeCredits.adjust(credit.id, { amount: '-30.00' }),
      /negative balance/i,
    );
    // Balance is unchanged after the rejected adjustment.
    const after = await commerce.storeCredits.get(credit.id);
    assert.equal(after.currentBalance, '20.00');
  });

  await t.test('getTransactions returns the apply and adjust entries', async () => {
    const txns = await commerce.storeCredits.getTransactions(credit.id);
    assert.ok(txns.length >= 2, `expected >=2 transactions, got ${txns.length}`);
  });

  await t.test('list finds the customer\'s credits', async () => {
    const list = await commerce.storeCredits.list({ customerId: customer.id });
    assert.ok(list.some((c) => c.id === credit.id), 'listed credits include the created one');
  });
});
