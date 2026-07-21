/**
 * Topology Snapshot Tools Module
 *
 * MCP tool definitions for operational topology health snapshots.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const topologySnapshotTools = withPolicyDomain('topology-snapshots', [
  {
    name: 'list_topology_snapshots',
    description: 'List operational topology snapshots.',
    inputSchema: {
      health: z
        .enum(['unknown', 'healthy', 'degraded', 'critical'])
        .optional()
        .describe('Health grade'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const snapshots = await commerce.topologySnapshots.list({
        health: params.health,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: snapshots.length, snapshots };
    },
  },
  {
    name: 'get_topology_snapshot',
    description: 'Get a topology snapshot by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Topology snapshot ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const snapshot = await commerce.topologySnapshots.get(params.id);
      if (!snapshot) {
        return { success: false, error: 'Topology snapshot not found' };
      }
      return { success: true, snapshot };
    },
  },
  {
    name: 'get_latest_topology_snapshot',
    description: 'Get the most recent topology snapshot.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const snapshot = await commerce.topologySnapshots.latest();
      if (!snapshot) {
        return { success: false, error: 'No topology snapshot found' };
      }
      return { success: true, snapshot };
    },
  },
  {
    name: 'capture_topology_snapshot',
    description: 'Capture a topology snapshot; health is derived from the supplied metrics.',
    inputSchema: {
      channelsTotal: z.string().min(1).describe('Total channels'),
      channelsActive: z.string().min(1).describe('Active channels'),
      warehousesTotal: z.string().min(1).describe('Total warehouses'),
      productsTotal: z.string().min(1).describe('Total products'),
      openOrders: z.string().min(1).describe('Open orders'),
      signals: z.string().min(1).optional().describe('Signals as a JSON string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Capture topology snapshot', params);
      }
      const snapshot = await commerce.topologySnapshots.capture({
        channelsTotal: params.channelsTotal,
        channelsActive: params.channelsActive,
        warehousesTotal: params.warehousesTotal,
        productsTotal: params.productsTotal,
        openOrders: params.openOrders,
        signals: params.signals,
      });
      return { success: true, message: 'Topology snapshot captured', snapshot };
    },
  },
  {
    name: 'delete_topology_snapshot',
    description: 'Delete a topology snapshot.',
    inputSchema: {
      id: z.string().min(1).describe('Topology snapshot ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete topology snapshot', params);
      }
      await commerce.topologySnapshots.delete(params.id);
      return { success: true, message: 'Topology snapshot deleted' };
    },
  },
]);

export default topologySnapshotTools;
