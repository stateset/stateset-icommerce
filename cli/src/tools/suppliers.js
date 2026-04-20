/**
 * Supplier & Purchase Order Tools Module
 *
 * MCP tool definitions for supplier management and purchase order operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

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
    description: 'List all purchase orders.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const purchaseOrders = await commerce.purchaseOrders.list();
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
        .describe('JSON array: [{"sku":"X","name":"Y","quantity":10,"unitPrice":5.00}]'),
      notes: z.string().optional().describe('Notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create PO', params);
      }

      let items;
      try {
        items = JSON.parse(params.items);
      } catch (err) {
        return {
          success: false,
          error: `Invalid items JSON: ${err.message}. Expected format: [{"sku":"X","name":"Y","quantity":10,"unitPrice":5.00}]`,
        };
      }
      const po = await commerce.purchaseOrders.create({
        supplierId: params.supplierId,
        items,
        notes: params.notes,
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
];

export default supplierTools;
