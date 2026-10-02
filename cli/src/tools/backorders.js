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
    name: 'auto_allocate_inventory',
    description:
      "Allocate available stock to a SKU's open backorders, in priority order (critical first, then oldest first). Call after stock arrives.",
    inputSchema: {
      sku: z.string().min(1).describe('SKU to allocate against'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Auto-allocate inventory to backorders', params);
      }
      const allocations = await commerce.backorder.autoAllocateInventory(params.sku);
      return {
        success: true,
        message: `Allocated ${allocations.length} backorder(s)`,
        allocations,
      };
    },
  },
  {
    name: 'allocate_backorder',
    description: 'Reserve a specific quantity of stock against one backorder.',
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
      quantity: z.number().positive().describe('Quantity to allocate'),
      locationId: z.number().int().optional().describe('Source location ID'),
      lotId: z.string().min(1).optional().describe('Lot ID'),
      expiresAt: z.string().min(1).optional().describe('Allocation expiry (RFC 3339)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Allocate backorder', params);
      }
      const allocation = await commerce.backorder.allocateBackorder({
        backorderId: params.backorderId,
        quantity: params.quantity,
        locationId: params.locationId,
        lotId: params.lotId,
        expiresAt: params.expiresAt,
      });
      return { success: true, message: 'Backorder allocated', allocation };
    },
  },
  {
    name: 'get_backorder_allocations',
    description: 'List the allocations recorded against one backorder.',
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const allocations = await commerce.backorder.getAllocations(params.backorderId);
      return { success: true, count: allocations.length, allocations };
    },
  },
  {
    name: 'confirm_backorder_allocation',
    description: 'Confirm a reserved allocation, committing the stock to the backorder.',
    inputSchema: {
      allocationId: z.string().min(1).describe('Allocation ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Confirm backorder allocation', params);
      }
      const allocation = await commerce.backorder.confirmAllocation(params.allocationId);
      return { success: true, message: 'Allocation confirmed', allocation };
    },
  },
  {
    name: 'release_backorder_allocation',
    description: 'Release a reserved allocation, returning the stock to available.',
    inputSchema: {
      allocationId: z.string().min(1).describe('Allocation ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Release backorder allocation', params);
      }
      const allocation = await commerce.backorder.releaseAllocation(params.allocationId);
      return { success: true, message: 'Allocation released', allocation };
    },
  },
  {
    name: 'expire_backorder_allocations',
    description:
      'Expire every allocation whose hold has lapsed, freeing the stock it held. Returns how many were swept.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Expire lapsed backorder allocations', params);
      }
      const expired = await commerce.backorder.expireAllocations();
      return { success: true, message: `Expired ${expired} allocation(s)`, expired };
    },
  },
  {
    name: 'fulfill_backorder',
    description: 'Record a fulfilment against a backorder, drawing on the named source.',
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
      quantity: z.number().positive().describe('Quantity fulfilled'),
      sourceType: z
        .enum(['inventory', 'purchase_order', 'transfer', 'production'])
        .describe('Where the stock came from'),
      sourceId: z.string().min(1).optional().describe('Source record ID'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
      fulfilledBy: z.string().min(1).optional().describe('Who fulfilled it'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Fulfill backorder', params);
      }
      const backorder = await commerce.backorder.fulfillBackorder({
        backorderId: params.backorderId,
        quantity: params.quantity,
        sourceType: params.sourceType,
        sourceId: params.sourceId,
        notes: params.notes,
        fulfilledBy: params.fulfilledBy,
      });
      return { success: true, message: 'Backorder fulfilled', backorder };
    },
  },
  {
    name: 'get_backorder_fulfillment_history',
    description: 'The fulfilment history recorded against one backorder.',
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const history = await commerce.backorder.getFulfillmentHistory(params.backorderId);
      return { success: true, count: history.length, history };
    },
  },
  {
    name: 'list_backorders_for_customer',
    description: 'Every backorder raised for one customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const backorders = await commerce.backorder.getBackordersForCustomer(params.customerId);
      return { success: true, count: backorders.length, backorders };
    },
  },
  {
    name: 'get_sku_backorder_summary',
    description: 'Open backorder totals for one SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const summary = await commerce.backorder.getSkuSummary(params.sku);
      if (!summary) {
        return { success: true, summary: null, message: 'No open backorders for this SKU' };
      }
      return { success: true, summary };
    },
  },
  {
    name: 'update_backorder',
    description: "Update a backorder's priority, dates, source location or notes.",
    inputSchema: {
      backorderId: z.string().min(1).describe('Backorder ID'),
      priority: z.string().min(1).optional().describe('Priority'),
      expectedDate: z.string().min(1).optional().describe('Expected date (RFC 3339)'),
      promisedDate: z.string().min(1).optional().describe('Promised date (RFC 3339)'),
      sourceLocationId: z.number().int().optional().describe('Source location ID'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update backorder', params);
      }
      const backorder = await commerce.backorder.updateBackorder(params.backorderId, {
        priority: params.priority,
        expectedDate: params.expectedDate,
        promisedDate: params.promisedDate,
        sourceLocationId: params.sourceLocationId,
        notes: params.notes,
      });
      return { success: true, message: 'Backorder updated', backorder };
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
