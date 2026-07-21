/**
 * Vendor Return Tools Module
 *
 * MCP tool definitions for return-to-supplier (vendor RMA) flows.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const vendorReturnTools = withPolicyDomain('vendor-returns', [
  {
    name: 'list_vendor_returns',
    description: 'List vendor returns.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Supplier ID'),
      status: z
        .enum(['draft', 'pending', 'processed', 'cancelled'])
        .optional()
        .describe('Vendor return status'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const returns = await commerce.vendorReturns.list({
        supplierId: params.supplierId,
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: returns.length, returns };
    },
  },
  {
    name: 'get_vendor_return',
    description: 'Get a vendor return by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Vendor return ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const vendorReturn = await commerce.vendorReturns.get(params.id);
      if (!vendorReturn) {
        return { success: false, error: 'Vendor return not found' };
      }
      return { success: true, vendorReturn };
    },
  },
  {
    name: 'create_vendor_return',
    description: 'Create a draft vendor return.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      items: z
        .array(
          z.object({
            productId: z.string().min(1).describe('Product ID'),
            quantity: z.string().min(1).describe('Quantity (exact decimal string)'),
            unitCost: z.string().min(1).describe('Unit cost (exact decimal string)'),
            reason: z
              .enum(['defective', 'overage', 'wrong_item', 'other'])
              .optional()
              .describe('Return reason'),
          }),
        )
        .min(1)
        .describe('Return line items'),
      purchaseOrderId: z.string().min(1).optional().describe('Originating purchase order ID'),
      currency: z.string().min(1).optional().describe('Currency code'),
      notes: z.string().min(1).optional().describe('Notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create vendor return', params);
      }
      const vendorReturn = await commerce.vendorReturns.create({
        supplierId: params.supplierId,
        purchaseOrderId: params.purchaseOrderId,
        currency: params.currency,
        items: params.items,
        notes: params.notes,
      });
      return { success: true, message: 'Vendor return created', vendorReturn };
    },
  },
  {
    name: 'submit_vendor_return',
    description: 'Submit a draft vendor return to the supplier.',
    inputSchema: {
      id: z.string().min(1).describe('Vendor return ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Submit vendor return', params);
      }
      const vendorReturn = await commerce.vendorReturns.submit(params.id);
      return { success: true, message: 'Vendor return submitted', vendorReturn };
    },
  },
  {
    name: 'process_vendor_return',
    description: 'Process a vendor return, optionally generating a vendor credit.',
    inputSchema: {
      id: z.string().min(1).describe('Vendor return ID'),
      generateCredit: z.boolean().describe('Whether to generate a vendor credit'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Process vendor return', params);
      }
      const vendorReturn = await commerce.vendorReturns.process(params.id, params.generateCredit);
      return { success: true, message: 'Vendor return processed', vendorReturn };
    },
  },
  {
    name: 'cancel_vendor_return',
    description: 'Cancel a vendor return.',
    inputSchema: {
      id: z.string().min(1).describe('Vendor return ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel vendor return', params);
      }
      const vendorReturn = await commerce.vendorReturns.cancel(params.id);
      return { success: true, message: 'Vendor return cancelled', vendorReturn };
    },
  },
]);

export default vendorReturnTools;
