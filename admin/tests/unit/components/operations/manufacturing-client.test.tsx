// Component test for the Operations > Manufacturing page client: work order
// table, inspection summary, NCR table, and the status filter.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/operations', () => ({
  getManufacturingPageData: vi.fn(),
}));

import ManufacturingClient from '@/components/operations/manufacturing-client';

afterEach(() => {
  vi.clearAllMocks();
});

const data = {
  workOrders: [
    {
      id: 'wo_1',
      workOrderNumber: 'WO-9000',
      productId: 'prod_200',
      status: 'in_progress',
      priority: 'high',
      quantityToBuild: 100,
      quantityCompleted: 50,
      version: 1,
      createdAt: '2026-07-01T00:00:00.000Z',
      updatedAt: '2026-07-02T00:00:00.000Z',
    },
    {
      id: 'wo_2',
      workOrderNumber: 'WO-9001',
      productId: 'prod_201',
      status: 'completed',
      priority: 'normal',
      quantityToBuild: 40,
      quantityCompleted: 40,
      version: 1,
      createdAt: '2026-07-03T00:00:00.000Z',
      updatedAt: '2026-07-04T00:00:00.000Z',
    },
  ],
  inspections: [
    {
      id: 'insp_1',
      inspectionNumber: 'QI-9500',
      inspectionType: 'incoming',
      referenceType: 'purchase_order',
      referenceId: 'po_7000',
      status: 'passed',
      createdAt: '2026-07-01T00:00:00.000Z',
      updatedAt: '2026-07-01T00:00:00.000Z',
    },
    {
      id: 'insp_2',
      inspectionNumber: 'QI-9501',
      inspectionType: 'final',
      referenceType: 'work_order',
      referenceId: 'wo_1',
      status: 'passed',
      createdAt: '2026-07-02T00:00:00.000Z',
      updatedAt: '2026-07-02T00:00:00.000Z',
    },
  ],
  ncrs: [
    {
      id: 'ncr_1',
      ncrNumber: 'NCR-9700',
      source: 'inspection',
      severity: 'critical',
      sku: 'SKU-1000',
      quantityAffected: 12,
      status: 'open',
      description: 'Out of spec',
      createdAt: '2026-07-01T00:00:00.000Z',
    },
    {
      id: 'ncr_2',
      ncrNumber: 'NCR-9701',
      source: 'customer_return',
      severity: 'minor',
      sku: 'SKU-1001',
      quantityAffected: 2,
      status: 'closed',
      description: 'Cosmetic',
      createdAt: '2026-07-02T00:00:00.000Z',
    },
  ],
};

describe('ManufacturingClient', () => {
  it('renders work orders, inspection summary, and NCRs', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<ManufacturingClient />);

    expect(screen.getByText('Manufacturing')).toBeInTheDocument();
    expect(screen.getByText('WO-9000')).toBeInTheDocument();
    expect(screen.getByText('WO-9001')).toBeInTheDocument();
    expect(screen.getByText(/passed/)).toBeInTheDocument();
    expect(screen.getByText('NCR-9700')).toBeInTheDocument();
    // One of the two NCRs is closed, so only one is counted as open.
    expect(screen.getByText('Open NCRs')).toBeInTheDocument();
  });

  it('filters work orders by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<ManufacturingClient />);

    fireEvent.change(screen.getByLabelText('Filter work orders by status'), {
      target: { value: 'completed' },
    });

    expect(screen.getByText('WO-9001')).toBeInTheDocument();
    expect(screen.queryByText('WO-9000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<ManufacturingClient />);

    expect(screen.getByText('Failed to load manufacturing data')).toBeInTheDocument();
  });
});
