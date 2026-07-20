/**
 * Gift Cards API tests for @stateset/embedded Node.js bindings.
 *
 * Money is exchanged as exact decimal strings (no f64 precision loss).
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('GiftCards: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('giftCards API exists', () => {
    assert.ok(commerce.giftCards, 'giftCards API should exist');
  });

  let card;
  await t.test('create returns an active card with exact-string balances', async () => {
    card = await commerce.giftCards.create({
      code: 'GIFT-TEST-001',
      initialBalance: '50.00',
      currency: 'USD',
      recipientEmail: 'ada@example.com',
    });
    assert.equal(card.code, 'GIFT-TEST-001');
    assert.equal(card.initialBalance, '50.00');
    assert.equal(card.currentBalance, '50.00');
    assert.equal(card.currency, 'USD');
    assert.equal(card.status, 'active');
    assert.equal(card.recipientEmail, 'ada@example.com');
    assert.ok(card.id);
  });

  await t.test('getByCode finds the card', async () => {
    const found = await commerce.giftCards.getByCode('GIFT-TEST-001');
    assert.ok(found);
    assert.equal(found.id, card.id);
  });

  await t.test('charge debits the balance and records a transaction', async () => {
    const txn = await commerce.giftCards.charge(card.id, '19.99', 'order-123');
    assert.equal(txn.amount, '19.99');
    assert.equal(txn.balanceAfter, '30.01');
    assert.equal(txn.referenceId, 'order-123');

    const after = await commerce.giftCards.get(card.id);
    assert.equal(after.currentBalance, '30.01');
  });

  await t.test('refund credits the balance back', async () => {
    const txn = await commerce.giftCards.refund(card.id, '5.00', 'refund-1');
    assert.equal(txn.balanceAfter, '35.01');
    const after = await commerce.giftCards.get(card.id);
    assert.equal(after.currentBalance, '35.01');
  });

  await t.test('getTransactions returns the charge and refund', async () => {
    const txns = await commerce.giftCards.getTransactions(card.id);
    assert.ok(txns.length >= 2, `expected >=2 transactions, got ${txns.length}`);
  });

  await t.test('list returns the created card', async () => {
    const cards = await commerce.giftCards.list();
    assert.ok(cards.some((c) => c.id === card.id));
  });

  await t.test('disable marks the card disabled', async () => {
    const disabled = await commerce.giftCards.disable(card.id);
    assert.equal(disabled.status, 'disabled');
  });
});
