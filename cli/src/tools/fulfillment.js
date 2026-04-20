/**
 * Fulfillment Tools Module
 *
 * MCP tool definitions for warehouse waves and pick tasks.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const fulfillmentTools = withPolicyDomain('fulfillment', [
  {
    name: 'list_fulfillment_waves',
    description: 'List fulfillment waves.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const waves = await commerce.fulfillment.listWaves();
      return { success: true, count: waves.length, waves };
    },
  },
  {
    name: 'get_fulfillment_wave',
    description: 'Get a fulfillment wave by ID.',
    inputSchema: {
      waveId: z.string().min(1).describe('Wave ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const wave = await commerce.fulfillment.getWave(params.waveId);
      if (!wave) {
        return { success: false, error: 'Fulfillment wave not found' };
      }
      return { success: true, wave };
    },
  },
  {
    name: 'create_fulfillment_wave',
    description: 'Create a fulfillment wave.',
    inputSchema: {
      warehouseId: z.number().int().describe('Warehouse ID'),
      orderIds: z.array(z.string().min(1)).min(1).describe('Order IDs'),
      priority: z.number().int().optional().describe('Optional priority'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create fulfillment wave', params);
      }

      const wave = await commerce.fulfillment.createWave({
        warehouseId: params.warehouseId,
        orderIds: params.orderIds,
        priority: params.priority,
        notes: params.notes,
      });
      return { success: true, message: 'Fulfillment wave created', wave };
    },
  },
  {
    name: 'release_fulfillment_wave',
    description: 'Release a fulfillment wave for picking.',
    inputSchema: {
      waveId: z.string().min(1).describe('Wave ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Release fulfillment wave', params);
      }

      const wave = await commerce.fulfillment.releaseWave(params.waveId);
      return { success: true, message: 'Fulfillment wave released', wave };
    },
  },
  {
    name: 'complete_fulfillment_wave',
    description: 'Complete a fulfillment wave.',
    inputSchema: {
      waveId: z.string().min(1).describe('Wave ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete fulfillment wave', params);
      }

      const wave = await commerce.fulfillment.completeWave(params.waveId);
      return { success: true, message: 'Fulfillment wave completed', wave };
    },
  },
  {
    name: 'cancel_fulfillment_wave',
    description: 'Cancel a fulfillment wave.',
    inputSchema: {
      waveId: z.string().min(1).describe('Wave ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel fulfillment wave', params);
      }

      const wave = await commerce.fulfillment.cancelWave(params.waveId);
      return { success: true, message: 'Fulfillment wave canceled', wave };
    },
  },
  {
    name: 'list_pick_tasks',
    description: 'List pick tasks.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const picks = await commerce.fulfillment.listPicks();
      return { success: true, count: picks.length, picks };
    },
  },
  {
    name: 'get_pick_task',
    description: 'Get a pick task by ID.',
    inputSchema: {
      pickId: z.string().min(1).describe('Pick task ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const pick = await commerce.fulfillment.getPick(params.pickId);
      if (!pick) {
        return { success: false, error: 'Pick task not found' };
      }
      return { success: true, pick };
    },
  },
  {
    name: 'assign_pick_task',
    description: 'Assign a pick task.',
    inputSchema: {
      pickId: z.string().min(1).describe('Pick task ID'),
      assignedTo: z.string().min(1).describe('Assignee'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Assign pick task', params);
      }

      const pick = await commerce.fulfillment.assignPick(params.pickId, params.assignedTo);
      return { success: true, message: 'Pick task assigned', pick };
    },
  },
  {
    name: 'start_pick_task',
    description: 'Start a pick task.',
    inputSchema: {
      pickId: z.string().min(1).describe('Pick task ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Start pick task', params);
      }

      const pick = await commerce.fulfillment.startPick(params.pickId);
      return { success: true, message: 'Pick task started', pick };
    },
  },
  {
    name: 'cancel_pick_task',
    description: 'Cancel a pick task.',
    inputSchema: {
      pickId: z.string().min(1).describe('Pick task ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel pick task', params);
      }

      const pick = await commerce.fulfillment.cancelPick(params.pickId);
      return { success: true, message: 'Pick task canceled', pick };
    },
  },
  {
    name: 'check_order_ready_to_pack',
    description: 'Check whether an order is ready to pack.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const ready = await commerce.fulfillment.isOrderReadyToPack(params.orderId);
      return { success: true, orderId: params.orderId, readyToPack: ready };
    },
  },
  {
    name: 'check_order_ready_to_ship',
    description: 'Check whether an order is ready to ship.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const ready = await commerce.fulfillment.isOrderReadyToShip(params.orderId);
      return { success: true, orderId: params.orderId, readyToShip: ready };
    },
  },
  {
    name: 'count_fulfillment_waves',
    description: 'Count fulfillment waves.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.fulfillment.countWaves();
      return { success: true, count };
    },
  },
]);

export default fulfillmentTools;
