// Component test for the Finance > Bills page client: aging summary cards,
// bills table, and the status filter.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/finance', () => ({
  getBillsPageData: vi.fn(),
}));

import BillsClient from '@/components/finance/bills-client';

afterEach(() => {
  vi.clearAllMocks();
});

const bill = (overrides: Partial<Record<string, unknown>>) => ({
  id: 'bill_1',
  billNumber: 'BILL-3000',
  supplierId: 'sup_100',
  status: 'open',
  totalAmount: 1200.5,
  amountPaid: 0,
  amountDue: 1200.5,
  dueDate: '2026-08-01',
  createdAt: '2026-07-01T00:00:00.000Z',
  ...overrides,
});

const data = {
  bills: [
    bill({ id: 'bill_1', billNumber: 'BILL-3000', status: 'open' }),
    bill({
      id: 'bill_2',
      billNumber: 'BILL-3001',
      status: 'paid',
      amountPaid: 500,
      amountDue: 0,
      totalAmount: 500,
    }),
  ],
  aging: {
    current: 1000,
    days130: 200.5,
    days3160: 0,
    days6190: 0,
    daysOver90: 0,
    total: 1200.5,
  },
};

describe('BillsClient', () => {
  it('renders aging buckets and the bills table', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<BillsClient />);

    expect(screen.getByText('Bills')).toBeInTheDocument();
    expect(screen.getByText('1–30 days')).toBeInTheDocument();
    expect(screen.getByText('$200.50')).toBeInTheDocument();
    expect(screen.getByText('BILL-3000')).toBeInTheDocument();
    expect(screen.getByText('BILL-3001')).toBeInTheDocument();
  });

  it('filters bills by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<BillsClient />);

    fireEvent.change(screen.getByLabelText('Filter bills by status'), {
      target: { value: 'paid' },
    });

    expect(screen.getByText('BILL-3001')).toBeInTheDocument();
    expect(screen.queryByText('BILL-3000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<BillsClient />);

    expect(screen.getByText('Failed to load accounts payable')).toBeInTheDocument();
  });
});
