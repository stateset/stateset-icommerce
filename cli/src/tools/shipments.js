/**
 * Shipment Tools Module
 *
 * MCP tool definitions for shipment tracking and delivery operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Shipment tool definitions
 */
export const shipmentTools = [
  {
    name: 'list_shipments',
    description: 'List all shipments.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const shipments = await commerce.shipments.list();
      const count = await commerce.shipments.count();
      return { success: true, count, shipments };
    },
  },

  {
    name: 'create_shipment',
    description: 'Create a shipment for an order.',
    inputSchema: {
      orderId: z.string().describe('Order ID'),
      carrier: z.string().optional().describe('Carrier: USPS, UPS, FedEx, DHL'),
      service: z.string().optional().describe('Service level'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipment', params);
      }

      const shipment = await commerce.shipments.create({
        orderId: params.orderId,
        carrier: params.carrier,
        service: params.service,
      });
      return { success: true, message: 'Shipment created', shipment };
    },
  },

  {
    name: 'deliver_shipment',
    description: 'Mark a shipment as delivered.',
    inputSchema: {
      shipmentId: z.string().describe('Shipment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { shipmentId } = params;
      if (!allowApply) {
        return applyRequired('Deliver shipment', params);
      }

      const shipment = await commerce.shipments.deliver(shipmentId);
      return { success: true, message: 'Shipment delivered', shipment };
    },
  },
];

export default shipmentTools;
