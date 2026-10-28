/**
 * Supplier & Purchase Order Tools Module
 *
 * MCP tool definitions for supplier management and purchase order operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';
const exactCost = z
  .string()
  .max(25)
  .regex(/^(0|[1-9]\d{0,17})(\.\d{1,6})?$/);
const purchaseLines = z
  .array(
    z
      .object({
        sku: z.string().min(1).max(64),
        name: z.string().min(1).max(255),
        quantity: z.number().int().positive().max(1000000),
        unitCostExact: exactCost,
      })
      .strict(),
  )
  .min(1)
  .max(100);

/**
 * Supplier tool definitions
 */
export const supplierTools = [
  {
    name: 'list_suppliers',
    description: 'List all suppliers.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const suppliers = await commerce.purchaseOrders.listSuppliers();
      return { success: true, count: suppliers.length, suppliers };
    },
  },

  {
    name: 'get_supplier',
    description: 'Get a supplier by ID.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const supplier = await commerce.purchaseOrders.getSupplier(params.supplierId);
      if (!supplier) {
        return { success: false, error: 'Supplier not found' };
      }
      return { success: true, supplier };
    },
  },

  {
    name: 'create_supplier',
    description: 'Create a new supplier.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Supplier name'),
      email: z.string().email().optional().describe('Contact email'),
      phone: z.string().max(30).optional().describe('Phone number'),
      address: z.string().max(500).optional().describe('Address'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create supplier', params);
      }

      const supplier = await commerce.purchaseOrders.createSupplier({
        name: params.name,
        email: params.email,
        phone: params.phone,
        address: params.address,
      });
      return { success: true, message: 'Supplier created', supplier };
    },
  },

  {
    name: 'list_purchase_orders',
    description:
      'List purchase orders, optionally filtered by supplier, status, date range or total.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Filter by supplier ID'),
      status: z.string().min(1).optional().describe('Filter by status'),
      fromDate: z
        .string()
        .min(1)
        .optional()
        .describe('Inclusive lower bound on order date (RFC 3339)'),
      toDate: z
        .string()
        .min(1)
        .optional()
        .describe('Inclusive upper bound on order date (RFC 3339)'),
      minTotal: z.string().min(1).optional().describe('Minimum order total (exact decimal string)'),
      maxTotal: z.string().min(1).optional().describe('Maximum order total (exact decimal string)'),
      limit: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Maximum results (server default 500, cap 1000)'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = {
        supplierId: params.supplierId,
        status: params.status,
        fromDate: params.fromDate,
        toDate: params.toDate,
        minTotal: params.minTotal,
        maxTotal: params.maxTotal,
        limit: params.limit,
        offset: params.offset,
      };
      const purchaseOrders = await commerce.purchaseOrders.list(filter);
      const count = await commerce.purchaseOrders.count();
      return { success: true, count, purchaseOrders };
    },
  },

  {
    name: 'get_purchase_order',
    description: 'Get a purchase order by ID.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('Purchase order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const purchaseOrder = await commerce.purchaseOrders.get(params.purchaseOrderId);
      if (!purchaseOrder) {
        return { success: false, error: 'Purchase order not found' };
      }
      return { success: true, purchaseOrder };
    },
  },

  {
    name: 'create_purchase_order',
    description: 'Create a purchase order to a supplier.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      items: z
        .string()
        .min(1)
        .max(65536)
        .describe(
          'JSON array with exact money: [{"sku":"X","name":"Y","quantity":10,"unitCostExact":"5.00"}]',
        ),
      notes: z.string().optional().describe('Notes'),
      currency: z
        .enum(['USD', 'EUR', 'GBP'])
        .optional()
        .describe('Ledger currency; defaults to supplier currency'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create PO', params);
      }

      let items;
      try {
        items = purchaseLines.parse(JSON.parse(params.items));
      } catch (err) {
        return {
          success: false,
          error: `Invalid items JSON: ${err.message}. Expected format: [{"sku":"X","name":"Y","quantity":10,"unitCostExact":"5.00"}]`,
        };
      }
      const po = await commerce.purchaseOrders.create({
        supplierId: params.supplierId,
        items,
        notes: params.notes,
        currency: params.currency,
      });
      return { success: true, message: 'PO created', purchaseOrder: po };
    },
  },

  {
    name: 'submit_purchase_order',
    description: 'Submit a purchase order for approval.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('PO ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Submit PO', params);
      }

      const po = await commerce.purchaseOrders.submit(params.purchaseOrderId);
      return { success: true, message: 'PO submitted', purchaseOrder: po };
    },
  },

  {
    name: 'approve_purchase_order',
    description: 'Approve a purchase order.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('PO ID'),
      approvedBy: z.string().min(1).describe('Approver name'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { purchaseOrderId, approvedBy } = params;
      if (!allowApply) {
        return applyRequired('Approve PO', params);
      }

      const po = await commerce.purchaseOrders.approve(purchaseOrderId, approvedBy);
      return { success: true, message: 'PO approved', purchaseOrder: po };
    },
  },

  {
    name: 'send_purchase_order',
    description: 'Send a PO to the supplier.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('PO ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { purchaseOrderId } = params;
      if (!allowApply) {
        return applyRequired('Send PO', params);
      }

      const po = await commerce.purchaseOrders.send(purchaseOrderId);
      return { success: true, message: 'PO sent to supplier', purchaseOrder: po };
    },
  },

  {
    name: 'cancel_purchase_order',
    description: 'Cancel a purchase order.',
    inputSchema: {
      purchaseOrderId: z.string().min(1).describe('PO ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel PO', params);
      }

      const po = await commerce.purchaseOrders.cancel(params.purchaseOrderId);
      return { success: true, message: 'PO cancelled', purchaseOrder: po };
    },
  },
  {
    name: 'get_purchase_order_items',
    description:
      'Read persisted purchase-order lines with exact costs and ordered/received quantities.',
    inputSchema: { purchaseOrderId: z.string().uuid().describe('PO ID') },
    permission: 'read',
    handler: async ({ commerce, params }) => ({
      success: true,
      items: await commerce.purchaseOrders.getItems(params.purchaseOrderId),
    }),
  },
  {
    name: 'receive_purchase_order',
    description:
      'Receive exact quantities against sent PO lines into an explicit warehouse location. PO quantities, receipt documents, and inventory stock are posted atomically.',
    inputSchema: {
      purchaseOrderId: z.string().uuid().describe('PO ID'),
      warehouseId: z.number().int().positive().describe('Receiving warehouse ID'),
      receivingLocationId: z
        .number()
        .int()
        .positive()
        .describe('Active receiving location in that warehouse'),
      items: z
        .array(
          z
            .object({
              itemId: z.string().uuid(),
              quantityReceivedExact: exactCost.refine(
                (value) => /[1-9]/.test(value),
                'Received quantity must be positive',
              ),
            })
            .strict(),
        )
        .min(1)
        .max(100),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Receive PO quantities and post stock', params);
      return {
        success: true,
        purchaseOrder: await commerce.purchaseOrders.receive(
          params.purchaseOrderId,
          params.items,
          params.warehouseId,
          params.receivingLocationId,
        ),
      };
    },
  },
];

export default supplierTools;
