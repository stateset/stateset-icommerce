/**
 * Invoice Tools Module
 *
 * MCP tool definitions for invoice creation, sending, and payment recording.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Invoice tool definitions
 */
export const invoiceTools = [
  {
    name: 'list_invoices',
    description: 'List all invoices.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const invoices = await commerce.invoices.list();
      const count = await commerce.invoices.count();
      return { success: true, count, invoices };
    },
  },

  {
    name: 'create_invoice',
    description: 'Create an invoice for a customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      orderId: z.string().optional().describe('Order ID'),
      items: z
        .string()
        .min(1)
        .describe('JSON array: [{"description":"X","quantity":1,"unitPrice":10.00}]'),
      dueDate: z.string().max(30).optional().describe('Due date ISO'),
      notes: z.string().max(1000).optional().describe('Notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create invoice', params);
      }

      let items;
      try {
        items = JSON.parse(params.items);
      } catch (err) {
        return {
          success: false,
          error: `Invalid items JSON: ${err.message}. Expected format: [{"description":"X","quantity":1,"unitPrice":10.00}]`,
        };
      }
      const invoice = await commerce.invoices.create({
        customerId: params.customerId,
        orderId: params.orderId,
        items,
        dueDate: params.dueDate,
        notes: params.notes,
      });
      return { success: true, message: 'Invoice created', invoice };
    },
  },

  {
    name: 'send_invoice',
    description: 'Send an invoice to the customer.',
    inputSchema: {
      invoiceId: z.string().min(1).describe('Invoice ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { invoiceId } = params;
      if (!allowApply) {
        return applyRequired('Send invoice', params);
      }

      const invoice = await commerce.invoices.send(invoiceId);
      return { success: true, message: 'Invoice sent', invoice };
    },
  },

  {
    name: 'record_invoice_payment',
    description: 'Record payment on an invoice.',
    inputSchema: {
      invoiceId: z.string().min(1).describe('Invoice ID'),
      amount: z.number().positive().describe('Amount paid'),
      paymentMethod: z.string().max(50).optional().describe('Payment method'),
      reference: z.string().max(100).optional().describe('Check/reference number'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Record payment', params);
      }

      const invoice = await commerce.invoices.recordPayment(params.invoiceId, {
        amount: params.amount,
        paymentMethod: params.paymentMethod,
        reference: params.reference,
      });
      return { success: true, message: 'Payment recorded', invoice };
    },
  },

  {
    name: 'get_overdue_invoices',
    description: 'Get all overdue invoices.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const invoices = await commerce.invoices.getOverdue();
      return { success: true, count: invoices.length, overdueInvoices: invoices };
    },
  },
];

export default invoiceTools;
