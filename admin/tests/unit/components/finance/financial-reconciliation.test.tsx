// Component test for FinancialReconciliation. Mirrors the pattern used
// by the other tremor-driven dashboard tests: mock framer-motion, mock
// @tremor/react down to <div>s, and drive the useEmbeddedData hook.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getFinancialReconciliationData: vi.fn(),
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
    tr: ({ children, ...props }: HTMLAttributes<HTMLTableRowElement>) => (
      <tr {...props}>{children}</tr>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    ProgressBar: Wrapper,
    AreaChart: Chart,
    BarChart: Chart,
    DonutChart: Chart,
  };
});

import FinancialReconciliation from '@/components/finance/financial-reconciliation';

afterEach(() => {
  vi.clearAllMocks();
});

const SUCCESS_DATA = {
  summary: {
    totalReconciled: 1_000_000,
    reconciledRate: 0.95,
    pendingAmount: 5_000,
    pendingCount: 3,
    discrepancyAmount: 1_200,
    discrepancyCount: 2,
    netCash: 250_000,
  },
  cashFlow: [],
  discrepancies: {
    items: [
      {
        id: 'd-1',
        description: 'Missing receipt',
        transactionId: 'txn-9',
        source: 'gateway',
        difference: -200,
        expected: 200,
        type: 'amount',
        status: 'discrepancy',
      },
    ],
    byType: [],
  },
  transactions: [
    { id: 't-1', description: 'Order #100', amount: 500, status: 'reconciled', date: '2026-05-01' },
  ],
  reconciliationRate: { byCategory: [], overall: 0.95 },
};

describe('FinancialReconciliation', () => {
  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, isLoading: false, error: new Error('x') });
    render(<FinancialReconciliation />);
    expect(screen.getByText('Failed to load reconciliation data')).toBeInTheDocument();
  });

  it('renders metric headings and the supplied discrepancy row on success', () => {
    useEmbeddedDataMock.mockReturnValue({ data: SUCCESS_DATA, isLoading: false, error: null });
    render(<FinancialReconciliation />);
    expect(screen.getByText('Total Reconciled')).toBeInTheDocument();
    expect(screen.getByText('Pending Review')).toBeInTheDocument();
    // Both "Discrepancies" (metric) and "Discrepancy ..." headings exist;
    // "Flagged Discrepancies" is unique to the supplied-data path.
    expect(screen.getByText('Flagged Discrepancies')).toBeInTheDocument();
    expect(screen.getByText('Missing receipt')).toBeInTheDocument();
    // Reconciliation rates are 0-1 ratios.
    expect(screen.getByText('95.0% of transactions')).toBeInTheDocument();
    expect(screen.getByText('95.0% Complete')).toBeInTheDocument();
  });

  it('renders measured zeros as zeros, not stand-in figures', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        ...SUCCESS_DATA,
        // No open returns and no net cash: both are real zeros.
        summary: { ...SUCCESS_DATA.summary, pendingCount: 0, netCash: 0 },
      },
      isLoading: false,
      error: null,
    });
    render(<FinancialReconciliation />);
    expect(screen.getByText('0 transactions')).toBeInTheDocument();
    expect(screen.getByText('$0.00')).toBeInTheDocument();
    expect(screen.queryByText('23 transactions')).toBeNull();
    expect(screen.queryByText('$125,000.00')).toBeNull();
  });

  it('shows missing figures and lists as unavailable instead of demo data', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { summary: undefined },
      isLoading: false,
      error: null,
    });
    render(<FinancialReconciliation />);
    // Four amounts and three captions read "—" (not $458,000 / 98.2% / 23 ...).
    expect(screen.getAllByText('—')).toHaveLength(7);
    expect(screen.queryByText('$458,000.00')).toBeNull();
    expect(screen.queryByText(/Complete$/)).toBeNull();
    // No demo discrepancies, categories or transactions.
    expect(screen.queryByText('Payment amount mismatch')).toBeNull();
    expect(screen.queryByText('TXN-2890')).toBeNull();
    expect(screen.getByText('No flagged discrepancies')).toBeInTheDocument();
    expect(screen.getByText('No reconciliation categories')).toBeInTheDocument();
    expect(screen.getByText('No transactions')).toBeInTheDocument();
  });
});
