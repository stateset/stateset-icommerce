/**
 * Prepayment Tools Module
 *
 * MCP tool definitions for supplier prepayments (advance payments).
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const prepaymentTools = withPolicyDomain('prepayments', [
  {
    name: 'check_prepayments_supported',
    description: 'Check whether the prepayments backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.prepayments.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_prepayments',
    description: 'List supplier prepayments with optional filtering.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Filter by supplier ID'),
      status: z
        .enum(['open', 'applied', 'refunded', 'cancelled'])
        .optional()
        .describe('Filter by status'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const prepayments = await commerce.prepayments.list({
        supplierId: params.supplierId,
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: prepayments.length, prepayments };
    },
  },
  {
    name: 'get_prepayment',
    description: 'Get a prepayment by ID.',
    inputSchema: {
      prepaymentId: z.string().min(1).describe('Prepayment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const prepayment = await commerce.prepayments.get(params.prepaymentId);
      if (!prepayment) {
        return { success: false, error: 'Prepayment not found' };
      }
      return { success: true, prepayment };
    },
  },
  {
    name: 'create_prepayment',
    description: 'Create a supplier prepayment.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      amount: z.string().min(1).describe('Amount as an exact decimal string, e.g. "1000.00"'),
      currency: z.string().min(1).optional().describe('Currency code, e.g. "USD"'),
      method: z.string().min(1).optional().describe('Payment method, e.g. "wire", "ach"'),
      reference: z.string().min(1).optional().describe('Optional reference'),
      memo: z.string().max(2000).optional().describe('Optional memo'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create prepayment', params);
      }

      const prepayment = await commerce.prepayments.create({
        supplierId: params.supplierId,
        amount: params.amount,
        currency: params.currency,
        method: params.method,
        reference: params.reference,
        memo: params.memo,
      });
      return { success: true, message: 'Prepayment created', prepayment };
    },
  },
  {
    name: 'apply_prepayment',
    description: 'Apply a prepayment against a bill or payment obligation.',
    inputSchema: {
      prepaymentId: z.string().min(1).describe('Prepayment ID'),
      targetType: z.enum(['bill', 'payment_obligation']).describe('Application target type'),
      targetId: z.string().min(1).describe('Target ID'),
      amount: z.string().min(1).describe('Amount to apply as an exact decimal string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Apply prepayment', params);
      }

      const prepayment = await commerce.prepayments.apply(params.prepaymentId, {
        targetType: params.targetType,
        targetId: params.targetId,
        amount: params.amount,
      });
      return { success: true, message: 'Prepayment applied', prepayment };
    },
  },
  {
    name: 'list_prepayment_applications',
    description: 'List applications for a prepayment.',
    inputSchema: {
      prepaymentId: z.string().min(1).describe('Prepayment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const applications = await commerce.prepayments.listApplications(params.prepaymentId);
      return { success: true, count: applications.length, applications };
    },
  },
  {
    name: 'reverse_prepayment_application',
    description: 'Reverse a previously-recorded prepayment application.',
    inputSchema: {
      prepaymentId: z.string().min(1).describe('Prepayment ID'),
      applicationId: z.string().min(1).describe('Application ID to reverse'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Reverse prepayment application', params);
      }

      const prepayment = await commerce.prepayments.reverseApplication(
        params.prepaymentId,
        params.applicationId,
      );
      return { success: true, message: 'Prepayment application reversed', prepayment };
    },
  },
  {
    name: 'refund_prepayment',
    description: 'Refund the remaining balance of a prepayment, closing it.',
    inputSchema: {
      prepaymentId: z.string().min(1).describe('Prepayment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Refund prepayment', params);
      }

      const prepayment = await commerce.prepayments.refund(params.prepaymentId);
      return { success: true, message: 'Prepayment refunded', prepayment };
    },
  },
]);

export default prepaymentTools;
