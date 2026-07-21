// Component test for the Operations > Traceability page client: lots
// (with expiry highlighting), serial numbers, and receipts.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/operations', () => ({
  getTraceabilityPageData: vi.fn(),
}));

import TraceabilityClient from '@/components/operations/traceability-client';

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date('2026-07-20T00:00:00.000Z'));
});

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

const data = {
  lots: [
    {
      id: 'lot_3000',
      lotNumber: 'LOT-3000',
      sku: 'SKU-1000',
      quantityProduced: 500,
      quantityAvailable: 480,
      quantityReserved: 20,
      status: 'active',
      productionDate: '2026-05-01',
      expirationDate: '2026-08-01', // within 30 days -> near expiry
      createdAt: '2026-05-01T00:00:00.000Z',
    },
    {
      id: 'lot_3001',
      lotNumber: 'LOT-3001',
      sku: 'SKU-1001',
      quantityProduced: 200,
      quantityAvailable: 0,
      quantityReserved: 0,
      status: 'expired',
      productionDate: '2025-12-01',
      expirationDate: '2026-06-01',
      createdAt: '2025-12-01T00:00:00.000Z',
    },
  ],
  serials: [
    {
      id: 'ser_2000',
      serial: 'SN-00002000',
      sku: 'SKU-1000',
      lotId: 'lot_3000',
      status: 'available',
      locationId: 10,
      createdAt: '2026-06-01T00:00:00.000Z',
    },
    {
      id: 'ser_2001',
      serial: 'SN-00002001',
      sku: 'SKU-1001',
      lotId: 'lot_3001',
      status: 'sold',
      ownerId: 'cus_400',
      createdAt: '2026-06-02T00:00:00.000Z',
    },
  ],
  receipts: [
    {
      id: 'rcpt_1500',
      receiptNumber: 'RCV-1500',
      receiptType: 'purchase_order',
      warehouseId: 1,
      status: 'completed',
      carrier: 'UPS',
      trackingNumber: '1Z90000',
      createdAt: '2026-07-01T00:00:00.000Z',
    },
  ],
};

describe('TraceabilityClient', () => {
  it('renders lots, serials, and receipts with expiry highlighting', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    const { container } = render(<TraceabilityClient />);

    expect(screen.getByRole('heading', { level: 1, name: 'Traceability' })).toBeInTheDocument();
    expect(screen.getAllByText('LOT-3000')).toHaveLength(2); // lot row + serial cross-ref
    expect(screen.getByText('SN-00002000')).toBeInTheDocument();
    expect(screen.getByText('RCV-1500')).toBeInTheDocument();
    expect(container.querySelector('[data-expiry="near"]')).not.toBeNull();
    expect(container.querySelector('[data-expiry="expired"]')).not.toBeNull();
  });

  it('filters lots by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<TraceabilityClient />);

    fireEvent.change(screen.getByLabelText('Filter lots by status'), {
      target: { value: 'expired' },
    });

    // Lot table row removed; only the serial cross-reference remains.
    expect(screen.getAllByText('LOT-3001')).toHaveLength(2);
    expect(screen.getAllByText('LOT-3000')).toHaveLength(1);
  });

  it('filters serials by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<TraceabilityClient />);

    fireEvent.change(screen.getByLabelText('Filter serials by status'), {
      target: { value: 'sold' },
    });

    expect(screen.getByText('SN-00002001')).toBeInTheDocument();
    expect(screen.queryByText('SN-00002000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<TraceabilityClient />);

    expect(screen.getByText('Failed to load traceability data')).toBeInTheDocument();
  });
});
