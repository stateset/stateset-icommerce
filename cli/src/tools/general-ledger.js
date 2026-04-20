/**
 * General Ledger Tools Module
 *
 * MCP tool definitions for GL accounts, journal entries, and statements.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const generalLedgerTools = withPolicyDomain('general_ledger', [
  {
    name: 'list_gl_accounts',
    description: 'List general ledger accounts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const accounts = await commerce.generalLedger.listAccounts();
      return { success: true, count: accounts.length, accounts };
    },
  },
  {
    name: 'get_gl_account',
    description: 'Get a general ledger account by ID or account number.',
    inputSchema: {
      accountId: z.string().min(1).optional().describe('GL account ID'),
      accountNumber: z.string().min(1).optional().describe('GL account number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const account = params.accountId
        ? await commerce.generalLedger.getAccount(params.accountId)
        : params.accountNumber
          ? await commerce.generalLedger.getAccountByNumber(params.accountNumber)
          : null;
      if (!account) {
        return { success: false, error: 'GL account not found' };
      }
      return { success: true, account };
    },
  },
  {
    name: 'create_gl_account',
    description: 'Create a general ledger account.',
    inputSchema: {
      accountNumber: z.string().min(1).describe('Account number'),
      name: z.string().min(1).describe('Account name'),
      accountType: z.string().min(1).describe('Account type'),
      description: z.string().max(2000).optional().describe('Optional description'),
      currency: z.string().min(1).max(10).optional().describe('Optional currency code'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create GL account', params);
      }

      const account = await commerce.generalLedger.createAccount({
        accountNumber: params.accountNumber,
        name: params.name,
        accountType: params.accountType,
        description: params.description,
        currency: params.currency,
      });
      return { success: true, message: 'GL account created', account };
    },
  },
  {
    name: 'initialize_chart_of_accounts',
    description: 'Initialize the standard chart of accounts.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Initialize chart of accounts');
      }

      const accounts = await commerce.generalLedger.initializeChartOfAccounts();
      return {
        success: true,
        message: 'Chart of accounts initialized',
        count: accounts.length,
        accounts,
      };
    },
  },
  {
    name: 'list_journal_entries',
    description: 'List journal entries.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const journalEntries = await commerce.generalLedger.listJournalEntries();
      return { success: true, count: journalEntries.length, journalEntries };
    },
  },
  {
    name: 'get_journal_entry',
    description: 'Get a journal entry by ID.',
    inputSchema: {
      journalEntryId: z.string().min(1).describe('Journal entry ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const journalEntry = await commerce.generalLedger.getJournalEntry(params.journalEntryId);
      if (!journalEntry) {
        return { success: false, error: 'Journal entry not found' };
      }
      return { success: true, journalEntry };
    },
  },
  {
    name: 'post_journal_entry',
    description: 'Post a journal entry.',
    inputSchema: {
      journalEntryId: z.string().min(1).describe('Journal entry ID'),
      postedBy: z.string().min(1).describe('User posting the entry'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Post journal entry', params);
      }

      const journalEntry = await commerce.generalLedger.postJournalEntry(
        params.journalEntryId,
        params.postedBy,
      );
      return { success: true, message: 'Journal entry posted', journalEntry };
    },
  },
  {
    name: 'void_journal_entry',
    description: 'Void a journal entry.',
    inputSchema: {
      journalEntryId: z.string().min(1).describe('Journal entry ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Void journal entry', params);
      }

      const journalEntry = await commerce.generalLedger.voidJournalEntry(params.journalEntryId);
      return { success: true, message: 'Journal entry voided', journalEntry };
    },
  },
  {
    name: 'get_trial_balance',
    description: 'Get the trial balance as of a date.',
    inputSchema: {
      asOfDate: z.string().min(1).describe('As-of date in ISO 8601'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const trialBalance = await commerce.generalLedger.getTrialBalance(params.asOfDate);
      return { success: true, trialBalance };
    },
  },
  {
    name: 'get_balance_sheet',
    description: 'Get the balance sheet as of a date.',
    inputSchema: {
      asOfDate: z.string().min(1).describe('As-of date in ISO 8601'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const balanceSheet = await commerce.generalLedger.getBalanceSheet(params.asOfDate);
      return { success: true, balanceSheet };
    },
  },
  {
    name: 'get_income_statement',
    description: 'Get the income statement for a date range.',
    inputSchema: {
      startDate: z.string().min(1).describe('Start date in ISO 8601'),
      endDate: z.string().min(1).describe('End date in ISO 8601'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const incomeStatement = await commerce.generalLedger.getIncomeStatement(
        params.startDate,
        params.endDate,
      );
      return { success: true, incomeStatement };
    },
  },
  {
    name: 'get_gl_account_balance',
    description: 'Get the balance of a general ledger account.',
    inputSchema: {
      accountId: z.string().min(1).describe('GL account ID'),
      asOfDate: z.string().optional().describe('Optional as-of date in ISO 8601'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const balance = await commerce.generalLedger.getAccountBalance(
        params.accountId,
        params.asOfDate,
      );
      return { success: true, accountId: params.accountId, asOfDate: params.asOfDate, balance };
    },
  },
]);

export default generalLedgerTools;
