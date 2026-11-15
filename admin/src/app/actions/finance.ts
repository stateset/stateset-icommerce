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

import { z } from 'zod';
import { addDecimal, isPositiveDecimal } from '@/lib/finance/decimal';
import { requireFinanceActor } from '@/lib/shared/finance-actor';

import {
  type AutoPostingConfigInput,
  type AutoPostingConfig,
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
import { closeMonthArgsSchema, runCloseMonthArgsSchema, validateArgs } from '@/lib/shared/schemas';

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
  const args = validateArgs({ periodId }, closeMonthArgsSchema);
  return generalLedgerApi.closeMonth(args.periodId, { dryRun: true });
}

/**
 * Run the REAL month-end close. Posts closing entries and closes the period —
 * this is not reversible from the admin UI. The close page requires an
 * explicit typed confirmation before calling this.
 */
export async function runCloseMonth(
  periodId: string,
  _closedBy?: string,
): Promise<CloseMonthReport> {
  await requireAdminSession();
  const args = validateArgs({ periodId }, runCloseMonthArgsSchema);
  const actor = await requireFinanceActor();
  return generalLedgerApi.closeMonth(args.periodId, {
    dryRun: false,
    closedBy: actor,
  });
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
  currency: string;
  current: string;
  days130: string;
  days3160: string;
  days6190: string;
  daysOver90: string;
  total: string;
}

/** Trailing window used for the DSO stat on the receivables page. */
const DSO_WINDOW_DAYS = 30;

function bucketByCustomer(invoices: Invoice[]): ArCustomerAging[] {
  const now = Date.now();
  const dayMs = 24 * 60 * 60 * 1000;
  const byCustomer = new Map<string, ArCustomerAging>();

  for (const invoice of invoices) {
    const amountDue = invoice.balanceDue ?? addDecimal(invoice.total, invoice.amountPaid, true);
    if (
      !['sent', 'viewed', 'partially_paid', 'overdue'].includes(invoice.status) ||
      !isPositiveDecimal(amountDue)
    ) {
      continue;
    }
    const row =
      byCustomer.get(`${invoice.customerId}:${invoice.currency ?? 'USD'}`) ||
      ({
        customerId: invoice.customerId,
        currency: invoice.currency ?? 'USD',
        current: '0',
        days130: '0',
        days3160: '0',
        days6190: '0',
        daysOver90: '0',
        total: '0',
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
    row[bucket] = addDecimal(row[bucket], amountDue);
    row.total = addDecimal(row.total, amountDue);
    byCustomer.set(`${invoice.customerId}:${row.currency}`, row);
  }

  return Array.from(byCustomer.values()).sort((left, right) =>
    left.customerId.localeCompare(right.customerId),
  );
}

export async function getArAgingSummary(): Promise<ArAgingSummary> {
  await requireAdminSession();
  return accountsReceivableApi.getAgingSummary();
}

/** Aging summary + DSO + per-customer aging in one round trip. */
export async function getReceivablesPageData(currency = 'USD'): Promise<{
  aging: ArAgingSummary;
  /** Null when DSO is unavailable or would mix invoice currencies. */
  dso: string | number | null;
  dsoWindowDays: number;
  customers: ArCustomerAging[];
  currencies: string[];
}> {
  await requireAdminSession();
  const [dso, invoices] = await Promise.all([
    accountsReceivableApi.getDso(DSO_WINDOW_DAYS),
    accountsReceivableApi.listInvoices(),
  ]);
  const invoiceCurrencies = new Set(invoices.map((invoice) => invoice.currency ?? 'USD'));
  const customers = bucketByCustomer(
    invoices.filter((invoice) => (invoice.currency ?? 'USD') === currency),
  );
  const exactAging: ArAgingSummary = {
    current: '0',
    days130: '0',
    days3160: '0',
    days6190: '0',
    daysOver90: '0',
    total: '0',
  };
  for (const row of customers)
    for (const key of [
      'current',
      'days130',
      'days3160',
      'days6190',
      'daysOver90',
      'total',
    ] as const)
      exactAging[key] = addDecimal(exactAging[key], row[key]);
  return {
    aging: exactAging,
    dso:
      invoiceCurrencies.size <= 1 &&
      (invoiceCurrencies.size === 0 || invoiceCurrencies.has(currency))
        ? dso
        : null,
    dsoWindowDays: DSO_WINDOW_DAYS,
    customers,
    currencies: [...new Set(['USD', ...invoices.map((i) => i.currency ?? 'USD')])].sort(),
  };
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
export async function getBillsPageData(currency = 'USD'): Promise<{
  bills: Bill[];
  aging: ApAgingSummary;
  currencies: string[];
}> {
  await requireAdminSession();
  const bills = await accountsPayableApi.listBills();
  const aging: ApAgingSummary = {
    current: '0',
    days130: '0',
    days3160: '0',
    days6190: '0',
    daysOver90: '0',
    total: '0',
  };
  for (const bill of bills) {
    if (
      (bill.currency ?? 'USD') !== currency ||
      !isPositiveDecimal(bill.amountDue) ||
      ['draft', 'pending', 'cancelled', 'paid'].includes(bill.status.toLowerCase())
    )
      continue;
    const days = Math.floor((Date.now() - Date.parse(bill.dueDate)) / 86400000);
    const key =
      days <= 0
        ? 'current'
        : days <= 30
          ? 'days130'
          : days <= 60
            ? 'days3160'
            : days <= 90
              ? 'days6190'
              : 'daysOver90';
    aging[key] = addDecimal(aging[key], bill.amountDue);
    aging.total = addDecimal(aging.total, bill.amountDue);
  }
  return {
    bills,
    aging,
    currencies: [...new Set(['USD', ...bills.map((b) => b.currency ?? 'USD')])].sort(),
  };
}

const postingConfigSchema = z.object({
  configName: z.string().trim().min(1),
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
  autoPostDepreciation: z.boolean(),
  autoPostRevenueRecognition: z.boolean(),
});

export async function getGlPostingConfig(): Promise<AutoPostingConfig | null> {
  await requireAdminSession();
  return generalLedgerApi.getAutoPostingConfig();
}

export async function setGlPostingConfig(
  input: AutoPostingConfigInput,
): Promise<AutoPostingConfig> {
  await requireFinanceActor();
  return generalLedgerApi.setAutoPostingConfig(postingConfigSchema.parse(input));
}
