/**
 * Order Tools Module
 *
 * MCP tool definitions for order operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Order tool definitions
 */
export const orderTools = [
  {
    name: 'list_orders',
    description:
      'List all orders. Shows order number, status, customer, total amount, and item count.',
    inputSchema: {
      limit: z.number().optional().default(50).describe('Maximum number of orders to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { limit } = params;
      const orders = await commerce.orders.list();
      const count = await commerce.orders.count();
      const limitedOrders = orders.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limitedOrders.length,
        orders: limitedOrders.map((o) => ({
          id: o.id,
          orderNumber: o.orderNumber,
          customerId: o.customerId,
          status: o.status,
          totalAmount: o.totalAmount,
          currency: o.currency,
          paymentStatus: o.paymentStatus,
          fulfillmentStatus: o.fulfillmentStatus,
          itemCount: o.items?.length || 0,
          createdAt: o.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_order',
    description:
      'Get a specific order by ID or order number. Returns full order details including line items.',
    inputSchema: {
      identifier: z.string().describe('Order ID (UUID) or order number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      const order = await commerce.orders.get(identifier);

      if (!order) {
        return { error: 'Order not found' };
      }

      return {
        success: true,
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          customerId: order.customerId,
          status: order.status,
          totalAmount: order.totalAmount,
          currency: order.currency,
          paymentStatus: order.paymentStatus,
          fulfillmentStatus: order.fulfillmentStatus,
          trackingNumber: order.trackingNumber,
          items: order.items?.map((i) => ({
            id: i.id,
            sku: i.sku,
            name: i.name,
            quantity: i.quantity,
            unitPrice: i.unitPrice,
            total: i.total,
          })),
          createdAt: order.createdAt,
          updatedAt: order.updatedAt,
        },
      };
    },
  },

  {
    name: 'create_order',
    description: 'Create a new order for a customer with line items.',
    inputSchema: {
      customerId: z.string().describe('Customer ID (UUID)'),
      items: z
        .array(
          z.object({
            sku: z.string().describe('Product SKU'),
            name: z.string().describe('Product name'),
            quantity: z.number().describe('Quantity'),
            unitPrice: z.number().describe('Unit price'),
          }),
        )
        .describe('Order line items'),
      currency: z.string().optional().default('USD').describe('Currency code'),
      notes: z.string().optional().describe('Order notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return {
          error: 'Create operation not allowed. The --apply flag must be set to create orders.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: {
            customerId: params.customerId,
            itemCount: params.items.length,
            estimatedTotal: params.items.reduce((sum, i) => sum + i.quantity * i.unitPrice, 0),
          },
        };
      }

      const order = await commerce.orders.create(params);
      if (autoIndexEntity) autoIndexEntity('order', order);

      return {
        success: true,
        message: 'Order created successfully',
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          status: order.status,
          totalAmount: order.totalAmount,
        },
      };
    },
  },

  {
    name: 'update_order_status',
    description:
      'Update the status of an order. Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded.',
    inputSchema: {
      orderId: z.string().describe('Order ID (UUID)'),
      status: z
        .enum([
          'pending',
          'confirmed',
          'processing',
          'shipped',
          'delivered',
          'cancelled',
          'refunded',
        ])
        .describe('New order status'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId, status } = params;

      if (!allowApply) {
        return {
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: { orderId, newStatus: status },
        };
      }

      const order = await commerce.orders.updateStatus(orderId, status);

      return {
        success: true,
        message: `Order status updated to ${status}`,
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          status: order.status,
        },
      };
    },
  },

  {
    name: 'ship_order',
    description: 'Mark an order as shipped with optional tracking number.',
    inputSchema: {
      orderId: z.string().describe('Order ID (UUID)'),
      trackingNumber: z.string().optional().describe('Shipping tracking number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId, trackingNumber } = params;

      if (!allowApply) {
        return {
          error: 'Ship operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldShip: { orderId, trackingNumber },
        };
      }

      const order = await commerce.orders.ship(orderId, trackingNumber);

      return {
        success: true,
        message: 'Order shipped successfully',
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          status: order.status,
          trackingNumber: order.trackingNumber,
        },
      };
    },
  },

  {
    name: 'cancel_order',
    description: 'Cancel an order. Only pending or confirmed orders can be cancelled.',
    inputSchema: {
      orderId: z.string().describe('Order ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId } = params;

      if (!allowApply) {
        return {
          error: 'Cancel operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCancel: { orderId },
        };
      }

      const order = await commerce.orders.cancel(orderId);

      return {
        success: true,
        message: 'Order cancelled successfully',
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          status: order.status,
        },
      };
    },
  },
];

/**
 * Get all order tools
 */
export function getOrderTools() {
  return orderTools;
}

/**
 * Get order tool by name
 */
export function getOrderTool(name) {
  return orderTools.find((t) => t.name === name);
}

export default orderTools;
