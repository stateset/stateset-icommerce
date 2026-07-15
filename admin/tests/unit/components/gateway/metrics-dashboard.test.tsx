import { render, screen, waitFor } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { GatewayMetrics } from '@/lib/types/gateway';
import MetricsDashboard from '@/components/gateway/metrics-dashboard';

const firstMetrics: GatewayMetrics = {
  uptime: '5m',
  uptimeMs: 300000,
  totals: {
    messagesReceived: 10,
    responsesSent: 9,
    errors: 1,
    avgResponseMs: 120,
    blocked: 0,
  },
  channels: {
    api: {
      messagesReceived: 10,
      responsesSent: 9,
      errors: 1,
      blocked: 0,
      avgResponseMs: 120,
      lastMessageAt: '2026-03-01T10:00:00.000Z',
    },
  },
  commandUsage: {},
};

const secondMetrics: GatewayMetrics = {
  ...firstMetrics,
  totals: {
    messagesReceived: 18,
    responsesSent: 17,
    errors: 1,
    avgResponseMs: 135,
    blocked: 0,
  },
  channels: {
    api: {
      ...firstMetrics.channels.api,
      messagesReceived: 18,
      responsesSent: 17,
      avgResponseMs: 135,
    },
  },
};

const useEmbeddedDataMock = vi.fn(
  (_fetcher?: () => Promise<unknown>, _options?: unknown) => ({
    data: firstMetrics,
    isLoading: false,
  })
);

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/lib/gateway-client', () => ({
  getGatewayMetrics: vi.fn(),
}));

vi.mock('@/components/ui/loading-skeleton', () => ({
  default: () => <div>loading</div>,
}));

vi.mock('@/components/gateway/metrics-summary', () => ({
  MetricsSummary: () => <div>metrics-summary</div>,
}));

vi.mock('@/components/gateway/channel-metrics-chart', () => ({
  ChannelMetricsChart: () => <div>channel-metrics-chart</div>,
}));

vi.mock('@/components/gateway/command-usage-table', () => ({
  CommandUsageTable: () => <div>command-usage-table</div>,
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
    Grid: Wrapper,
    Metric: Wrapper,
    Badge: Wrapper,
    AreaChart: ({ data }: { data: unknown[] }) => (
      <div data-testid="area-chart">{data.length}</div>
    ),
  };
});

// The component now uses the design system's (Radix-backed) Tabs, which only
// mount the active panel. This test asserts content across multiple panels at
// once to verify history accumulation, so render the ds Tabs parts as simple
// passthroughs (everything else in @stateset/design stays real).
vi.mock('@stateset/design', async (importActual) => {
  const actual = await importActual<typeof import('@stateset/design')>();
  const Pass = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  return { ...actual, Tabs: Pass, TabsList: Pass, TabsTrigger: Pass, TabsContent: Pass };
});

describe('MetricsDashboard', () => {
  it('accumulates history from metric updates without mutating state during render', async () => {
    useEmbeddedDataMock.mockReturnValueOnce({ data: firstMetrics, isLoading: false });

    const { rerender } = render(<MetricsDashboard />);

    expect(screen.getAllByText('Accumulating data...')).toHaveLength(2);

    useEmbeddedDataMock.mockReturnValueOnce({ data: secondMetrics, isLoading: false });
    rerender(<MetricsDashboard />);

    await waitFor(() => {
      expect(screen.getAllByTestId('area-chart')).toHaveLength(2);
    });
  });
});
