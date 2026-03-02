import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { loyaltyTools } from '../../src/tools/loyalty.js';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(loyaltyTools.map((t) => [t.name, t]));

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

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('loyaltyTools — module exports', () => {
  it('exports an array of 8 tools', () => {
    assert.ok(Array.isArray(loyaltyTools));
    assert.equal(loyaltyTools.length, 8);
  });

  it('exports expected tool names in order', () => {
    const names = loyaltyTools.map((t) => t.name);
    assert.deepStrictEqual(names, EXPECTED_NAMES);
  });

  it('all tools have handler functions', () => {
    for (const tool of loyaltyTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of loyaltyTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of loyaltyTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have an inputSchema object', () => {
    for (const tool of loyaltyTools) {
      assert.ok(tool.inputSchema && typeof tool.inputSchema === 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permission checks
// ---------------------------------------------------------------------------

describe('loyaltyTools — permission assignments', () => {
  it('create_loyalty_program is admin', () => {
    assert.equal(byName['create_loyalty_program'].permission, 'admin');
  });

  it('get_loyalty_program is read', () => {
    assert.equal(byName['get_loyalty_program'].permission, 'read');
  });

  it('enroll_customer is write', () => {
    assert.equal(byName['enroll_customer'].permission, 'write');
  });

  it('get_loyalty_account is read', () => {
    assert.equal(byName['get_loyalty_account'].permission, 'read');
  });

  it('earn_points is write', () => {
    assert.equal(byName['earn_points'].permission, 'write');
  });

  it('redeem_points is write', () => {
    assert.equal(byName['redeem_points'].permission, 'write');
  });

  it('list_rewards is read', () => {
    assert.equal(byName['list_rewards'].permission, 'read');
  });

  it('create_reward is admin', () => {
    assert.equal(byName['create_reward'].permission, 'admin');
  });
});

// ---------------------------------------------------------------------------
// Input schema validation
// ---------------------------------------------------------------------------

describe('loyaltyTools — input schemas', () => {
  it('create_loyalty_program has name, pointsPerDollar, currency, tiers', () => {
    const schema = byName['create_loyalty_program'].inputSchema;
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.description, 'missing description');
    assert.ok(schema.pointsPerDollar, 'missing pointsPerDollar');
    assert.ok(schema.currency, 'missing currency');
    assert.ok(schema.tiers, 'missing tiers');
  });

  it('get_loyalty_program has programId', () => {
    const schema = byName['get_loyalty_program'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
  });

  it('enroll_customer has programId and customerId', () => {
    const schema = byName['enroll_customer'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.customerId, 'missing customerId');
  });

  it('get_loyalty_account has programId and customerId', () => {
    const schema = byName['get_loyalty_account'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.customerId, 'missing customerId');
  });

  it('earn_points has programId, customerId, points, reason, orderId, note', () => {
    const schema = byName['earn_points'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.customerId, 'missing customerId');
    assert.ok(schema.points, 'missing points');
    assert.ok(schema.reason, 'missing reason');
    assert.ok(schema.orderId, 'missing orderId');
    assert.ok(schema.note, 'missing note');
  });

  it('redeem_points has programId, customerId, points, rewardId, orderId', () => {
    const schema = byName['redeem_points'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.customerId, 'missing customerId');
    assert.ok(schema.points, 'missing points');
    assert.ok(schema.rewardId, 'missing rewardId');
    assert.ok(schema.orderId, 'missing orderId');
    assert.ok(schema.note, 'missing note');
  });

  it('list_rewards has programId, tier, limit', () => {
    const schema = byName['list_rewards'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.tier, 'missing tier');
    assert.ok(schema.limit, 'missing limit');
  });

  it('create_reward has programId, name, pointsCost, type, value, and optional fields', () => {
    const schema = byName['create_reward'].inputSchema;
    assert.ok(schema.programId, 'missing programId');
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.pointsCost, 'missing pointsCost');
    assert.ok(schema.type, 'missing type');
    assert.ok(schema.value, 'missing value');
    assert.ok(schema.tier, 'missing tier');
    assert.ok(schema.maxRedemptions, 'missing maxRedemptions');
    assert.ok(schema.stock, 'missing stock');
  });
});

// ---------------------------------------------------------------------------
// Handler apply-guard (write/admin tools)
// ---------------------------------------------------------------------------

describe('loyaltyTools — apply-guard on write/admin tools', () => {
  it('create_loyalty_program requires --apply', async () => {
    const result = await byName['create_loyalty_program'].handler({
      params: { name: 'Gold Club' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('enroll_customer requires --apply', async () => {
    const result = await byName['enroll_customer'].handler({
      params: { programId: 'prg-1', customerId: 'cust-1' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('earn_points requires --apply', async () => {
    const result = await byName['earn_points'].handler({
      params: { programId: 'prg-1', customerId: 'cust-1', points: 100 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('redeem_points requires --apply', async () => {
    const result = await byName['redeem_points'].handler({
      params: { programId: 'prg-1', customerId: 'cust-1', points: 50 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('create_reward requires --apply', async () => {
    const result = await byName['create_reward'].handler({
      params: {
        programId: 'prg-1',
        name: '10% off coupon',
        pointsCost: 500,
        type: 'discount_percentage',
        value: 10,
      },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });
});

// ---------------------------------------------------------------------------
// Handler error paths (commerce stub missing methods)
// ---------------------------------------------------------------------------

describe('loyaltyTools — handler error paths', () => {
  it('get_loyalty_program fails gracefully with empty commerce', async () => {
    try {
      await byName['get_loyalty_program'].handler({
        params: { programId: 'prg-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('get_loyalty_account fails gracefully with empty commerce', async () => {
    try {
      await byName['get_loyalty_account'].handler({
        params: { programId: 'prg-1', customerId: 'cust-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('list_rewards fails gracefully with empty commerce', async () => {
    try {
      await byName['list_rewards'].handler({
        params: { programId: 'prg-1', limit: 10 },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_loyalty_program fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_loyalty_program'].handler({
        params: { name: 'Test Program' },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('enroll_customer fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['enroll_customer'].handler({
        params: { programId: 'prg-1', customerId: 'cust-1' },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('earn_points fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['earn_points'].handler({
        params: { programId: 'prg-1', customerId: 'cust-1', points: 100 },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('redeem_points fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['redeem_points'].handler({
        params: { programId: 'prg-1', customerId: 'cust-1', points: 50 },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_reward fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_reward'].handler({
        params: {
          programId: 'prg-1',
          name: 'Free mug',
          pointsCost: 1000,
          type: 'free_product',
          value: 1,
        },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });
});

// ---------------------------------------------------------------------------
// Handler success paths (mocked commerce)
// ---------------------------------------------------------------------------

describe('loyaltyTools — handler success paths (mocked commerce)', () => {
  const mockProgram = {
    id: 'prg-001',
    name: 'Gold Club',
    description: 'VIP rewards program',
    pointsPerDollar: 2,
    currency: 'USD',
    tiers: [{ name: 'Gold', minPoints: 0, multiplier: 1, perks: [] }],
    totalMembers: 42,
    status: 'active',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-15T00:00:00Z',
  };

  const mockAccount = {
    id: 'acct-001',
    programId: 'prg-001',
    customerId: 'cust-1',
    pointsBalance: 500,
    lifetimePoints: 1200,
    currentTier: 'Gold',
    nextTier: 'Platinum',
    pointsToNextTier: 800,
    enrolledAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-15T00:00:00Z',
  };

  const mockRewards = [
    {
      id: 'rwd-1',
      name: '$10 off',
      description: 'Flat $10 discount',
      pointsCost: 500,
      type: 'discount_fixed',
      value: 10,
      tier: null,
      status: 'active',
      remainingStock: 100,
    },
  ];

  const mockTransaction = { id: 'txn-1', points: 100, type: 'earn', createdAt: '2026-01-15T00:00:00Z' };

  const commerce = {
    loyalty: {
      createProgram: async (data) => ({ ...mockProgram, ...data }),
      getProgram: async (id) => (id === 'prg-001' ? mockProgram : null),
      enrollCustomer: async () => mockAccount,
      getAccount: async (_pid, cid) => (cid === 'cust-1' ? mockAccount : null),
      earnPoints: async () => mockTransaction,
      redeemPoints: async () => ({ ...mockTransaction, type: 'redeem' }),
      listRewards: async () => mockRewards,
      createReward: async (_pid, data) => ({ id: 'rwd-2', ...data }),
    },
  };

  it('create_loyalty_program returns success with allowApply', async () => {
    const result = await byName['create_loyalty_program'].handler({
      params: { name: 'Gold Club' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.program);
  });

  it('get_loyalty_program returns success for existing program', async () => {
    const result = await byName['get_loyalty_program'].handler({
      params: { programId: 'prg-001' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.program);
    assert.equal(result.program.id, 'prg-001');
    assert.equal(result.program.totalMembers, 42);
  });

  it('get_loyalty_program returns not-found for missing program', async () => {
    const result = await byName['get_loyalty_program'].handler({
      params: { programId: 'nonexistent' },
      commerce,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('enroll_customer returns success with allowApply', async () => {
    const result = await byName['enroll_customer'].handler({
      params: { programId: 'prg-001', customerId: 'cust-1' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('enrolled'));
    assert.ok(result.account);
  });

  it('get_loyalty_account returns success for existing account', async () => {
    const result = await byName['get_loyalty_account'].handler({
      params: { programId: 'prg-001', customerId: 'cust-1' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.account);
    assert.equal(result.account.pointsBalance, 500);
    assert.equal(result.account.currentTier, 'Gold');
  });

  it('get_loyalty_account returns not-found for missing account', async () => {
    const result = await byName['get_loyalty_account'].handler({
      params: { programId: 'prg-001', customerId: 'nonexistent' },
      commerce,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('earn_points returns success with point count in message', async () => {
    const result = await byName['earn_points'].handler({
      params: { programId: 'prg-001', customerId: 'cust-1', points: 100 },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('100'));
    assert.ok(result.message.includes('points'));
    assert.ok(result.transaction);
  });

  it('redeem_points returns success with point count in message', async () => {
    const result = await byName['redeem_points'].handler({
      params: { programId: 'prg-001', customerId: 'cust-1', points: 50 },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('50'));
    assert.ok(result.message.includes('redeemed'));
    assert.ok(result.transaction);
  });

  it('list_rewards returns rewards array', async () => {
    const result = await byName['list_rewards'].handler({
      params: { programId: 'prg-001', limit: 20 },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.rewards));
    assert.equal(result.rewards[0].name, '$10 off');
    assert.equal(result.programId, 'prg-001');
  });

  it('create_reward returns success with allowApply', async () => {
    const result = await byName['create_reward'].handler({
      params: {
        programId: 'prg-001',
        name: 'Free shipping',
        pointsCost: 200,
        type: 'free_shipping',
        value: 1,
      },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.reward);
  });
});
