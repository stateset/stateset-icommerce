/**
 * Receiving Tools Module
 *
 * MCP tool definitions for inbound receipts and receiving workflows.
 */

import { z } from 'zod';
import { positiveDecimalString, nonnegativeDecimalString } from '../utils/decimal-schema.js';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const receivingTools = withPolicyDomain('receiving', [
  {
    name: 'list_receipts',
    description: 'List receipts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const receipts = await commerce.receiving.listReceipts();
      return { success: true, count: receipts.length, receipts };
    },
  },
  {
    name: 'get_receipt',
    description: 'Get a receipt by ID or receipt number.',
    inputSchema: {
      receiptId: z.string().min(1).optional().describe('Receipt ID'),
      receiptNumber: z.string().min(1).optional().describe('Receipt number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const receipt = params.receiptId
        ? await commerce.receiving.getReceipt(params.receiptId)
        : params.receiptNumber
          ? await commerce.receiving.getReceiptByNumber(params.receiptNumber)
          : null;
      if (!receipt) {
        return { success: false, error: 'Receipt not found' };
      }
      const items = await commerce.receiving.getReceiptItems(receipt.id);
      return { success: true, receipt, items };
    },
  },
  {
    name: 'create_receipt',
    description: 'Create a receipt.',
    inputSchema: {
      items: z
        .array(
          z.object({
            sku: z.string().min(1),
            description: z.string().optional(),
            poLineId: z.string().uuid().optional(),
            expectedQuantity: positiveDecimalString,
            unitCost: nonnegativeDecimalString.optional(),
            lotNumber: z.string().optional(),
            expirationDate: z.string().datetime().optional(),
            notes: z.string().optional(),
          }),
        )
        .optional(),
      receiptType: z.string().min(1).describe('Receipt type'),
      warehouseId: z.number().int().describe('Warehouse ID'),
      purchaseOrderId: z.string().min(1).optional().describe('Optional purchase order ID'),
      supplierId: z
        .string()
        .uuid()
        .optional()
        .describe('Supplier ID; required for a purchase-order receipt'),
      carrier: z.string().min(1).optional().describe('Optional carrier'),
      trackingNumber: z.string().min(1).optional().describe('Optional tracking number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create receipt', params);
      }

      const receipt = await commerce.receiving.createReceipt({
        items: params.items,
        receiptType: params.receiptType,
        warehouseId: params.warehouseId,
        purchaseOrderId: params.purchaseOrderId,
        supplierId: params.supplierId,
        carrier: params.carrier,
        trackingNumber: params.trackingNumber,
      });
      return { success: true, message: 'Receipt created', receipt };
    },
  },
  {
    name: 'create_receipt_from_purchase_order',
    description: 'Create a receipt from a purchase order.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('Purchase order ID'),
      warehouseId: z.number().int().describe('Warehouse ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create receipt from purchase order', params);
      }

      const receipt = await commerce.receiving.createReceiptFromPo(
        params.purchaseOrderId,
        params.warehouseId,
      );
      return { success: true, message: 'Receipt created from purchase order', receipt };
    },
  },
  {
    name: 'start_receiving',
    description: 'Start receiving against a receipt.',
    inputSchema: {
      receiptId: z.string().min(1).describe('Receipt ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Start receiving', params);
      }

      const receipt = await commerce.receiving.startReceiving(params.receiptId);
      return { success: true, message: 'Receiving started', receipt };
    },
  },
  {
    name: 'complete_receiving',
    description: 'Complete receiving against a receipt.',
    inputSchema: {
      receiptId: z.string().min(1).describe('Receipt ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete receiving', params);
      }

      const receipt = await commerce.receiving.completeReceiving(params.receiptId);
      return { success: true, message: 'Receiving completed', receipt };
    },
  },
  {
    name: 'cancel_receipt',
    description: 'Cancel a receipt.',
    inputSchema: {
      receiptId: z.string().min(1).describe('Receipt ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel receipt', params);
      }

      const receipt = await commerce.receiving.cancelReceipt(params.receiptId);
      return { success: true, message: 'Receipt canceled', receipt };
    },
  },
  {
    name: 'count_receipts',
    description: 'Count receipts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.receiving.countReceipts();
      return { success: true, count };
    },
  },

  {
    name: 'receive_receipt_items',
    description:
      'Receive accepted goods into an explicit receiving location and record rejected quantities, lots and serials.',
    permission: 'write',
    inputSchema: {
      receiptId: z.string().uuid(),
      receivingLocationId: z.number().int().positive(),
      items: z
        .array(
          z.object({
            receiptItemId: z.string().uuid(),
            quantityReceived: positiveDecimalString,
            quantityRejected: nonnegativeDecimalString.optional(),
            rejectionReason: z.string().optional(),
            lotNumber: z.string().optional(),
            serialNumbers: z.array(z.string()).optional(),
            expirationDate: z.string().datetime().optional(),
            notes: z.string().optional(),
          }),
        )
        .min(1),
    },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Receive stock', params);
      return {
        success: true,
        receipt: await commerce.receiving.receiveItems({ ...params, receivedBy: actorId }),
      };
    },
  },
  {
    name: 'create_put_away',
    description: 'Create a put-away task from a received line into its destination location.',
    permission: 'write',
    inputSchema: {
      receiptId: z.string().uuid(),
      receiptItemId: z.string().uuid(),
      sku: z.string().min(1),
      fromLocationId: z.number().int().positive().optional(),
      toLocationId: z.number().int().positive(),
      quantity: positiveDecimalString,
      lotId: z.string().uuid().optional(),
      assignedTo: z.string().optional(),
      notes: z.string().optional(),
    },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Create put-away', params);
      return { success: true, putAway: await commerce.receiving.createPutAway(params) };
    },
  },
  {
    name: 'list_put_aways',
    description: 'List put-away tasks by receipt, warehouse, status or assignee.',
    permission: 'read',
    inputSchema: {
      receiptId: z.string().uuid().optional(),
      warehouseId: z.number().int().positive().optional(),
      status: z.enum(['pending', 'assigned', 'in_progress', 'completed', 'cancelled']).optional(),
      assignedTo: z.string().optional(),
      limit: z.number().int().min(1).max(200).optional(),
      offset: z.number().int().nonnegative().optional(),
    },
    handler: async ({ commerce, params }) => ({
      success: true,
      putAways: await commerce.receiving.listPutAways(params),
    }),
  },
  {
    name: 'get_put_away',
    description: 'Get one put-away task.',
    permission: 'read',
    inputSchema: { putAwayId: z.string().uuid() },
    handler: async ({ commerce, params }) => ({
      success: true,
      putAway: await commerce.receiving.getPutAway(params.putAwayId),
    }),
  },
  {
    name: 'assign_put_away',
    description: 'Assign a put-away task to a warehouse worker.',
    permission: 'write',
    inputSchema: { putAwayId: z.string().uuid(), assignedTo: z.string().min(1) },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Assign put-away', params);
      return {
        success: true,
        putAway: await commerce.receiving.assignPutAway(params.putAwayId, params.assignedTo),
      };
    },
  },
  {
    name: 'complete_put_away',
    description: 'Complete a put-away task and move its received stock to the destination.',
    permission: 'write',
    inputSchema: {
      putAwayId: z.string().uuid(),
      actualLocationId: z.number().int().positive().optional(),
      notes: z.string().optional(),
    },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Complete put-away', params);
      return {
        success: true,
        putAway: await commerce.receiving.completePutAway({ ...params, completedBy: actorId }),
      };
    },
  },
  {
    name: 'start_put_away',
    description: 'Start a put-away task.',
    permission: 'write',
    inputSchema: { putAwayId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Start put-away', params);
      return { success: true, putAway: await commerce.receiving.startPutAway(params.putAwayId) };
    },
  },
  {
    name: 'cancel_put_away',
    description: 'Cancel a put-away task.',
    permission: 'write',
    inputSchema: { putAwayId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Cancel put-away', params);
      return { success: true, putAway: await commerce.receiving.cancelPutAway(params.putAwayId) };
    },
  },
]);

export default receivingTools;
