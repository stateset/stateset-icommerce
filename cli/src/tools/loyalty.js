/**
 * Loyalty Program Tools Module
 *
 * MCP tool definitions for loyalty programs, points, and rewards management.
 *
 * Points are integers. A customer's points live on a loyalty account (one per
 * customer per program); every earn/redeem is an `adjustPoints` transaction on
 * that account, and the engine refuses any adjustment that would take the
 * balance below zero.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/** Every `LoyaltyRewardType` the binding accepts. */
const REWARD_TYPES = [
  'discount',
  'free_shipping',
  'free_product',
  'store_credit',
  'exclusive_access',
];

/** Exact decimal amount, as a string (money is never a float). */
const decimalString = z
  .string()
  .regex(/^\d+(\.\d+)?$/, 'Must be a non-negative decimal string, e.g. "10.00"');

/** Shape a `LoyaltyAccountOutput` for a tool response. */
function formatAccount(account) {
  return {
    id: account.id,
    programId: account.programId,
    customerId: account.customerId,
    pointsBalance: account.pointsBalance,
    lifetimePoints: account.lifetimePoints,
    tier: account.tier,
    createdAt: account.createdAt,
    updatedAt: account.updatedAt,
  };
}

/** The customer's account in the program, or null when they are not enrolled. */
function findAccount(commerce, { programId, customerId }) {
  return commerce.loyalty.getAccountByCustomer(customerId, programId);
}

const NOT_ENROLLED = { success: false, error: 'Customer is not enrolled in this loyalty program' };

/**
 * Loyalty tool definitions
 */
export const loyaltyTools = [
  {
    name: 'create_loyalty_program',
    description: 'Create a loyalty program with an earning rate and optional tiers.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Program name'),
      description: z.string().max(1000).optional().describe('Program description'),
      pointsPerDollar: z
        .number()
        .int()
        .positive()
        .optional()
        .default(1)
        .describe('Points earned per dollar spent'),
      tiers: z
        .array(
          z.object({
            name: z.string().min(1).max(100).describe('Tier name (e.g., Bronze, Silver, Gold)'),
            minPoints: z.number().int().min(0).describe('Minimum points to reach this tier'),
            multiplier: z
              .number()
              .positive()
              .optional()
              .default(1)
              .describe('Points earning multiplier for this tier'),
            perks: z
              .array(z.string().max(200))
              .optional()
              .default([])
              .describe('Tier perks/benefits'),
          }),
        )
        .min(1)
        .max(10)
        .optional()
        .describe('Loyalty tiers'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create loyalty program', params);
      }

      const program = await commerce.loyalty.createProgram({
        name: params.name,
        description: params.description,
        pointsPerDollar: params.pointsPerDollar ?? 1,
        tiers: params.tiers?.map((t) => ({
          name: t.name,
          minPoints: t.minPoints,
          multiplier: t.multiplier ?? 1,
          perks: t.perks ?? [],
        })),
      });
      return { success: true, message: 'Loyalty program created', program };
    },
  },

  {
    name: 'get_loyalty_program',
    description: 'Get loyalty program details including its tiers.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const program = await commerce.loyalty.getProgram(params.programId);

      if (!program) {
        return { success: false, error: 'Loyalty program not found' };
      }

      return {
        success: true,
        program: {
          id: program.id,
          name: program.name,
          description: program.description,
          pointsPerDollar: program.pointsPerDollar,
          tiers: program.tiers,
          status: program.status,
          createdAt: program.createdAt,
          updatedAt: program.updatedAt,
        },
      };
    },
  },

  {
    name: 'enroll_customer',
    description: 'Enroll a customer in a loyalty program.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID to enroll'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Enroll customer in loyalty program', params);
      }

      const account = await commerce.loyalty.enroll({
        customerId: params.customerId,
        programId: params.programId,
      });
      return {
        success: true,
        message: 'Customer enrolled in loyalty program',
        account: formatAccount(account),
      };
    },
  },

  {
    name: 'get_loyalty_account',
    description: "Get a customer's loyalty account in a program: points balance and tier.",
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const account = await findAccount(commerce, params);

      if (!account) {
        return { success: false, error: 'Loyalty account not found' };
      }

      return { success: true, account: formatAccount(account) };
    },
  },

  {
    name: 'earn_points',
    description: "Award loyalty points to a customer's account in a program.",
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      points: z.number().int().positive().describe('Number of points to award'),
      reason: z
        .enum(['purchase', 'referral', 'birthday', 'review', 'promotion', 'manual'])
        .optional()
        .describe('Reason for earning points (recorded in the transaction description)'),
      orderId: z
        .string()
        .min(1)
        .optional()
        .describe('Associated order ID (recorded as the reference)'),
      note: z.string().max(500).optional().describe('Note for the transaction'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Award loyalty points', params);
      }

      const account = await findAccount(commerce, params);
      if (!account) return NOT_ENROLLED;

      const reason = params.reason || 'manual';
      const transaction = await commerce.loyalty.adjustPoints({
        accountId: account.id,
        points: params.points,
        transactionType: 'earn',
        referenceId: params.orderId,
        description: params.note ? `${reason}: ${params.note}` : reason,
      });
      return { success: true, message: `${params.points} points awarded`, transaction };
    },
  },

  {
    name: 'redeem_points',
    description:
      "Redeem loyalty points from a customer's account, optionally for a reward in the program. " +
      'The engine refuses a redemption larger than the balance.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      points: z.number().int().positive().describe('Number of points to redeem'),
      rewardId: z.string().min(1).optional().describe('Reward ID being redeemed for'),
      orderId: z
        .string()
        .min(1)
        .optional()
        .describe('Order the redemption applies to (recorded as the reference)'),
      note: z.string().max(500).optional().describe('Note for the transaction'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Redeem loyalty points', params);
      }

      const account = await findAccount(commerce, params);
      if (!account) return NOT_ENROLLED;

      let reward = null;
      if (params.rewardId) {
        reward = await commerce.loyalty.getReward(params.rewardId);
        if (!reward || reward.programId !== params.programId) {
          return { success: false, error: 'Reward not found in this loyalty program' };
        }
        if (!reward.isActive) {
          return { success: false, error: 'Reward is not active' };
        }
      }

      const parts = [];
      if (reward) parts.push(`reward ${reward.id} (${reward.name})`);
      if (params.note) parts.push(params.note);

      const transaction = await commerce.loyalty.adjustPoints({
        accountId: account.id,
        points: -params.points,
        transactionType: 'redeem',
        referenceId: params.orderId ?? reward?.id,
        description: parts.length ? parts.join(': ') : undefined,
      });
      return { success: true, message: `${params.points} points redeemed`, transaction };
    },
  },

  {
    name: 'list_rewards',
    description: 'List rewards in a loyalty program.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      rewardType: z.enum(REWARD_TYPES).optional().describe('Filter by reward type'),
      activeOnly: z.boolean().optional().describe('Only active rewards'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(20)
        .describe('Maximum number of rewards to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { programId, rewardType, activeOnly } = params;
      const rewards = await commerce.loyalty.listRewards({
        programId,
        rewardType,
        isActive: activeOnly ? true : undefined,
        limit: params.limit ?? 20,
      });

      return {
        success: true,
        programId,
        returned: rewards.length,
        rewards: rewards.map((r) => ({
          id: r.id,
          name: r.name,
          description: r.description,
          pointsCost: r.pointsCost,
          rewardType: r.rewardType,
          value: r.value,
          isActive: r.isActive,
        })),
      };
    },
  },

  {
    name: 'create_reward',
    description: 'Create a redeemable reward in a loyalty program.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      name: z.string().min(1).max(255).describe('Reward name'),
      description: z.string().max(1000).optional().describe('Reward description'),
      pointsCost: z.number().int().positive().describe('Points required to redeem'),
      rewardType: z.enum(REWARD_TYPES).describe('Reward type'),
      value: decimalString
        .optional()
        .describe('Monetary value as an exact decimal string, e.g. "10.00" (optional)'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create reward', params);
      }

      const reward = await commerce.loyalty.createReward({
        programId: params.programId,
        name: params.name,
        description: params.description,
        pointsCost: params.pointsCost,
        rewardType: params.rewardType,
        value: params.value,
      });
      return { success: true, message: 'Reward created', reward };
    },
  },
];

export default loyaltyTools;
