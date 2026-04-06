import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import SystemHealth from '@/components/operations/system-health';
import type { SystemHealthData } from '@/lib/types/dashboard-data';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getSystemHealthData: vi.fn(),
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
    ProgressBar: Wrapper,
    AreaChart: ({ data }: { data: unknown[] }) => <div data-testid="area-chart">{data.length}</div>,
  };
});

afterEach(() => {
  vi.clearAllMocks();
});

const healthData: SystemHealthData = {
  summary: {
    overallStatus: 'healthy',
    uptime: 99.99,
    healthyServices: 3,
    totalServices: 3,
  },
  services: [
    { name: 'Order Service', status: 'healthy', latency: 12, successRate: 99.9 },
    { name: 'Inventory Service', status: 'healthy', latency: 8, successRate: 99.8 },
    { name: 'Analytics Service', status: 'healthy', latency: 15, successRate: 99.7 },
  ],
  performance: {
    cpuUsage: 20,
    memoryUsage: 30,
    requestsPerSecond: 100,
    timeline: [],
  },
  database: {
    latency: 1.2,
    connections: 8,
    maxConnections: 50,
    avgQueryTime: 1.2,
    queriesPerSecond: 70,
    size: 'Unavailable',
  },
  recentEvents: [],
};

describe('SystemHealth', () => {
  it('renders explicit empty states when historical telemetry is unavailable', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: healthData,
      isLoading: false,
      error: null,
    });

    render(<SystemHealth />);

    expect(
      screen.getByText('Performance history is not available from the embedded engine yet.'),
    ).toBeTruthy();
    expect(
      screen.getByText('No recent system events are available from the embedded engine.'),
    ).toBeTruthy();
    expect(screen.queryByText('Database backup completed')).toBeNull();
  });
});
