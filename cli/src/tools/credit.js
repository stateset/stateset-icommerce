/**
 * Credit Tools Module
 *
 * MCP tool definitions for customer credit accounts and checks.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const creditTools = withPolicyDomain('credit', [
  {
    name: 'list_credit_accounts',
    description: 'List credit accounts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const creditAccounts = await commerce.credit.listCreditAccounts();
      return { success: true, count: creditAccounts.length, creditAccounts };
    },
  },
  {
    name: 'get_credit_account',
    description: 'Get a credit account by account ID or customer ID.',
    inputSchema: {
      creditAccountId: z.string().min(1).optional().describe('Credit account ID'),
      customerId: z.string().min(1).optional().describe('Customer ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const creditAccount = params.creditAccountId
        ? await commerce.credit.getCreditAccount(params.creditAccountId)
        : params.customerId
          ? await commerce.credit.getCreditAccountByCustomer(params.customerId)
          : null;
      if (!creditAccount) {
        return { success: false, error: 'Credit account not found' };
      }
      return { success: true, creditAccount };
    },
  },
  {
    name: 'create_credit_account',
    description: 'Create a customer credit account.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      creditLimit: z.number().positive().describe('Credit limit'),
      paymentTerms: z.string().min(1).optional().describe('Payment terms'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create credit account', params);
      }

      const creditAccount = await commerce.credit.createCreditAccount({
        customerId: params.customerId,
        creditLimit: params.creditLimit,
        paymentTerms: params.paymentTerms,
        notes: params.notes,
      });
      return { success: true, message: 'Credit account created', creditAccount };
    },
  },
  {
    name: 'check_customer_credit',
    description: 'Check customer credit availability for an order amount.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      orderAmount: z.number().positive().describe('Order amount'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const creditCheck = await commerce.credit.checkCredit(params.customerId, params.orderAmount);
      return { success: true, creditCheck };
    },
  },
  {
    name: 'adjust_credit_limit',
    description: 'Adjust a customer credit limit.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      newLimit: z.number().positive().describe('New credit limit'),
      reason: z.string().min(1).max(2000).describe('Adjustment reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Adjust credit limit', params);
      }

      const creditAccount = await commerce.credit.adjustCreditLimit(
        params.customerId,
        params.newLimit,
        params.reason,
      );
      return { success: true, message: 'Credit limit adjusted', creditAccount };
    },
  },
  {
    name: 'suspend_credit_account',
    description: 'Suspend a customer credit account.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      reason: z.string().min(1).max(2000).describe('Suspension reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Suspend credit account', params);
      }

      const creditAccount = await commerce.credit.suspendCreditAccount(
        params.customerId,
        params.reason,
      );
      return { success: true, message: 'Credit account suspended', creditAccount };
    },
  },
  {
    name: 'reactivate_credit_account',
    description: 'Reactivate a customer credit account.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Reactivate credit account', params);
      }

      const creditAccount = await commerce.credit.reactivateCreditAccount(params.customerId);
      return { success: true, message: 'Credit account reactivated', creditAccount };
    },
  },
  {
    name: 'list_over_limit_credit_accounts',
    description: 'List over-limit credit accounts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const creditAccounts = await commerce.credit.getOverLimitCustomers();
      return { success: true, count: creditAccounts.length, creditAccounts };
    },
  },
]);

export default creditTools;
