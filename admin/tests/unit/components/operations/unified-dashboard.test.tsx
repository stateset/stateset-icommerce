import { act, render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import UnifiedDashboard from '@/components/operations/unified-dashboard';
import type { DashboardMetrics, HourlyActivity, SystemHealth } from '@/lib/types';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: { refreshInterval?: number }) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getDashboardMetrics: vi.fn(),
  getHourlyActivity: vi.fn(),
  getSystemHealth: vi.fn(),
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
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Col: Wrapper,
    Metric: Wrapper,
    ProgressBar: Wrapper,
    AreaChart: ({ data }: { data: unknown[] }) => <div data-testid="area-chart">{data.length}</div>,
  };
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

// Rates and changes are 0-1 ratios, as mockDashboardMetrics reports them.
const metrics: DashboardMetrics = {
  gmvToday: 12000,
  gmvChange: 0.12,
  ordersToday: 240,
  ordersChange: 0.09,
  averageOrderValue: 50,
  aovChange: 0.02,
  conversionRate: 0.042,
  conversionChange: 0.006,
  activeCustomers: 180,
  newCustomers: 24,
  returnRate: 0.018,
  inventoryHealth: 0.95,
};

const hourlyActivity: HourlyActivity[] = [];

const systemHealth: SystemHealth = {
  databaseLatency: 18,
  errorRate: 0.05,
  activeConnections: 12,
  queueDepth: 10,
  processingSpeed: 98,
};

function serve(dashboardMetrics: DashboardMetrics) {
  useEmbeddedDataMock.mockImplementation(
    (_fetcher: () => Promise<unknown>, options?: { refreshInterval?: number }) => {
      switch (options?.refreshInterval) {
        case 30000:
          return { data: dashboardMetrics, isLoading: false, error: null };
        case 60000:
          return { data: hourlyActivity, isLoading: false, error: null };
        case 10000:
          return { data: systemHealth, isLoading: false, error: null };
        default:
          return { data: null, isLoading: false, error: null };
      }
    },
  );
}

describe('UnifiedDashboard', () => {
  it('replaces hardcoded alerts with live coverage and derived insights', async () => {
    vi.useFakeTimers();
    serve(metrics);

    render(<UnifiedDashboard />);

    expect(screen.getByText('loading')).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(screen.getByText('Live Data Coverage')).toBeTruthy();
    expect(screen.getByText('No active operational alerts in the current snapshot.')).toBeTruthy();
    expect(screen.getByText('GMV is up 12.0% today')).toBeTruthy();
    expect(screen.queryByText('Inventory low for SKU-1234')).toBeNull();
  });

  it('renders the 0-1 dashboard rates as percentages against a 3.5% target', async () => {
    vi.useFakeTimers();
    serve({ ...metrics, inventoryHealth: 0.85 });

    render(<UnifiedDashboard />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    // Conversion 0.042 is 4.2%, above the 3.5% target (it used to read 0.042%).
    expect(screen.getByText('4.2%')).toBeTruthy();
    expect(screen.getByText('Conversion is above target at 4.2%')).toBeTruthy();
    expect(screen.getByText('Target 3.5% · 120% attained')).toBeTruthy();
    expect(screen.getByText('▲ 12.0% vs. yesterday')).toBeTruthy();
    // Inventory health 0.85 is below the 90% alert line, not "0.85%".
    expect(screen.getByText('85.0%')).toBeTruthy();
    expect(screen.getByText('Inventory health is 85.0%')).toBeTruthy();
  });
});
