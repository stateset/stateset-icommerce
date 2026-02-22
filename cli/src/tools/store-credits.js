/**
 * Store Credit Tools Module
 *
 * MCP tool definitions for store credit issuance, adjustment, and application.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Store credit tool definitions
 */
export const storeCreditTools = [
  {
    name: 'create_store_credit',
    description: 'Issue store credit to a customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      amount: z.number().positive().describe('Credit amount'),
      currency: z
        .string()
        .min(1)
        .max(10)
        .optional()
        .default('USD')
        .describe('Currency code (default: USD)'),
      reason: z
        .enum(['refund', 'goodwill', 'promotion', 'return', 'loyalty', 'other'])
        .optional()
        .describe('Reason for issuing credit'),
      note: z.string().max(500).optional().describe('Internal note'),
      expiresAt: z.string().optional().describe('Expiration date (ISO 8601)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create store credit', params);
      }

      const credit = await commerce.storeCredits.create({
        customerId: params.customerId,
        amount: String(params.amount),
        currency: params.currency || 'USD',
        reason: params.reason || 'other',
        note: params.note,
        expiresAt: params.expiresAt,
      });
      return { success: true, message: 'Store credit issued', credit };
    },
  },

  {
    name: 'get_store_credit',
    description: 'Get store credit details by ID.',
    inputSchema: {
      creditId: z.string().min(1).describe('Store credit ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { creditId } = params;
      const credit = await commerce.storeCredits.get(creditId);

      if (!credit) {
        return { success: false, error: 'Store credit not found' };
      }

      return {
        success: true,
        credit: {
          id: credit.id,
          customerId: credit.customerId,
          originalAmount: credit.originalAmount,
          currentBalance: credit.currentBalance,
          currency: credit.currency,
          reason: credit.reason,
          status: credit.status,
          expiresAt: credit.expiresAt,
          createdAt: credit.createdAt,
          updatedAt: credit.updatedAt,
        },
      };
    },
  },

  {
    name: 'list_store_credits',
    description: 'List store credits with optional filters.',
    inputSchema: {
      customerId: z.string().min(1).optional().describe('Filter by customer ID'),
      status: z.enum(['active', 'expired', 'fully_used']).optional().describe('Filter by status'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of credits to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { customerId, status, limit } = params;
      const credits = await commerce.storeCredits.list({ customerId, status });
      const count = await commerce.storeCredits.count({ customerId, status });
      const limited = credits.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limited.length,
        credits: limited.map((c) => ({
          id: c.id,
          customerId: c.customerId,
          originalAmount: c.originalAmount,
          currentBalance: c.currentBalance,
          currency: c.currency,
          reason: c.reason,
          status: c.status,
          expiresAt: c.expiresAt,
          createdAt: c.createdAt,
        })),
      };
    },
  },

  {
    name: 'adjust_store_credit',
    description: 'Adjust a store credit balance (add or subtract).',
    inputSchema: {
      creditId: z.string().min(1).describe('Store credit ID'),
      amount: z.number().describe('Adjustment amount (positive to add, negative to subtract)'),
      reason: z.string().min(1).max(500).describe('Reason for the adjustment'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Adjust store credit', params);
      }

      const credit = await commerce.storeCredits.adjust({
        creditId: params.creditId,
        amount: String(params.amount),
        reason: params.reason,
      });
      return { success: true, message: 'Store credit adjusted', credit };
    },
  },

  {
    name: 'apply_store_credit',
    description: 'Apply store credit to an order.',
    inputSchema: {
      creditId: z.string().min(1).describe('Store credit ID'),
      orderId: z.string().min(1).describe('Order ID to apply credit to'),
      amount: z.number().positive().describe('Amount of credit to apply'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Apply store credit', params);
      }

      const transaction = await commerce.storeCredits.apply({
        creditId: params.creditId,
        orderId: params.orderId,
        amount: String(params.amount),
      });
      return { success: true, message: 'Store credit applied to order', transaction };
    },
  },
];

export default storeCreditTools;
