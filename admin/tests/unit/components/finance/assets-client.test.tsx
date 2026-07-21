// Component test for the Finance > Assets page client: asset register table,
// status filter, and the expandable depreciation-schedule row.

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
const getAssetDepreciationScheduleMock = vi.fn();
vi.mock('@/app/actions/finance', () => ({
  getFixedAssets: vi.fn(),
  getAssetDepreciationSchedule: (assetId: string) => getAssetDepreciationScheduleMock(assetId),
}));

import AssetsClient from '@/components/finance/assets-client';

afterEach(() => {
  vi.clearAllMocks();
});

const asset = (overrides: Partial<Record<string, unknown>>) => ({
  id: 'fa_5000',
  assetNumber: 'FA-5000',
  name: 'Forklift A',
  category: 'equipment',
  acquisitionDate: '2025-06-01',
  acquisitionCost: '42000.00',
  salvageValue: '2000.00',
  usefulLifeMonths: 120,
  depreciationMethod: 'straight_line',
  status: 'in_service',
  inServiceDate: '2025-06-01',
  accumulatedDepreciation: '14000.00',
  bookValue: '28000.00',
  currency: 'USD',
  createdAt: '2025-06-01T00:00:00.000Z',
  updatedAt: '2025-06-01T00:00:00.000Z',
  ...overrides,
});

const data = [
  asset({}),
  asset({
    id: 'fa_5001',
    assetNumber: 'FA-5001',
    name: 'Packing line',
    status: 'draft',
    accumulatedDepreciation: '0.00',
    bookValue: '96000.00',
    acquisitionCost: '96000.00',
  }),
];

describe('AssetsClient', () => {
  it('renders the asset register with exact-decimal book values', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<AssetsClient />);

    expect(screen.getByText('Fixed assets')).toBeInTheDocument();
    expect(screen.getByText('FA-5000')).toBeInTheDocument();
    expect(screen.getByText('$28,000.00')).toBeInTheDocument();
  });

  it('filters assets by status', () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    render(<AssetsClient />);

    fireEvent.change(screen.getByLabelText('Filter assets by status'), {
      target: { value: 'draft' },
    });

    expect(screen.getByText('FA-5001')).toBeInTheDocument();
    expect(screen.queryByText('FA-5000')).not.toBeInTheDocument();
  });

  it('expands a row and loads its depreciation schedule', async () => {
    useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
    getAssetDepreciationScheduleMock.mockResolvedValue({
      assetId: 'fa_5000',
      method: 'straight_line',
      entries: [
        {
          period: 1,
          amount: '333.33',
          accumulated: '333.33',
          bookValue: '41666.67',
          status: 'posted',
        },
      ],
      totalDepreciation: '333.33',
    });
    render(<AssetsClient />);

    fireEvent.click(screen.getByText('FA-5000'));

    expect(getAssetDepreciationScheduleMock).toHaveBeenCalledWith('fa_5000');
    await waitFor(() => {
      expect(screen.getByText('$41,666.67')).toBeInTheDocument();
    });
  });

  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('x'),
    });
    render(<AssetsClient />);

    expect(screen.getByText('Failed to load fixed assets')).toBeInTheDocument();
  });
});
