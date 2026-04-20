/**
 * Lot Tools Module
 *
 * MCP tool definitions for batch and lot tracking.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

async function resolveLot(lotsApi, identifier) {
  return lotsApi.get(identifier) || lotsApi.getByNumber(identifier);
}

export const lotTools = withPolicyDomain('lots', [
  {
    name: 'list_lots',
    description: 'List lots.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const lots = await commerce.lots.list();
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'get_lot',
    description: 'Get a lot by ID or lot number.',
    inputSchema: {
      identifier: z.string().min(1).describe('Lot ID or lot number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const lot = await resolveLot(commerce.lots, params.identifier);
      if (!lot) {
        return { success: false, error: 'Lot not found' };
      }
      return { success: true, lot };
    },
  },
  {
    name: 'create_lot',
    description: 'Create a lot.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      quantityProduced: z.number().positive().describe('Quantity produced'),
      lotNumber: z.string().min(1).optional().describe('Optional lot number'),
      productionDate: z.string().optional().describe('Production date in ISO 8601'),
      expirationDate: z.string().optional().describe('Expiration date in ISO 8601'),
      supplierLotNumber: z.string().min(1).optional().describe('Optional supplier lot number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create lot', params);
      }

      const lot = await commerce.lots.create({
        sku: params.sku,
        quantityProduced: params.quantityProduced,
        lotNumber: params.lotNumber,
        productionDate: params.productionDate,
        expirationDate: params.expirationDate,
        supplierLotNumber: params.supplierLotNumber,
      });
      return { success: true, message: 'Lot created', lot };
    },
  },
  {
    name: 'list_active_lots',
    description: 'List active lots for a SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const lots = await commerce.lots.getActiveLots(params.sku);
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'list_available_lots_for_sku',
    description: 'List available lots for a SKU in FIFO order.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const lots = await commerce.lots.getAvailableLotsForSku(params.sku);
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'quarantine_lot',
    description: 'Quarantine a lot.',
    inputSchema: {
      lotId: z.string().min(1).describe('Lot ID'),
      reason: z.string().min(1).max(2000).describe('Quarantine reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Quarantine lot', params);
      }

      const lot = await commerce.lots.quarantine(params.lotId, params.reason);
      return { success: true, message: 'Lot quarantined', lot };
    },
  },
  {
    name: 'release_lot_quarantine',
    description: 'Release a lot from quarantine.',
    inputSchema: {
      lotId: z.string().min(1).describe('Lot ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Release lot quarantine', params);
      }

      const lot = await commerce.lots.releaseQuarantine(params.lotId);
      return { success: true, message: 'Lot released from quarantine', lot };
    },
  },
  {
    name: 'list_expiring_lots',
    description: 'List lots expiring within a number of days.',
    inputSchema: {
      days: z.number().int().min(0).describe('Days until expiration'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const lots = await commerce.lots.getExpiringLots(params.days);
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'list_expired_lots',
    description: 'List expired lots.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const lots = await commerce.lots.getExpiredLots();
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'list_quarantined_lots',
    description: 'List quarantined lots.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const lots = await commerce.lots.getQuarantined();
      return { success: true, count: lots.length, lots };
    },
  },
  {
    name: 'count_lots',
    description: 'Count lots.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.lots.count();
      return { success: true, count };
    },
  },
]);

export default lotTools;
