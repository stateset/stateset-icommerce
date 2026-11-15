/**
 * Stock Snapshot Tools Module
 *
 * MCP tool definitions for point-in-time inventory snapshots.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const stockSnapshotTools = withPolicyDomain('stock-snapshots', [
  {
    name: 'list_stock_snapshots',
    description: 'List stock snapshots (header level).',
    inputSchema: {
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const snapshots = await commerce.stockSnapshots.list({
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: snapshots.length, snapshots };
    },
  },
  {
    name: 'get_stock_snapshot',
    description: 'Get a stock snapshot by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Stock snapshot ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const snapshot = await commerce.stockSnapshots.get(params.id);
      if (!snapshot) {
        return { success: false, error: 'Stock snapshot not found' };
      }
      return { success: true, snapshot };
    },
  },
  {
    name: 'get_latest_stock_snapshot',
    description: 'Get the most recent stock snapshot.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const snapshot = await commerce.stockSnapshots.latest();
      if (!snapshot) {
        return { success: false, error: 'No stock snapshot found' };
      }
      return { success: true, snapshot };
    },
  },
  {
    name: 'capture_stock_snapshot',
    description: 'Capture on-hand and available quantities from the stored stock ledger.',
    inputSchema: {
      label: z.string().min(1).optional().describe('Optional snapshot label'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Capture stock snapshot', params);
      }
      const snapshot = await commerce.stockSnapshots.capture({
        label: params.label,
      });
      return { success: true, message: 'Stock snapshot captured', snapshot };
    },
  },
  {
    name: 'delete_stock_snapshot',
    description: 'Delete a stock snapshot.',
    inputSchema: {
      id: z.string().min(1).describe('Stock snapshot ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete stock snapshot', params);
      }
      await commerce.stockSnapshots.delete(params.id);
      return { success: true, message: 'Stock snapshot deleted' };
    },
  },
]);

export default stockSnapshotTools;
