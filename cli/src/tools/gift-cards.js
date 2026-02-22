/**
 * Gift Card Tools Module
 *
 * MCP tool definitions for gift card creation, redemption, and balance management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Gift card tool definitions
 */
export const giftCardTools = [
  {
    name: 'create_gift_card',
    description: 'Create a new gift card with an initial balance.',
    inputSchema: {
      initialBalance: z.number().positive().describe('Initial balance in store currency'),
      currency: z
        .string()
        .min(1)
        .max(10)
        .optional()
        .default('USD')
        .describe('Currency code (default: USD)'),
      customerId: z
        .string()
        .min(1)
        .optional()
        .describe('Customer ID to associate the gift card with'),
      recipientEmail: z
        .string()
        .email()
        .optional()
        .describe('Recipient email for digital delivery'),
      recipientName: z.string().min(1).max(200).optional().describe('Recipient name'),
      message: z.string().max(500).optional().describe('Personal message to include'),
      expiresAt: z.string().optional().describe('Expiration date (ISO 8601)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create gift card', params);
      }

      const giftCard = await commerce.giftCards.create({
        initialBalance: String(params.initialBalance),
        currency: params.currency || 'USD',
        customerId: params.customerId,
        recipientEmail: params.recipientEmail,
        recipientName: params.recipientName,
        message: params.message,
        expiresAt: params.expiresAt,
      });
      return { success: true, message: 'Gift card created', giftCard };
    },
  },

  {
    name: 'get_gift_card',
    description: 'Get a gift card by ID or code.',
    inputSchema: {
      identifier: z.string().min(1).describe('Gift card ID (UUID) or redemption code'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      const giftCard = await commerce.giftCards.get(identifier);

      if (!giftCard) {
        return { success: false, error: 'Gift card not found' };
      }

      return {
        success: true,
        giftCard: {
          id: giftCard.id,
          code: giftCard.code,
          initialBalance: giftCard.initialBalance,
          currentBalance: giftCard.currentBalance,
          currency: giftCard.currency,
          status: giftCard.status,
          customerId: giftCard.customerId,
          expiresAt: giftCard.expiresAt,
          createdAt: giftCard.createdAt,
        },
      };
    },
  },

  {
    name: 'list_gift_cards',
    description: 'List all gift cards with optional filters.',
    inputSchema: {
      status: z
        .enum(['active', 'disabled', 'expired', 'fully_redeemed'])
        .optional()
        .describe('Filter by status'),
      customerId: z.string().min(1).optional().describe('Filter by customer ID'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of gift cards to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { status, customerId, limit } = params;
      const giftCards = await commerce.giftCards.list({ status, customerId });
      const count = await commerce.giftCards.count({ status, customerId });
      const limited = giftCards.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limited.length,
        giftCards: limited.map((gc) => ({
          id: gc.id,
          code: gc.code,
          initialBalance: gc.initialBalance,
          currentBalance: gc.currentBalance,
          currency: gc.currency,
          status: gc.status,
          customerId: gc.customerId,
          expiresAt: gc.expiresAt,
          createdAt: gc.createdAt,
        })),
      };
    },
  },

  {
    name: 'charge_gift_card',
    description: 'Charge (deduct) an amount from a gift card balance.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
      amount: z.number().positive().describe('Amount to charge'),
      orderId: z.string().min(1).optional().describe('Order ID for the charge'),
      note: z.string().max(500).optional().describe('Note for the transaction'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Charge gift card', params);
      }

      const transaction = await commerce.giftCards.charge({
        giftCardId: params.giftCardId,
        amount: String(params.amount),
        orderId: params.orderId,
        note: params.note,
      });
      return { success: true, message: 'Gift card charged', transaction };
    },
  },

  {
    name: 'refund_to_gift_card',
    description: 'Refund an amount back to a gift card.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
      amount: z.number().positive().describe('Amount to refund'),
      orderId: z.string().min(1).optional().describe('Order ID associated with the refund'),
      reason: z.string().max(500).optional().describe('Reason for the refund'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Refund to gift card', params);
      }

      const transaction = await commerce.giftCards.refund({
        giftCardId: params.giftCardId,
        amount: String(params.amount),
        orderId: params.orderId,
        reason: params.reason,
      });
      return { success: true, message: 'Refund applied to gift card', transaction };
    },
  },

  {
    name: 'disable_gift_card',
    description: 'Disable a gift card so it can no longer be used.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
      reason: z.string().max(500).optional().describe('Reason for disabling'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Disable gift card', params);
      }

      const giftCard = await commerce.giftCards.disable(params.giftCardId, params.reason);
      return { success: true, message: 'Gift card disabled', giftCard };
    },
  },

  {
    name: 'check_gift_card_balance',
    description: 'Check the current balance of a gift card by ID or code.',
    inputSchema: {
      identifier: z.string().min(1).describe('Gift card ID (UUID) or redemption code'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      const giftCard = await commerce.giftCards.get(identifier);

      if (!giftCard) {
        return { success: false, error: 'Gift card not found' };
      }

      return {
        success: true,
        giftCardId: giftCard.id,
        code: giftCard.code,
        currentBalance: giftCard.currentBalance,
        currency: giftCard.currency,
        status: giftCard.status,
      };
    },
  },
];

export default giftCardTools;
