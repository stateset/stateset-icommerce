/**
 * General Ledger Tools Module
 *
 * MCP tool definitions for GL accounts, journal entries, and statements.
 */

import { z } from 'zod';
import { nonnegativeDecimalString, positiveDecimalString } from '../utils/decimal-schema.js';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const generalLedgerTools = withPolicyDomain('general_ledger', [
  {
    name: 'get_gl_posting_config',
    description: 'Read the active automatic ledger posting settings.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => ({
      success: true,
      config: await commerce.generalLedger.getAutoPostingConfig(),
    }),
  },
  {
    name: 'set_gl_posting_config',
    description:
      'Configure accounts for automatic ledger postings. Replaces the active settings atomically.',
    inputSchema: {
      configName: z.string().min(1),
      cashAccountId: z.string().uuid(),
      accountsReceivableAccountId: z.string().uuid(),
      inventoryAccountId: z.string().uuid(),
      accountsPayableAccountId: z.string().uuid(),
      unearnedRevenueAccountId: z.string().uuid().optional(),
      salesRevenueAccountId: z.string().uuid(),
      shippingRevenueAccountId: z.string().uuid().optional(),
      salesTaxPayableAccountId: z.string().uuid().optional(),
      goodsReceivedNotInvoicedAccountId: z.string().uuid().optional(),
      inventoryAdjustmentAccountId: z.string().uuid().optional(),
      workInProgressAccountId: z.string().uuid().optional(),
      cogsAccountId: z.string().uuid(),
      badDebtExpenseAccountId: z.string().uuid().optional(),
      fxGainLossAccountId: z.string().uuid().optional(),
      autoPostDepreciation: z.boolean().default(false),
      autoPostRevenueRecognition: z.boolean().default(false),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Configure ledger posting', params);
      return { success: true, config: await commerce.generalLedger.setAutoPostingConfig(params) };
    },
  },
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
    permission: 'admin',
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
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) {
        return applyRequired('Post journal entry', params);
      }

      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      const journalEntry = await commerce.generalLedger.postJournalEntry(
        params.journalEntryId,
        actorId,
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
    permission: 'admin',
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
    name: 'revalue_gl',
    description: 'Revalue foreign-currency general ledger balances as of a date.',
    inputSchema: {
      asOfDate: z.string().min(1).describe('As-of date in ISO 8601'),
      baseCurrency: z.string().min(1).max(10).optional().describe('Optional base currency code'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Revalue general ledger', params);
      }

      const revaluation = await commerce.generalLedger.revalue(
        params.asOfDate,
        params.baseCurrency,
      );
      return { success: true, message: 'General ledger revalued', revaluation };
    },
  },
  {
    name: 'close_month',
    description:
      'Close the month: post scheduled depreciation, recognize revenue through period end, ' +
      'revalue foreign-currency balances, then run the period close. ' +
      'Use dryRun to preview per-step counts and amounts without writing.',
    inputSchema: {
      periodId: z.string().min(1).describe('Accounting period ID'),
      dryRun: z.boolean().optional().describe('Preview only; nothing is written'),
      skipDepreciation: z.boolean().optional().describe('Skip posting scheduled depreciation'),
      skipRevenueRecognition: z.boolean().optional().describe('Skip revenue recognition'),
      skipFxRevaluation: z.boolean().optional().describe('Skip FX revaluation'),
      skipPeriodClose: z.boolean().optional().describe('Skip closing entries and period close'),
      failOnWarnings: z
        .boolean()
        .optional()
        .describe('Refuse with an error when any step reports warnings (strict mode)'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply, actorId }) => {
      const dryRun = params.dryRun === true;
      if (!dryRun && !allowApply) {
        return applyRequired('Close month', params);
      }

      if (!actorId && !dryRun)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      const report = await commerce.generalLedger.closeMonth(params.periodId, {
        dryRun,
        skipDepreciation: params.skipDepreciation,
        skipRevenueRecognition: params.skipRevenueRecognition,
        skipFxRevaluation: params.skipFxRevaluation,
        skipPeriodClose: params.skipPeriodClose,
        failOnWarnings: params.failOnWarnings,
        closedBy: actorId || undefined,
      });
      return {
        success: true,
        message: dryRun ? 'Close month dry run computed' : 'Month closed',
        report,
      };
    },
  },
  {
    name: 'create_gl_period',
    description: 'Create an accounting period.',
    inputSchema: {
      periodName: z.string().min(1).describe('Period name, e.g. "January 2026"'),
      fiscalYear: z.number().int().describe('Fiscal year'),
      periodNumber: z.number().int().min(1).max(13).describe('Period number within the year'),
      startDate: z.string().min(1).describe('Start date in ISO 8601'),
      endDate: z.string().min(1).describe('End date in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create GL period', params);
      }

      const period = await commerce.generalLedger.createPeriod({
        periodName: params.periodName,
        fiscalYear: params.fiscalYear,
        periodNumber: params.periodNumber,
        startDate: params.startDate,
        endDate: params.endDate,
      });
      return { success: true, message: 'Period created', period };
    },
  },
  {
    name: 'list_gl_periods',
    description: 'List accounting periods with optional filtering.',
    inputSchema: {
      fiscalYear: z.number().int().optional().describe('Filter by fiscal year'),
      status: z
        .enum(['future', 'open', 'closed', 'locked'])
        .optional()
        .describe('Filter by status'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const periods = await commerce.generalLedger.listPeriods({
        fiscalYear: params.fiscalYear,
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: periods.length, periods };
    },
  },
  {
    name: 'open_gl_period',
    description: 'Open an accounting period so journal entries can be posted to it.',
    inputSchema: {
      periodId: z.string().min(1).describe('Accounting period ID'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Open GL period', params);
      }

      const period = await commerce.generalLedger.openPeriod(params.periodId);
      return { success: true, message: 'Period opened', period };
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

  {
    name: 'create_journal_entry',
    description: 'Create a balanced journal draft or post it atomically with exact decimal lines.',
    permission: 'admin',
    inputSchema: {
      entryDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
      entryType: z.enum(['standard', 'adjusting', 'closing', 'reversing', 'opening']).optional(),
      description: z.string().min(1),
      lines: z
        .array(
          z.object({
            accountId: z.string().uuid(),
            description: z.string().optional(),
            debitAmount: nonnegativeDecimalString.optional(),
            creditAmount: nonnegativeDecimalString.optional(),
          }),
        )
        .min(2),
      autoPost: z.boolean().optional(),
      currency: z.string().length(3).optional(),
      exchangeRate: positiveDecimalString.optional(),
    },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Create journal entry', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        journalEntry: await commerce.generalLedger.createJournalEntry(params, actorId),
      };
    },
  },
  {
    name: 'reverse_journal_entry',
    description: 'Reverse a posted entry on an open accounting date.',
    permission: 'admin',
    inputSchema: {
      journalEntryId: z.string().uuid(),
      reversalDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
    },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Reverse journal entry', params);
      return {
        success: true,
        journalEntry: await commerce.generalLedger.reverseJournalEntry(
          params.journalEntryId,
          params.reversalDate,
        ),
      };
    },
  },
  {
    name: 'get_gl_period',
    description: 'Get an accounting period.',
    permission: 'read',
    inputSchema: { periodId: z.string().uuid() },
    handler: async ({ commerce, params }) => ({
      success: true,
      period: await commerce.generalLedger.getPeriod(params.periodId),
    }),
  },
  {
    name: 'close_gl_period',
    description: 'Close an accounting period.',
    permission: 'admin',
    inputSchema: { periodId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Close period', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.generalLedger.closePeriod(params.periodId, actorId),
      };
    },
  },
  {
    name: 'lock_gl_period',
    description: 'Lock an accounting period.',
    permission: 'admin',
    inputSchema: { periodId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Lock period', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.generalLedger.lockPeriod(params.periodId, actorId),
      };
    },
  },
  {
    name: 'reopen_gl_period',
    description: 'Reopen an accounting period.',
    permission: 'admin',
    inputSchema: { periodId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) return applyRequired('Reopen period', params);
      return { success: true, result: await commerce.generalLedger.reopenPeriod(params.periodId) };
    },
  },
  {
    name: 'reclose_gl_period',
    description: 'Reclose an accounting period.',
    permission: 'admin',
    inputSchema: { periodId: z.string().uuid() },
    handler: async ({ commerce, params, allowApply, actorId }) => {
      if (!allowApply) return applyRequired('Reclose period', params);
      if (!actorId)
        return {
          success: false,
          error: 'Configure --actor (STATESET_ERP_ACTOR) for financial audit identity',
        };
      return {
        success: true,
        result: await commerce.generalLedger.reclosePeriod(params.periodId, actorId),
      };
    },
  },
]);

export default generalLedgerTools;
