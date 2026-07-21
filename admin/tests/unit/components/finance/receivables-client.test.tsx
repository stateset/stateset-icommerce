// Component test for the Finance > Receivables page client: AR aging cards,
// DSO stat, and the per-customer aging table.

import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/finance', () => ({
  getReceivablesPageData: vi.fn(),
}));

import ReceivablesClient from '@/components/finance/receivables-client';

afterEach(() => {
  vi.clearAllMocks();
});

const data = {
  aging: {
    current: 3200,
    days130: 410.25,
    days3160: 0,
    days6190: 0,
    daysOver90: 90,
    total: 3700.25,
  },
  dso: 31.5,
  dsoWindowDays: 30,
  customers: [
    {
      customerId: 'cus_100',
      current: 3200,
      days130: 0,
      days3160: 0,
      days6190: 0,
      daysOver90: 0,
      total: 3200,
    },
    {
      customerId: 'cus_101',
      current: 0,
      days130: 410.25,
      days3160: 0,
      days6190: 0,
      daysOver90: 90,
      total: 500.25,
    },
  ],
};

describe('ReceivablesClient', () => {
  it('renders aging buckets, the DSO stat, and per-customer rows', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<ReceivablesClient />);

    expect(screen.getByText('Receivables')).toBeInTheDocument();
    expect(screen.getByText('1–30 days')).toBeInTheDocument();
    expect(screen.getByText('$3,700.25')).toBeInTheDocument();
    expect(screen.getByTestId('dso-stat')).toHaveTextContent('31.5 days');
    expect(screen.getByText('cus_100')).toBeInTheDocument();
    expect(screen.getByText('cus_101')).toBeInTheDocument();
  });

  it('hides the DSO stat when the engine build does not expose it', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { ...data, dso: null },
      isLoading: false,
      error: null,
    });
    render(<ReceivablesClient />);

    expect(screen.queryByTestId('dso-stat')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<ReceivablesClient />);

    expect(screen.getByText('Failed to load accounts receivable')).toBeInTheDocument();
  });
});
