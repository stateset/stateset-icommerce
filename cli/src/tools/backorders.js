/**
 * Backorder Tools Module
 *
 * MCP tool definitions for backorder management.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const backorderTools = withPolicyDomain('backorders', [
  {
    name: 'list_backorders',
    description: 'List backorders.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const backorders = await commerce.backorder.listBackorders();
      return { success: true, count: backorders.length, backorders };
    },
  },
  {
    name: 'get_backorder',
    description: 'Get a backorder by ID or backorder number.',
    inputSchema: {
      backorderId: z.string().min(1).optional().describe('Backorder ID'),
      backorderNumber: z.string().min(1).optional().describe('Backorder number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const backorder = params.backorderId
        ? await commerce.backorder.getBackorder(params.backorderId)
        : params.backorderNumber
          ? await commerce.backorder.getBackorderByNumber(params.backorderNumber)
          : null;
      if (!backorder) {
        return { success: false, error: 'Backorder not found' };
      }
      return { success: true, backorder };
    },
  },
  {
    name: 'create_backorder',
    description: 'Create a backorder.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      sku: z.string().min(1).describe('SKU'),
      quantity: z.number().positive().describe('Quantity'),
      priority: z.string().min(1).optional().describe('Priority'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create backorder', params);
      }

      const backorder = await commerce.backorder.createBackorder({
        orderId: params.orderId,
        customerId: params.customerId,
        sku: params.sku,
        quantity: params.quantity,
        priority: params.priority,
        notes: params.notes,
      });
      return { success: true, message: 'Backorder created', backorder };
    },
  },
  {
    name: 'cancel_backorder',
    description: 'Cancel a backorder.',
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel backorder', params);
      }

      const backorder = await commerce.backorder.cancelBackorder(params.backorderId);
      return { success: true, message: 'Backorder canceled', backorder };
    },
  },
  {
    name: 'list_backorders_for_order',
    description: 'List backorders for an order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const backorders = await commerce.backorder.getBackordersForOrder(params.orderId);
      return { success: true, count: backorders.length, backorders };
    },
  },
  {
    name: 'list_backorders_for_sku',
    description: 'List backorders for a SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const backorders = await commerce.backorder.getBackordersForSku(params.sku);
      return { success: true, count: backorders.length, backorders };
    },
  },
  {
    name: 'list_overdue_backorders',
    description: 'List overdue backorders.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const backorders = await commerce.backorder.getOverdueBackorders();
      return { success: true, count: backorders.length, backorders };
    },
  },
  {
    name: 'get_backorder_summary',
    description: 'Get the backorder summary.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const summary = await commerce.backorder.getSummary();
      return { success: true, summary };
    },
  },
  {
    name: 'count_pending_backorders',
    description: 'Count pending backorders.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.backorder.countPending();
      return { success: true, count };
    },
  },
]);

export default backorderTools;
