/**
 * Order Tools Module
 *
 * MCP tool definitions for order operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

import { acceptedFrom, allowedNextField } from '../state-machine-catalog.js';
import { statusEnum } from '../utils/status-schema.js';

/** "a", "a or b", "a, b or c". */
const listOr = (items) =>
  items.length <= 1 ? items.join('') : `${items.slice(0, -1).join(', ')} or ${items.at(-1)}`;

/**
 * The statuses update_order_status offers: every catalogued order move a plain
 * status update can make. shipped/partially_shipped belong to ship_order (the
 * kernel refuses them here with commerce.shipment_command_required), and no
 * state moves back to pending.
 */
const ORDER_STATUS_UPDATES = ['confirmed', 'processing', 'delivered', 'cancelled', 'refunded'];
/** "confirmed (from pending), processing (from confirmed), ..." -- from the catalog. */
const movesInto = (targets) =>
  targets.map((to) => `${to} (from ${listOr(acceptedFrom('order', to))})`).join(', ');
const SHIPPABLE_FROM = acceptedFrom('order', 'shipped');
const CANCELLABLE_FROM = acceptedFrom('order', 'cancelled');

/**
 * Order tool definitions
 */
export const orderTools = [
  {
    name: 'list_orders',
    description:
      'List all orders. Shows order number, status, customer, total amount, and item count.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of orders to return'),
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
          totalAmountExact: o.totalAmountExact,
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
      'Get a specific order by ID or order number. Returns full order details including line ' +
      'items, and `allowedNext`: the statuses the order can move to from its current one.',
    inputSchema: {
      identifier: z.string().min(1).describe('Order ID (UUID) or order number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      // The binding's `get` takes a UUID; anything else is an order number.
      const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        identifier,
      );
      const order =
        !isUuid && typeof commerce.orders.getByNumber === 'function'
          ? await commerce.orders.getByNumber(identifier)
          : await commerce.orders.get(identifier);

      if (!order) {
        return { success: false, error: 'Order not found' };
      }

      return {
        success: true,
        order: {
          id: order.id,
          orderNumber: order.orderNumber,
          customerId: order.customerId,
          status: order.status,
          totalAmount: order.totalAmount,
          totalAmountExact: order.totalAmountExact,
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
            unitPriceExact: i.unitPriceExact,
            total: i.total,
            totalExact: i.totalExact,
          })),
          createdAt: order.createdAt,
          updatedAt: order.updatedAt,
        },
        ...allowedNextField('order', order.status),
      };
    },
  },

  {
    name: 'create_order',
    description: 'Create a new order for a customer with line items.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
      items: z
        .array(
          z.object({
            sku: z.string().min(1).max(100).describe('Product SKU'),
            name: z.string().min(1).max(255).describe('Product name'),
            quantity: z.number().int().min(1).describe('Quantity'),
            unitPrice: z.number().positive().describe('Unit price'),
          }),
        )
        .min(1)
        .max(100)
        .describe('Order line items'),
      currency: z.string().max(10).optional().default('USD').describe('Currency code'),
      notes: z.string().max(1000).optional().describe('Order notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return {
          success: false,
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
          totalAmountExact: order.totalAmountExact,
        },
      };
    },
  },

  {
    name: 'update_order_status',
    description:
      `Move an order to a new status: ${movesInto(ORDER_STATUS_UPDATES)}. The engine refuses ` +
      'any other move; get_order returns `allowedNext` for an order. Shipping is not a status ' +
      'update: call ship_order (pass `lines` to ship part of the order). cancel_order also ' +
      'cancels. Cancelling is refused while captured payments still hold money (refund first).',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID)'),
      status: statusEnum(ORDER_STATUS_UPDATES, {
        shipped: 'shipping is its own operation; call ship_order.',
        partially_shipped:
          'shipping is its own operation; call ship_order with `lines` for the units shipped now.',
        pending: 'an order is pending only until it is confirmed; nothing moves back to pending.',
      }).describe('New order status'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId, status } = params;

      if (!allowApply) {
        return {
          success: false,
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
    description:
      `Ship an order (the engine accepts this from ${listOr(SHIPPABLE_FROM)}), with an optional ` +
      'tracking number. Omit `lines` to ship every remaining unit (-> shipped); pass `lines` to ' +
      'ship part of the order (-> partially_shipped until every unit has shipped). This is the ' +
      'only way an order reaches shipped or partially_shipped.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID)'),
      trackingNumber: z.string().optional().describe('Shipping tracking number'),
      lines: z
        .array(
          z.object({
            orderItemId: z.string().min(1).describe('Order line (item) ID being shipped'),
            quantity: z
              .number()
              .int()
              .positive()
              .describe('Units of that line shipped now (at most what is left to ship)'),
          }),
        )
        .min(1)
        .max(500)
        .optional()
        .describe('Ship only these units (a partial shipment). Omit to ship everything remaining'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId, trackingNumber, lines } = params;

      if (!allowApply) {
        return {
          success: false,
          error: 'Ship operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldShip: { orderId, trackingNumber, ...(lines ? { lines } : {}) },
        };
      }

      // The binding's direct `ship` ships every remaining unit; per-line
      // shipment runs only as the governed `orders.ship` kernel command.
      if (Array.isArray(lines) && lines.length > 0) {
        return {
          success: false,
          error:
            'A partial shipment (`lines`) runs only through the governed kernel command ' +
            'orders.ship; this direct path can ship every remaining unit only.',
          hint: 'Configure the kernel, or omit `lines` to ship the whole order.',
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
    description:
      `Cancel an order. The engine accepts this from ${listOr(CANCELLABLE_FROM)}; once an ` +
      'order has shipped, use a return instead. Refused while captured payments still hold ' +
      'money: refund them first.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { orderId } = params;

      if (!allowApply) {
        return {
          success: false,
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

export default orderTools;
