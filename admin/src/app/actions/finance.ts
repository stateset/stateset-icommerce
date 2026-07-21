'use server';

/**
 * Finance server actions (General Ledger + Accounts Payable).
 *
 * Every exported action is gated by `requireAdminSession()` — server actions
 * bypass the API middleware, so each one must enforce the admin session
 * itself (skipped in the auth-disabled dev mode, like middleware).
 *
 * Money handling: amounts returned by the engine (including exact decimal
 * strings on close-month steps) are passed through untouched — formatting is
 * display-only and happens in the client components.
 */

import {
  generalLedgerApi,
  accountsPayableApi,
  accountsReceivableApi,
  fixedAssetsApi,
  revenueRecognitionApi,
  type GlAccount,
  type JournalEntry,
  type TrialBalance,
  type GlPeriod,
  type CloseMonthReport,
  type Bill,
  type ApAgingSummary,
  type ArAgingSummary,
  type Invoice,
  type FixedAsset,
  type DepreciationSchedule,
  type RevenueContract,
} from '@/lib/embedded';
import { requireAdminSession } from '@/lib/shared/auth-session';

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

function assertIsoDate(value: string, label: string): void {
  if (!ISO_DATE.test(value)) {
    throw new Error(`${label} must be an ISO date (YYYY-MM-DD)`);
  }
}

function assertNonEmpty(value: string, label: string): void {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`${label} is required`);
  }
}

// ============================================================================
// General Ledger
// ============================================================================

export async function getGlAccounts(): Promise<GlAccount[]> {
  await requireAdminSession();
  return generalLedgerApi.listAccounts();
}

export async function getTrialBalance(asOfDate: string): Promise<TrialBalance> {
  await requireAdminSession();
  assertIsoDate(asOfDate, 'asOfDate');
  return generalLedgerApi.getTrialBalance(asOfDate);
}

export async function getJournalEntries(): Promise<JournalEntry[]> {
  await requireAdminSession();
  return generalLedgerApi.listJournalEntries();
}

/** Chart of accounts + trial balance in one round trip for the ledger page. */
export async function getLedgerPageData(asOfDate: string): Promise<{
  accounts: GlAccount[];
  trialBalance: TrialBalance;
  journalEntries: JournalEntry[];
}> {
  await requireAdminSession();
  assertIsoDate(asOfDate, 'asOfDate');
  const [accounts, trialBalance, journalEntries] = await Promise.all([
    generalLedgerApi.listAccounts(),
    generalLedgerApi.getTrialBalance(asOfDate),
    generalLedgerApi.listJournalEntries(),
  ]);
  return { accounts, trialBalance, journalEntries };
}

// ============================================================================
// Accounting periods + month-end close
// ============================================================================

export async function getGlPeriods(): Promise<GlPeriod[]> {
  await requireAdminSession();
  return generalLedgerApi.listPeriods();
}

/**
 * Dry-run the month-end close: computes the per-step report (depreciation,
 * revenue recognition, FX revaluation, period close) without writing anything.
 */
export async function closeMonthDryRun(periodId: string): Promise<CloseMonthReport> {
  await requireAdminSession();
  assertNonEmpty(periodId, 'periodId');
  return generalLedgerApi.closeMonth(periodId, { dryRun: true });
}

/**
 * Run the REAL month-end close. Posts closing entries and closes the period —
 * this is not reversible from the admin UI. The close page requires an
 * explicit typed confirmation before calling this.
 */
export async function runCloseMonth(
  periodId: string,
  closedBy?: string,
): Promise<CloseMonthReport> {
  await requireAdminSession();
  assertNonEmpty(periodId, 'periodId');
  return generalLedgerApi.closeMonth(periodId, { dryRun: false, closedBy });
}

// ============================================================================
// Accounts Payable
// ============================================================================

export async function getBills(): Promise<Bill[]> {
  await requireAdminSession();
  return accountsPayableApi.listBills();
}

export async function getApAgingSummary(): Promise<ApAgingSummary> {
  await requireAdminSession();
  return accountsPayableApi.getAgingSummary();
}

// ============================================================================
// Accounts Receivable
// ============================================================================

/** Per-customer AR aging computed from open invoices (display-only numbers). */
export interface ArCustomerAging {
  customerId: string;
  current: number;
  days130: number;
  days3160: number;
  days6190: number;
  daysOver90: number;
  total: number;
}

/** Trailing window used for the DSO stat on the receivables page. */
const DSO_WINDOW_DAYS = 30;

const round2 = (value: number): number => Number(value.toFixed(2));

function bucketByCustomer(invoices: Invoice[]): ArCustomerAging[] {
  const now = Date.now();
  const dayMs = 24 * 60 * 60 * 1000;
  const byCustomer = new Map<string, ArCustomerAging>();

  for (const invoice of invoices) {
    const amountDue = invoice.total - invoice.amountPaid;
    if (invoice.status === 'paid' || invoice.status === 'void' || amountDue <= 0) {
      continue;
    }
    const row =
      byCustomer.get(invoice.customerId) ||
      ({
        customerId: invoice.customerId,
        current: 0,
        days130: 0,
        days3160: 0,
        days6190: 0,
        daysOver90: 0,
        total: 0,
      } satisfies ArCustomerAging);
    const daysPastDue = Math.floor((now - Date.parse(invoice.dueDate)) / dayMs);
    const bucket =
      daysPastDue <= 0
        ? 'current'
        : daysPastDue <= 30
          ? 'days130'
          : daysPastDue <= 60
            ? 'days3160'
            : daysPastDue <= 90
              ? 'days6190'
              : 'daysOver90';
    row[bucket] = round2(row[bucket] + amountDue);
    row.total = round2(row.total + amountDue);
    byCustomer.set(invoice.customerId, row);
  }

  return Array.from(byCustomer.values()).sort((left, right) => right.total - left.total);
}

export async function getArAgingSummary(): Promise<ArAgingSummary> {
  await requireAdminSession();
  return accountsReceivableApi.getAgingSummary();
}

/** Aging summary + DSO + per-customer aging in one round trip. */
export async function getReceivablesPageData(): Promise<{
  aging: ArAgingSummary;
  /** Null when this engine build does not expose DSO. */
  dso: number | null;
  dsoWindowDays: number;
  customers: ArCustomerAging[];
}> {
  await requireAdminSession();
  const [aging, dso, invoices] = await Promise.all([
    accountsReceivableApi.getAgingSummary(),
    accountsReceivableApi.getDso(DSO_WINDOW_DAYS),
    accountsReceivableApi.listInvoices(),
  ]);
  return { aging, dso, dsoWindowDays: DSO_WINDOW_DAYS, customers: bucketByCustomer(invoices) };
}

// ============================================================================
// Fixed Assets
// ============================================================================

export async function getFixedAssets(status?: string): Promise<FixedAsset[]> {
  await requireAdminSession();
  if (status !== undefined) {
    assertNonEmpty(status, 'status');
  }
  return fixedAssetsApi.list(status ? { status } : undefined);
}

export async function getAssetDepreciationSchedule(
  assetId: string,
): Promise<DepreciationSchedule | null> {
  await requireAdminSession();
  assertNonEmpty(assetId, 'assetId');
  return fixedAssetsApi.getSchedule(assetId);
}

// ============================================================================
// Revenue Recognition
// ============================================================================

export async function getRevenueContracts(status?: string): Promise<RevenueContract[]> {
  await requireAdminSession();
  if (status !== undefined) {
    assertNonEmpty(status, 'status');
  }
  return revenueRecognitionApi.listContracts(status ? { status } : undefined);
}

/** Bills + AP aging in one round trip for the bills page. */
export async function getBillsPageData(): Promise<{
  bills: Bill[];
  aging: ApAgingSummary;
}> {
  await requireAdminSession();
  const [bills, aging] = await Promise.all([
    accountsPayableApi.listBills(),
    accountsPayableApi.getAgingSummary(),
  ]);
  return { bills, aging };
}
