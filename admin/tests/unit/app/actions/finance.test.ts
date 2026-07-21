/**
 * Tests for the finance server actions (GL + AP).
 *
 * Server actions bypass the API middleware, so every exported action in
 * `@/app/actions/finance` must enforce the admin session itself via
 * `requireAdminSession()`. These tests lock down that contract, plus the
 * dry-run/real-close option wiring and input validation.
 *
 * @module tests/unit/app/actions/finance
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { ADMIN_SESSION_COOKIE } from '@/lib/shared/auth-session';

// Mock next/headers cookies() — must be before imports
const cookieStore = vi.hoisted(() => new Map<string, { value: string }>());
vi.mock('next/headers', () => ({
  cookies: vi.fn(() =>
    Promise.resolve({
      get: (name: string) => cookieStore.get(name),
      set: (name: string, value: string, _opts?: unknown) => {
        cookieStore.set(name, { value });
      },
      delete: (name: string) => {
        cookieStore.delete(name);
      },
    })
  ),
}));

// Mock the embedded engine wrappers. Enumerate every named export explicitly
// (a Proxy-based mock makes the module look thenable and hangs the import).
vi.mock('@/lib/embedded', () => ({
  generalLedgerApi: {
    listAccounts: vi.fn().mockResolvedValue([]),
    getTrialBalance: vi.fn().mockResolvedValue({
      asOfDate: '2026-07-20',
      totalDebits: 100,
      totalCredits: 100,
      isBalanced: true,
    }),
    listJournalEntries: vi.fn().mockResolvedValue([]),
    listPeriods: vi.fn().mockResolvedValue([]),
    closeMonth: vi.fn().mockResolvedValue({
      periodId: 'per_1',
      periodName: '2026-02',
      dryRun: true,
      depreciation: { status: 'dry_run', entryCount: 0, totalAmount: '0.00', warnings: [] },
      revenueRecognition: { status: 'dry_run', entryCount: 0, totalAmount: '0.00', warnings: [] },
      fxRevaluation: { status: 'dry_run', entryCount: 0, totalAmount: '0.00', warnings: [] },
      periodClose: { status: 'dry_run', entryCount: 0, totalAmount: '0.00', warnings: [] },
      periodStatus: 'open',
    }),
  },
  accountsPayableApi: {
    listBills: vi.fn().mockResolvedValue([]),
    getAgingSummary: vi.fn().mockResolvedValue({
      current: 0,
      days130: 0,
      days3160: 0,
      days6190: 0,
      daysOver90: 0,
      total: 0,
    }),
  },
  accountsReceivableApi: {
    getAgingSummary: vi.fn().mockResolvedValue({
      current: 0,
      days130: 0,
      days3160: 0,
      days6190: 0,
      daysOver90: 0,
      total: 0,
    }),
    getDso: vi.fn().mockResolvedValue(31.5),
    listInvoices: vi.fn().mockResolvedValue([]),
  },
  fixedAssetsApi: {
    list: vi.fn().mockResolvedValue([]),
    getSchedule: vi.fn().mockResolvedValue(null),
  },
  revenueRecognitionApi: {
    listContracts: vi.fn().mockResolvedValue([]),
  },
}));

import {
  getGlAccounts,
  getTrialBalance,
  getJournalEntries,
  getLedgerPageData,
  getGlPeriods,
  closeMonthDryRun,
  runCloseMonth,
  getBills,
  getApAgingSummary,
  getBillsPageData,
  getArAgingSummary,
  getReceivablesPageData,
  getFixedAssets,
  getAssetDepreciationSchedule,
  getRevenueContracts,
} from '@/app/actions/finance';
import {
  generalLedgerApi,
  accountsPayableApi,
  accountsReceivableApi,
  fixedAssetsApi,
  revenueRecognitionApi,
} from '@/lib/embedded';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('finance actions auth guard', () => {
  describe('without a session', () => {
    it('rejects GL reads and never reaches the embedded engine', async () => {
      await expect(getGlAccounts()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getTrialBalance('2026-07-20')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getJournalEntries()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getLedgerPageData('2026-07-20')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getGlPeriods()).rejects.toMatchObject(UNAUTHORIZED);
      expect(generalLedgerApi.listAccounts).not.toHaveBeenCalled();
      expect(generalLedgerApi.getTrialBalance).not.toHaveBeenCalled();
    });

    it('rejects AP reads', async () => {
      await expect(getBills()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getApAgingSummary()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getBillsPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(accountsPayableApi.listBills).not.toHaveBeenCalled();
    });

    it('rejects AR, fixed-asset, and revenue reads', async () => {
      await expect(getArAgingSummary()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getReceivablesPageData()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getFixedAssets()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getAssetDepreciationSchedule('fa_1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getRevenueContracts()).rejects.toMatchObject(UNAUTHORIZED);
      expect(accountsReceivableApi.getAgingSummary).not.toHaveBeenCalled();
      expect(fixedAssetsApi.list).not.toHaveBeenCalled();
      expect(revenueRecognitionApi.listContracts).not.toHaveBeenCalled();
    });

    it('rejects close-month actions', async () => {
      await expect(closeMonthDryRun('per_1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(runCloseMonth('per_1')).rejects.toMatchObject(UNAUTHORIZED);
      expect(generalLedgerApi.closeMonth).not.toHaveBeenCalled();
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('allows GL reads through to the embedded engine', async () => {
      await expect(getGlAccounts()).resolves.toEqual([]);
      expect(generalLedgerApi.listAccounts).toHaveBeenCalled();

      const trialBalance = await getTrialBalance('2026-07-20');
      expect(trialBalance.isBalanced).toBe(true);
      expect(generalLedgerApi.getTrialBalance).toHaveBeenCalledWith('2026-07-20');
    });

    it('aggregates the ledger page data in one action', async () => {
      const result = await getLedgerPageData('2026-07-01');
      expect(result).toMatchObject({
        accounts: [],
        journalEntries: [],
        trialBalance: { isBalanced: true },
      });
      expect(generalLedgerApi.getTrialBalance).toHaveBeenCalledWith('2026-07-01');
      expect(generalLedgerApi.listAccounts).toHaveBeenCalled();
      expect(generalLedgerApi.listJournalEntries).toHaveBeenCalled();
    });

    it('rejects a malformed as-of date before touching the engine', async () => {
      await expect(getTrialBalance('20-07-2026')).rejects.toThrow(/ISO date/);
      await expect(getLedgerPageData('not-a-date')).rejects.toThrow(/ISO date/);
      expect(generalLedgerApi.getTrialBalance).not.toHaveBeenCalled();
    });

    it('aggregates bills + aging for the bills page', async () => {
      const result = await getBillsPageData();
      expect(result).toMatchObject({ bills: [], aging: { total: 0 } });
      expect(accountsPayableApi.listBills).toHaveBeenCalled();
      expect(accountsPayableApi.getAgingSummary).toHaveBeenCalled();
    });

    it('closeMonthDryRun always forces dryRun: true', async () => {
      await closeMonthDryRun('per_1');
      expect(generalLedgerApi.closeMonth).toHaveBeenCalledWith('per_1', { dryRun: true });
    });

    it('runCloseMonth passes dryRun: false and the closer identity', async () => {
      await runCloseMonth('per_1', 'dom@stateset.com');
      expect(generalLedgerApi.closeMonth).toHaveBeenCalledWith('per_1', {
        dryRun: false,
        closedBy: 'dom@stateset.com',
      });
    });

    it('aggregates aging + DSO + per-customer rows for the receivables page', async () => {
      vi.mocked(accountsReceivableApi.listInvoices).mockResolvedValueOnce([
        {
          id: 'inv_1',
          invoiceNumber: 'INV-1',
          customerId: 'cus_1',
          status: 'sent',
          subtotal: 100,
          taxAmount: 0,
          total: 100,
          amountPaid: 0,
          // Far in the past — lands in the 90+ bucket
          dueDate: '2020-01-01',
          createdAt: '2020-01-01T00:00:00.000Z',
          updatedAt: '2020-01-01T00:00:00.000Z',
        },
        {
          id: 'inv_2',
          invoiceNumber: 'INV-2',
          customerId: 'cus_1',
          status: 'paid',
          subtotal: 50,
          taxAmount: 0,
          total: 50,
          amountPaid: 50,
          dueDate: '2020-01-01',
          createdAt: '2020-01-01T00:00:00.000Z',
          updatedAt: '2020-01-01T00:00:00.000Z',
        },
      ]);

      const result = await getReceivablesPageData();
      expect(result.dso).toBe(31.5);
      expect(result.dsoWindowDays).toBeGreaterThan(0);
      expect(result.aging.total).toBe(0);
      // Paid invoice excluded; open invoice bucketed at 90+
      expect(result.customers).toEqual([
        {
          customerId: 'cus_1',
          current: 0,
          days130: 0,
          days3160: 0,
          days6190: 0,
          daysOver90: 100,
          total: 100,
        },
      ]);
    });

    it('passes the status filter through to fixed assets and validates it', async () => {
      await getFixedAssets();
      expect(fixedAssetsApi.list).toHaveBeenCalledWith(undefined);
      await getFixedAssets('in_service');
      expect(fixedAssetsApi.list).toHaveBeenCalledWith({ status: 'in_service' });
      await expect(getFixedAssets('   ')).rejects.toThrow(/required/);
    });

    it('rejects an empty asset id before touching the engine', async () => {
      await expect(getAssetDepreciationSchedule('  ')).rejects.toThrow(/required/);
      expect(fixedAssetsApi.getSchedule).not.toHaveBeenCalled();
      await getAssetDepreciationSchedule('fa_1');
      expect(fixedAssetsApi.getSchedule).toHaveBeenCalledWith('fa_1');
    });

    it('passes the status filter through to revenue contracts and validates it', async () => {
      await getRevenueContracts();
      expect(revenueRecognitionApi.listContracts).toHaveBeenCalledWith(undefined);
      await getRevenueContracts('active');
      expect(revenueRecognitionApi.listContracts).toHaveBeenCalledWith({ status: 'active' });
      await expect(getRevenueContracts('')).rejects.toThrow(/required/);
    });

    it('rejects an empty period id before touching the engine', async () => {
      await expect(closeMonthDryRun('   ')).rejects.toThrow(/required/);
      await expect(runCloseMonth('')).rejects.toThrow(/required/);
      expect(generalLedgerApi.closeMonth).not.toHaveBeenCalled();
    });
  });

  describe('when admin auth is disabled (dev mode)', () => {
    it('skips the session requirement, mirroring the middleware bypass', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
      await expect(getGlPeriods()).resolves.toEqual([]);
      expect(generalLedgerApi.listPeriods).toHaveBeenCalled();
    });

    it('still requires a session in production even with the flag set', async () => {
      vi.stubEnv('NODE_ENV', 'production');
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
      await expect(getGlPeriods()).rejects.toMatchObject(UNAUTHORIZED);
      expect(generalLedgerApi.listPeriods).not.toHaveBeenCalled();
    });
  });
});
