/**
 * Payment Tools Module
 *
 * MCP tool definitions for payment processing and refund operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Payment tool definitions
 */
export const paymentTools = [
  {
    name: 'list_payments',
    description: 'List all payments in the system.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const payments = await commerce.payments.list();
      const count = await commerce.payments.count();
      return { success: true, count, payments };
    },
  },

  {
    name: 'get_payment',
    description: 'Get a payment by ID.',
    inputSchema: {
      paymentId: z.string().describe('Payment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { paymentId } = params;
      const payment = await commerce.payments.get(paymentId);
      return { success: true, payment };
    },
  },

  {
    name: 'create_payment',
    description: 'Create a payment for an order.',
    inputSchema: {
      orderId: z.string().describe('Order ID'),
      amount: z.number().describe('Payment amount'),
      currency: z.string().optional().describe('Currency (default: USD)'),
      method: z
        .string()
        .optional()
        .describe('Payment method: credit_card, paypal, bank_transfer, crypto'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Create payment requires --apply flag.',
          wouldCreate: params,
        };
      }

      const payment = await commerce.payments.create({
        orderId: params.orderId,
        amount: String(params.amount),
        currency: params.currency || 'USD',
        method: params.method || 'credit_card',
      });
      return { success: true, message: 'Payment created', payment };
    },
  },

  {
    name: 'complete_payment',
    description: 'Mark a payment as completed.',
    inputSchema: {
      paymentId: z.string().describe('Payment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { paymentId } = params;
      if (!allowApply) {
        return { error: 'Complete payment requires --apply flag.' };
      }

      const payment = await commerce.payments.markCompleted(paymentId);
      return { success: true, message: 'Payment completed', payment };
    },
  },

  {
    name: 'create_refund',
    description: 'Create a refund for a payment.',
    inputSchema: {
      paymentId: z.string().describe('Payment ID to refund'),
      amount: z.number().describe('Refund amount'),
      reason: z.string().optional().describe('Refund reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { error: 'Create refund requires --apply flag.' };
      }

      const refund = await commerce.payments.createRefund({
        paymentId: params.paymentId,
        amount: String(params.amount),
        reason: params.reason,
      });
      return { success: true, message: 'Refund created', refund };
    },
  },
];

export default paymentTools;
