/**
 * Loyalty Program Tools Module
 *
 * MCP tool definitions for loyalty programs, points, and rewards management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Loyalty tool definitions
 */
export const loyaltyTools = [
  {
    name: 'create_loyalty_program',
    description: 'Create a loyalty program with tiers and earning rules.',
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
      currency: z.string().min(1).max(10).optional().default('USD').describe('Currency code'),
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
            perks: z.array(z.string().max(200)).optional().describe('Tier perks/benefits'),
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
        pointsPerDollar: params.pointsPerDollar || 1,
        currency: params.currency || 'USD',
        tiers: params.tiers,
      });
      return { success: true, message: 'Loyalty program created', program };
    },
  },

  {
    name: 'get_loyalty_program',
    description: 'Get loyalty program details including tiers and reward catalog.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { programId } = params;
      const program = await commerce.loyalty.getProgram(programId);

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
          currency: program.currency,
          tiers: program.tiers,
          totalMembers: program.totalMembers,
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

      const account = await commerce.loyalty.enrollCustomer(params.programId, params.customerId);
      return { success: true, message: 'Customer enrolled in loyalty program', account };
    },
  },

  {
    name: 'get_loyalty_account',
    description: 'Get a customer loyalty account including points balance and tier.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { programId, customerId } = params;
      const account = await commerce.loyalty.getAccount(programId, customerId);

      if (!account) {
        return { success: false, error: 'Loyalty account not found' };
      }

      return {
        success: true,
        account: {
          id: account.id,
          programId: account.programId,
          customerId: account.customerId,
          pointsBalance: account.pointsBalance,
          lifetimePoints: account.lifetimePoints,
          currentTier: account.currentTier,
          nextTier: account.nextTier,
          pointsToNextTier: account.pointsToNextTier,
          enrolledAt: account.enrolledAt,
          updatedAt: account.updatedAt,
        },
      };
    },
  },

  {
    name: 'earn_points',
    description: 'Award loyalty points to a customer account.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      points: z.number().int().positive().describe('Number of points to award'),
      reason: z
        .enum(['purchase', 'referral', 'birthday', 'review', 'promotion', 'manual'])
        .optional()
        .describe('Reason for earning points'),
      orderId: z.string().min(1).optional().describe('Associated order ID'),
      note: z.string().max(500).optional().describe('Note for the transaction'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Award loyalty points', params);
      }

      const transaction = await commerce.loyalty.earnPoints({
        programId: params.programId,
        customerId: params.customerId,
        points: params.points,
        reason: params.reason || 'manual',
        orderId: params.orderId,
        note: params.note,
      });
      return { success: true, message: `${params.points} points awarded`, transaction };
    },
  },

  {
    name: 'redeem_points',
    description: 'Redeem loyalty points for a reward or discount.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      points: z.number().int().positive().describe('Number of points to redeem'),
      rewardId: z.string().min(1).optional().describe('Reward ID to redeem for'),
      orderId: z.string().min(1).optional().describe('Order ID to apply discount to'),
      note: z.string().max(500).optional().describe('Note for the transaction'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Redeem loyalty points', params);
      }

      const transaction = await commerce.loyalty.redeemPoints({
        programId: params.programId,
        customerId: params.customerId,
        points: params.points,
        rewardId: params.rewardId,
        orderId: params.orderId,
        note: params.note,
      });
      return { success: true, message: `${params.points} points redeemed`, transaction };
    },
  },

  {
    name: 'list_rewards',
    description: 'List available rewards in a loyalty program.',
    inputSchema: {
      programId: z.string().min(1).describe('Loyalty program ID'),
      tier: z.string().min(1).optional().describe('Filter by tier name'),
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
      const { programId, tier, limit } = params;
      const rewards = await commerce.loyalty.listRewards(programId, { tier });
      const limited = rewards.slice(0, limit);

      return {
        success: true,
        programId,
        returned: limited.length,
        rewards: limited.map((r) => ({
          id: r.id,
          name: r.name,
          description: r.description,
          pointsCost: r.pointsCost,
          type: r.type,
          value: r.value,
          tier: r.tier,
          status: r.status,
          remainingStock: r.remainingStock,
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
      type: z
        .enum([
          'discount_percentage',
          'discount_fixed',
          'free_product',
          'free_shipping',
          'gift_card',
        ])
        .describe('Reward type'),
      value: z
        .number()
        .positive()
        .describe('Reward value (percentage or fixed amount depending on type)'),
      tier: z.string().min(1).optional().describe('Minimum tier required (if any)'),
      maxRedemptions: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Maximum total redemptions allowed'),
      stock: z.number().int().min(0).optional().describe('Available stock (null for unlimited)'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create reward', params);
      }

      const reward = await commerce.loyalty.createReward(params.programId, {
        name: params.name,
        description: params.description,
        pointsCost: params.pointsCost,
        type: params.type,
        value: String(params.value),
        tier: params.tier,
        maxRedemptions: params.maxRedemptions,
        stock: params.stock,
      });
      return { success: true, message: 'Reward created', reward };
    },
  },
];

export default loyaltyTools;
