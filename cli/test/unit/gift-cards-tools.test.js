/**
 * Gift card tool definitions: names, permissions, input schemas, the --apply
 * guard, and that the module only calls methods the binding's `GiftCards`
 * class declares.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { giftCardTools } from '../../src/tools/gift-cards.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(giftCardTools.map((t) => [t.name, t]));
const parse = (name, params) => z.object(byName[name].inputSchema).safeParse(params);

const EXPECTED_NAMES = [
  'create_gift_card',
  'get_gift_card',
  'list_gift_cards',
  'charge_gift_card',
  'refund_to_gift_card',
  'disable_gift_card',
  'check_gift_card_balance',
];

describe('giftCardTools -- module exports', () => {
  it('exports the expected tools in order', () => {
    assert.deepEqual(
      giftCardTools.map((t) => t.name),
      EXPECTED_NAMES,
    );
  });

  it('every tool has a handler, description and schema', () => {
    for (const tool of giftCardTools) {
      assert.equal(typeof tool.handler, 'function', tool.name);
      assert.ok(tool.description.length > 0, tool.name);
      assert.equal(typeof tool.inputSchema, 'object', tool.name);
    }
  });

  it('read and write permissions are unchanged', () => {
    for (const name of ['get_gift_card', 'list_gift_cards', 'check_gift_card_balance']) {
      assert.equal(byName[name].permission, 'read', name);
    }
    for (const name of [
      'create_gift_card',
      'charge_gift_card',
      'refund_to_gift_card',
      'disable_gift_card',
    ]) {
      assert.equal(byName[name].permission, 'write', name);
    }
  });
});

describe('giftCardTools -- binding surface', () => {
  const methods = bindingClassMethods('GiftCards');

  it('calls only methods the binding’s GiftCards class declares', () => {
    const called = toolModuleCalls('gift-cards.js', 'giftCards');
    assert.ok(called.size >= 7, `only found ${[...called]}`);
    for (const method of called) {
      assert.ok(methods.has(method), `commerce.giftCards.${method} is not on the binding`);
    }
  });

  it('the binding has no count(); list is the only way to total', () => {
    assert.equal(methods.has('count'), false);
    assert.equal(toolModuleCalls('gift-cards.js', 'giftCards').has('count'), false);
  });
});

describe('giftCardTools -- input schemas', () => {
  it('create_gift_card takes the fields CreateGiftCardInput has', () => {
    assert.deepEqual(Object.keys(byName.create_gift_card.inputSchema).sort(), [
      'code',
      'currency',
      'expiresAt',
      'initialBalance',
      'message',
      'recipientEmail',
      'senderName',
    ]);
    assert.ok(parse('create_gift_card', { initialBalance: 25 }).success);
    assert.equal(parse('create_gift_card', { initialBalance: 0 }).success, false);
  });

  it('list_gift_cards filters on the real GiftCardStatus values', () => {
    for (const status of ['active', 'depleted', 'expired', 'disabled']) {
      assert.ok(parse('list_gift_cards', { status }).success, status);
    }
    assert.equal(parse('list_gift_cards', { status: 'fully_redeemed' }).success, false);
    assert.equal(parse('list_gift_cards', { limit: 501 }).success, false);
    assert.equal(parse('list_gift_cards', {}).data.limit, 50);
  });

  it('charge/refund take id, amount and an optional order reference', () => {
    for (const name of ['charge_gift_card', 'refund_to_gift_card']) {
      assert.deepEqual(Object.keys(byName[name].inputSchema).sort(), [
        'amount',
        'giftCardId',
        'orderId',
      ]);
      assert.equal(parse(name, { giftCardId: 'g', amount: -1 }).success, false);
    }
  });

  it('disable_gift_card takes only the id', () => {
    assert.deepEqual(Object.keys(byName.disable_gift_card.inputSchema), ['giftCardId']);
  });
});

describe('giftCardTools -- --apply guard', () => {
  const untouchable = new Proxy(
    {},
    {
      get() {
        throw new Error('the binding must not be reached without --apply');
      },
    },
  );

  for (const [name, params] of [
    ['create_gift_card', { initialBalance: 10 }],
    ['charge_gift_card', { giftCardId: 'g', amount: 5 }],
    ['refund_to_gift_card', { giftCardId: 'g', amount: 5 }],
    ['disable_gift_card', { giftCardId: 'g' }],
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
