/**
 * Loyalty Program Tools Test Suite
 *
 * Tests for the loyaltyTools module (cli/src/tools/loyalty.js):
 * - create_loyalty_program (admin)
 * - get_loyalty_program (read)
 * - enroll_customer (write)
 * - get_loyalty_account (read)
 * - earn_points (write)
 * - redeem_points (write)
 * - list_rewards (read)
 * - create_reward (admin)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { loyaltyTools } from '../../src/tools/loyalty.js';

// ============================================================================
// Helper: find tool by name from a tools array
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data
// ============================================================================

const mockProgram = {
  id: 'lp_001',
  name: 'Gold Rewards',
  description: 'Earn points on every purchase',
  pointsPerDollar: 10,
  currency: 'USD',
  tiers: [{ name: 'Bronze', minPoints: 0, multiplier: 1, perks: [] }],
  totalMembers: 500,
  memberCount: 500,
  status: 'active',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const mockAccount = {
  id: 'la_001',
  customerId: 'cust_001',
  programId: 'lp_001',
  pointsBalance: 2500,
  lifetimePoints: 5000,
  currentTier: 'Bronze',
  tier: 'Bronze',
  nextTier: 'Silver',
  pointsToNextTier: 500,
  enrolledAt: '2026-01-15T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

const mockTx = {
  id: 'ltx_001',
  programId: 'lp_001',
  customerId: 'cust_001',
  type: 'earn',
  points: 100,
  balance: 2600,
  reason: 'purchase',
  orderId: null,
  note: null,
  createdAt: '2026-02-01T00:00:00Z',
};

const mockReward = {
  id: 'rw_001',
  programId: 'lp_001',
  name: 'Free Shipping',
  description: 'Free standard shipping on your next order',
  pointsCost: 500,
  type: 'free_shipping',
  value: '0',
  tier: null,
  status: 'active',
  remainingStock: null,
  createdAt: '2026-01-01T00:00:00Z',
};

// ============================================================================
// Mock commerce factory
// ============================================================================

function makeLoyaltyCommerce(overrides = {}) {
  return {
    loyalty: {
      createProgram: async (data) => ({ ...mockProgram, ...data }),
      getProgram: async (id) => (id === 'lp_001' ? mockProgram : null),
      enrollCustomer: async (programId, customerId) => ({ ...mockAccount, programId, customerId }),
      getAccount: async (programId, customerId) =>
        programId === 'lp_001' && customerId === 'cust_001' ? mockAccount : null,
      earnPoints: async (data) => ({ ...mockTx, ...data, type: 'earn' }),
      redeemPoints: async (data) => ({ ...mockTx, ...data, type: 'redeem', points: -data.points }),
      listRewards: async (_programId, _opts) => [mockReward],
      createReward: async (programId, data) => ({ ...mockReward, programId, ...data }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Loyalty Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(loyaltyTools));
  });

  it('exports exactly 8 tools', () => {
    assert.equal(loyaltyTools.length, 8);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of loyaltyTools) {
      assert.ok(tool.name, `missing name`);
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });

  it('admin tools have permission: admin', () => {
    const adminTools = ['create_loyalty_program', 'create_reward'];
    for (const name of adminTools) {
      const tool = findTool(loyaltyTools, name);
      assert.equal(tool.permission, 'admin', `${name} should have admin permission`);
    }
  });

  it('write tools have permission: write', () => {
    const writeTools = ['enroll_customer', 'earn_points', 'redeem_points'];
    for (const name of writeTools) {
      const tool = findTool(loyaltyTools, name);
      assert.equal(tool.permission, 'write', `${name} should have write permission`);
    }
  });

  it('read tools have permission: read', () => {
    const readTools = ['get_loyalty_program', 'get_loyalty_account', 'list_rewards'];
    for (const name of readTools) {
      const tool = findTool(loyaltyTools, name);
      assert.equal(tool.permission, 'read', `${name} should have read permission`);
    }
  });
});

// ============================================================================
// create_loyalty_program
// ============================================================================

describe('create_loyalty_program', () => {
  const tool = findTool(loyaltyTools, 'create_loyalty_program');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { name: 'Gold Rewards', pointsPerDollar: 10 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
    assert.ok(result.hint);
  });

  it('creates program with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: {
        name: 'Gold Rewards',
        pointsPerDollar: 10,
        currency: 'USD',
        tiers: [{ name: 'Bronze', minPoints: 0, multiplier: 1, perks: [] }],
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.program);
    assert.equal(result.program.name, 'Gold Rewards');
  });

  it('passes all fields to commerce.loyalty.createProgram', async () => {
    let calledWith;
    const commerce = makeLoyaltyCommerce({
      createProgram: async (data) => {
        calledWith = data;
        return { ...mockProgram, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        name: 'Silver Tier',
        description: 'Mid-range program',
        pointsPerDollar: 5,
        currency: 'EUR',
        tiers: [{ name: 'Entry', minPoints: 0, multiplier: 1 }],
      },
      allowApply: true,
    });
    assert.equal(calledWith.name, 'Silver Tier');
    assert.equal(calledWith.description, 'Mid-range program');
    assert.equal(calledWith.pointsPerDollar, 5);
    assert.equal(calledWith.currency, 'EUR');
  });

  it('defaults pointsPerDollar to 1 when omitted', async () => {
    let calledWith;
    const commerce = makeLoyaltyCommerce({
      createProgram: async (data) => {
        calledWith = data;
        return { ...mockProgram, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { name: 'Basic Rewards' },
      allowApply: true,
    });
    assert.ok(calledWith.pointsPerDollar >= 1);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      createProgram: async () => { throw new Error('program creation failed'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { name: 'Test' }, allowApply: true }),
      /program creation failed/,
    );
  });
});

// ============================================================================
// get_loyalty_program
// ============================================================================

describe('get_loyalty_program', () => {
  const tool = findTool(loyaltyTools, 'get_loyalty_program');

  it('returns program details for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.program.id, 'lp_001');
    assert.equal(result.program.name, 'Gold Rewards');
    assert.equal(result.program.pointsPerDollar, 10);
    assert.ok(Array.isArray(result.program.tiers));
  });

  it('returns success: false for unknown program ID', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_nope' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      getProgram: async () => { throw new Error('DB error'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001' } }),
      /DB error/,
    );
  });
});

// ============================================================================
// enroll_customer
// ============================================================================

describe('enroll_customer', () => {
  const tool = findTool(loyaltyTools, 'enroll_customer');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('enrolls customer with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('enrolled'));
    assert.ok(result.account);
    assert.equal(result.account.customerId, 'cust_001');
  });

  it('calls commerce.loyalty.enrollCustomer with programId and customerId', async () => {
    let calledProgramId, calledCustomerId;
    const commerce = makeLoyaltyCommerce({
      enrollCustomer: async (pid, cid) => {
        calledProgramId = pid;
        calledCustomerId = cid;
        return { ...mockAccount, programId: pid, customerId: cid };
      },
    });
    await tool.handler({
      commerce,
      params: { programId: 'lp_001', customerId: 'cust_002' },
      allowApply: true,
    });
    assert.equal(calledProgramId, 'lp_001');
    assert.equal(calledCustomerId, 'cust_002');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      enrollCustomer: async () => { throw new Error('already enrolled'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001', customerId: 'cust_001' }, allowApply: true }),
      /already enrolled/,
    );
  });
});

// ============================================================================
// get_loyalty_account
// ============================================================================

describe('get_loyalty_account', () => {
  const tool = findTool(loyaltyTools, 'get_loyalty_account');

  it('returns account details for valid program+customer', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.account.customerId, 'cust_001');
    assert.equal(result.account.programId, 'lp_001');
    assert.equal(result.account.pointsBalance, 2500);
    assert.equal(result.account.lifetimePoints, 5000);
    assert.ok(result.account.currentTier);
  });

  it('returns success: false when account not found', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_nope' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      getAccount: async () => { throw new Error('account lookup failed'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001', customerId: 'cust_001' } }),
      /account lookup failed/,
    );
  });
});

// ============================================================================
// earn_points
// ============================================================================

describe('earn_points', () => {
  const tool = findTool(loyaltyTools, 'earn_points');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001', points: 100 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('awards points with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001', points: 100, reason: 'purchase' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('100'));
    assert.ok(result.transaction);
  });

  it('includes points count in success message', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001', points: 250 },
      allowApply: true,
    });
    assert.ok(result.message.includes('250'));
  });

  it('passes all optional fields to commerce.loyalty.earnPoints', async () => {
    let calledWith;
    const commerce = makeLoyaltyCommerce({
      earnPoints: async (data) => {
        calledWith = data;
        return { ...mockTx, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        programId: 'lp_001',
        customerId: 'cust_001',
        points: 75,
        reason: 'referral',
        orderId: 'ord_001',
        note: 'Referred cust_002',
      },
      allowApply: true,
    });
    assert.equal(calledWith.points, 75);
    assert.equal(calledWith.reason, 'referral');
    assert.equal(calledWith.orderId, 'ord_001');
    assert.equal(calledWith.note, 'Referred cust_002');
  });

  it('defaults reason to manual when not provided', async () => {
    let calledWith;
    const commerce = makeLoyaltyCommerce({
      earnPoints: async (data) => {
        calledWith = data;
        return { ...mockTx, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { programId: 'lp_001', customerId: 'cust_001', points: 50 },
      allowApply: true,
    });
    assert.equal(calledWith.reason, 'manual');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      earnPoints: async () => { throw new Error('points award failed'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001', customerId: 'cust_001', points: 100 }, allowApply: true }),
      /points award failed/,
    );
  });
});

// ============================================================================
// redeem_points
// ============================================================================

describe('redeem_points', () => {
  const tool = findTool(loyaltyTools, 'redeem_points');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001', customerId: 'cust_001', points: 500 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('redeems points with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: {
        programId: 'lp_001',
        customerId: 'cust_001',
        points: 500,
        rewardId: 'rw_001',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('500'));
    assert.ok(result.transaction);
  });

  it('passes rewardId and orderId to commerce.loyalty.redeemPoints', async () => {
    let calledWith;
    const commerce = makeLoyaltyCommerce({
      redeemPoints: async (data) => {
        calledWith = data;
        return { ...mockTx, ...data, type: 'redeem' };
      },
    });
    await tool.handler({
      commerce,
      params: {
        programId: 'lp_001',
        customerId: 'cust_001',
        points: 500,
        rewardId: 'rw_001',
        orderId: 'ord_002',
        note: 'Reward redemption',
      },
      allowApply: true,
    });
    assert.equal(calledWith.rewardId, 'rw_001');
    assert.equal(calledWith.orderId, 'ord_002');
    assert.equal(calledWith.note, 'Reward redemption');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      redeemPoints: async () => { throw new Error('insufficient points'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001', customerId: 'cust_001', points: 9999 }, allowApply: true }),
      /insufficient points/,
    );
  });
});

// ============================================================================
// list_rewards
// ============================================================================

describe('list_rewards', () => {
  const tool = findTool(loyaltyTools, 'list_rewards');

  it('returns rewards for a program', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: { programId: 'lp_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.programId, 'lp_001');
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.rewards));
    assert.equal(result.rewards[0].id, 'rw_001');
    assert.equal(result.rewards[0].name, 'Free Shipping');
    assert.equal(result.rewards[0].pointsCost, 500);
  });

  it('passes programId and tier filter to commerce.loyalty.listRewards', async () => {
    let calledProgramId, calledOpts;
    const commerce = makeLoyaltyCommerce({
      listRewards: async (pid, opts) => {
        calledProgramId = pid;
        calledOpts = opts;
        return [];
      },
    });
    await tool.handler({ commerce, params: { programId: 'lp_001', tier: 'Gold' } });
    assert.equal(calledProgramId, 'lp_001');
    assert.equal(calledOpts.tier, 'Gold');
  });

  it('slices results to limit', async () => {
    const manyRewards = Array.from({ length: 15 }, (_, i) => ({
      ...mockReward,
      id: `rw_${String(i).padStart(3, '0')}`,
    }));
    const commerce = makeLoyaltyCommerce({
      listRewards: async () => manyRewards,
    });
    const result = await tool.handler({ commerce, params: { programId: 'lp_001', limit: 4 } });
    assert.equal(result.returned, 4);
    assert.equal(result.rewards.length, 4);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      listRewards: async () => { throw new Error('rewards query failed'); },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { programId: 'lp_001' } }),
      /rewards query failed/,
    );
  });
});

// ============================================================================
// create_reward
// ============================================================================

describe('create_reward', () => {
  const tool = findTool(loyaltyTools, 'create_reward');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: {
        programId: 'lp_001',
        name: 'Free Shipping',
        pointsCost: 500,
        type: 'free_shipping',
        value: 0,
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
    assert.ok(result.hint);
  });

  it('creates reward with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeLoyaltyCommerce(),
      params: {
        programId: 'lp_001',
        name: 'Free Shipping',
        pointsCost: 500,
        type: 'free_shipping',
        value: 0,
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.reward);
    assert.equal(result.reward.name, 'Free Shipping');
  });

  it('passes all fields to commerce.loyalty.createReward', async () => {
    let calledProgramId, calledData;
    const commerce = makeLoyaltyCommerce({
      createReward: async (pid, data) => {
        calledProgramId = pid;
        calledData = data;
        return { ...mockReward, programId: pid, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        programId: 'lp_001',
        name: '10% Off',
        description: 'Ten percent off your order',
        pointsCost: 1000,
        type: 'discount_percentage',
        value: 10,
        tier: 'Gold',
        maxRedemptions: 100,
        stock: 50,
      },
      allowApply: true,
    });
    assert.equal(calledProgramId, 'lp_001');
    assert.equal(calledData.name, '10% Off');
    assert.equal(calledData.pointsCost, 1000);
    assert.equal(calledData.type, 'discount_percentage');
    assert.equal(calledData.value, '10');
    assert.equal(calledData.tier, 'Gold');
    assert.equal(calledData.maxRedemptions, 100);
    assert.equal(calledData.stock, 50);
  });

  it('converts numeric value to string before passing to commerce', async () => {
    let calledData;
    const commerce = makeLoyaltyCommerce({
      createReward: async (_pid, data) => {
        calledData = data;
        return { ...mockReward, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { programId: 'lp_001', name: 'Gift Card $25', pointsCost: 2500, type: 'gift_card', value: 25 },
      allowApply: true,
    });
    assert.equal(typeof calledData.value, 'string');
    assert.equal(calledData.value, '25');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeLoyaltyCommerce({
      createReward: async () => { throw new Error('reward creation failed'); },
    });
    await assert.rejects(
      () => tool.handler({
        commerce,
        params: { programId: 'lp_001', name: 'Test', pointsCost: 100, type: 'free_shipping', value: 0 },
        allowApply: true,
      }),
      /reward creation failed/,
    );
  });
});
