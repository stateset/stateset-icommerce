/**
 * Inventory Tools Module
 *
 * MCP tool definitions for inventory/stock operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Inventory tool definitions
 */
export const inventoryTools = [
  {
    name: 'get_stock',
    description:
      'Get current stock level for a SKU. Shows on-hand, allocated, and available quantities.',
    inputSchema: {
      sku: z.string().describe('Product SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { sku } = params;
      const stock = await commerce.inventory.getStock(sku);

      if (!stock) {
        return { success: false, error: `No inventory item found for SKU ${sku}` };
      }

      return {
        success: true,
        stock: {
          sku: stock.sku,
          name: stock.name,
          totalOnHand: stock.totalOnHand,
          totalAllocated: stock.totalAllocated,
          totalAvailable: stock.totalAvailable,
        },
      };
    },
  },

  {
    name: 'create_inventory_item',
    description: 'Create a new inventory item for a SKU.',
    inputSchema: {
      sku: z.string().describe('Product SKU'),
      name: z.string().describe('Item name'),
      description: z.string().optional().describe('Item description'),
      initialQuantity: z.number().optional().default(0).describe('Initial stock quantity'),
      reorderPoint: z.number().optional().describe('Reorder point threshold'),
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

      const item = await commerce.inventory.createItem(params);
      return {
        success: true,
        message: 'Inventory item created successfully',
        item: {
          id: item.id,
          sku: item.sku,
          name: item.name,
        },
      };
    },
  },

  {
    name: 'adjust_inventory',
    description:
      'Adjust inventory quantity for a SKU. Use positive numbers to add stock, negative to remove.',
    inputSchema: {
      sku: z.string().describe('Product SKU'),
      quantity: z.number().describe('Quantity adjustment (positive to add, negative to subtract)'),
      reason: z
        .string()
        .describe('Reason for adjustment (e.g., "Received shipment", "Damaged goods")'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { sku, quantity, reason } = params;

      if (!allowApply) {
        const stock = await commerce.inventory.getStock(sku);
        return {
          success: false,
          error: 'Adjust operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldAdjust: {
            sku,
            currentOnHand: stock?.totalOnHand || 0,
            adjustment: quantity,
            newOnHand: (stock?.totalOnHand || 0) + quantity,
            reason,
          },
        };
      }

      await commerce.inventory.adjust(sku, quantity, reason);
      const stock = await commerce.inventory.getStock(sku);
      return {
        success: true,
        message: `Inventory adjusted by ${quantity > 0 ? '+' : ''}${quantity}`,
        stock: {
          sku: stock.sku,
          totalOnHand: stock.totalOnHand,
          totalAvailable: stock.totalAvailable,
        },
      };
    },
  },

  {
    name: 'reserve_inventory',
    description:
      'Reserve inventory for an order. Reserved stock is allocated but not yet deducted.',
    inputSchema: {
      sku: z.string().describe('Product SKU'),
      quantity: z.number().describe('Quantity to reserve'),
      referenceType: z.string().describe('Reference type (e.g., "order", "transfer")'),
      referenceId: z.string().describe('Reference ID (e.g., order ID)'),
      expiresInSeconds: z.number().optional().describe('Reservation expiry in seconds'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Reserve operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldReserve: params,
        };
      }

      const reservation = await commerce.inventory.reserve(
        params.sku,
        params.quantity,
        params.referenceType,
        params.referenceId,
        params.expiresInSeconds,
      );
      return {
        success: true,
        message: 'Inventory reserved successfully',
        reservation: {
          id: reservation.id,
          quantity: reservation.quantity,
          status: reservation.status,
        },
      };
    },
  },

  {
    name: 'confirm_reservation',
    description: 'Confirm an inventory reservation, deducting the reserved quantity from stock.',
    inputSchema: {
      reservationId: z.string().describe('Reservation ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { reservationId } = params;

      if (!allowApply) {
        return {
          success: false,
          error: 'Confirm operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldConfirm: { reservationId },
        };
      }

      await commerce.inventory.confirmReservation(reservationId);
      return {
        success: true,
        message: 'Reservation confirmed and stock deducted',
      };
    },
  },

  {
    name: 'release_reservation',
    description:
      'Release an inventory reservation, returning the reserved quantity to available stock.',
    inputSchema: {
      reservationId: z.string().describe('Reservation ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { reservationId } = params;

      if (!allowApply) {
        return {
          success: false,
          error: 'Release operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldRelease: { reservationId },
        };
      }

      await commerce.inventory.releaseReservation(reservationId);
      return {
        success: true,
        message: 'Reservation released and stock returned to available',
      };
    },
  },
];

export default inventoryTools;
