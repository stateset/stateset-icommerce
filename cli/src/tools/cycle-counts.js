/**
 * Cycle Count Tools Module
 *
 * MCP tool definitions for inventory cycle counting.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const cycleCountTools = withPolicyDomain('cycle_counts', [
  {
    name: 'list_cycle_counts',
    description: 'List cycle counts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const cycleCounts = await commerce.cycleCounts.list();
      return { success: true, count: cycleCounts.length, cycleCounts };
    },
  },
  {
    name: 'get_cycle_count',
    description: 'Get a cycle count by ID.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const cycleCount = await commerce.cycleCounts.get(params.cycleCountId);
      if (!cycleCount) {
        return { success: false, error: 'Cycle count not found' };
      }
      return { success: true, cycleCount };
    },
  },
  {
    name: 'create_cycle_count',
    description: 'Create a cycle count.',
    inputSchema: {
      warehouseId: z.number().int().optional().describe('Optional warehouse ID'),
      skus: z.array(z.string().min(1)).optional().describe('Optional SKUs to count'),
      scheduledDate: z.string().min(1).optional().describe('Scheduled date in ISO 8601'),
      assignedTo: z.string().min(1).optional().describe('Optional assignee'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.create({
        warehouseId: params.warehouseId,
        skus: params.skus,
        scheduledDate: params.scheduledDate,
        assignedTo: params.assignedTo,
        notes: params.notes,
      });
      return { success: true, message: 'Cycle count created', cycleCount };
    },
  },
  {
    name: 'start_cycle_count',
    description: 'Start a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Start cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.start(params.cycleCountId);
      return { success: true, message: 'Cycle count started', cycleCount };
    },
  },
  {
    name: 'record_cycle_counts',
    description: 'Record counted quantities for a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
      counts: z
        .array(
          z.object({
            sku: z.string().min(1).describe('SKU'),
            countedQuantity: z.number().int().min(0).describe('Counted quantity'),
            countedBy: z.string().min(1).optional().describe('Optional counter'),
          }),
        )
        .min(1)
        .describe('Counted line items'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Record cycle counts', params);
      }

      const cycleCount = await commerce.cycleCounts.recordCounts(
        params.cycleCountId,
        params.counts,
      );
      return { success: true, message: 'Cycle counts recorded', cycleCount };
    },
  },
  {
    name: 'complete_cycle_count',
    description: 'Complete a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.complete(params.cycleCountId);
      return { success: true, message: 'Cycle count completed', cycleCount };
    },
  },
  {
    name: 'cancel_cycle_count',
    description: 'Cancel a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.cancel(params.cycleCountId);
      return { success: true, message: 'Cycle count canceled', cycleCount };
    },
  },
]);

export default cycleCountTools;
