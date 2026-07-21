// Component test for the Operations > Warehouse page client: warehouses,
// locations, cycle counts, and the two filters.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/operations', () => ({
  getWarehousePageData: vi.fn(),
}));

import WarehouseClient from '@/components/operations/warehouse-client';

afterEach(() => {
  vi.clearAllMocks();
});

const data = {
  warehouses: [
    {
      id: 1,
      code: 'WH-MAIN',
      name: 'Reno Distribution Center',
      warehouseType: 'distribution',
      isActive: true,
      timezone: 'America/Los_Angeles',
      createdAt: '2026-01-01T00:00:00.000Z',
    },
    {
      id: 2,
      code: 'WH-EAST',
      name: 'Columbus Fulfillment',
      warehouseType: 'fulfillment',
      isActive: true,
      createdAt: '2026-01-02T00:00:00.000Z',
    },
  ],
  locations: [
    {
      id: 10,
      warehouseId: 1,
      code: 'WH-MAIN-A1-101',
      locationType: 'bin',
      zone: 'A',
      aisle: '1',
      rack: '1',
      bin: '01',
      isActive: true,
      isPickable: true,
      isReceivable: false,
    },
    {
      id: 11,
      warehouseId: 2,
      code: 'WH-EAST-B2-202',
      locationType: 'receiving',
      zone: 'B',
      aisle: '2',
      rack: '2',
      bin: '02',
      isActive: true,
      isPickable: false,
      isReceivable: true,
    },
  ],
  cycleCounts: [
    {
      id: 'cc_8000',
      warehouseId: 1,
      status: 'completed',
      scheduledDate: '2026-07-01',
      countedBy: 'operator_1',
      lines: [],
      createdAt: '2026-07-01T00:00:00.000Z',
      updatedAt: '2026-07-01T00:00:00.000Z',
    },
    {
      id: 'cc_8001',
      warehouseId: 2,
      status: 'draft',
      scheduledDate: '2026-07-05',
      lines: [],
      createdAt: '2026-07-05T00:00:00.000Z',
      updatedAt: '2026-07-05T00:00:00.000Z',
    },
  ],
};

describe('WarehouseClient', () => {
  it('renders warehouses, locations, and cycle counts', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<WarehouseClient />);

    expect(screen.getByRole('heading', { level: 1, name: 'Warehouse' })).toBeInTheDocument();
    expect(screen.getByText('WH-MAIN-A1-101')).toBeInTheDocument();
    expect(screen.getByText('WH-EAST-B2-202')).toBeInTheDocument();
    expect(screen.getByText('pickable')).toBeInTheDocument();
    expect(screen.getByText('receivable')).toBeInTheDocument();
    expect(screen.getByText('cc_8000')).toBeInTheDocument();
    expect(screen.getByText('cc_8001')).toBeInTheDocument();
  });

  it('filters locations and cycle counts by warehouse', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<WarehouseClient />);

    fireEvent.change(screen.getByLabelText('Filter by warehouse'), {
      target: { value: '1' },
    });

    expect(screen.getByText('WH-MAIN-A1-101')).toBeInTheDocument();
    expect(screen.queryByText('WH-EAST-B2-202')).not.toBeInTheDocument();
    expect(screen.queryByText('cc_8001')).not.toBeInTheDocument();
  });

  it('filters cycle counts by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<WarehouseClient />);

    fireEvent.change(screen.getByLabelText('Filter cycle counts by status'), {
      target: { value: 'draft' },
    });

    expect(screen.getByText('cc_8001')).toBeInTheDocument();
    expect(screen.queryByText('cc_8000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<WarehouseClient />);

    expect(screen.getByText('Failed to load warehouse data')).toBeInTheDocument();
  });
});
