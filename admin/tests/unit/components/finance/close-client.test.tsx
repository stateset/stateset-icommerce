// Component test for the Finance > Close page client: period list, dry-run
// report rendering, and the typed-confirmation guard on the real close.

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
const closeMonthDryRun = vi.fn();
const runCloseMonth = vi.fn();
vi.mock('@/app/actions/finance', () => ({
  getGlPeriods: vi.fn(),
  closeMonthDryRun: (...args: unknown[]) => closeMonthDryRun(...args),
  runCloseMonth: (...args: unknown[]) => runCloseMonth(...args),
}));

import CloseClient from '@/components/finance/close-client';

afterEach(() => {
  vi.clearAllMocks();
});

const periods = [
  {
    id: 'per_closed',
    periodName: '2026-01',
    fiscalYear: 2026,
    periodNumber: 1,
    startDate: '2026-01-01',
    endDate: '2026-01-31',
    status: 'closed',
    closedBy: 'system',
  },
  {
    id: 'per_open',
    periodName: '2026-02',
    fiscalYear: 2026,
    periodNumber: 2,
    startDate: '2026-02-01',
    endDate: '2026-02-28',
    status: 'open',
  },
];

const step = (status: string) => ({
  status,
  entryCount: 3,
  totalAmount: '1250.00',
  warnings: [],
});

const dryRunReport = {
  periodId: 'per_open',
  periodName: '2026-02',
  dryRun: true,
  depreciation: step('dry_run'),
  revenueRecognition: step('dry_run'),
  fxRevaluation: {
    status: 'dry_run',
    entryCount: 0,
    totalAmount: '0.00',
    warnings: ['No foreign-currency balances to revalue'],
  },
  periodClose: step('dry_run'),
  periodStatus: 'open',
};

function withPeriods() {
  useEmbeddedDataMock.mockReturnValue({
    data: periods,
    isLoading: false,
    error: null,
    refetch: vi.fn(),
  });
}

describe('CloseClient', () => {
  it('renders the period list with statuses and disables closed-period selection', () => {
    withPeriods();
    render(<CloseClient />);

    expect(screen.getByText('Month-End Close')).toBeInTheDocument();
    expect(screen.getByText('2026-01')).toBeInTheDocument();
    expect(screen.getByLabelText('Select period 2026-01')).toBeDisabled();
    expect(screen.getByLabelText('Select period 2026-02')).toBeEnabled();
    // Dry run requires a selected open period.
    expect(screen.getByRole('button', { name: 'Run dry run' })).toBeDisabled();
  });

  it('runs a dry run and renders the per-step report with warnings', async () => {
    withPeriods();
    closeMonthDryRun.mockResolvedValue(dryRunReport);
    render(<CloseClient />);

    fireEvent.click(screen.getByLabelText('Select period 2026-02'));
    fireEvent.click(screen.getByRole('button', { name: 'Run dry run' }));

    expect(await screen.findByText('Dry run — nothing was written')).toBeInTheDocument();
    expect(closeMonthDryRun).toHaveBeenCalledWith('per_open');
    expect(screen.getByText('Depreciation')).toBeInTheDocument();
    expect(screen.getByText('Revenue recognition')).toBeInTheDocument();
    expect(screen.getByText('FX revaluation')).toBeInTheDocument();
    expect(screen.getByText('Period close')).toBeInTheDocument();
    expect(screen.getByText('No foreign-currency balances to revalue')).toBeInTheDocument();
    expect(screen.getAllByText('$1,250.00').length).toBeGreaterThan(0);
  });

  it('guards the real close behind a typed confirmation', async () => {
    withPeriods();
    closeMonthDryRun.mockResolvedValue(dryRunReport);
    runCloseMonth.mockResolvedValue({
      ...dryRunReport,
      dryRun: false,
      depreciation: step('executed'),
      revenueRecognition: step('executed'),
      fxRevaluation: step('executed'),
      periodClose: step('executed'),
      periodStatus: 'closed',
    });
    render(<CloseClient />);

    fireEvent.click(screen.getByLabelText('Select period 2026-02'));
    fireEvent.click(screen.getByRole('button', { name: 'Run dry run' }));
    await screen.findByText('Dry run — nothing was written');

    const closeButton = screen.getByRole('button', { name: 'Run close' });
    expect(closeButton).toBeDisabled();

    // Wrong confirmation text keeps the button disabled.
    fireEvent.change(screen.getByLabelText('Type the period name to confirm close'), {
      target: { value: '2026-03' },
    });
    expect(closeButton).toBeDisabled();

    // Typing the exact period name arms the button.
    fireEvent.change(screen.getByLabelText('Type the period name to confirm close'), {
      target: { value: '2026-02' },
    });
    expect(closeButton).toBeEnabled();

    fireEvent.click(closeButton);
    await waitFor(() => expect(runCloseMonth).toHaveBeenCalledWith('per_open'));
    expect(await screen.findByText('Close executed')).toBeInTheDocument();
  });

  it('renders the error card when periods fail to load', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
      refetch: vi.fn(),
    });
    render(<CloseClient />);

    expect(screen.getByText('Failed to load accounting periods')).toBeInTheDocument();
  });
});
