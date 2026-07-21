/**
 * Production Batch Tools Module
 *
 * MCP tool definitions for grouping work orders into production batches.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const productionBatchTools = withPolicyDomain('production_batches', [
  {
    name: 'check_production_batches_supported',
    description: 'Check whether the production-batches backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.productionBatches.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_production_batches',
    description: 'List production batches with optional filtering.',
    inputSchema: {
      status: z
        .enum(['planned', 'in_progress', 'completed', 'cancelled'])
        .optional()
        .describe('Filter by status'),
      vendorId: z.string().min(1).optional().describe('Filter by vendor ID'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const productionBatches = await commerce.productionBatches.list({
        status: params.status,
        vendorId: params.vendorId,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: productionBatches.length, productionBatches };
    },
  },
  {
    name: 'get_production_batch',
    description: 'Get a production batch by ID.',
    inputSchema: {
      batchId: z.string().min(1).describe('Production batch ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const productionBatch = await commerce.productionBatches.get(params.batchId);
      if (!productionBatch) {
        return { success: false, error: 'Production batch not found' };
      }
      return { success: true, productionBatch };
    },
  },
  {
    name: 'create_production_batch',
    description: 'Create a production batch.',
    inputSchema: {
      name: z.string().min(1).describe('Batch name'),
      vendorId: z.string().min(1).optional().describe('Optional vendor ID'),
      workOrderIds: z
        .array(z.string().min(1))
        .optional()
        .describe('Work order IDs to link at creation'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
      scheduledStart: z.string().optional().describe('Scheduled start timestamp in ISO 8601'),
      scheduledEnd: z.string().optional().describe('Scheduled end timestamp in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create production batch', params);
      }

      const productionBatch = await commerce.productionBatches.create({
        name: params.name,
        vendorId: params.vendorId,
        workOrderIds: params.workOrderIds,
        notes: params.notes,
        scheduledStart: params.scheduledStart,
        scheduledEnd: params.scheduledEnd,
      });
      return { success: true, message: 'Production batch created', productionBatch };
    },
  },
  {
    name: 'update_production_batch',
    description: 'Update a production batch.',
    inputSchema: {
      batchId: z.string().min(1).describe('Production batch ID'),
      name: z.string().min(1).optional().describe('New name'),
      vendorId: z.string().min(1).optional().describe('New vendor ID'),
      status: z
        .enum(['planned', 'in_progress', 'completed', 'cancelled'])
        .optional()
        .describe('New status'),
      notes: z.string().max(2000).optional().describe('New notes'),
      scheduledStart: z.string().optional().describe('New scheduled start timestamp in ISO 8601'),
      scheduledEnd: z.string().optional().describe('New scheduled end timestamp in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update production batch', params);
      }

      const productionBatch = await commerce.productionBatches.update(params.batchId, {
        name: params.name,
        vendorId: params.vendorId,
        status: params.status,
        notes: params.notes,
        scheduledStart: params.scheduledStart,
        scheduledEnd: params.scheduledEnd,
      });
      return { success: true, message: 'Production batch updated', productionBatch };
    },
  },
  {
    name: 'delete_production_batch',
    description: 'Delete a production batch.',
    inputSchema: {
      batchId: z.string().min(1).describe('Production batch ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete production batch', params);
      }

      await commerce.productionBatches.delete(params.batchId);
      return { success: true, message: 'Production batch deleted' };
    },
  },
  {
    name: 'add_production_batch_work_orders',
    description: 'Link work orders to a production batch.',
    inputSchema: {
      batchId: z.string().min(1).describe('Production batch ID'),
      workOrderIds: z.array(z.string().min(1)).min(1).describe('Work order IDs to link'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Add production batch work orders', params);
      }

      const productionBatch = await commerce.productionBatches.addWorkOrders(
        params.batchId,
        params.workOrderIds,
      );
      return { success: true, message: 'Work orders added to production batch', productionBatch };
    },
  },
  {
    name: 'remove_production_batch_work_order',
    description: 'Remove a work order from a production batch.',
    inputSchema: {
      batchId: z.string().min(1).describe('Production batch ID'),
      workOrderId: z.string().min(1).describe('Work order ID to remove'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Remove production batch work order', params);
      }

      const productionBatch = await commerce.productionBatches.removeWorkOrder(
        params.batchId,
        params.workOrderId,
      );
      return {
        success: true,
        message: 'Work order removed from production batch',
        productionBatch,
      };
    },
  },
]);

export default productionBatchTools;
