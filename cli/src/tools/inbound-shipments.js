/**
 * Inbound Shipment Tools Module
 *
 * MCP tool definitions for inbound (supplier-to-warehouse) shipments.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const inboundShipmentTools = withPolicyDomain('inbound_shipments', [
  {
    name: 'check_inbound_shipments_supported',
    description: 'Check whether the inbound-shipments backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.inboundShipments.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_inbound_shipments',
    description: 'List inbound shipments with optional filtering.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Filter by supplier ID'),
      warehouseId: z.string().min(1).optional().describe('Filter by warehouse ID'),
      status: z
        .enum(['pending', 'in_transit', 'arrived', 'partially_received', 'received', 'cancelled'])
        .optional()
        .describe('Filter by status'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const inboundShipments = await commerce.inboundShipments.list({
        supplierId: params.supplierId,
        warehouseId: params.warehouseId,
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: inboundShipments.length, inboundShipments };
    },
  },
  {
    name: 'get_inbound_shipment',
    description: 'Get an inbound shipment by ID.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Inbound shipment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const inboundShipment = await commerce.inboundShipments.get(params.shipmentId);
      if (!inboundShipment) {
        return { success: false, error: 'Inbound shipment not found' };
      }
      return { success: true, inboundShipment };
    },
  },
  {
    name: 'create_inbound_shipment',
    description: 'Create an inbound shipment.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      purchaseOrderId: z.string().min(1).optional().describe('Optional related purchase order ID'),
      warehouseId: z.string().min(1).optional().describe('Optional destination warehouse ID'),
      carrier: z.string().min(1).optional().describe('Optional carrier'),
      trackingNumber: z.string().min(1).optional().describe('Optional tracking number'),
      expectedAt: z.string().optional().describe('Expected arrival timestamp in ISO 8601'),
      items: z
        .array(
          z.object({
            productId: z.string().min(1).describe('Product ID'),
            sku: z.string().min(1).describe('SKU'),
            quantityExpected: z
              .string()
              .min(1)
              .describe('Expected quantity as an exact decimal string'),
          }),
        )
        .min(1)
        .describe('Expected line items'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create inbound shipment', params);
      }

      const inboundShipment = await commerce.inboundShipments.create({
        supplierId: params.supplierId,
        purchaseOrderId: params.purchaseOrderId,
        warehouseId: params.warehouseId,
        carrier: params.carrier,
        trackingNumber: params.trackingNumber,
        expectedAt: params.expectedAt,
        items: params.items,
        notes: params.notes,
      });
      return { success: true, message: 'Inbound shipment created', inboundShipment };
    },
  },
  {
    name: 'mark_inbound_shipment_in_transit',
    description: 'Mark an inbound shipment as in transit.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Inbound shipment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Mark inbound shipment in transit', params);
      }

      const inboundShipment = await commerce.inboundShipments.markInTransit(params.shipmentId);
      return { success: true, message: 'Inbound shipment marked in transit', inboundShipment };
    },
  },
  {
    name: 'mark_inbound_shipment_arrived',
    description: 'Mark an inbound shipment as arrived.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Inbound shipment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Mark inbound shipment arrived', params);
      }

      const inboundShipment = await commerce.inboundShipments.markArrived(params.shipmentId);
      return { success: true, message: 'Inbound shipment marked arrived', inboundShipment };
    },
  },
  {
    name: 'receive_inbound_shipment_line',
    description: 'Receive a quantity against an inbound shipment line.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Inbound shipment ID'),
      itemId: z.string().min(1).describe('Shipment line item ID'),
      quantity: z.string().min(1).describe('Quantity received as an exact decimal string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Receive inbound shipment line', params);
      }

      const inboundShipment = await commerce.inboundShipments.receiveLine(
        params.shipmentId,
        params.itemId,
        params.quantity,
      );
      return { success: true, message: 'Inbound shipment line received', inboundShipment };
    },
  },
  {
    name: 'cancel_inbound_shipment',
    description: 'Cancel an inbound shipment.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Inbound shipment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel inbound shipment', params);
      }

      const inboundShipment = await commerce.inboundShipments.cancel(params.shipmentId);
      return { success: true, message: 'Inbound shipment cancelled', inboundShipment };
    },
  },
]);

export default inboundShipmentTools;
