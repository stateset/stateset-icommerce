/**
 * Vendor Credit Tools Module
 *
 * MCP tool definitions for vendor credits (supplier-issued credit memos).
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const vendorCreditTools = withPolicyDomain('vendor_credits', [
  {
    name: 'check_vendor_credits_supported',
    description: 'Check whether the vendor-credits backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.vendorCredits.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_vendor_credits',
    description: 'List vendor credits with optional filtering.',
    inputSchema: {
      supplierId: z.string().min(1).optional().describe('Filter by supplier ID'),
      status: z.enum(['open', 'applied', 'cancelled']).optional().describe('Filter by status'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const vendorCredits = await commerce.vendorCredits.list({
        supplierId: params.supplierId,
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: vendorCredits.length, vendorCredits };
    },
  },
  {
    name: 'get_vendor_credit',
    description: 'Get a vendor credit by ID.',
    inputSchema: {
      vendorCreditId: z.string().min(1).describe('Vendor credit ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const vendorCredit = await commerce.vendorCredits.get(params.vendorCreditId);
      if (!vendorCredit) {
        return { success: false, error: 'Vendor credit not found' };
      }
      return { success: true, vendorCredit };
    },
  },
  {
    name: 'create_vendor_credit',
    description: 'Create a vendor credit.',
    inputSchema: {
      supplierId: z.string().min(1).describe('Supplier ID'),
      amount: z.string().min(1).describe('Amount as an exact decimal string, e.g. "250.00"'),
      currency: z.string().min(1).optional().describe('Currency code, e.g. "USD"'),
      vendorReturnId: z.string().min(1).optional().describe('Optional related vendor return ID'),
      memo: z.string().max(2000).optional().describe('Optional memo'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create vendor credit', params);
      }

      const vendorCredit = await commerce.vendorCredits.create({
        supplierId: params.supplierId,
        amount: params.amount,
        currency: params.currency,
        vendorReturnId: params.vendorReturnId,
        memo: params.memo,
      });
      return { success: true, message: 'Vendor credit created', vendorCredit };
    },
  },
  {
    name: 'apply_vendor_credit',
    description: 'Apply a vendor credit against a bill or payment obligation.',
    inputSchema: {
      vendorCreditId: z.string().min(1).describe('Vendor credit ID'),
      targetType: z.enum(['bill', 'payment_obligation']).describe('Application target type'),
      targetId: z.string().min(1).describe('Target ID'),
      amount: z.string().min(1).describe('Amount to apply as an exact decimal string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Apply vendor credit', params);
      }

      const vendorCredit = await commerce.vendorCredits.apply(params.vendorCreditId, {
        targetType: params.targetType,
        targetId: params.targetId,
        amount: params.amount,
      });
      return { success: true, message: 'Vendor credit applied', vendorCredit };
    },
  },
  {
    name: 'list_vendor_credit_applications',
    description: 'List applications for a vendor credit.',
    inputSchema: {
      vendorCreditId: z.string().min(1).describe('Vendor credit ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const applications = await commerce.vendorCredits.listApplications(params.vendorCreditId);
      return { success: true, count: applications.length, applications };
    },
  },
  {
    name: 'reverse_vendor_credit_application',
    description: 'Reverse a previously-recorded vendor credit application.',
    inputSchema: {
      vendorCreditId: z.string().min(1).describe('Vendor credit ID'),
      applicationId: z.string().min(1).describe('Application ID to reverse'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Reverse vendor credit application', params);
      }

      const vendorCredit = await commerce.vendorCredits.reverseApplication(
        params.vendorCreditId,
        params.applicationId,
      );
      return { success: true, message: 'Vendor credit application reversed', vendorCredit };
    },
  },
  {
    name: 'cancel_vendor_credit',
    description: 'Cancel a vendor credit.',
    inputSchema: {
      vendorCreditId: z.string().min(1).describe('Vendor credit ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel vendor credit', params);
      }

      const vendorCredit = await commerce.vendorCredits.cancel(params.vendorCreditId);
      return { success: true, message: 'Vendor credit cancelled', vendorCredit };
    },
  },
]);

export default vendorCreditTools;
