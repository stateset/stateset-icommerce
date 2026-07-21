/**
 * Accounts Payable Tools Module
 *
 * MCP tool definitions for supplier bills and AP reporting.
 */

import { z } from 'zod';
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
      supplierId: z.string().min(1).describe('Supplier ID'),
      dueDate: z.string().min(1).describe('Due date in ISO 8601'),
      paymentTerms: z.string().min(1).optional().describe('Payment terms'),
      referenceNumber: z.string().min(1).optional().describe('Reference number'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create bill', params);
      }

      const bill = await commerce.accountsPayable.createBill({
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
    description: 'Approve a bill.',
    inputSchema: {
      billId: z.string().min(1).describe('Bill ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Approve bill', params);
      }

      const bill = await commerce.accountsPayable.approveBill(params.billId);
      return { success: true, message: 'Bill approved', bill };
    },
  },
  {
    name: 'cancel_bill',
    description: 'Cancel a bill.',
    inputSchema: {
      billId: z.string().min(1).describe('Bill ID'),
    },
    permission: 'write',
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
]);

export default accountsPayableTools;
