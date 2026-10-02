/**
 * Loyalty Program Tools — handler behaviour
 *
 * Mocks here define only methods the Node binding's `Loyalty` class really has
 * (checked against bindings/node/index.d.ts in loyalty-tools.test.js), and each
 * test asserts the exact arguments the binding receives.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { loyaltyTools } from '../../src/tools/loyalty.js';

function findTool(name) {
  const tool = loyaltyTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data — shaped like the binding's Loyalty*Output / RewardOutput
// ============================================================================

const PROGRAM_ID = 'lp_001';
const CUSTOMER_ID = 'cust_001';
const ACCOUNT_ID = 'la_001';

const mockProgram = {
  id: PROGRAM_ID,
  name: 'Gold Rewards',
  description: 'Earn points on every purchase',
  pointsPerDollar: 10,
  tiers: [{ name: 'Bronze', minPoints: 0, multiplier: 1, perks: [] }],
  status: 'active',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const mockAccount = {
  id: ACCOUNT_ID,
  programId: PROGRAM_ID,
  customerId: CUSTOMER_ID,
  pointsBalance: 500,
  lifetimePoints: 800,
  tier: 'bronze',
  createdAt: '2026-01-15T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

const mockReward = {
  id: 'rw_001',
  programId: PROGRAM_ID,
  name: '$10 off',
  description: 'Ten dollars off',
  pointsCost: 200,
  rewardType: 'discount',
  value: '10.00',
  isActive: true,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

/** A `commerce.loyalty` mock that records every call. */
function makeCommerce(overrides = {}) {
  const calls = [];
  const record =
    (name, impl) =>
    async (...args) => {
      calls.push({ name, args });
      return impl(...args);
    };
  const loyalty = {
    createProgram: record('createProgram', (input) => ({ ...mockProgram, ...input })),
    getProgram: record('getProgram', (id) => (id === PROGRAM_ID ? mockProgram : null)),
    enroll: record('enroll', (input) => ({ ...mockAccount, ...input, pointsBalance: 0 })),
    getAccountByCustomer: record('getAccountByCustomer', (customerId, programId) =>
      customerId === CUSTOMER_ID && programId === PROGRAM_ID ? mockAccount : null,
    ),
    adjustPoints: record('adjustPoints', (input) => ({
      id: 'lt_001',
      ...input,
      createdAt: '2026-02-01T00:00:00Z',
    })),
    getReward: record('getReward', (id) => (id === mockReward.id ? mockReward : null)),
    listRewards: record('listRewards', () => [mockReward]),
    createReward: record('createReward', (input) => ({ ...mockReward, ...input })),
  };
  for (const [name, impl] of Object.entries(overrides)) loyalty[name] = record(name, impl);
  return { commerce: { loyalty }, calls };
}

// ============================================================================
// create_loyalty_program / get_loyalty_program
// ============================================================================

describe('create_loyalty_program', () => {
  const tool = findTool('create_loyalty_program');

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { name: 'X' } });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('passes a CreateLoyaltyProgramInput, filling tier multiplier and perks', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: {
        name: 'Gold Rewards',
        description: 'desc',
        pointsPerDollar: 10,
        tiers: [
          { name: 'Bronze', minPoints: 0 },
          { name: 'Gold', minPoints: 1000, multiplier: 1.5, perks: ['free shipping'] },
        ],
      },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      {
        name: 'createProgram',
        args: [
          {
            name: 'Gold Rewards',
            description: 'desc',
            pointsPerDollar: 10,
            tiers: [
              { name: 'Bronze', minPoints: 0, multiplier: 1, perks: [] },
              { name: 'Gold', minPoints: 1000, multiplier: 1.5, perks: ['free shipping'] },
            ],
          },
        ],
      },
    ]);
    assert.equal(result.success, true);
  });

  it('defaults pointsPerDollar to 1 and omits tiers when none are given', async () => {
    const { commerce, calls } = makeCommerce();
    await tool.handler({ commerce, params: { name: 'Basic' }, allowApply: true });
    assert.deepEqual(calls[0].args[0], {
      name: 'Basic',
      description: undefined,
      pointsPerDollar: 1,
      tiers: undefined,
    });
  });
});

describe('get_loyalty_program', () => {
  const tool = findTool('get_loyalty_program');

  it('returns only fields LoyaltyProgramOutput has', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { programId: PROGRAM_ID } });
    assert.deepEqual(calls, [{ name: 'getProgram', args: [PROGRAM_ID] }]);
    assert.deepEqual(result, { success: true, program: mockProgram });
  });

  it('returns success: false for an unknown program', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({ commerce, params: { programId: 'nope' } });
    assert.deepEqual(result, { success: false, error: 'Loyalty program not found' });
  });
});

// ============================================================================
// enroll_customer / get_loyalty_account
// ============================================================================

describe('enroll_customer', () => {
  const tool = findTool('enroll_customer');
  const params = { programId: PROGRAM_ID, customerId: CUSTOMER_ID };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('calls commerce.loyalty.enroll with an EnrollCustomerInput', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params, allowApply: true });
    assert.deepEqual(calls, [
      { name: 'enroll', args: [{ customerId: CUSTOMER_ID, programId: PROGRAM_ID }] },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.account.id, ACCOUNT_ID);
    assert.equal(result.account.pointsBalance, 0);
  });

  it('propagates binding errors', async () => {
    const { commerce } = makeCommerce({
      enroll: () => {
        throw new Error('already enrolled');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params, allowApply: true }),
      /already enrolled/,
    );
  });
});

describe('get_loyalty_account', () => {
  const tool = findTool('get_loyalty_account');

  it('looks the account up with getAccountByCustomer(customerId, programId)', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { programId: PROGRAM_ID, customerId: CUSTOMER_ID },
    });
    assert.deepEqual(calls, [{ name: 'getAccountByCustomer', args: [CUSTOMER_ID, PROGRAM_ID] }]);
    assert.deepEqual(result, { success: true, account: mockAccount });
  });

  it('returns success: false when the customer is not enrolled', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { programId: PROGRAM_ID, customerId: 'other' },
    });
    assert.deepEqual(result, { success: false, error: 'Loyalty account not found' });
  });
});

// ============================================================================
// earn_points / redeem_points
// ============================================================================

describe('earn_points', () => {
  const tool = findTool('earn_points');
  const base = { programId: PROGRAM_ID, customerId: CUSTOMER_ID, points: 100 };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: base });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('adjusts the account by +points as an earn transaction', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { ...base, reason: 'purchase', orderId: 'ord_1', note: 'first order' },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      { name: 'getAccountByCustomer', args: [CUSTOMER_ID, PROGRAM_ID] },
      {
        name: 'adjustPoints',
        args: [
          {
            accountId: ACCOUNT_ID,
            points: 100,
            transactionType: 'earn',
            referenceId: 'ord_1',
            description: 'purchase: first order',
          },
        ],
      },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.message, '100 points awarded');
    assert.equal(result.transaction.transactionType, 'earn');
  });

  it('records reason "manual" when none is given', async () => {
    const { commerce, calls } = makeCommerce();
    await tool.handler({ commerce, params: base, allowApply: true });
    assert.equal(calls[1].args[0].description, 'manual');
    assert.equal(calls[1].args[0].referenceId, undefined);
  });

  it('refuses a customer who is not enrolled, without adjusting', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { ...base, customerId: 'other' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.match(result.error, /not enrolled/);
    assert.deepEqual(
      calls.map((c) => c.name),
      ['getAccountByCustomer'],
    );
  });
});

describe('redeem_points', () => {
  const tool = findTool('redeem_points');
  const base = { programId: PROGRAM_ID, customerId: CUSTOMER_ID, points: 200 };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: base });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('adjusts the account by -points as a redeem transaction', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { ...base, orderId: 'ord_2', note: 'checkout' },
      allowApply: true,
    });
    assert.deepEqual(calls[1], {
      name: 'adjustPoints',
      args: [
        {
          accountId: ACCOUNT_ID,
          points: -200,
          transactionType: 'redeem',
          referenceId: 'ord_2',
          description: 'checkout',
        },
      ],
    });
    assert.equal(result.message, '200 points redeemed');
  });

  it('checks the reward and references it when no order is given', async () => {
    const { commerce, calls } = makeCommerce();
    await tool.handler({
      commerce,
      params: { ...base, rewardId: 'rw_001' },
      allowApply: true,
    });
    assert.deepEqual(
      calls.map((c) => c.name),
      ['getAccountByCustomer', 'getReward', 'adjustPoints'],
    );
    assert.deepEqual(calls[2].args[0], {
      accountId: ACCOUNT_ID,
      points: -200,
      transactionType: 'redeem',
      referenceId: 'rw_001',
      description: 'reward rw_001 ($10 off)',
    });
  });

  it('refuses a reward that is not in the program', async () => {
    const { commerce, calls } = makeCommerce({
      getReward: () => ({ ...mockReward, programId: 'other' }),
    });
    const result = await tool.handler({
      commerce,
      params: { ...base, rewardId: 'rw_001' },
      allowApply: true,
    });
    assert.deepEqual(result, {
      success: false,
      error: 'Reward not found in this loyalty program',
    });
    assert.equal(
      calls.some((c) => c.name === 'adjustPoints'),
      false,
    );
  });

  it('refuses an inactive reward', async () => {
    const { commerce } = makeCommerce({ getReward: () => ({ ...mockReward, isActive: false }) });
    const result = await tool.handler({
      commerce,
      params: { ...base, rewardId: 'rw_001' },
      allowApply: true,
    });
    assert.deepEqual(result, { success: false, error: 'Reward is not active' });
  });

  it('leaves the overdraft check to the engine and propagates its refusal', async () => {
    const { commerce } = makeCommerce({
      adjustPoints: () => {
        throw new Error('Failed to adjust points: Validation error: Insufficient points balance');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { ...base, points: 9999 }, allowApply: true }),
      /Insufficient points balance/,
    );
  });
});

// ============================================================================
// list_rewards / create_reward
// ============================================================================

describe('list_rewards', () => {
  const tool = findTool('list_rewards');

  it('passes a RewardFilterInput and maps RewardOutput fields', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { programId: PROGRAM_ID, rewardType: 'discount', activeOnly: true, limit: 5 },
    });
    assert.deepEqual(calls, [
      {
        name: 'listRewards',
        args: [{ programId: PROGRAM_ID, rewardType: 'discount', isActive: true, limit: 5 }],
      },
    ]);
    assert.deepEqual(result.rewards, [
      {
        id: 'rw_001',
        name: '$10 off',
        description: 'Ten dollars off',
        pointsCost: 200,
        rewardType: 'discount',
        value: '10.00',
        isActive: true,
      },
    ]);
    assert.equal(result.returned, 1);
  });

  it('does not filter on isActive unless activeOnly is set', async () => {
    const { commerce, calls } = makeCommerce();
    await tool.handler({ commerce, params: { programId: PROGRAM_ID, limit: 20 } });
    assert.equal(calls[0].args[0].isActive, undefined);
  });
});

describe('create_reward', () => {
  const tool = findTool('create_reward');
  const params = {
    programId: PROGRAM_ID,
    name: '$10 off',
    description: 'Ten dollars off',
    pointsCost: 200,
    rewardType: 'discount',
    value: '10.00',
  };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('passes a CreateRewardInput (programId inside, decimal-string value)', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params, allowApply: true });
    assert.deepEqual(calls, [{ name: 'createReward', args: [params] }]);
    assert.equal(result.success, true);
    assert.equal(result.reward.value, '10.00');
  });

  it('refuses a float value and an unknown reward type at the schema', () => {
    assert.equal(tool.inputSchema.value.safeParse(10.5).success, false);
    assert.equal(tool.inputSchema.value.safeParse('10.50').success, true);
    assert.equal(tool.inputSchema.rewardType.safeParse('gift_card').success, false);
  });
});
