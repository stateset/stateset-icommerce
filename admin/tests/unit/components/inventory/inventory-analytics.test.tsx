// Component test for InventoryAnalytics. The component derives a simple
// healthScore from `lowStockItems` and `outOfStockItems` and surfaces
// "Needs attention" / "Critical" badges when those counts are non-zero —
// we exercise both arms.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getInventoryAnalyticsData: vi.fn(),
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    ProgressBar: Wrapper,
    BarChart: Chart,
    DonutChart: Chart,
  };
});

import InventoryAnalytics from '@/components/inventory/inventory-analytics';

afterEach(() => {
  vi.clearAllMocks();
});

const baseData = {
  totalSKUs: 100,
  totalUnits: 5000,
  totalValue: 250_000,
  lowStockItems: 0,
  outOfStockItems: 0,
  categories: [],
  topMovingItems: [],
  slowMovingItems: [],
};

describe('InventoryAnalytics', () => {
  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, isLoading: false, error: new Error('x') });
    render(<InventoryAnalytics />);
    expect(screen.getByText('Failed to load inventory analytics')).toBeInTheDocument();
  });

  it('does not show low-stock / out-of-stock badges when both counts are zero', () => {
    useEmbeddedDataMock.mockReturnValue({ data: baseData, isLoading: false, error: null });
    render(<InventoryAnalytics />);
    expect(screen.queryByText('Needs attention')).toBeNull();
    expect(screen.queryByText('Critical')).toBeNull();
  });

  it('shows the "Needs attention" badge when lowStockItems > 0', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { ...baseData, lowStockItems: 4 },
      isLoading: false,
      error: null,
    });
    render(<InventoryAnalytics />);
    expect(screen.getByText('Needs attention')).toBeInTheDocument();
  });

  it('shows the "Critical" badge when outOfStockItems > 0', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { ...baseData, outOfStockItems: 2 },
      isLoading: false,
      error: null,
    });
    render(<InventoryAnalytics />);
    expect(screen.getByText('Critical')).toBeInTheDocument();
  });
});
