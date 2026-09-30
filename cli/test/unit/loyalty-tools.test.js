/**
 * Loyalty tools — module shape, schemas, apply guard, and binding surface.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { loyaltyTools } from '../../src/tools/loyalty.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(loyaltyTools.map((t) => [t.name, t]));
const parse = (name, params) => z.object(byName[name].inputSchema).safeParse(params);

const EXPECTED_NAMES = [
  'create_loyalty_program',
  'get_loyalty_program',
  'enroll_customer',
  'get_loyalty_account',
  'earn_points',
  'redeem_points',
  'list_rewards',
  'create_reward',
];

describe('loyaltyTools — module exports', () => {
  it('exports the expected tools in order', () => {
    assert.deepEqual(
      loyaltyTools.map((t) => t.name),
      EXPECTED_NAMES,
    );
  });

  it('every tool has a handler, description, and inputSchema', () => {
    for (const tool of loyaltyTools) {
      assert.equal(typeof tool.handler, 'function', tool.name);
      assert.ok(tool.description.length > 0, tool.name);
      assert.equal(typeof tool.inputSchema, 'object', tool.name);
    }
  });

  it('keeps its permission assignments', () => {
    assert.deepEqual(Object.fromEntries(loyaltyTools.map((t) => [t.name, t.permission])), {
      create_loyalty_program: 'admin',
      get_loyalty_program: 'read',
      enroll_customer: 'write',
      get_loyalty_account: 'read',
      earn_points: 'write',
      redeem_points: 'write',
      list_rewards: 'read',
      create_reward: 'admin',
    });
  });
});

describe('loyaltyTools — binding surface', () => {
  const loyaltyMethods = bindingClassMethods('Loyalty');

  it('calls only methods the binding’s Loyalty class declares', () => {
    const called = toolModuleCalls('loyalty.js', 'loyalty');
    assert.ok(called.size >= 8, `only found ${[...called]}`);
    for (const method of called) {
      assert.ok(loyaltyMethods.has(method), `commerce.loyalty.${method} is not on the binding`);
    }
  });

  it('the handler-test mock methods all exist on the binding', () => {
    for (const method of [
      'createProgram',
      'getProgram',
      'enroll',
      'getAccountByCustomer',
      'adjustPoints',
      'getReward',
      'listRewards',
      'createReward',
    ]) {
      assert.ok(loyaltyMethods.has(method), method);
    }
  });
});

describe('loyaltyTools — input schemas', () => {
  it('create_loyalty_program no longer takes a currency the engine does not store', () => {
    assert.equal('currency' in byName.create_loyalty_program.inputSchema, false);
    assert.equal(parse('create_loyalty_program', { name: 'P' }).success, true);
  });

  it('enroll/get account/earn/redeem are keyed by programId + customerId', () => {
    for (const name of ['enroll_customer', 'get_loyalty_account', 'earn_points', 'redeem_points']) {
      const keys = Object.keys(byName[name].inputSchema);
      assert.ok(keys.includes('programId') && keys.includes('customerId'), name);
    }
  });

  it('earn/redeem require a positive integer point count', () => {
    const base = { programId: 'p', customerId: 'c' };
    assert.equal(parse('earn_points', { ...base, points: 0 }).success, false);
    assert.equal(parse('earn_points', { ...base, points: 1.5 }).success, false);
    assert.equal(parse('redeem_points', { ...base, points: -5 }).success, false);
    assert.equal(parse('redeem_points', { ...base, points: 5 }).success, true);
  });

  it('list_rewards filters by rewardType/activeOnly, not tier', () => {
    assert.deepEqual(Object.keys(byName.list_rewards.inputSchema).sort(), [
      'activeOnly',
      'limit',
      'programId',
      'rewardType',
    ]);
  });

  it('create_reward takes rewardType and an optional decimal-string value', () => {
    assert.equal(
      parse('create_reward', {
        programId: 'p',
        name: 'Free ship',
        pointsCost: 50,
        rewardType: 'free_shipping',
      }).success,
      true,
    );
    for (const gone of ['type', 'tier', 'maxRedemptions', 'stock']) {
      assert.equal(gone in byName.create_reward.inputSchema, false, gone);
    }
  });
});

describe('loyaltyTools — apply guard', () => {
  const previews = {
    create_loyalty_program: { name: 'P' },
    enroll_customer: { programId: 'p', customerId: 'c' },
    earn_points: { programId: 'p', customerId: 'c', points: 5 },
    redeem_points: { programId: 'p', customerId: 'c', points: 5 },
    create_reward: { programId: 'p', name: 'R', pointsCost: 5, rewardType: 'discount' },
  };
  for (const [name, params] of Object.entries(previews)) {
    it(`${name} does not touch the binding without --apply`, async () => {
      const result = await byName[name].handler({ commerce: {}, params });
      assert.equal(result.success, false);
      assert.match(result.error, /--apply/);
      assert.deepEqual(result.wouldDo, params);
    });
  }
});

describe('loyaltyTools — handler error paths (binding missing)', () => {
  for (const [name, params, allowApply] of [
    ['get_loyalty_program', { programId: 'p' }, false],
    ['get_loyalty_account', { programId: 'p', customerId: 'c' }, false],
    ['list_rewards', { programId: 'p' }, false],
    ['enroll_customer', { programId: 'p', customerId: 'c' }, true],
    ['earn_points', { programId: 'p', customerId: 'c', points: 1 }, true],
  ]) {
    it(`${name} throws a TypeError when commerce.loyalty is absent`, async () => {
      await assert.rejects(
        () => byName[name].handler({ commerce: {}, params, allowApply }),
        TypeError,
      );
    });
  }
});
