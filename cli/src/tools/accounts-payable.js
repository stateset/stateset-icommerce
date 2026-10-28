/**
 * Accounts Payable Tools Module
 *
 * MCP tool definitions for supplier bills and AP reporting.
 */

import { z } from 'zod';
import { positiveDecimalString, nonnegativeDecimalString } from '../utils/decimal-schema.js';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const accountsPayableTools = withPolicyDomain('accounts_payable', [
  {
    name: 'list_bills',
    description: 'List accounts payable bills.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const bills = await commerce.accountsPayable.listBills();
      return { success: true, count: bills.length, bills };
    },
  },
  {
    name: 'get_bill',
    description: 'Get a bill by ID or bill number.',
    inputSchema: {
      billId: z.string().min(1).optional().describe('Bill ID'),
      billNumber: z.string().min(1).optional().describe('Bill number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const bill = params.billId
        ? await commerce.accountsPayable.getBill(params.billId)
        : params.billNumber
          ? await commerce.accountsPayable.getBillByNumber(params.billNumber)
          : null;
      if (!bill) {
        return { success: false, error: 'Bill not found' };
      }
      return { success: true, bill };
    },
  },
  {
    name: 'create_bill',
    description: 'Create an accounts payable bill.',
    inputSchema: {
      currency: z.string().length(3).optional(),
      purchaseOrderId: z.string().uuid().optional(),
      items: z
        .array(
          z.object({
            description: z.string().min(1),
            accountCode: z.string().optional(),
            quantity: positiveDecimalString,
            unitPrice: nonnegativeDecimalString,
            taxRate: nonnegativeDecimalString.optional(),
            poLineId: z.string().uuid().optional(),
          }),
        )
        .min(1),
      supplierId: z.string().min(1).describe('Supplier ID'),
      dueDate: z.string().min(1).describe('Due date in ISO 8601'),
      paymentTerms: z.string().min(1).optional().describe('Payment terms'),
      referenceNumber: z.string().min(1).optional().describe('Reference number'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) {
        return applyRequired('Create bill', params);
      }

      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for bill maker identity',
        };
      const bill = await commerce.accountsPayable.createBill({
        createdBy: actorId,
        items: params.items,
        currency: params.currency,
        purchaseOrderId: params.purchaseOrderId,
        supplierId: params.supplierId,
        dueDate: params.dueDate,
        paymentTerms: params.paymentTerms,
        referenceNumber: params.referenceNumber,
        notes: params.notes,
      });
      return { success: true, message: 'Bill created', bill };
    },
  },
  {
    name: 'approve_bill',
    description:
      'Approve a bill after three-way matching; a variance requires an explicit override reason.',
    inputSchema: {
      billId: z.string().min(1).describe('Bill ID'),
      overrideReason: z
        .string()
        .trim()
        .min(1)
        .max(2000)
        .optional()
        .describe('Reason for explicitly accepting a purchase/receipt mismatch'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) {
        return applyRequired('Approve bill', params);
      }

      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for bill approver identity',
        };
      const bill = await commerce.accountsPayable.approveBill(
        params.billId,
        params.overrideReason,
        actorId,
      );
      return { success: true, message: 'Bill approved', bill };
    },
  },
  {
    name: 'dispute_bill',
    description:
      'Dispute bill; resolution restores its prior approval state and current payment balance.',
    inputSchema: { billId: z.string().min(1).describe('Bill ID') },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Dispute bill', params);
      const bill = await commerce.accountsPayable.disputeBill(params.billId);
      return { success: true, bill };
    },
  },
  {
    name: 'resolve_bill_dispute',
    description:
      'Resolve bill dispute; resolution restores its prior approval state and current payment balance.',
    inputSchema: { billId: z.string().min(1).describe('Bill ID') },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Resolve bill dispute', params);
      const bill = await commerce.accountsPayable.resolveBillDispute(params.billId);
      return { success: true, bill };
    },
  },
  {
    name: 'cancel_bill',
    description: 'Cancel a bill.',
    inputSchema: {
      billId: z.string().min(1).describe('Bill ID'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel bill', params);
      }

      const bill = await commerce.accountsPayable.cancelBill(params.billId);
      return { success: true, message: 'Bill canceled', bill };
    },
  },
  {
    name: 'list_overdue_bills',
    description: 'List overdue bills.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const bills = await commerce.accountsPayable.getOverdueBills();
      return { success: true, count: bills.length, bills };
    },
  },
  {
    name: 'list_bills_due_soon',
    description: 'List bills due soon.',
    inputSchema: {
      days: z.number().int().min(0).describe('Days ahead to inspect'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const bills = await commerce.accountsPayable.getBillsDueSoon(params.days);
      return { success: true, count: bills.length, bills };
    },
  },
  {
    name: 'get_accounts_payable_aging_summary',
    description: 'Get the accounts payable aging summary.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const summary = await commerce.accountsPayable.getAgingSummary();
      return { success: true, summary };
    },
  },
  {
    name: 'get_accounts_payable_total_outstanding',
    description: 'Get the total accounts payable outstanding balance.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const totalOutstanding = await commerce.accountsPayable.getTotalOutstanding();
      return { success: true, totalOutstanding };
    },
  },
  {
    name: 'three_way_match_bill',
    description: 'Run a three-way match (bill vs purchase order vs receipt) for a bill.',
    inputSchema: {
      billId: z.string().min(1).describe('Bill ID'),
      tolerancePercent: z
        .number()
        .min(0)
        .optional()
        .describe('Optional variance tolerance percent'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const match = await commerce.accountsPayable.threeWayMatch(
        params.billId,
        params.tolerancePercent,
      );
      return { success: true, match };
    },
  },
  {
    name: 'count_accounts_payable_bills',
    description: 'Count accounts payable bills.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.accountsPayable.countBills();
      return { success: true, count };
    },
  },
  {
    name: 'create_bill_payment',
    description: 'Pay approved supplier bills with explicit allocations',
    permission: 'admin',
    inputSchema: {
      supplierId: z.string().uuid(),
      paymentDate: z.string().datetime().optional(),
      paymentMethod: z.enum(['check', 'ach', 'wire', 'credit_card', 'cash', 'other']),
      amount: positiveDecimalString,
      currency: z.string().length(3).optional(),
      referenceNumber: z.string().optional(),
      bankAccount: z.string().optional(),
      checkNumber: z.string().optional(),
      memo: z.string().optional(),
      allocations: z
        .array(z.object({ billId: z.string().uuid(), amount: positiveDecimalString }))
        .min(1),
    },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply)
        return applyRequired('Pay approved supplier bills with explicit allocations', params);
      return { success: true, result: await commerce.accountsPayable.createPayment(params) };
    },
  },
  {
    name: 'get_bill_payment',
    description: 'Get a supplier bill payment',
    permission: 'read',
    inputSchema: { paymentId: z.string().uuid() },
    handler: async ({ commerce, params }) => {
      return { success: true, result: await commerce.accountsPayable.getPayment(params.paymentId) };
    },
  },
  {
    name: 'void_bill_payment',
    description: 'Void a supplier cash payment and reverse its allocations',
    permission: 'admin',
    inputSchema: { paymentId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply)
        return applyRequired('Void a supplier cash payment and reverse its allocations', params);
      return {
        success: true,
        result: await commerce.accountsPayable.voidPayment(params.paymentId),
      };
    },
  },
  {
    name: 'create_bill_payment_run',
    description: 'Prepare a single-currency supplier payment run',
    permission: 'admin',
    inputSchema: {
      paymentDate: z.string().datetime(),
      paymentMethod: z.enum(['check', 'ach', 'wire', 'credit_card', 'cash', 'other']),
      billIds: z.array(z.string().uuid()).min(1),
      notes: z.string().optional(),
    },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply)
        return applyRequired('Prepare a single-currency supplier payment run', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.accountsPayable.createPaymentRun(params, actorId),
      };
    },
  },
  {
    name: 'get_bill_payment_run',
    description: 'Get a supplier payment run',
    permission: 'read',
    inputSchema: { runId: z.string().uuid() },
    handler: async ({ commerce, params }) => {
      return { success: true, result: await commerce.accountsPayable.getPaymentRun(params.runId) };
    },
  },
  {
    name: 'list_bill_payment_runs',
    description: 'List supplier payment runs',
    permission: 'read',
    inputSchema: {
      status: z.string().optional(),
      fromDate: z.string().datetime().optional(),
      toDate: z.string().datetime().optional(),
      limit: z.number().int().min(1).max(200).optional(),
      offset: z.number().int().nonnegative().optional(),
    },
    handler: async ({ commerce, params }) => {
      return { success: true, result: await commerce.accountsPayable.listPaymentRuns(params) };
    },
  },
  {
    name: 'approve_bill_payment_run',
    description: 'Approve a supplier payment run',
    permission: 'admin',
    inputSchema: { runId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Approve a supplier payment run', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.accountsPayable.approvePaymentRun(params.runId, actorId),
      };
    },
  },
  {
    name: 'process_bill_payment_run',
    description: 'Process a supplier payment run',
    permission: 'admin',
    inputSchema: { runId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Process a supplier payment run', params);
      return {
        success: true,
        result: await commerce.accountsPayable.processPaymentRun(params.runId),
      };
    },
  },
  {
    name: 'cancel_bill_payment_run',
    description: 'Cancel a supplier payment run',
    permission: 'admin',
    inputSchema: { runId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Cancel a supplier payment run', params);
      return {
        success: true,
        result: await commerce.accountsPayable.cancelPaymentRun(params.runId),
      };
    },
  },
]);

export default accountsPayableTools;
