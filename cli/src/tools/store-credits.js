/**
 * Store Credit Tools Module
 *
 * MCP tool definitions for store credit issuance, adjustment, and application.
 * Every call here targets the real `StoreCredits` class of `@stateset/embedded`:
 * `create`, `get`, `list`, `adjust(id, { amount, note, referenceId })`,
 * `apply(id, amount, referenceId?)`. Money crosses the binding as exact
 * decimal strings.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const STORE_CREDIT_STATUSES = ['active', 'depleted', 'expired', 'voided'];
const STORE_CREDIT_REASONS = [
  'return',
  'loyalty',
  'compensation',
  'promotion',
  'manual',
  'gift_card',
];

/** Page size used to count every matching row; below the engine's list cap. */
const LIST_PAGE_SIZE = 500;

/**
 * Read every store credit matching `filter`, page by page, so the reported
 * total is the real total rather than whatever one page returned.
 * @param {any} commerce
 * @param {object} filter
 */
async function listAllStoreCredits(commerce, filter) {
  const all = [];
  for (let offset = 0; ; offset += LIST_PAGE_SIZE) {
    const page = await commerce.storeCredits.list({ ...filter, limit: LIST_PAGE_SIZE, offset });
    all.push(...page);
    if (page.length < LIST_PAGE_SIZE) return all;
  }
}

function summarizeCredit(credit) {
  return {
    id: credit.id,
    customerId: credit.customerId,
    originalBalance: credit.originalBalance,
    currentBalance: credit.currentBalance,
    currency: credit.currency,
    reason: credit.reason,
    status: credit.status,
    referenceId: credit.referenceId,
    note: credit.note,
    expiresAt: credit.expiresAt,
    createdAt: credit.createdAt,
    updatedAt: credit.updatedAt,
  };
}

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
        .enum(STORE_CREDIT_REASONS)
        .optional()
        .describe('Reason for issuing credit (engine default: return)'),
      referenceId: z
        .string()
        .min(1)
        .optional()
        .describe('Source reference (e.g. the return or order ID)'),
      note: z.string().max(500).optional().describe('Internal note'),
      expiresAt: z.string().optional().describe('Expiration date (RFC 3339)'),
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
        reason: params.reason,
        referenceId: params.referenceId,
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
      const credit = await commerce.storeCredits.get(params.creditId);

      if (!credit) {
        return { success: false, error: 'Store credit not found' };
      }

      return { success: true, credit: summarizeCredit(credit) };
    },
  },

  {
    name: 'list_store_credits',
    description: 'List store credits with optional filters.',
    inputSchema: {
      customerId: z.string().min(1).optional().describe('Filter by customer ID'),
      status: z.enum(STORE_CREDIT_STATUSES).optional().describe('Filter by status'),
      reason: z.enum(STORE_CREDIT_REASONS).optional().describe('Filter by reason'),
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
      const { customerId, status, reason, limit } = params;
      const credits = await listAllStoreCredits(commerce, { customerId, status, reason });
      const limited = credits.slice(0, limit);

      return {
        success: true,
        totalCount: credits.length,
        returned: limited.length,
        credits: limited.map(summarizeCredit),
      };
    },
  },

  {
    name: 'adjust_store_credit',
    description: 'Adjust a store credit balance (add or subtract).',
    inputSchema: {
      creditId: z.string().min(1).describe('Store credit ID'),
      amount: z.number().describe('Adjustment amount (positive to add, negative to subtract)'),
      reason: z
        .string()
        .min(1)
        .max(500)
        .describe('Reason for the adjustment (recorded as the ledger note)'),
      referenceId: z.string().min(1).optional().describe('Reference for the adjustment'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Adjust store credit', params);
      }

      const credit = await commerce.storeCredits.adjust(params.creditId, {
        amount: String(params.amount),
        note: params.reason,
        referenceId: params.referenceId,
      });
      return { success: true, message: 'Store credit adjusted', credit };
    },
  },

  {
    name: 'apply_store_credit',
    description: 'Apply store credit to an order.',
    inputSchema: {
      creditId: z.string().min(1).describe('Store credit ID'),
      orderId: z
        .string()
        .min(1)
        .describe('Order ID to apply credit to (recorded as the transaction reference)'),
      amount: z.number().positive().describe('Amount of credit to apply'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Apply store credit', params);
      }

      const transaction = await commerce.storeCredits.apply(
        params.creditId,
        String(params.amount),
        params.orderId,
      );
      return { success: true, message: 'Store credit applied to order', transaction };
    },
  },
];

export default storeCreditTools;
