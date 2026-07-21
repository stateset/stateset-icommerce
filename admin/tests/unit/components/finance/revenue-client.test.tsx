// Component test for the Finance > Revenue page client: contracts table,
// status filter, and the expandable per-contract obligations row.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/finance', () => ({
  getRevenueContracts: vi.fn(),
}));

import RevenueClient from '@/components/finance/revenue-client';

afterEach(() => {
  vi.clearAllMocks();
});

const obligation = (overrides: Partial<Record<string, unknown>>) => ({
  id: 'rc_1_ob_1',
  contractId: 'rc_1',
  description: 'Platform subscription',
  allocatedAmount: '9000.00',
  recognitionMethod: 'ratable_over_time',
  recognizedAmount: '4500.00',
  deferredAmount: '4500.00',
  createdAt: '2026-01-01T00:00:00.000Z',
  updatedAt: '2026-01-01T00:00:00.000Z',
  ...overrides,
});

const contract = (overrides: Partial<Record<string, unknown>>) => ({
  id: 'rc_1',
  contractNumber: 'RC-6000',
  customerId: 'cus_100',
  transactionPrice: '12000.00',
  currency: 'USD',
  status: 'active',
  effectiveDate: '2026-01-01',
  obligations: [
    obligation({}),
    obligation({
      id: 'rc_1_ob_2',
      description: 'Onboarding services',
      allocatedAmount: '3000.00',
      recognitionMethod: 'point_in_time',
      recognizedAmount: '0.00',
      deferredAmount: '3000.00',
    }),
  ],
  totalRecognized: '4500.00',
  deferredBalance: '7500.00',
  createdAt: '2026-01-01T00:00:00.000Z',
  updatedAt: '2026-01-01T00:00:00.000Z',
  ...overrides,
});

const data = [
  contract({}),
  contract({
    id: 'rc_2',
    contractNumber: 'RC-6001',
    status: 'completed',
    totalRecognized: '12000.00',
    deferredBalance: '0.00',
    obligations: [],
  }),
];

describe('RevenueClient', () => {
  it('renders contracts with recognized and deferred totals', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<RevenueClient />);

    expect(screen.getByText('Revenue recognition')).toBeInTheDocument();
    expect(screen.getByText('RC-6000')).toBeInTheDocument();
    expect(screen.getByText('$7,500.00')).toBeInTheDocument();
  });

  it('filters contracts by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<RevenueClient />);

    fireEvent.change(screen.getByLabelText('Filter contracts by status'), {
      target: { value: 'completed' },
    });

    expect(screen.getByText('RC-6001')).toBeInTheDocument();
    expect(screen.queryByText('RC-6000')).not.toBeInTheDocument();
  });

  it('expands a contract row to show its obligations', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<RevenueClient />);

    expect(screen.queryByText('Platform subscription')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('RC-6000'));

    expect(screen.getByText('Platform subscription')).toBeInTheDocument();
    expect(screen.getByText('Onboarding services')).toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<RevenueClient />);

    expect(screen.getByText('Failed to load revenue contracts')).toBeInTheDocument();
  });
});
