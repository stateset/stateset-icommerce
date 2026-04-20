/**
 * Receiving Tools Module
 *
 * MCP tool definitions for inbound receipts and receiving workflows.
 */

import { z } from 'zod';
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
      return { success: true, receipt };
    },
  },
  {
    name: 'create_receipt',
    description: 'Create a receipt.',
    inputSchema: {
      receiptType: z.string().min(1).describe('Receipt type'),
      warehouseId: z.number().int().describe('Warehouse ID'),
      purchaseOrderId: z.string().min(1).optional().describe('Optional purchase order ID'),
      carrier: z.string().min(1).optional().describe('Optional carrier'),
      trackingNumber: z.string().min(1).optional().describe('Optional tracking number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create receipt', params);
      }

      const receipt = await commerce.receiving.createReceipt({
        receiptType: params.receiptType,
        warehouseId: params.warehouseId,
        purchaseOrderId: params.purchaseOrderId,
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
]);

export default receivingTools;
