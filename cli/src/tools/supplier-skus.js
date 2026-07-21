/**
 * Supplier SKU Tools Module
 *
 * MCP tool definitions for supplier part-number (SKU) cross-references.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const supplierSkuTools = withPolicyDomain('supplier_skus', [
  {
    name: 'check_supplier_skus_supported',
    description: 'Check whether the supplier-SKUs backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.supplierSkus.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_supplier_skus',
    description: 'List supplier SKUs with optional filtering.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Filter by supplier ID'),
      productId: z.string().min(1).optional().describe('Filter by product ID'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const supplierSkus = await commerce.supplierSkus.list({
        supplierId: params.supplierId,
        productId: params.productId,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: supplierSkus.length, supplierSkus };
    },
  },
  {
    name: 'get_supplier_sku',
    description: 'Get a supplier SKU by ID.',
    inputSchema: {
      supplierSkuId: z.string().min(1).describe('Supplier SKU ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const supplierSku = await commerce.supplierSkus.get(params.supplierSkuId);
      if (!supplierSku) {
        return { success: false, error: 'Supplier SKU not found' };
      }
      return { success: true, supplierSku };
    },
  },
  {
    name: 'create_supplier_sku',
    description: 'Create a supplier SKU cross-reference.',
    inputSchema: {
      productId: z.string().min(1).describe('Internal product ID'),
      supplierId: z.string().min(1).describe('Supplier ID'),
      sku: z.string().min(1).describe('Supplier part number / SKU'),
      unitCost: z.string().min(1).optional().describe('Unit cost as an exact decimal string'),
      currency: z.string().min(1).optional().describe('Currency code, e.g. "USD"'),
      minOrderQty: z
        .string()
        .min(1)
        .optional()
        .describe('Minimum order quantity as an exact decimal string'),
      leadTimeDays: z.number().int().min(0).optional().describe('Lead time in days'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create supplier SKU', params);
      }

      const supplierSku = await commerce.supplierSkus.create({
        productId: params.productId,
        supplierId: params.supplierId,
        sku: params.sku,
        unitCost: params.unitCost,
        currency: params.currency,
        minOrderQty: params.minOrderQty,
        leadTimeDays: params.leadTimeDays,
      });
      return { success: true, message: 'Supplier SKU created', supplierSku };
    },
  },
  {
    name: 'update_supplier_sku',
    description: 'Update a supplier SKU.',
    inputSchema: {
      supplierSkuId: z.string().min(1).describe('Supplier SKU ID'),
      sku: z.string().min(1).optional().describe('New supplier part number / SKU'),
      unitCost: z.string().min(1).optional().describe('New unit cost as an exact decimal string'),
      currency: z.string().min(1).optional().describe('New currency code'),
      minOrderQty: z
        .string()
        .min(1)
        .optional()
        .describe('New minimum order quantity as an exact decimal string'),
      leadTimeDays: z.number().int().min(0).optional().describe('New lead time in days'),
      isPreferred: z.boolean().optional().describe('Preferred supplier flag'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update supplier SKU', params);
      }

      const supplierSku = await commerce.supplierSkus.update(params.supplierSkuId, {
        sku: params.sku,
        unitCost: params.unitCost,
        currency: params.currency,
        minOrderQty: params.minOrderQty,
        leadTimeDays: params.leadTimeDays,
        isPreferred: params.isPreferred,
      });
      return { success: true, message: 'Supplier SKU updated', supplierSku };
    },
  },
  {
    name: 'delete_supplier_sku',
    description: 'Delete a supplier SKU.',
    inputSchema: {
      supplierSkuId: z.string().min(1).describe('Supplier SKU ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete supplier SKU', params);
      }

      await commerce.supplierSkus.delete(params.supplierSkuId);
      return { success: true, message: 'Supplier SKU deleted' };
    },
  },
  {
    name: 'bulk_upsert_supplier_skus',
    description: 'Bulk upsert supplier SKUs for a supplier, keyed by internal product.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      items: z
        .array(
          z.object({
            productId: z.string().min(1).describe('Internal product ID'),
            sku: z.string().min(1).describe('Supplier part number / SKU'),
            unitCost: z.string().min(1).optional().describe('Unit cost as an exact decimal string'),
          }),
        )
        .min(1)
        .describe('Supplier SKU records to upsert'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Bulk upsert supplier SKUs', params);
      }

      const upserted = await commerce.supplierSkus.bulkUpsert(params.supplierId, params.items);
      return { success: true, message: 'Supplier SKUs upserted', upserted };
    },
  },
]);

export default supplierSkuTools;
