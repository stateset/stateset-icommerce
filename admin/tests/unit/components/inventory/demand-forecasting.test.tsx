import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import DemandForecasting from '@/components/inventory/demand-forecasting';
import type { DemandForecastingData } from '@/lib/types/dashboard-data';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getDemandForecastingData: vi.fn(),
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
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    AreaChart: ({ data }: { data: unknown[] }) => <div data-testid="area-chart">{data.length}</div>,
    BarChart: ({ data }: { data: unknown[] }) => <div data-testid="bar-chart">{data.length}</div>,
  };
});

afterEach(() => {
  vi.clearAllMocks();
});

const emptyForecastData: DemandForecastingData = {
  forecast: {
    predictedRevenue: 0,
    trendScore: 0,
    timeline: [],
    categoryDemand: [],
  },
  topProducts: {
    highDemand: [],
  },
  alerts: [],
  accuracy: {
    overall: 0,
  },
};

describe('DemandForecasting', () => {
  it('shows empty states when no live forecast coverage exists', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: emptyForecastData,
      isLoading: false,
      error: null,
    });

    render(<DemandForecasting />);

    expect(
      screen.getByText(
        'No live demand forecast is available yet. Enable forecast coverage for at least one SKU.',
      ),
    ).toBeTruthy();
    expect(
      screen.getByText('No forecast-backed product demand signals are available yet.'),
    ).toBeTruthy();
    expect(screen.getByText('No active restock alerts from live forecast data.')).toBeTruthy();
    expect(screen.queryByText('Wireless Headphones')).toBeNull();
  });
});
