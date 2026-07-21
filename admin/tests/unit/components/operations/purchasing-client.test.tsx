// Component test for the Operations > Purchasing page client: purchase order
// table, supplier table, and the status filter.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/operations', () => ({
  getPurchasingPageData: vi.fn(),
}));

import PurchasingClient from '@/components/operations/purchasing-client';

afterEach(() => {
  vi.clearAllMocks();
});

const data = {
  purchaseOrders: [
    {
      id: 'po_1',
      poNumber: 'PO-7000',
      supplierId: 'sup_100',
      status: 'approved',
      subtotal: 1000,
      total: 1070,
      createdAt: '2026-07-01T00:00:00.000Z',
      updatedAt: '2026-07-02T00:00:00.000Z',
    },
    {
      id: 'po_2',
      poNumber: 'PO-7001',
      supplierId: 'sup_101',
      status: 'received',
      subtotal: 500,
      total: 535,
      createdAt: '2026-07-03T00:00:00.000Z',
      updatedAt: '2026-07-04T00:00:00.000Z',
    },
  ],
  suppliers: [
    {
      id: 'sup_100',
      name: 'Northwind Components',
      supplierCode: 'SUP-NWC',
      email: 'orders@northwind.example',
      phone: '+1-555-0110',
      isActive: true,
      createdAt: '2026-01-01T00:00:00.000Z',
    },
  ],
};

describe('PurchasingClient', () => {
  it('renders purchase orders and suppliers', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<PurchasingClient />);

    expect(screen.getByText('Purchasing')).toBeInTheDocument();
    expect(screen.getByText('PO-7000')).toBeInTheDocument();
    expect(screen.getByText('PO-7001')).toBeInTheDocument();
    // Supplier id resolved to a name on the PO row.
    expect(screen.getAllByText('Northwind Components').length).toBeGreaterThan(0);
    expect(screen.getByText('$1,070.00')).toBeInTheDocument();
    expect(screen.getByText('SUP-NWC')).toBeInTheDocument();
  });

  it('filters purchase orders by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<PurchasingClient />);

    fireEvent.change(screen.getByLabelText('Filter purchase orders by status'), {
      target: { value: 'received' },
    });

    expect(screen.getByText('PO-7001')).toBeInTheDocument();
    expect(screen.queryByText('PO-7000')).not.toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<PurchasingClient />);

    expect(screen.getByText('Failed to load purchasing')).toBeInTheDocument();
  });
});
