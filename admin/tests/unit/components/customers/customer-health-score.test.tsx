// Component test for CustomerHealthScore. We exercise the error and
// happy-path branches and confirm that the demo fallbacks fire when the
// payload omits the optional sub-shapes.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getCustomerHealthData: vi.fn(),
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
    DonutChart: Chart,
    BarChart: Chart,
  };
});

import CustomerHealthScore from '@/components/customers/customer-health-score';

afterEach(() => {
  vi.clearAllMocks();
});

describe('CustomerHealthScore', () => {
  it('renders the error card when the hook errors', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, isLoading: false, error: new Error('x') });
    render(<CustomerHealthScore />);
    expect(screen.getByText('Failed to load customer health data')).toBeInTheDocument();
  });

  it('renders demo at-risk customers when the payload omits them', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: { overallScore: 80, totalCustomers: 100, atRiskCount: 4, avgLifetimeValue: 500 },
        segments: {},
        atRiskCustomers: undefined,
        trends: undefined,
      },
      isLoading: false,
      error: null,
    });
    render(<CustomerHealthScore />);
    expect(screen.getByText('Overall Health Score')).toBeInTheDocument();
    // Demo at-risk fallback name
    expect(screen.getByText('John Smith')).toBeInTheDocument();
  });

  it('renders provided at-risk customers when supplied', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: { overallScore: 90, totalCustomers: 1, atRiskCount: 1, avgLifetimeValue: 250 },
        segments: { excellent: 1 },
        atRiskCustomers: [
          {
            id: 'cust-1',
            name: 'Alice Liddell',
            email: 'alice@example.com',
            healthScore: 30,
            riskReason: 'inactive',
            lifetimeValue: 250,
            daysSinceLastOrder: 90,
          },
        ],
        trends: { timeline: [] },
      },
      isLoading: false,
      error: null,
    });
    render(<CustomerHealthScore />);
    expect(screen.getByText('Alice Liddell')).toBeInTheDocument();
    expect(screen.queryByText('John Smith')).toBeNull();
  });
});
