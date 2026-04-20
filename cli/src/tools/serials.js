/**
 * Serial Tools Module
 *
 * MCP tool definitions for serial-number tracking.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

async function resolveSerial(serialsApi, identifier) {
  return serialsApi.get(identifier) || serialsApi.getBySerial(identifier);
}

export const serialTools = withPolicyDomain('serials', [
  {
    name: 'list_serials',
    description: 'List serial numbers.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const serials = await commerce.serials.list();
      return { success: true, count: serials.length, serials };
    },
  },
  {
    name: 'get_serial',
    description: 'Get a serial by ID or serial string.',
    inputSchema: {
      identifier: z.string().min(1).describe('Serial ID or serial string'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const serial = await resolveSerial(commerce.serials, params.identifier);
      if (!serial) {
        return { success: false, error: 'Serial not found' };
      }
      return { success: true, serial };
    },
  },
  {
    name: 'create_serial',
    description: 'Create a serial number.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      serial: z.string().min(1).optional().describe('Optional explicit serial string'),
      lotNumber: z.string().min(1).optional().describe('Optional lot number'),
      manufacturedAt: z.string().optional().describe('Manufacture timestamp in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create serial', params);
      }

      const serial = await commerce.serials.create({
        sku: params.sku,
        serial: params.serial,
        lotNumber: params.lotNumber,
        manufacturedAt: params.manufacturedAt,
      });
      return { success: true, message: 'Serial created', serial };
    },
  },
  {
    name: 'list_available_serials',
    description: 'List available serials for a SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      limit: z.number().int().min(1).max(1000).optional().default(50).describe('Max serials'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const serials = await commerce.serials.getAvailable(params.sku, params.limit || 50);
      return { success: true, count: serials.length, serials };
    },
  },
  {
    name: 'mark_serial_sold',
    description: 'Mark a serial number as sold.',
    inputSchema: {
      serialId: z.string().min(1).describe('Serial ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      orderId: z.string().min(1).optional().describe('Optional order ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Mark serial sold', params);
      }

      const serial = await commerce.serials.markSold(
        params.serialId,
        params.customerId,
        params.orderId,
      );
      return { success: true, message: 'Serial marked as sold', serial };
    },
  },
  {
    name: 'quarantine_serial',
    description: 'Quarantine a serial number.',
    inputSchema: {
      serialId: z.string().min(1).describe('Serial ID'),
      reason: z.string().min(1).max(2000).describe('Quarantine reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Quarantine serial', params);
      }

      const serial = await commerce.serials.quarantine(params.serialId, params.reason);
      return { success: true, message: 'Serial quarantined', serial };
    },
  },
  {
    name: 'check_serial_availability',
    description: 'Check whether a serial string is available.',
    inputSchema: {
      serial: z.string().min(1).describe('Serial string'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const available = await commerce.serials.isAvailable(params.serial);
      return { success: true, serial: params.serial, available };
    },
  },
  {
    name: 'count_serials',
    description: 'Count serial numbers.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.serials.count();
      return { success: true, count };
    },
  },
]);

export default serialTools;
