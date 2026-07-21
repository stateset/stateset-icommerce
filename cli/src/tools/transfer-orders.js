/**
 * Transfer Order Tools Module
 *
 * MCP tool definitions for warehouse-to-warehouse transfer orders.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const transferOrderTools = withPolicyDomain('transfer_orders', [
  {
    name: 'check_transfer_orders_supported',
    description: 'Check whether the transfer-orders backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.transferOrders.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_transfer_orders',
    description: 'List transfer orders with optional filtering.',
    inputSchema: {
      status: z
        .enum(['draft', 'pending', 'in_transit', 'partially_received', 'received', 'cancelled'])
        .optional()
        .describe('Filter by status'),
      sourceWarehouseId: z.string().min(1).optional().describe('Filter by source warehouse'),
      destinationWarehouseId: z
        .string()
        .min(1)
        .optional()
        .describe('Filter by destination warehouse'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const transferOrders = await commerce.transferOrders.list({
        status: params.status,
        sourceWarehouseId: params.sourceWarehouseId,
        destinationWarehouseId: params.destinationWarehouseId,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: transferOrders.length, transferOrders };
    },
  },
  {
    name: 'get_transfer_order',
    description: 'Get a transfer order by ID.',
    inputSchema: {
      transferOrderId: z.string().min(1).describe('Transfer order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const transferOrder = await commerce.transferOrders.get(params.transferOrderId);
      if (!transferOrder) {
        return { success: false, error: 'Transfer order not found' };
      }
      return { success: true, transferOrder };
    },
  },
  {
    name: 'create_transfer_order',
    description: 'Create a transfer order between warehouses.',
    inputSchema: {
      sourceWarehouseId: z.string().min(1).describe('Source warehouse ID'),
      destinationWarehouseId: z.string().min(1).describe('Destination warehouse ID'),
      items: z
        .array(
          z.object({
            productId: z.string().min(1).describe('Product ID'),
            quantity: z.string().min(1).describe('Quantity as an exact decimal string'),
          }),
        )
        .min(1)
        .describe('Line items to transfer'),
      expectedAt: z.string().optional().describe('Expected arrival timestamp in ISO 8601'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create transfer order', params);
      }

      const transferOrder = await commerce.transferOrders.create({
        sourceWarehouseId: params.sourceWarehouseId,
        destinationWarehouseId: params.destinationWarehouseId,
        items: params.items,
        expectedAt: params.expectedAt,
        notes: params.notes,
      });
      return { success: true, message: 'Transfer order created', transferOrder };
    },
  },
  {
    name: 'ship_transfer_order',
    description: 'Mark a transfer order as shipped from the source warehouse.',
    inputSchema: {
      transferOrderId: z.string().min(1).describe('Transfer order ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Ship transfer order', params);
      }

      const transferOrder = await commerce.transferOrders.ship(params.transferOrderId);
      return { success: true, message: 'Transfer order shipped', transferOrder };
    },
  },
  {
    name: 'receive_transfer_order_line',
    description: 'Receive a quantity against a transfer order line at the destination.',
    inputSchema: {
      transferOrderId: z.string().min(1).describe('Transfer order ID'),
      itemId: z.string().min(1).describe('Transfer order line item ID'),
      quantity: z.string().min(1).describe('Quantity received as an exact decimal string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Receive transfer order line', params);
      }

      const transferOrder = await commerce.transferOrders.receiveLine(
        params.transferOrderId,
        params.itemId,
        params.quantity,
      );
      return { success: true, message: 'Transfer order line received', transferOrder };
    },
  },
  {
    name: 'cancel_transfer_order',
    description: 'Cancel a transfer order.',
    inputSchema: {
      transferOrderId: z.string().min(1).describe('Transfer order ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel transfer order', params);
      }

      const transferOrder = await commerce.transferOrders.cancel(params.transferOrderId);
      return { success: true, message: 'Transfer order cancelled', transferOrder };
    },
  },
]);

export default transferOrderTools;
