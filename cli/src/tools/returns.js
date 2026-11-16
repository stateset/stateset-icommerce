/**
 * Returns Tools Module
 *
 * MCP tool definitions for return/RMA operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { acceptedFrom, allowedNextField } from '../state-machine-catalog.js';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

/** "requested", "received or inspecting": where the engine accepts a move into `to`. */
const from = (to) => {
  const states = acceptedFrom('return', to);
  return states.length <= 1
    ? states.join('')
    : `${states.slice(0, -1).join(', ')} or ${states.at(-1)}`;
};

/** The `return_items.disposition` vocabulary (`ReturnDisposition`). */
const RETURN_DISPOSITIONS = ['restock', 'refurbish', 'scrap', 'return_to_vendor', 'quarantine'];

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
    description:
      'Get a specific return by ID, with its items and their dispositions, and `allowedNext`: ' +
      'the statuses the return can move to from its current one.',
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

      return { success: true, return: ret, ...allowedNextField('return', ret.status) };
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
    description: `Approve a return request (the engine accepts this from ${from('approved')}).`,
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
    description: `Reject a return with a reason (the engine accepts this from ${from('rejected')}).`,
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
    description:
      'Mark a return as physically received at the warehouse. The engine accepts this only ' +
      `from ${from('received')}, which add_return_tracking sets on an approved return; an ` +
      'approved return without tracking is refused.',
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
    description:
      `Complete a return (the engine accepts this from ${from('completed')}). It closes the ` +
      'return only: no refund is created. Refund the payment separately with create_refund.',
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
    description: `Cancel a return (the engine accepts this from ${from('cancelled')}).`,
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
    description:
      'Add a return-shipping tracking number and mark the return in transit (the engine ' +
      `accepts this from ${from('in_transit')}). This is the step before mark_return_received.`,
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

  {
    name: 'set_return_item_disposition',
    description:
      'Record what the warehouse did with a received return item: restock or quarantine ' +
      '(back into stock; the SKU needs an inventory item), refurbish, scrap or return_to_vendor. ' +
      'The return must be received or inspecting, and each item is dispositioned once. ' +
      'complete_return needs every item dispositioned. Item ids are in get_return.',
    inputSchema: {
      returnId: z.string().min(1).describe('Return ID (UUID)'),
      returnItemId: z.string().min(1).describe('Return item ID (UUID) from the return items'),
      disposition: z
        .enum(RETURN_DISPOSITIONS)
        .describe(
          'What happened to the units: restock, refurbish, scrap, return_to_vendor, quarantine',
        ),
      note: z
        .string()
        .max(1000)
        .optional()
        .describe('Inspection note (recorded on the governed outbox fact)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set return item disposition', params);
      }

      const item = await commerce.returns.setItemDisposition(params.returnId, params.returnItemId, {
        disposition: params.disposition,
      });
      return {
        success: true,
        message: `Return item ${item.disposition}`,
        returnItem: item,
      };
    },
  },
]);

export default returnTools;
