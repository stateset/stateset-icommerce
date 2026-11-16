/**
 * Accounts Receivable Tools Module
 *
 * MCP tool definitions for AR reporting and credit memos.
 */

import { z } from 'zod';
import { positiveDecimalString } from '../utils/decimal-schema.js';
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
      amount: positiveDecimalString.describe('Credit memo amount'),
      reason: z.string().min(1).describe('Reason'),
      originalInvoiceId: z.string().min(1).optional().describe('Optional invoice ID'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'admin',
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
    permission: 'admin',
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
  {
    name: 'apply_payment_to_invoices',
    description: 'Apply a completed customer payment to issued invoices',
    permission: 'admin',
    inputSchema: {
      paymentId: z.string().uuid(),
      applications: z
        .array(z.object({ invoiceId: z.string().uuid(), amount: positiveDecimalString }))
        .min(1),
    },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply)
        return applyRequired('Apply a completed customer payment to issued invoices', params);
      return {
        success: true,
        result: await commerce.accountsReceivable.applyPaymentToInvoices(params),
      };
    },
  },
  {
    name: 'list_invoice_payment_applications',
    description: 'List allocations for a customer payment',
    permission: 'read',
    inputSchema: { paymentId: z.string().uuid() },
    handler: async ({ commerce, params }) => {
      return {
        success: true,
        result: await commerce.accountsReceivable.getPaymentApplications(params.paymentId),
      };
    },
  },
  {
    name: 'unapply_invoice_payment',
    description: 'Reverse a customer payment allocation',
    permission: 'admin',
    inputSchema: { applicationId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Reverse a customer payment allocation', params);
      return {
        success: true,
        result:
          (await commerce.accountsReceivable.unapplyPayment(params.applicationId),
          { unapplied: true }),
      };
    },
  },
  {
    name: 'apply_credit_memo',
    description: 'Apply customer credit to an issued invoice',
    permission: 'admin',
    inputSchema: {
      creditMemoId: z.string().uuid(),
      invoiceId: z.string().uuid(),
      amount: positiveDecimalString,
    },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Apply customer credit to an issued invoice', params);
      return { success: true, result: await commerce.accountsReceivable.applyCreditMemo(params) };
    },
  },
  {
    name: 'create_write_off',
    description: 'Record a partial or full bad-debt write-off',
    permission: 'admin',
    inputSchema: {
      invoiceId: z.string().uuid(),
      amount: positiveDecimalString,
      reason: z.string().min(1),
      notes: z.string().optional(),
    },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Record a partial or full bad-debt write-off', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.accountsReceivable.createWriteOff(params, actorId),
      };
    },
  },
  {
    name: 'reverse_write_off',
    description: 'Reverse a bad-debt write-off',
    permission: 'admin',
    inputSchema: { writeOffId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Reverse a bad-debt write-off', params);
      return {
        success: true,
        result: await commerce.accountsReceivable.reverseWriteOff(params.writeOffId),
      };
    },
  },
  {
    name: 'list_write_offs',
    description: 'List bad-debt write-offs and their reversals',
    permission: 'read',
    inputSchema: {
      customerId: z.string().uuid().optional(),
      invoiceId: z.string().uuid().optional(),
      reason: z.string().optional(),
      includeReversed: z.boolean().optional(),
      fromDate: z.string().datetime().optional(),
      toDate: z.string().datetime().optional(),
      limit: z.number().int().min(1).max(200).optional(),
      offset: z.number().int().nonnegative().optional(),
    },
    handler: async ({ commerce, params }) => {
      return { success: true, result: await commerce.accountsReceivable.listWriteOffs(params) };
    },
  },
]);

export default accountsReceivableTools;
