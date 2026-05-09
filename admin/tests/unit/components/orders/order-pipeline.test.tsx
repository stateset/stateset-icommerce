// Component test for the Order Pipeline dashboard. Drive the
// useEmbeddedData hook through its loading / error / data branches.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getOrderPipelineData: vi.fn(),
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
    BarChart: Chart,
  };
});

import OrderPipeline from '@/components/orders/order-pipeline';

afterEach(() => {
  vi.clearAllMocks();
});

describe('OrderPipeline', () => {
  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, isLoading: false, error: new Error('x') });
    render(<OrderPipeline />);
    expect(screen.getByText('Failed to load order pipeline')).toBeInTheDocument();
  });

  it('renders the summary metric headings on success', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        summary: {
          totalOrders: 250,
          totalValue: 125_000,
          averageOrderValue: 500,
          deliveredRate: 96.5,
          processingRate: 0.8,
          cancelledRate: 0.05,
        },
        statusGroups: [],
      },
      isLoading: false,
      error: null,
    });
    render(<OrderPipeline />);
    expect(screen.getByText('Total Orders')).toBeInTheDocument();
  });
});
