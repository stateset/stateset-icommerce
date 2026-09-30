/**
 * Gift card tool handlers against a recording mock of the binding's
 * `GiftCards` class. The mock defines only methods the binding declares
 * (checked below), and every test asserts the exact call the handler makes.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { giftCardTools } from '../../src/tools/gift-cards.js';
import { bindingClassMethods } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(giftCardTools.map((t) => [t.name, t]));

const CARD_ID = '4e5696ba-e162-4ecd-a875-e35761020cab';

/** A GiftCardOutput, exactly as the binding shapes it. */
function giftCard(overrides = {}) {
  return {
    id: CARD_ID,
    code: '5275-7995-D8BE-4615',
    initialBalance: '100',
    currentBalance: '74.75',
    currency: 'USD',
    status: 'active',
    recipientEmail: 'a@example.com',
    senderName: 'Bo',
    createdAt: '2026-09-27T00:00:00+00:00',
    updatedAt: '2026-09-27T00:00:00+00:00',
    ...overrides,
  };
}

/** A GiftCardTransactionOutput. */
function transaction(overrides = {}) {
  return {
    id: 'tx-1',
    giftCardId: CARD_ID,
    amount: '30.25',
    balanceAfter: '69.75',
    transactionType: 'charge',
    referenceId: 'ord-1',
    createdAt: '2026-09-27T00:00:00+00:00',
    ...overrides,
  };
}

/** Recording mock: `{ commerce, calls }`, with `calls` as `[method, ...args]`. */
function makeCommerce(impls = {}) {
  const calls = [];
  const defaults = {
    create: async (input) => giftCard({ ...input }),
    get: async () => giftCard(),
    getByCode: async () => giftCard(),
    list: async () => [giftCard()],
    charge: async () => transaction(),
    refund: async () => transaction({ transactionType: 'refund' }),
    disable: async () => giftCard({ status: 'disabled' }),
  };
  const giftCards = {};
  for (const [name, fn] of Object.entries({ ...defaults, ...impls })) {
    giftCards[name] = async (...args) => {
      calls.push([name, ...args]);
      return fn(...args);
    };
  }
  return { commerce: { giftCards }, calls };
}

describe('gift card mock', () => {
  it('defines only methods the binding’s GiftCards class declares', () => {
    const real = bindingClassMethods('GiftCards');
    for (const name of Object.keys(makeCommerce().commerce.giftCards)) {
      assert.ok(real.has(name), `mock invents giftCards.${name}`);
    }
  });
});

describe('create_gift_card', () => {
  it('sends CreateGiftCardInput with the balance as an exact decimal string', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.create_gift_card.handler({
      commerce,
      params: {
        initialBalance: 25.5,
        currency: 'EUR',
        code: 'GIFT-1',
        recipientEmail: 'r@example.com',
        senderName: 'Sam',
        message: 'hi',
        expiresAt: '2027-01-01T00:00:00Z',
      },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      [
        'create',
        {
          code: 'GIFT-1',
          initialBalance: '25.5',
          currency: 'EUR',
          recipientEmail: 'r@example.com',
          senderName: 'Sam',
          message: 'hi',
          expiresAt: '2027-01-01T00:00:00Z',
        },
      ],
    ]);
    assert.equal(result.success, true);
    assert.equal(result.giftCard.initialBalance, '25.5');
  });

  it('defaults currency to USD', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.create_gift_card.handler({
      commerce,
      params: { initialBalance: 10 },
      allowApply: true,
    });
    assert.equal(calls[0][1].currency, 'USD');
  });
});

describe('get_gift_card / check_gift_card_balance', () => {
  it('a UUID is looked up by id', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.get_gift_card.handler({
      commerce,
      params: { identifier: CARD_ID },
    });
    assert.deepEqual(calls, [['get', CARD_ID]]);
    assert.deepEqual(result.giftCard, {
      id: CARD_ID,
      code: '5275-7995-D8BE-4615',
      initialBalance: '100',
      currentBalance: '74.75',
      currency: 'USD',
      status: 'active',
      recipientEmail: 'a@example.com',
      expiresAt: undefined,
      createdAt: '2026-09-27T00:00:00+00:00',
    });
  });

  it('a redemption code is looked up with getByCode', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.check_gift_card_balance.handler({
      commerce,
      params: { identifier: '5275-7995-D8BE-4615' },
    });
    assert.deepEqual(calls, [['getByCode', '5275-7995-D8BE-4615']]);
    assert.deepEqual(result, {
      success: true,
      giftCardId: CARD_ID,
      code: '5275-7995-D8BE-4615',
      currentBalance: '74.75',
      currency: 'USD',
      status: 'active',
    });
  });

  it('an unknown UUID falls back to a code lookup, then reports not found', async () => {
    const { commerce, calls } = makeCommerce({
      get: async () => null,
      getByCode: async () => null,
    });
    const result = await byName.get_gift_card.handler({
      commerce,
      params: { identifier: CARD_ID },
    });
    assert.deepEqual(
      calls.map((c) => c[0]),
      ['get', 'getByCode'],
    );
    assert.deepEqual(result, { success: false, error: 'Gift card not found' });
  });

  it('propagates binding errors', async () => {
    const { commerce } = makeCommerce({
      getByCode: async () => {
        throw new Error('db down');
      },
    });
    await assert.rejects(
      byName.check_gift_card_balance.handler({ commerce, params: { identifier: 'X' } }),
      /db down/,
    );
  });
});

describe('list_gift_cards', () => {
  it('passes a real GiftCardFilterInput with an explicit page limit and offset', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.list_gift_cards.handler({
      commerce,
      params: { status: 'disabled', code: 'ABC', limit: 50 },
    });
    assert.deepEqual(calls, [['list', { status: 'disabled', code: 'ABC', limit: 500, offset: 0 }]]);
  });

  it('pages until a short page, so totalCount is the real total', async () => {
    const cards = Array.from({ length: 1203 }, (_, i) => giftCard({ id: `gc-${i}` }));
    const { commerce, calls } = makeCommerce({
      list: async ({ limit, offset }) => cards.slice(offset, offset + limit),
    });
    const result = await byName.list_gift_cards.handler({ commerce, params: { limit: 5 } });
    assert.deepEqual(
      calls.map(([, filter]) => filter.offset),
      [0, 500, 1000],
    );
    assert.equal(result.totalCount, 1203);
    assert.equal(result.returned, 5);
    assert.deepEqual(
      result.giftCards.map((g) => g.id),
      ['gc-0', 'gc-1', 'gc-2', 'gc-3', 'gc-4'],
    );
  });

  it('an exact multiple of the page size costs one extra empty page', async () => {
    const cards = Array.from({ length: 500 }, (_, i) => giftCard({ id: `gc-${i}` }));
    const { commerce, calls } = makeCommerce({
      list: async ({ limit, offset }) => cards.slice(offset, offset + limit),
    });
    const result = await byName.list_gift_cards.handler({ commerce, params: { limit: 50 } });
    assert.equal(calls.length, 2);
    assert.equal(result.totalCount, 500);
    assert.equal(result.returned, 50);
  });
});

describe('charge_gift_card / refund_to_gift_card / disable_gift_card', () => {
  it('charge(id, amount, referenceId) with an exact amount string', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.charge_gift_card.handler({
      commerce,
      params: { giftCardId: CARD_ID, amount: 30.25, orderId: 'ord-1' },
      allowApply: true,
    });
    assert.deepEqual(calls, [['charge', CARD_ID, '30.25', 'ord-1']]);
    assert.equal(result.transaction.balanceAfter, '69.75');
  });

  it('refund(id, amount, referenceId)', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.refund_to_gift_card.handler({
      commerce,
      params: { giftCardId: CARD_ID, amount: 5 },
      allowApply: true,
    });
    assert.deepEqual(calls, [['refund', CARD_ID, '5', undefined]]);
  });

  it('disable(id)', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.disable_gift_card.handler({
      commerce,
      params: { giftCardId: CARD_ID },
      allowApply: true,
    });
    assert.deepEqual(calls, [['disable', CARD_ID]]);
    assert.equal(result.giftCard.status, 'disabled');
  });
});
