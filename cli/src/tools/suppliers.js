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
    name: 'create_supplier',
    description: 'Create a new supplier.',
    inputSchema: {
      name: z.string().describe('Supplier name'),
      email: z.string().optional().describe('Contact email'),
      phone: z.string().optional().describe('Phone number'),
      address: z.string().optional().describe('Address'),
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
    name: 'create_purchase_order',
    description: 'Create a purchase order to a supplier.',
    inputSchema: {
      supplierId: z.string().describe('Supplier ID'),
      items: z
        .string()
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
    name: 'approve_purchase_order',
    description: 'Approve a purchase order.',
    inputSchema: {
      purchaseOrderId: z.string().describe('PO ID'),
      approvedBy: z.string().describe('Approver name'),
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
      purchaseOrderId: z.string().describe('PO ID'),
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
];

export default supplierTools;
