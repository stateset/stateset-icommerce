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

  it('explains a capability the engine lacks instead of failing', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { unavailable: 'The embedded engine has no customer health and retention analytics' },
      isLoading: false,
      error: null,
    });
    render(<CustomerHealthScore />);
    expect(screen.getByText('Customer health unavailable')).toBeInTheDocument();
  });

  it('shows no invented customers or figures when the payload omits them', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: undefined,
        segments: {},
        atRiskCustomers: undefined,
        trends: undefined,
      },
      isLoading: false,
      error: null,
    });
    render(<CustomerHealthScore />);
    expect(screen.getByText('Overall Health Score')).toBeInTheDocument();
    // The four key metrics read "—", not 78/100, 2,450, 127 and $485.
    expect(screen.getAllByText('—')).toHaveLength(4);
    expect(screen.queryByText('127')).toBeNull();
    expect(screen.queryByText('78/100')).toBeNull();
    // No demo at-risk customers, health metrics or segments.
    expect(screen.queryByText('John Smith')).toBeNull();
    expect(screen.queryByText('Purchase Frequency')).toBeNull();
    expect(screen.queryByText('Avg LTV: $1,250.00')).toBeNull();
    expect(screen.getByText('No at-risk customers')).toBeInTheDocument();
    expect(screen.getByText('No health metrics reported')).toBeInTheDocument();
  });

  it('renders a store with no at-risk customers as zero', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: { overallScore: 0, totalCustomers: 0, atRiskCount: 0, avgLifetimeValue: 0 },
        segments: {},
        atRiskCustomers: [],
        trends: { timeline: [] },
      },
      isLoading: false,
      error: null,
    });
    render(<CustomerHealthScore />);
    // `|| 127` used to turn a real zero into 127 at-risk customers.
    expect(screen.getByText('0/100')).toBeInTheDocument();
    expect(screen.getByText('0 customers')).toBeInTheDocument();
    expect(screen.getByText('$0.00')).toBeInTheDocument();
    expect(screen.queryByText('127')).toBeNull();
    expect(screen.queryByText('—')).toBeNull();
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
