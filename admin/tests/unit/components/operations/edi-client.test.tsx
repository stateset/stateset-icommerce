// Component test for the Operations > EDI page client: summary cards,
// by-type breakdown, documents table, and the status filter.

import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
vi.mock('@/app/actions/edi', () => ({
  getEdiPageData: vi.fn(),
}));

import EdiClient from '@/components/operations/edi-client';

afterEach(() => {
  vi.clearAllMocks();
});

const doc = (overrides: Partial<Record<string, unknown>>) => ({
  id: 'edi_1',
  documentType: '850',
  direction: 'inbound',
  status: 'processed',
  partner: 'ACME-RETAIL',
  reference: 'PO-7000',
  createdAt: '2026-07-01T00:00:00.000Z',
  updatedAt: '2026-07-01T00:00:00.000Z',
  ...overrides,
});

const data = {
  documents: [
    doc({ id: 'edi_1', documentType: '850', direction: 'inbound', status: 'processed' }),
    doc({
      id: 'edi_2',
      documentType: '856',
      direction: 'outbound',
      status: 'error',
      partner: 'NORTHWIND',
      reference: 'PO-7001',
      errorMessage: 'Missing mandatory segment: BEG',
    }),
  ],
  summary: {
    total: 2,
    byStatus: [
      { key: 'error', count: 1 },
      { key: 'processed', count: 1 },
    ],
    byType: [
      { key: '850', count: 1 },
      { key: '856', count: 1 },
    ],
  },
};

describe('EdiClient', () => {
  it('renders summary cards, by-type breakdown, and the documents table', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<EdiClient />);

    expect(screen.getByText('EDI Documents')).toBeInTheDocument();
    expect(screen.getByText('Total')).toBeInTheDocument();
    expect(screen.getByTestId('edi-type-850')).toHaveTextContent('Purchase Order');
    expect(screen.getByTestId('edi-type-856')).toHaveTextContent('Advance Ship Notice');
    expect(screen.getByText('ACME-RETAIL')).toBeInTheDocument();
    expect(screen.getByText('NORTHWIND')).toBeInTheDocument();
    expect(screen.getByText('Missing mandatory segment: BEG')).toBeInTheDocument();
    expect(screen.getByText('Documents (2)')).toBeInTheDocument();
  });

  it('filters documents by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<EdiClient />);

    fireEvent.change(screen.getByLabelText('Filter EDI documents by status'), {
      target: { value: 'error' },
    });

    expect(screen.getByText('NORTHWIND')).toBeInTheDocument();
    expect(screen.queryByText('ACME-RETAIL')).not.toBeInTheDocument();
    expect(screen.getByText('Documents (1)')).toBeInTheDocument();
  });

  it('renders the empty state when the engine has no EDI documents', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { documents: [], summary: { total: 0, byStatus: [], byType: [] } },
      isLoading: false,
      error: null,
    });
    render(<EdiClient />);

    expect(screen.getByText(/No EDI documents yet/)).toBeInTheDocument();
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<EdiClient />);

    expect(screen.getByText('Failed to load EDI documents')).toBeInTheDocument();
  });
});
