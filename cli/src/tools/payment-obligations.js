/**
 * Payment Obligation Tools Module
 *
 * MCP tool definitions for scheduled accounts-payable obligations.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const statusSchema = z
  .enum(['pending', 'scheduled', 'partially_paid', 'paid', 'cancelled'])
  .describe('Payment obligation status');

export const paymentObligationTools = withPolicyDomain('payment-obligations', [
  {
    name: 'list_payment_obligations',
    description: 'List payment obligations.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Supplier ID'),
      status: statusSchema.optional(),
      dueBefore: z.string().min(1).optional().describe('Due before date (YYYY-MM-DD)'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const obligations = await commerce.paymentObligations.list({
        supplierId: params.supplierId,
        status: params.status,
        dueBefore: params.dueBefore,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: obligations.length, obligations };
    },
  },
  {
    name: 'get_payment_obligation',
    description: 'Get a payment obligation by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Payment obligation ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const obligation = await commerce.paymentObligations.get(params.id);
      if (!obligation) {
        return { success: false, error: 'Payment obligation not found' };
      }
      return { success: true, obligation };
    },
  },
  {
    name: 'get_payment_obligation_dashboard',
    description: 'Aggregate payment obligation dashboard as of a date.',
    inputSchema: {
      today: z.string().min(1).describe('As-of date (YYYY-MM-DD)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const dashboard = await commerce.paymentObligations.dashboard(params.today);
      return { success: true, dashboard };
    },
  },
  {
    name: 'create_payment_obligation',
    description: 'Create a payment obligation.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      amount: z.string().min(1).describe('Obligation amount (exact decimal string)'),
      dueDate: z.string().min(1).describe('Due date (YYYY-MM-DD)'),
      purchaseOrderId: z.string().min(1).optional().describe('Linked purchase order ID'),
      currency: z.string().min(1).optional().describe('Currency code'),
      notes: z.string().min(1).optional().describe('Notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create payment obligation', params);
      }
      const obligation = await commerce.paymentObligations.create({
        supplierId: params.supplierId,
        purchaseOrderId: params.purchaseOrderId,
        amount: params.amount,
        currency: params.currency,
        dueDate: params.dueDate,
        notes: params.notes,
      });
      return { success: true, message: 'Payment obligation created', obligation };
    },
  },
  {
    name: 'record_payment_obligation_payment',
    description: 'Record a payment against an obligation.',
    inputSchema: {
      id: z.string().min(1).describe('Payment obligation ID'),
      amount: z.string().min(1).describe('Payment amount (exact decimal string)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Record payment obligation payment', params);
      }
      const obligation = await commerce.paymentObligations.recordPayment(params.id, params.amount);
      return { success: true, message: 'Payment recorded', obligation };
    },
  },
  {
    name: 'set_payment_obligation_status',
    description: 'Set the status of a payment obligation.',
    inputSchema: {
      id: z.string().min(1).describe('Payment obligation ID'),
      status: statusSchema,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set payment obligation status', params);
      }
      const obligation = await commerce.paymentObligations.setStatus(params.id, params.status);
      return { success: true, message: 'Payment obligation status updated', obligation };
    },
  },
  {
    name: 'link_payment_obligation_bill',
    description: 'Link an accounts-payable bill to a payment obligation.',
    inputSchema: {
      id: z.string().min(1).describe('Payment obligation ID'),
      billId: z.string().min(1).describe('AP bill ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Link payment obligation bill', params);
      }
      const obligation = await commerce.paymentObligations.linkBill(params.id, params.billId);
      return { success: true, message: 'Bill linked', obligation };
    },
  },
]);

export default paymentObligationTools;
