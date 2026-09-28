/**
 * Store credit tool handlers against a recording mock of the binding's
 * `StoreCredits` class. The mock defines only methods the binding declares
 * (checked below), and every test asserts the exact call the handler makes.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { storeCreditTools } from '../../src/tools/store-credits.js';
import { createCallableApiAccessor } from '../../src/mcp/commerce-adapter.js';
import { bindingClassMethods } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(storeCreditTools.map((t) => [t.name, t]));

/** A StoreCreditOutput, exactly as the binding shapes it. */
function credit(overrides = {}) {
  return {
    id: 'sc-1',
    customerId: 'cust-1',
    originalBalance: '40',
    currentBalance: '37.5',
    currency: 'USD',
    status: 'active',
    reason: 'compensation',
    note: 'late',
    createdAt: '2026-09-27T00:00:00+00:00',
    updatedAt: '2026-09-27T00:00:00+00:00',
    ...overrides,
  };
}

/** Recording mock: `{ storeCredits, calls }`, with `calls` as `[method, ...args]`. */
function makeStoreCredits(impls = {}) {
  const calls = [];
  const defaults = {
    create: async (input) => credit({ originalBalance: input.amount }),
    get: async () => credit(),
    list: async () => [credit()],
    adjust: async () => credit(),
    apply: async (id, amount, referenceId) => ({
      id: 'tx-1',
      storeCreditId: id,
      amount: `-${amount}`,
      balanceAfter: '30.0',
      transactionType: 'apply',
      referenceId,
      createdAt: '2026-09-27T00:00:00+00:00',
    }),
  };
  const storeCredits = {};
  for (const [name, fn] of Object.entries({ ...defaults, ...impls })) {
    storeCredits[name] = async (...args) => {
      calls.push([name, ...args]);
      return fn(...args);
    };
  }
  return { storeCredits, calls };
}

describe('store credit mock', () => {
  it('defines only methods the binding’s StoreCredits class declares', () => {
    const real = bindingClassMethods('StoreCredits');
    for (const name of Object.keys(makeStoreCredits().storeCredits)) {
      assert.ok(real.has(name), `mock invents storeCredits.${name}`);
    }
  });
});

describe('create_store_credit', () => {
  it('sends CreateStoreCreditInput with an exact amount string', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    const result = await byName.create_store_credit.handler({
      commerce: { storeCredits },
      params: {
        customerId: 'cust-1',
        amount: 40,
        currency: 'USD',
        reason: 'compensation',
        referenceId: 'ret-1',
        note: 'late',
      },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      [
        'create',
        {
          customerId: 'cust-1',
          amount: '40',
          currency: 'USD',
          reason: 'compensation',
          referenceId: 'ret-1',
          note: 'late',
          expiresAt: undefined,
        },
      ],
    ]);
    assert.equal(result.credit.originalBalance, '40');
  });

  it('leaves reason unset so the engine default (return) applies', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    await byName.create_store_credit.handler({
      commerce: { storeCredits },
      params: { customerId: 'cust-1', amount: 10 },
      allowApply: true,
    });
    assert.equal(calls[0][1].reason, undefined);
    assert.equal(calls[0][1].currency, 'USD');
  });
});

describe('get_store_credit', () => {
  it('maps the real StoreCreditOutput fields', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    const result = await byName.get_store_credit.handler({
      commerce: { storeCredits },
      params: { creditId: 'sc-1' },
    });
    assert.deepEqual(calls, [['get', 'sc-1']]);
    assert.equal(result.credit.originalBalance, '40');
    assert.equal(result.credit.currentBalance, '37.5');
    assert.equal(result.credit.reason, 'compensation');
    assert.equal('originalAmount' in result.credit, false);
  });

  it('reports not found', async () => {
    const { storeCredits } = makeStoreCredits({ get: async () => null });
    const result = await byName.get_store_credit.handler({
      commerce: { storeCredits },
      params: { creditId: 'nope' },
    });
    assert.deepEqual(result, { success: false, error: 'Store credit not found' });
  });
});

describe('list_store_credits', () => {
  it('passes a real StoreCreditFilterInput with an explicit page limit', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    await byName.list_store_credits.handler({
      commerce: { storeCredits },
      params: { customerId: 'cust-1', status: 'active', reason: 'return', limit: 50 },
    });
    assert.deepEqual(calls, [
      ['list', { customerId: 'cust-1', status: 'active', reason: 'return', limit: 500, offset: 0 }],
    ]);
  });

  it('pages until a short page, so totalCount is the real total', async () => {
    const credits = Array.from({ length: 734 }, (_, i) => credit({ id: `sc-${i}` }));
    const { storeCredits, calls } = makeStoreCredits({
      list: async ({ limit, offset }) => credits.slice(offset, offset + limit),
    });
    const result = await byName.list_store_credits.handler({
      commerce: { storeCredits },
      params: { limit: 2 },
    });
    assert.equal(calls.length, 2);
    assert.equal(result.totalCount, 734);
    assert.equal(result.returned, 2);
    assert.deepEqual(
      result.credits.map((c) => c.id),
      ['sc-0', 'sc-1'],
    );
  });
});

describe('adjust_store_credit', () => {
  it('adjust(id, { amount, note, referenceId }) with a signed exact amount', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    await byName.adjust_store_credit.handler({
      commerce: { storeCredits },
      params: { creditId: 'sc-1', amount: -2.5, reason: 'fix' },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      ['adjust', 'sc-1', { amount: '-2.5', note: 'fix', referenceId: undefined }],
    ]);
  });
});

describe('apply_store_credit', () => {
  it('apply(id, amount, referenceId) on a plain StoreCredits object', async () => {
    const { storeCredits, calls } = makeStoreCredits();
    const result = await byName.apply_store_credit.handler({
      commerce: { storeCredits },
      params: { creditId: 'sc-1', orderId: 'ord-2', amount: 7.5 },
      allowApply: true,
    });
    assert.deepEqual(calls, [['apply', 'sc-1', '7.5', 'ord-2']]);
    assert.equal(result.transaction.referenceId, 'ord-2');
  });

  it('reaches the binding’s apply through the MCP adapter’s callable accessor', async () => {
    // On the adapter's Proxy, `.apply` is Function.prototype.apply; calling it
    // as a method threw "CreateListFromArrayLike called on non-object".
    const { storeCredits, calls } = makeStoreCredits();
    const accessor = createCallableApiAccessor(() => storeCredits);
    const result = await byName.apply_store_credit.handler({
      commerce: { storeCredits: accessor },
      params: { creditId: 'sc-1', orderId: 'ord-2', amount: 7.5 },
      allowApply: true,
    });
    assert.deepEqual(calls, [['apply', 'sc-1', '7.5', 'ord-2']]);
    assert.equal(result.success, true);
  });
});
