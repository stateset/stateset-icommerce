/**
 * Accounts Receivable Tools Module
 *
 * MCP tool definitions for AR reporting and credit memos.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const accountsReceivableTools = withPolicyDomain('accounts_receivable', [
  {
    name: 'get_accounts_receivable_aging_summary',
    description: 'Get the accounts receivable aging summary.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const summary = await commerce.accountsReceivable.getAgingSummary();
      return { success: true, summary };
    },
  },
  {
    name: 'get_accounts_receivable_total_outstanding',
    description: 'Get the total accounts receivable outstanding balance.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const totalOutstanding = await commerce.accountsReceivable.getTotalOutstanding();
      return { success: true, totalOutstanding };
    },
  },
  {
    name: 'get_days_sales_outstanding',
    description: 'Get days sales outstanding over a rolling window.',
    inputSchema: {
      days: z.number().int().min(1).describe('Rolling window in days'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const dso = await commerce.accountsReceivable.getDso(params.days);
      return { success: true, days: params.days, dso };
    },
  },
  {
    name: 'list_credit_memos',
    description: 'List credit memos.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const creditMemos = await commerce.accountsReceivable.listCreditMemos();
      return { success: true, count: creditMemos.length, creditMemos };
    },
  },
  {
    name: 'get_credit_memo',
    description: 'Get a credit memo by ID.',
    inputSchema: {
      creditMemoId: z.string().min(1).describe('Credit memo ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const creditMemo = await commerce.accountsReceivable.getCreditMemo(params.creditMemoId);
      if (!creditMemo) {
        return { success: false, error: 'Credit memo not found' };
      }
      return { success: true, creditMemo };
    },
  },
  {
    name: 'create_credit_memo',
    description: 'Create a credit memo.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      amount: z.number().positive().describe('Credit memo amount'),
      reason: z.string().min(1).describe('Reason'),
      originalInvoiceId: z.string().min(1).optional().describe('Optional invoice ID'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create credit memo', params);
      }

      const creditMemo = await commerce.accountsReceivable.createCreditMemo({
        customerId: params.customerId,
        amount: params.amount,
        reason: params.reason,
        originalInvoiceId: params.originalInvoiceId,
        notes: params.notes,
      });
      return { success: true, message: 'Credit memo created', creditMemo };
    },
  },
  {
    name: 'void_credit_memo',
    description: 'Void a credit memo.',
    inputSchema: {
      creditMemoId: z.string().min(1).describe('Credit memo ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Void credit memo', params);
      }

      const creditMemo = await commerce.accountsReceivable.voidCreditMemo(params.creditMemoId);
      return { success: true, message: 'Credit memo voided', creditMemo };
    },
  },
  {
    name: 'list_unapplied_credits',
    description: 'List unapplied credits for a customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const creditMemos = await commerce.accountsReceivable.getUnappliedCredits(params.customerId);
      return { success: true, count: creditMemos.length, creditMemos };
    },
  },
]);

export default accountsReceivableTools;
