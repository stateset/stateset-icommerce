// Component test for the AgentPerformance dashboard. The component is
// driven by `useEmbeddedData` (one fetcher per render) and falls back to
// demo numbers when `summary` is undefined. We mock the hook so we can
// drive each branch deterministically.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getAgentPerformanceData: vi.fn(),
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
    tr: ({ children, ...props }: HTMLAttributes<HTMLTableRowElement>) => (
      <tr {...props}>{children}</tr>
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
    AreaChart: Chart,
    BarChart: Chart,
    DonutChart: Chart,
  };
});

import AgentPerformance from '@/components/agents/agent-performance';

afterEach(() => {
  vi.clearAllMocks();
});

describe('AgentPerformance', () => {
  it('renders an error card when the hook reports an error', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('boom'),
    });
    render(<AgentPerformance />);
    expect(screen.getByText('Failed to load agent performance data')).toBeInTheDocument();
  });

  it('renders the metric headings + the supplied agent name on success', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: {
          activeAgents: 3,
          onlinePercentage: 100,
          tasksCompleted: 42,
          avgResponseTime: 0.9,
          successRate: 0.97,
        },
        agents: [
          {
            id: 'a-1',
            name: 'Order Bot',
            type: 'orders',
            status: 'online',
            tasksCompleted: 12,
            avgResponseTime: 0.8,
            successRate: 0.99,
            utilization: 0.5,
          },
        ],
        taskMetrics: { recentTasks: [], distribution: [], dailyOutcomes: [] },
        responseTimeTrend: { daily: [] },
      },
      isLoading: false,
      error: null,
    });

    render(<AgentPerformance />);
    expect(screen.getByText('Active Agents')).toBeInTheDocument();
    expect(screen.getByText('Tasks Completed')).toBeInTheDocument();
    expect(screen.getByText('Order Bot')).toBeInTheDocument();
  });
});
