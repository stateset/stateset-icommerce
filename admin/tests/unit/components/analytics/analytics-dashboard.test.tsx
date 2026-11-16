// Component test for AnalyticsDashboard. It reads six actions through
// useEmbeddedData; each mocked action returns a key, and the hook mock serves
// the response registered for that key.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DashboardMetrics } from '@/lib/types';

const responses = vi.hoisted(() => new Map<string, unknown>());

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => unknown) => ({
    data: responses.get(fetcher() as string) ?? null,
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/app/actions/commerce', () => ({
  getDashboardMetrics: () => 'metrics',
  getOrderAnalytics: () => 'orders',
  getCustomerAnalytics: () => 'customers',
  getRevenueByPeriod: () => 'revenue',
  getTopProducts: () => 'topProducts',
  getConversionFunnel: () => 'funnel',
}));

vi.mock('@/components/ui/loading-skeleton', () => ({
  default: () => <div>loading</div>,
}));

vi.mock('@/components/ui/error-boundary', () => ({
  ErrorBoundary: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return { AreaChart: Chart, BarChart: Chart, DonutChart: Chart };
});

import AnalyticsDashboard from '@/components/analytics/analytics-dashboard';

afterEach(() => {
  responses.clear();
});

// Changes are 0-1 ratios, today vs. yesterday (mockDashboardMetrics).
const metrics: DashboardMetrics = {
  gmvToday: 1200,
  gmvChange: 0.12,
  ordersToday: 24,
  ordersChange: -0.05,
  averageOrderValue: 50,
  aovChange: 0,
  conversionRate: 0.04,
  conversionChange: 0.01,
  activeCustomers: 10,
  newCustomers: 2,
  returnRate: 0.02,
  inventoryHealth: 0.9,
};

describe('AnalyticsDashboard', () => {
  it('renders the 0-1 funnel rates as percentages', () => {
    responses.set('funnel', [
      { stage: 'Visits', count: 1000, rate: 1 },
      { stage: 'Cart', count: 290, rate: 0.29 },
    ]);
    const { container } = render(<AnalyticsDashboard />);

    // 0.29 is 29%, and the bar is 29% wide (it read "0.29%" and was 0.29% wide).
    expect(screen.getByText('29.0%')).toBeInTheDocument();
    expect(screen.getByText('100.0%')).toBeInTheDocument();
    const widths = Array.from(container.querySelectorAll<HTMLElement>('[style]')).map((element) =>
      parseFloat(element.style.width),
    );
    expect(widths).toEqual(expect.arrayContaining([100, 29]));
  });

  it('captions the ratio changes as signed percentages', () => {
    responses.set('metrics', metrics);
    render(<AnalyticsDashboard />);

    expect(screen.getByText('+12.0% today vs. yesterday')).toBeInTheDocument();
    expect(screen.getByText('-5.0% today vs. yesterday')).toBeInTheDocument();
    expect(screen.getByText('0.0% today vs. yesterday')).toBeInTheDocument();
  });

  it('shows no change caption when there are no dashboard metrics', () => {
    // The native engine has no dashboard aggregate; nothing measured a change.
    render(<AnalyticsDashboard />);

    expect(screen.getByText('Total Revenue')).toBeInTheDocument();
    expect(screen.queryByText(/vs\. yesterday|vs last period/)).toBeNull();
  });
});
