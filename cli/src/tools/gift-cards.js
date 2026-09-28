/**
 * Gift Card Tools Module
 *
 * MCP tool definitions for gift card creation, redemption, and balance management.
 * Every call here targets the real `GiftCards` class of `@stateset/embedded`:
 * `create`, `get`, `getByCode`, `list`, `charge(id, amount, referenceId?)`,
 * `refund(id, amount, referenceId?)`, `disable(id)`. Money crosses the binding
 * as exact decimal strings.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Page size used to count every matching row; below the engine's list cap. */
const LIST_PAGE_SIZE = 500;

/**
 * Resolve a gift card from an ID (UUID) or a redemption code.
 * @param {any} commerce
 * @param {string} identifier
 */
async function findGiftCard(commerce, identifier) {
  if (UUID_RE.test(identifier)) {
    const byId = await commerce.giftCards.get(identifier);
    if (byId) return byId;
  }
  return commerce.giftCards.getByCode(identifier);
}

/**
 * Read every gift card matching `filter`, page by page, so the reported total
 * is the real total rather than whatever one page returned.
 * @param {any} commerce
 * @param {object} filter
 */
async function listAllGiftCards(commerce, filter) {
  const all = [];
  for (let offset = 0; ; offset += LIST_PAGE_SIZE) {
    const page = await commerce.giftCards.list({ ...filter, limit: LIST_PAGE_SIZE, offset });
    all.push(...page);
    if (page.length < LIST_PAGE_SIZE) return all;
  }
}

function summarizeGiftCard(giftCard) {
  return {
    id: giftCard.id,
    code: giftCard.code,
    initialBalance: giftCard.initialBalance,
    currentBalance: giftCard.currentBalance,
    currency: giftCard.currency,
    status: giftCard.status,
    recipientEmail: giftCard.recipientEmail,
    expiresAt: giftCard.expiresAt,
    createdAt: giftCard.createdAt,
  };
}

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
      code: z
        .string()
        .min(1)
        .max(100)
        .optional()
        .describe('Redemption code (auto-generated if omitted)'),
      recipientEmail: z
        .string()
        .email()
        .optional()
        .describe('Recipient email for digital delivery'),
      senderName: z.string().min(1).max(200).optional().describe('Sender name'),
      message: z.string().max(500).optional().describe('Personal message to include'),
      expiresAt: z.string().optional().describe('Expiration date (RFC 3339)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create gift card', params);
      }

      const giftCard = await commerce.giftCards.create({
        code: params.code,
        initialBalance: String(params.initialBalance),
        currency: params.currency || 'USD',
        recipientEmail: params.recipientEmail,
        senderName: params.senderName,
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
      const giftCard = await findGiftCard(commerce, params.identifier);

      if (!giftCard) {
        return { success: false, error: 'Gift card not found' };
      }

      return { success: true, giftCard: summarizeGiftCard(giftCard) };
    },
  },

  {
    name: 'list_gift_cards',
    description: 'List all gift cards with optional filters.',
    inputSchema: {
      status: z
        .enum(['active', 'depleted', 'expired', 'disabled'])
        .optional()
        .describe('Filter by status'),
      code: z.string().min(1).optional().describe('Filter by redemption code'),
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
      const { status, code, limit } = params;
      const giftCards = await listAllGiftCards(commerce, { status, code });
      const limited = giftCards.slice(0, limit);

      return {
        success: true,
        totalCount: giftCards.length,
        returned: limited.length,
        giftCards: limited.map(summarizeGiftCard),
      };
    },
  },

  {
    name: 'charge_gift_card',
    description: 'Charge (deduct) an amount from a gift card balance.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
      amount: z.number().positive().describe('Amount to charge'),
      orderId: z
        .string()
        .min(1)
        .optional()
        .describe('Order ID for the charge (recorded as the transaction reference)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Charge gift card', params);
      }

      const transaction = await commerce.giftCards.charge(
        params.giftCardId,
        String(params.amount),
        params.orderId,
      );
      return { success: true, message: 'Gift card charged', transaction };
    },
  },

  {
    name: 'refund_to_gift_card',
    description: 'Refund an amount back to a gift card.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
      amount: z.number().positive().describe('Amount to refund'),
      orderId: z
        .string()
        .min(1)
        .optional()
        .describe('Order ID associated with the refund (recorded as the transaction reference)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Refund to gift card', params);
      }

      const transaction = await commerce.giftCards.refund(
        params.giftCardId,
        String(params.amount),
        params.orderId,
      );
      return { success: true, message: 'Refund applied to gift card', transaction };
    },
  },

  {
    name: 'disable_gift_card',
    description: 'Disable a gift card so it can no longer be used.',
    inputSchema: {
      giftCardId: z.string().min(1).describe('Gift card ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Disable gift card', params);
      }

      const giftCard = await commerce.giftCards.disable(params.giftCardId);
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
      const giftCard = await findGiftCard(commerce, params.identifier);

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
