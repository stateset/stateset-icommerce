/**
 * Store credit tool definitions: names, permissions, input schemas, the
 * --apply guard, and that the module only calls methods the binding's
 * `StoreCredits` class declares.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { storeCreditTools } from '../../src/tools/store-credits.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(storeCreditTools.map((t) => [t.name, t]));
const parse = (name, params) => z.object(byName[name].inputSchema).safeParse(params);

describe('storeCreditTools -- module exports', () => {
  it('exports the expected tools in order', () => {
    assert.deepEqual(
      storeCreditTools.map((t) => t.name),
      [
        'create_store_credit',
        'get_store_credit',
        'list_store_credits',
        'adjust_store_credit',
        'apply_store_credit',
      ],
    );
  });

  it('read and write permissions are unchanged', () => {
    assert.equal(byName.get_store_credit.permission, 'read');
    assert.equal(byName.list_store_credits.permission, 'read');
    for (const name of ['create_store_credit', 'adjust_store_credit', 'apply_store_credit']) {
      assert.equal(byName[name].permission, 'write', name);
    }
  });
});

describe('storeCreditTools -- binding surface', () => {
  const methods = bindingClassMethods('StoreCredits');

  it('calls only methods the binding’s StoreCredits class declares', () => {
    const called = toolModuleCalls('store-credits.js', 'storeCredits');
    assert.ok(called.size >= 4, `only found ${[...called]}`);
    for (const method of called) {
      assert.ok(methods.has(method), `commerce.storeCredits.${method} is not on the binding`);
    }
    assert.equal(called.has('count'), false);
  });

  it('the binding has the apply method the tool calls', () => {
    // The MCP adapter used to shadow `apply` with Function.prototype.apply;
    // it now prefers the API's own members (see store-credits.test.js, which
    // drives the handler through the real accessor).
    assert.ok(methods.has('apply'));
  });
});

describe('storeCreditTools -- input schemas', () => {
  it('reasons are the real StoreCreditReason values', () => {
    for (const reason of [
      'return',
      'loyalty',
      'compensation',
      'promotion',
      'manual',
      'gift_card',
    ]) {
      assert.ok(parse('create_store_credit', { customerId: 'c', amount: 1, reason }).success);
      assert.ok(parse('list_store_credits', { reason }).success);
    }
    for (const reason of ['refund', 'goodwill', 'other']) {
      assert.equal(
        parse('create_store_credit', { customerId: 'c', amount: 1, reason }).success,
        false,
        reason,
      );
    }
  });

  it('statuses are the real StoreCreditStatus values', () => {
    for (const status of ['active', 'depleted', 'expired', 'voided']) {
      assert.ok(parse('list_store_credits', { status }).success, status);
    }
    assert.equal(parse('list_store_credits', { status: 'fully_used' }).success, false);
  });

  it('create requires a positive amount; adjust accepts a negative one', () => {
    assert.equal(parse('create_store_credit', { customerId: 'c', amount: 0 }).success, false);
    assert.ok(parse('adjust_store_credit', { creditId: 'x', amount: -5, reason: 'r' }).success);
    assert.equal(parse('adjust_store_credit', { creditId: 'x', amount: -5 }).success, false);
  });

  it('list limit defaults to 50 and caps at 500', () => {
    assert.equal(parse('list_store_credits', {}).data.limit, 50);
    assert.equal(parse('list_store_credits', { limit: 501 }).success, false);
  });
});

describe('storeCreditTools -- --apply guard', () => {
  const untouchable = new Proxy(
    {},
    {
      get() {
        throw new Error('the binding must not be reached without --apply');
      },
    },
  );

  for (const [name, params] of [
    ['create_store_credit', { customerId: 'c', amount: 10 }],
    ['adjust_store_credit', { creditId: 'x', amount: -1, reason: 'r' }],
    ['apply_store_credit', { creditId: 'x', orderId: 'o', amount: 1 }],
  ]) {
    it(`${name} previews without --apply`, async () => {
      const result = await byName[name].handler({
        commerce: untouchable,
        params,
        allowApply: false,
      });
      assert.equal(result.success, false);
      assert.match(result.error, /--apply/);
      assert.deepEqual(result.wouldDo, params);
    });
  }
});
