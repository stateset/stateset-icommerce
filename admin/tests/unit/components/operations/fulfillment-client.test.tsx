// Component test for the Operations > Fulfillment page client: waves,
// pick tasks, and the two status filters.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/operations', () => ({
  getFulfillmentPageData: vi.fn(),
}));

import FulfillmentClient from '@/components/operations/fulfillment-client';

afterEach(() => {
  vi.clearAllMocks();
});

const data = {
  waves: [
    {
      id: 'wave_4000',
      waveNumber: 'WAVE-4000',
      warehouseId: 1,
      orderCount: 5,
      status: 'released',
      createdAt: '2026-07-01T00:00:00.000Z',
    },
    {
      id: 'wave_4001',
      waveNumber: 'WAVE-4001',
      warehouseId: 2,
      orderCount: 9,
      status: 'completed',
      createdAt: '2026-07-02T00:00:00.000Z',
    },
  ],
  picks: [
    {
      id: 'pick_5000',
      waveId: 'wave_4000',
      orderId: 'ord_6000',
      sku: 'SKU-1000',
      quantityRequested: 4,
      quantityPicked: 0,
      status: 'pending',
      sourceLocationId: 10,
    },
    {
      id: 'pick_5001',
      waveId: 'wave_4001',
      orderId: 'ord_6001',
      sku: 'SKU-1001',
      quantityRequested: 2,
      quantityPicked: 2,
      status: 'picked',
      sourceLocationId: 11,
      assignedTo: 'picker_1',
    },
  ],
};

describe('FulfillmentClient', () => {
  it('renders waves and pick tasks', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<FulfillmentClient />);

    expect(screen.getByRole('heading', { level: 1, name: 'Fulfillment' })).toBeInTheDocument();
    expect(screen.getAllByText('WAVE-4000')).toHaveLength(2); // wave row + pick cross-ref
    expect(screen.getByText('pick_5000')).toBeInTheDocument();
    expect(screen.getByText('pick_5001')).toBeInTheDocument();
    expect(screen.getByText('picker_1')).toBeInTheDocument();
  });

  it('filters waves by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<FulfillmentClient />);

    fireEvent.change(screen.getByLabelText('Filter waves by status'), {
      target: { value: 'completed' },
    });

    // Wave table row removed; only the pick-task cross-reference remains.
    expect(screen.getAllByText('WAVE-4001')).toHaveLength(2);
    expect(screen.getAllByText('WAVE-4000')).toHaveLength(1);
  });

  it('filters pick tasks by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<FulfillmentClient />);

    fireEvent.change(screen.getByLabelText('Filter pick tasks by status'), {
      target: { value: 'picked' },
    });

    expect(screen.getByText('pick_5001')).toBeInTheDocument();
    expect(screen.queryByText('pick_5000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<FulfillmentClient />);

    expect(screen.getByText('Failed to load fulfillment data')).toBeInTheDocument();
  });
});
