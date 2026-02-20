/**
 * Returns Tools Module
 *
 * MCP tool definitions for return/RMA operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

export const returnTools = [
  {
    name: 'list_returns',
    description: 'List all returns. Shows return status, order, and reason.',
    inputSchema: {
      limit: z.number().optional().default(50).describe('Maximum number of returns to show'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { limit } = params;
      const returns = await commerce.returns.list();
      const count = await commerce.returns.count();
      const limitedReturns = returns.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limitedReturns.length,
        returns: limitedReturns.map((r) => ({
          id: r.id,
          orderId: r.orderId,
          status: r.status,
          reason: r.reason,
          createdAt: r.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_return',
    description: 'Get a specific return by ID.',
    inputSchema: {
      returnId: z.string().describe('Return ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { returnId } = params;
      const ret = await commerce.returns.get(returnId);

      if (!ret) {
        return { success: false, error: 'Return not found' };
      }

      return { success: true, return: ret };
    },
  },

  {
    name: 'create_return',
    description: 'Create a return request for an order.',
    inputSchema: {
      orderId: z.string().describe('Order ID (UUID)'),
      reason: z
        .enum([
          'defective',
          'wrong_item',
          'not_as_described',
          'changed_mind',
          'better_price_found',
          'no_longer_needed',
          'damaged',
          'other',
        ])
        .describe('Return reason'),
      reasonDetails: z.string().optional().describe('Additional details about the return reason'),
      items: z
        .array(
          z.object({
            orderItemId: z.string().describe('Order item ID to return'),
            quantity: z.number().describe('Quantity to return'),
          }),
        )
        .describe('Items to return'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      }

      const ret = await commerce.returns.create(params);
      return {
        success: true,
        message: 'Return created successfully',
        return: {
          id: ret.id,
          orderId: ret.orderId,
          status: ret.status,
          reason: ret.reason,
        },
      };
    },
  },

  {
    name: 'approve_return',
    description: 'Approve a return request.',
    inputSchema: {
      returnId: z.string().describe('Return ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { returnId } = params;

      if (!allowApply) {
        return {
          success: false,
          error: 'Approve operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldApprove: { returnId },
        };
      }

      const ret = await commerce.returns.approve(returnId);
      return {
        success: true,
        message: 'Return approved',
        return: { id: ret.id, status: ret.status },
      };
    },
  },

  {
    name: 'reject_return',
    description: 'Reject a return request with a reason.',
    inputSchema: {
      returnId: z.string().describe('Return ID (UUID)'),
      reason: z.string().describe('Reason for rejection'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { returnId, reason } = params;

      if (!allowApply) {
        return {
          success: false,
          error: 'Reject operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldReject: { returnId, reason },
        };
      }

      const ret = await commerce.returns.reject(returnId, reason);
      return {
        success: true,
        message: 'Return rejected',
        return: { id: ret.id, status: ret.status },
      };
    },
  },
];

export default returnTools;
