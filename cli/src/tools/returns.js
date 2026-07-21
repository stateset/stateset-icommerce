/**
 * Returns Tools Module
 *
 * MCP tool definitions for return/RMA operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

function returnSummary(ret) {
  return {
    id: ret.id,
    orderId: ret.orderId,
    status: ret.status,
    reason: ret.reason,
    createdAt: ret.createdAt,
  };
}

function returnList(returns) {
  return {
    success: true,
    count: returns.length,
    returns: returns.map(returnSummary),
  };
}

export const returnTools = withPolicyDomain('returns', [
  {
    name: 'list_returns',
    description: 'List all returns. Shows return status, order, and reason.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of returns to show'),
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
      returnId: z.string().min(1).describe('Return ID (UUID)'),
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
    name: 'list_returns_for_order',
    description: 'List all returns filed against a specific order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const returns = await commerce.returns.listForOrder(params.orderId);
      return returnList(returns);
    },
  },

  {
    name: 'list_returns_for_customer',
    description: 'List all returns filed by a specific customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const returns = await commerce.returns.listForCustomer(params.customerId);
      return returnList(returns);
    },
  },

  {
    name: 'list_pending_returns',
    description: 'List returns awaiting approval (status requested).',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const returns = await commerce.returns.listPending();
      return returnList(returns);
    },
  },

  {
    name: 'create_return',
    description: 'Create a return request for an order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID)'),
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
      reasonDetails: z
        .string()
        .max(1000)
        .optional()
        .describe('Additional details about the return reason'),
      items: z
        .array(
          z.object({
            orderItemId: z.string().min(1).describe('Order item ID to return'),
            quantity: z.number().int().min(1).describe('Quantity to return'),
          }),
        )
        .min(1)
        .max(50)
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
      returnId: z.string().min(1).describe('Return ID (UUID)'),
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
      returnId: z.string().min(1).describe('Return ID (UUID)'),
      reason: z.string().min(1).max(500).describe('Reason for rejection'),
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

  {
    name: 'mark_return_received',
    description: 'Mark a return as physically received at the warehouse.',
    inputSchema: {
      returnId: z.string().min(1).describe('Return ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Mark return received', params);
      }

      const ret = await commerce.returns.markReceived(params.returnId);
      return {
        success: true,
        message: 'Return marked as received',
        return: { id: ret.id, status: ret.status },
      };
    },
  },

  {
    name: 'complete_return',
    description: 'Complete a return and process the refund.',
    inputSchema: {
      returnId: z.string().min(1).describe('Return ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete return', params);
      }

      const ret = await commerce.returns.complete(params.returnId);
      return {
        success: true,
        message: 'Return completed',
        return: { id: ret.id, status: ret.status },
      };
    },
  },

  {
    name: 'cancel_return',
    description: 'Cancel a return request.',
    inputSchema: {
      returnId: z.string().min(1).describe('Return ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel return', params);
      }

      const ret = await commerce.returns.cancel(params.returnId);
      return {
        success: true,
        message: 'Return cancelled',
        return: { id: ret.id, status: ret.status },
      };
    },
  },

  {
    name: 'add_return_tracking',
    description: 'Add a return-shipping tracking number and mark the return in transit.',
    inputSchema: {
      returnId: z.string().min(1).describe('Return ID (UUID)'),
      trackingNumber: z.string().min(1).max(200).describe('Return shipment tracking number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Add return tracking', params);
      }

      const ret = await commerce.returns.addTracking(params.returnId, params.trackingNumber);
      return {
        success: true,
        message: 'Tracking added',
        return: { id: ret.id, status: ret.status },
      };
    },
  },
]);

export default returnTools;
